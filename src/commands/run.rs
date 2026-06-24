use anyhow::{Result, bail};
use clap::Subcommand;
use std::io::Read;
use std::time::{Duration, Instant};

use crate::client::Client;

#[derive(Subcommand)]
pub enum RunCommands {
    /// List recent workflow runs
    List {
        /// Repository (owner/repo)
        repo: String,
        /// Max results
        #[arg(short = 'l', long, default_value = "10")]
        limit: u32,
        /// Filter by status (queued, in_progress, completed)
        #[arg(short, long)]
        status: Option<String>,
        /// Filter by branch
        #[arg(short, long)]
        branch: Option<String>,
        /// Filter by workflow name or filename
        #[arg(short, long)]
        workflow: Option<String>,
    },
    /// View a specific run's details and jobs
    View {
        /// Repository (owner/repo)
        repo: String,
        /// Run ID
        run_id: u64,
    },
    /// View a specific workflow job's runner details and steps
    Job {
        /// Repository (owner/repo)
        repo: String,
        /// Job ID
        job_id: u64,
    },
    /// Poll a run until it reaches a terminal state
    Watch {
        /// Repository (owner/repo)
        repo: String,
        /// Run ID
        run_id: u64,
        /// Poll interval in seconds
        #[arg(short, long, default_value = "30")]
        interval: u64,
        /// Maximum seconds to wait before exiting with an error
        #[arg(short, long)]
        timeout: Option<u64>,
    },
    /// Show logs for failed jobs in a run
    Logs {
        /// Repository (owner/repo)
        repo: String,
        /// Run ID
        run_id: u64,
        /// Get logs for a specific job name
        #[arg(long)]
        job: Option<String>,
    },
    /// Cancel a queued or in-progress run
    Cancel {
        /// Repository (owner/repo)
        repo: String,
        /// Run ID
        run_id: u64,
    },
    /// Re-run a workflow run
    Rerun {
        /// Repository (owner/repo)
        repo: String,
        /// Run ID
        run_id: u64,
        /// Only re-run failed jobs
        #[arg(long)]
        failed: bool,
    },
}

#[cfg_attr(coverage_nightly, coverage(off))]
pub async fn handle(client: &Client, command: RunCommands) -> Result<()> {
    match command {
        RunCommands::List { .. } => handle_list_command(client, command).await?,
        RunCommands::View { repo, run_id } => handle_view(client, &repo, run_id).await?,
        RunCommands::Job { repo, job_id } => handle_job(client, &repo, job_id).await?,
        RunCommands::Watch {
            repo,
            run_id,
            interval,
            timeout,
        } => handle_watch(client, &repo, run_id, interval, timeout).await?,
        RunCommands::Logs { repo, run_id, job } => {
            show_logs(client, &repo, run_id, job.as_deref()).await?
        }
        RunCommands::Cancel { repo, run_id } => handle_cancel(client, &repo, run_id).await?,
        RunCommands::Rerun {
            repo,
            run_id,
            failed,
        } => handle_rerun(client, &repo, run_id, failed).await?,
    }
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn handle_job(client: &Client, repo: &str, job_id: u64) -> Result<()> {
    let job = client
        .get(&format!("/repos/{repo}/actions/jobs/{job_id}"))
        .await?;
    print_job_detail(&job);
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn handle_list_command(client: &Client, command: RunCommands) -> Result<()> {
    let RunCommands::List {
        repo,
        limit,
        status,
        branch,
        workflow,
    } = command
    else {
        bail!("expected list command");
    };

    handle_list(
        client,
        &repo,
        limit,
        status.as_deref(),
        branch.as_deref(),
        workflow.as_deref(),
    )
    .await
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn handle_list(
    client: &Client,
    repo: &str,
    limit: u32,
    status: Option<&str>,
    branch: Option<&str>,
    workflow: Option<&str>,
) -> Result<()> {
    let result = list_runs(client, repo, limit, status, branch).await?;
    print_runs(&result, workflow, limit);
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn handle_view(client: &Client, repo: &str, run_id: u64) -> Result<()> {
    let (run, jobs) = get_run_and_jobs(client, repo, run_id).await?;
    print_run_detail(&run, &jobs);
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn get_run_and_jobs(
    client: &Client,
    repo: &str,
    run_id: u64,
) -> Result<(serde_json::Value, serde_json::Value)> {
    let run = client
        .get(&format!("/repos/{repo}/actions/runs/{run_id}"))
        .await?;
    let jobs = client
        .get(&format!("/repos/{repo}/actions/runs/{run_id}/jobs"))
        .await?;
    Ok((run, jobs))
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn handle_watch(
    client: &Client,
    repo: &str,
    run_id: u64,
    interval_seconds: u64,
    timeout_seconds: Option<u64>,
) -> Result<()> {
    if interval_seconds == 0 {
        bail!("interval must be greater than 0 seconds");
    }

    let started = Instant::now();
    let timeout = timeout_seconds.map(Duration::from_secs);
    let interval = Duration::from_secs(interval_seconds);
    let mut last_signature = String::new();

    loop {
        let (run, jobs) = get_run_and_jobs(client, repo, run_id).await?;
        let signature = run_status_signature(&run, &jobs);
        if signature != last_signature {
            print_watch_snapshot(&run, &jobs);
            last_signature = signature;
        }

        if is_terminal_run(&run) {
            ensure_successful_run(&run)?;
            return Ok(());
        }

        if let Some(timeout) = timeout
            && started.elapsed() >= timeout
        {
            bail!("Timed out waiting for run {run_id}");
        }

        tokio::time::sleep(interval).await;
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn handle_cancel(client: &Client, repo: &str, run_id: u64) -> Result<()> {
    client
        .post_empty(&format!("/repos/{repo}/actions/runs/{run_id}/cancel"))
        .await?;
    println!("Cancelled run {run_id}");
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn handle_rerun(client: &Client, repo: &str, run_id: u64, failed: bool) -> Result<()> {
    let path = if failed {
        format!("/repos/{repo}/actions/runs/{run_id}/rerun-failed-jobs")
    } else {
        format!("/repos/{repo}/actions/runs/{run_id}/rerun")
    };
    client.post_empty(&path).await?;
    let scope = if failed { "failed jobs in" } else { "" };
    println!("Re-running {scope} run {run_id}");
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn list_runs(
    client: &Client,
    repo: &str,
    limit: u32,
    status: Option<&str>,
    branch: Option<&str>,
) -> Result<serde_json::Value> {
    let mut path = format!("/repos/{repo}/actions/runs?per_page={limit}");
    if let Some(s) = status {
        path.push_str(&format!("&status={s}"));
    }
    if let Some(b) = branch {
        path.push_str(&format!("&branch={}", urlencoding::encode(b)));
    }
    client.get(&path).await
}

fn format_duration_secs(secs: i64) -> String {
    if secs < 0 {
        return "-".to_string();
    }
    let h = secs / 3600;
    let m = (secs % 3600) / 60;
    let s = secs % 60;
    if h > 0 {
        format!("{h}h{m:02}m{s:02}s")
    } else if m > 0 {
        format!("{m}m{s:02}s")
    } else {
        format!("{s}s")
    }
}

fn duration_between(start: &str, end: &str) -> String {
    let parse = |s: &str| -> Option<i64> {
        // RFC3339 basic parse: strip trailing Z, split on +, take first part
        let s = s.trim_end_matches('Z');
        let s = s.split('+').next().unwrap_or(s);
        // format: 2024-01-15T10:30:00
        let parts: Vec<&str> = s.splitn(2, 'T').collect();
        if parts.len() != 2 {
            return None;
        }
        let date_parts: Vec<i64> = parts[0].split('-').filter_map(|p| p.parse().ok()).collect();
        let time_parts: Vec<i64> = parts[1].split(':').filter_map(|p| p.parse().ok()).collect();
        if date_parts.len() < 3 || time_parts.len() < 3 {
            return None;
        }
        // Days since epoch (simplified, good enough for duration diffs within same year/month)
        let days = date_parts[0] * 365 + date_parts[1] * 30 + date_parts[2];
        let total_secs = days * 86400 + time_parts[0] * 3600 + time_parts[1] * 60 + time_parts[2];
        Some(total_secs)
    };

    match (parse(start), parse(end)) {
        (Some(s), Some(e)) => format_duration_secs(e - s),
        _ => "-".to_string(),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn print_runs(value: &serde_json::Value, workflow_filter: Option<&str>, limit: u32) {
    let runs = value["workflow_runs"]
        .as_array()
        .map_or(&[][..], |r| r.as_slice());
    print_runs_header();
    for run in runs
        .iter()
        .filter(|run| run_matches_workflow_filter(run, workflow_filter))
        .take(limit as usize)
    {
        print_run_row(run);
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn print_runs_header() {
    println!(
        "{:<12} {:<12} {:<12} {:<30} {:<20} {:<10} {:<12} Duration",
        "ID", "Status", "Conclusion", "Workflow", "Branch", "Event", "Created"
    );
    println!("{}", "-".repeat(120));
}

fn run_matches_workflow_filter(run: &serde_json::Value, workflow_filter: Option<&str>) -> bool {
    let Some(filter) = workflow_filter else {
        return true;
    };
    let workflow_name = run["name"].as_str().unwrap_or("");
    let workflow_file = run["path"].as_str().unwrap_or("");
    workflow_name.contains(filter) || workflow_file.contains(filter)
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn print_run_row(run: &serde_json::Value) {
    let id = run["id"].as_u64().unwrap_or(0);
    let status = run["status"].as_str().unwrap_or("-");
    let conclusion = run["conclusion"].as_str().unwrap_or("-");
    let workflow_name = run["name"].as_str().unwrap_or("");
    let branch = run["head_branch"].as_str().unwrap_or("-");
    let event = run["event"].as_str().unwrap_or("-");
    let created = run["created_at"]
        .as_str()
        .unwrap_or("")
        .split('T')
        .next()
        .unwrap_or("-");
    let duration = run_duration(run);
    let name_truncated = truncate_with_ellipsis(workflow_name, 29, 28);
    let branch_truncated = truncate_with_ellipsis(branch, 19, 18);
    println!(
        "{:<12} {:<12} {:<12} {:<30} {:<20} {:<10} {:<12} {}",
        id, status, conclusion, name_truncated, branch_truncated, event, created, duration
    );
}

fn truncate_with_ellipsis(value: &str, max_len: usize, prefix_len: usize) -> String {
    if value.len() > max_len {
        return format!("{}…", &value[..prefix_len]);
    }
    value.to_string()
}

fn run_duration(run: &serde_json::Value) -> String {
    let started = run["run_started_at"].as_str().unwrap_or("");
    if started.is_empty() {
        return "-".to_string();
    }
    let updated = run["updated_at"].as_str().unwrap_or("");
    duration_between(started, updated)
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn print_job_detail(job: &serde_json::Value) {
    let id = job["id"].as_u64().unwrap_or(0);
    let name = job["name"].as_str().unwrap_or("");
    let status = job["status"].as_str().unwrap_or("");
    let conclusion = job["conclusion"].as_str().unwrap_or("");
    let started = job["started_at"].as_str().unwrap_or("");
    let completed = job["completed_at"].as_str().unwrap_or("");
    let runner_id = job["runner_id"].as_u64().unwrap_or(0);
    let runner_name = job["runner_name"].as_str().unwrap_or("");
    let runner_group_id = job["runner_group_id"].as_u64().unwrap_or(0);
    let runner_group_name = job["runner_group_name"].as_str().unwrap_or("");
    let labels = string_array(&job["labels"]);

    println!("Job: {name}");
    println!("ID: {id}");
    println!("Status: {status}");
    println!("Conclusion: {conclusion}");
    println!("Started: {started}");
    println!("Completed: {completed}");
    println!("Runner ID: {runner_id}");
    println!("Runner name: {runner_name}");
    println!("Runner group ID: {runner_group_id}");
    println!("Runner group name: {runner_group_name}");
    println!("Labels: {}", labels.join(", "));
    println!();
    print_job_steps(job);
}

fn string_array(value: &serde_json::Value) -> Vec<&str> {
    value
        .as_array()
        .map(|items| items.iter().filter_map(|item| item.as_str()).collect())
        .unwrap_or_default()
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn print_job_steps(job: &serde_json::Value) {
    let steps = job["steps"].as_array().map_or(&[][..], |s| s.as_slice());
    if steps.is_empty() {
        println!("No steps reported");
        return;
    }
    println!(
        "{:<6} {:<35} {:<12} {:<12} Duration",
        "Num", "Step", "Status", "Conclusion"
    );
    println!("{}", "-".repeat(90));
    for step in steps {
        let number = step["number"].as_u64().unwrap_or(0);
        let name = step["name"].as_str().unwrap_or("");
        let status = step["status"].as_str().unwrap_or("");
        let conclusion = step["conclusion"].as_str().unwrap_or("");
        let started = step["started_at"].as_str().unwrap_or("");
        let completed = step["completed_at"].as_str().unwrap_or("");
        println!(
            "{number:<6} {:<35} {:<12} {:<12} {}",
            truncate_with_ellipsis(name, 34, 33),
            status,
            conclusion,
            duration_between(started, completed)
        );
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn print_run_detail(run: &serde_json::Value, jobs_value: &serde_json::Value) {
    print_run_summary(run);
    print_run_jobs(jobs_value);
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn print_watch_snapshot(run: &serde_json::Value, jobs_value: &serde_json::Value) {
    print_run_summary(run);
    let (completed, in_progress, queued, failed) = count_jobs(jobs_value);
    println!(
        "Jobs: {completed} completed, {in_progress} in_progress, {queued} queued, {failed} failed/cancelled"
    );
    print_active_jobs(jobs_value);
    println!();
}

fn count_jobs(jobs_value: &serde_json::Value) -> (usize, usize, usize, usize) {
    let jobs = jobs_value["jobs"]
        .as_array()
        .map_or(&[][..], |j| j.as_slice());
    let completed = jobs
        .iter()
        .filter(|job| job["status"].as_str() == Some("completed"))
        .count();
    let in_progress = jobs
        .iter()
        .filter(|job| job["status"].as_str() == Some("in_progress"))
        .count();
    let queued = jobs
        .iter()
        .filter(|job| job["status"].as_str() == Some("queued"))
        .count();
    let failed = jobs.iter().filter(|job| is_failed_job(job)).count();
    (completed, in_progress, queued, failed)
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn print_active_jobs(jobs_value: &serde_json::Value) {
    let jobs = jobs_value["jobs"]
        .as_array()
        .map_or(&[][..], |j| j.as_slice());
    let active_jobs: Vec<&serde_json::Value> = jobs
        .iter()
        .filter(|job| job["status"].as_str() != Some("completed"))
        .collect();

    if active_jobs.is_empty() {
        print_run_jobs(jobs_value);
        return;
    }

    println!(
        "{:<40} {:<12} {:<12} Active step",
        "Active job", "Status", "Conclusion"
    );
    println!("{}", "-".repeat(90));
    for job in active_jobs {
        let name = job["name"].as_str().unwrap_or("-");
        let status = job["status"].as_str().unwrap_or("-");
        let conclusion = job["conclusion"].as_str().unwrap_or("-");
        let active_step = active_step_name(job);
        let name_truncated = truncate_with_ellipsis(name, 39, 38);
        println!(
            "{:<40} {:<12} {:<12} {}",
            name_truncated, status, conclusion, active_step
        );
    }
}

fn active_step_name(job: &serde_json::Value) -> &str {
    let steps = job["steps"].as_array().map_or(&[][..], |s| s.as_slice());
    steps
        .iter()
        .find(|step| step["status"].as_str() == Some("in_progress"))
        .and_then(|step| step["name"].as_str())
        .or_else(|| {
            steps
                .iter()
                .find(|step| step["status"].as_str() == Some("queued"))
                .and_then(|step| step["name"].as_str())
        })
        .unwrap_or("-")
}

fn run_status_signature(run: &serde_json::Value, jobs_value: &serde_json::Value) -> String {
    let mut parts = vec![
        run["status"].as_str().unwrap_or("-").to_string(),
        run["conclusion"].as_str().unwrap_or("-").to_string(),
        run["updated_at"].as_str().unwrap_or("-").to_string(),
    ];
    let jobs = jobs_value["jobs"]
        .as_array()
        .map_or(&[][..], |j| j.as_slice());
    for job in jobs {
        parts.push(format!(
            "{}:{}:{}:{}",
            job["id"].as_u64().unwrap_or(0),
            job["status"].as_str().unwrap_or("-"),
            job["conclusion"].as_str().unwrap_or("-"),
            active_step_name(job)
        ));
    }
    parts.join("|")
}

fn is_terminal_run(run: &serde_json::Value) -> bool {
    run["status"].as_str() == Some("completed")
}

fn ensure_successful_run(run: &serde_json::Value) -> Result<()> {
    match run["conclusion"].as_str().unwrap_or("") {
        "success" => Ok(()),
        conclusion => bail!("Run completed with conclusion: {conclusion}"),
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn print_run_summary(run: &serde_json::Value) {
    let id = run["id"].as_u64().unwrap_or(0);
    let name = run["name"].as_str().unwrap_or("");
    let status = run["status"].as_str().unwrap_or("-");
    let conclusion = run["conclusion"].as_str().unwrap_or("-");
    let branch = run["head_branch"].as_str().unwrap_or("-");
    let event = run["event"].as_str().unwrap_or("-");
    let created = run["created_at"].as_str().unwrap_or("-");
    let duration = run_duration(run);
    let url = run["html_url"].as_str().unwrap_or("");
    println!("Run: {name} (#{id})");
    println!("Status: {status}  Conclusion: {conclusion}");
    println!("Branch: {branch}  Event: {event}");
    println!("Created: {created}  Duration: {duration}");
    println!("URL: {url}");
    println!();
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn print_run_jobs(jobs_value: &serde_json::Value) {
    let jobs = jobs_value["jobs"]
        .as_array()
        .map_or(&[][..], |j| j.as_slice());
    if jobs.is_empty() {
        println!("No jobs found");
        return;
    }
    println!(
        "{:<40} {:<12} {:<12} Duration",
        "Job", "Status", "Conclusion"
    );
    println!("{}", "-".repeat(80));
    for job in jobs {
        print_run_job_row(job);
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn print_run_job_row(job: &serde_json::Value) {
    let job_name = job["name"].as_str().unwrap_or("-");
    let job_status = job["status"].as_str().unwrap_or("-");
    let job_conclusion = job["conclusion"].as_str().unwrap_or("-");
    let job_started = job["started_at"].as_str().unwrap_or("");
    let job_completed = job["completed_at"].as_str().unwrap_or("");
    let job_duration = if job_started.is_empty() {
        "-".to_string()
    } else {
        duration_between(job_started, job_completed)
    };
    let name_truncated = truncate_with_ellipsis(job_name, 39, 38);
    println!(
        "{:<40} {:<12} {:<12} {}",
        name_truncated, job_status, job_conclusion, job_duration
    );
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn show_logs(
    client: &Client,
    repo: &str,
    run_id: u64,
    job_filter: Option<&str>,
) -> Result<()> {
    let jobs_value = client
        .get(&format!("/repos/{repo}/actions/runs/{run_id}/jobs"))
        .await?;
    let jobs = jobs_value["jobs"]
        .as_array()
        .map_or(&[][..], |j| j.as_slice());
    let target_jobs = select_target_jobs(jobs, job_filter);
    ensure_target_jobs_exist(&target_jobs, run_id, job_filter)?;
    for job in target_jobs {
        print_job_logs(client, repo, job).await?;
    }
    Ok(())
}

fn select_target_jobs<'a>(
    jobs: &'a [serde_json::Value],
    job_filter: Option<&str>,
) -> Vec<&'a serde_json::Value> {
    if let Some(name) = job_filter {
        return jobs
            .iter()
            .filter(|job| job["name"].as_str().unwrap_or("").contains(name))
            .collect();
    }
    jobs.iter().filter(|job| is_failed_job(job)).collect()
}

fn is_failed_job(job: &serde_json::Value) -> bool {
    let conclusion = job["conclusion"].as_str().unwrap_or("");
    conclusion == "failure" || conclusion == "cancelled"
}

fn ensure_target_jobs_exist(
    target_jobs: &[&serde_json::Value],
    run_id: u64,
    job_filter: Option<&str>,
) -> Result<()> {
    if !target_jobs.is_empty() {
        return Ok(());
    }
    if job_filter.is_some() {
        bail!("No job matching that name found in run {run_id}");
    }
    println!("No failed jobs in run {run_id}");
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
async fn print_job_logs(client: &Client, repo: &str, job: &serde_json::Value) -> Result<()> {
    let job_id = job["id"].as_u64().unwrap_or(0);
    let job_name = job["name"].as_str().unwrap_or("?");
    println!("=== Job: {job_name} (id: {job_id}) ===");
    let log_bytes = client
        .get_bytes(&format!("/repos/{repo}/actions/jobs/{job_id}/logs"))
        .await?;
    extract_and_print_logs(&log_bytes, job_name)?;
    println!();
    Ok(())
}

#[cfg_attr(coverage_nightly, coverage(off))]
fn extract_and_print_logs(data: &bytes::Bytes, job_name: &str) -> Result<()> {
    // The API redirects to a zip archive. reqwest follows the redirect, so we receive the zip.
    if data.starts_with(b"PK") {
        let cursor = std::io::Cursor::new(data.as_ref());
        let mut archive = match zip::ZipArchive::new(cursor) {
            Ok(a) => a,
            Err(e) => {
                eprintln!("Failed to open zip archive for job '{job_name}': {e}");
                return Ok(());
            }
        };
        for i in 0..archive.len() {
            let mut file = archive.by_index(i)?;
            let mut contents = String::new();
            file.read_to_string(&mut contents)?;
            print!("{contents}");
        }
    } else {
        // Plain text fallback (some endpoints return raw log text)
        let text = std::str::from_utf8(data)?;
        print!("{text}");
    }
    Ok(())
}

#[cfg(test)]
#[path = "run_tests.rs"]
mod tests;
