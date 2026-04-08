use anyhow::{Result, bail};
use clap::Subcommand;
use std::io::Read;

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

pub async fn handle(client: &Client, command: RunCommands) -> Result<()> {
    match command {
        RunCommands::List { repo, limit, status, branch, workflow } => {
            handle_list(client, &repo, limit, status.as_deref(), branch.as_deref(), workflow.as_deref()).await?
        }
        RunCommands::View { repo, run_id } => handle_view(client, &repo, run_id).await?,
        RunCommands::Logs { repo, run_id, job } => {
            show_logs(client, &repo, run_id, job.as_deref()).await?
        }
        RunCommands::Cancel { repo, run_id } => handle_cancel(client, &repo, run_id).await?,
        RunCommands::Rerun { repo, run_id, failed } => handle_rerun(client, &repo, run_id, failed).await?,
    }
    Ok(())
}

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

async fn handle_view(client: &Client, repo: &str, run_id: u64) -> Result<()> {
    let run = client.get(&format!("/repos/{repo}/actions/runs/{run_id}")).await?;
    let jobs = client.get(&format!("/repos/{repo}/actions/runs/{run_id}/jobs")).await?;
    print_run_detail(&run, &jobs);
    Ok(())
}

async fn handle_cancel(client: &Client, repo: &str, run_id: u64) -> Result<()> {
    client
        .post_empty(&format!("/repos/{repo}/actions/runs/{run_id}/cancel"))
        .await?;
    println!("Cancelled run {run_id}");
    Ok(())
}

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

fn print_runs(value: &serde_json::Value, workflow_filter: Option<&str>, limit: u32) {
    let empty = vec![];
    let runs = value["workflow_runs"].as_array().unwrap_or(&empty);

    println!(
        "{:<12} {:<12} {:<12} {:<30} {:<20} {:<10} {:<12} {}",
        "ID", "Status", "Conclusion", "Workflow", "Branch", "Event", "Created", "Duration"
    );
    println!("{}", "-".repeat(120));

    let mut count = 0u32;
    for run in runs {
        let workflow_name = run["name"].as_str().unwrap_or("");
        let workflow_file = run["path"].as_str().unwrap_or("");
        if let Some(filter) = workflow_filter {
            let matches = workflow_name.contains(filter) || workflow_file.contains(filter);
            if !matches {
                continue;
            }
        }
        if count >= limit {
            break;
        }
        count += 1;

        let id = run["id"].as_u64().unwrap_or(0);
        let status = run["status"].as_str().unwrap_or("-");
        let conclusion = run["conclusion"].as_str().unwrap_or("-");
        let branch = run["head_branch"].as_str().unwrap_or("-");
        let event = run["event"].as_str().unwrap_or("-");
        let created = run["created_at"]
            .as_str()
            .unwrap_or("")
            .split('T')
            .next()
            .unwrap_or("-");
        let started = run["run_started_at"].as_str().unwrap_or("");
        let updated = run["updated_at"].as_str().unwrap_or("");
        let duration = if started.is_empty() {
            "-".to_string()
        } else {
            duration_between(started, updated)
        };

        let name_truncated = if workflow_name.len() > 29 {
            format!("{}…", &workflow_name[..28])
        } else {
            workflow_name.to_string()
        };
        let branch_truncated = if branch.len() > 19 {
            format!("{}…", &branch[..18])
        } else {
            branch.to_string()
        };

        println!(
            "{:<12} {:<12} {:<12} {:<30} {:<20} {:<10} {:<12} {}",
            id, status, conclusion, name_truncated, branch_truncated, event, created, duration
        );
    }
}

fn print_run_detail(run: &serde_json::Value, jobs_value: &serde_json::Value) {
    let id = run["id"].as_u64().unwrap_or(0);
    let name = run["name"].as_str().unwrap_or("");
    let status = run["status"].as_str().unwrap_or("-");
    let conclusion = run["conclusion"].as_str().unwrap_or("-");
    let branch = run["head_branch"].as_str().unwrap_or("-");
    let event = run["event"].as_str().unwrap_or("-");
    let created = run["created_at"].as_str().unwrap_or("-");
    let started = run["run_started_at"].as_str().unwrap_or("");
    let updated = run["updated_at"].as_str().unwrap_or("");
    let duration = if started.is_empty() {
        "-".to_string()
    } else {
        duration_between(started, updated)
    };
    let url = run["html_url"].as_str().unwrap_or("");

    println!("Run: {name} (#{id})");
    println!("Status: {status}  Conclusion: {conclusion}");
    println!("Branch: {branch}  Event: {event}");
    println!("Created: {created}  Duration: {duration}");
    println!("URL: {url}");
    println!();

    let empty = vec![];
    let jobs = jobs_value["jobs"].as_array().unwrap_or(&empty);
    if jobs.is_empty() {
        println!("No jobs found");
        return;
    }

    println!(
        "{:<40} {:<12} {:<12} {}",
        "Job", "Status", "Conclusion", "Duration"
    );
    println!("{}", "-".repeat(80));
    for job in jobs {
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
        let name_truncated = if job_name.len() > 39 {
            format!("{}…", &job_name[..38])
        } else {
            job_name.to_string()
        };
        println!(
            "{:<40} {:<12} {:<12} {}",
            name_truncated, job_status, job_conclusion, job_duration
        );
    }
}

async fn show_logs(
    client: &Client,
    repo: &str,
    run_id: u64,
    job_filter: Option<&str>,
) -> Result<()> {
    let jobs_value = client
        .get(&format!("/repos/{repo}/actions/runs/{run_id}/jobs"))
        .await?;
    let empty = vec![];
    let jobs = jobs_value["jobs"].as_array().unwrap_or(&empty);

    let target_jobs: Vec<&serde_json::Value> = if let Some(name) = job_filter {
        jobs.iter()
            .filter(|j| j["name"].as_str().unwrap_or("").contains(name))
            .collect()
    } else {
        // Default: show failed jobs
        jobs.iter()
            .filter(|j| {
                let conclusion = j["conclusion"].as_str().unwrap_or("");
                conclusion == "failure" || conclusion == "cancelled"
            })
            .collect()
    };

    if target_jobs.is_empty() {
        if job_filter.is_some() {
            bail!("No job matching that name found in run {run_id}");
        } else {
            println!("No failed jobs in run {run_id}");
            return Ok(());
        }
    }

    for job in target_jobs {
        let job_id = job["id"].as_u64().unwrap_or(0);
        let job_name = job["name"].as_str().unwrap_or("?");
        println!("=== Job: {job_name} (id: {job_id}) ===");
        let log_bytes = client
            .get_bytes(&format!("/repos/{repo}/actions/jobs/{job_id}/logs"))
            .await?;
        extract_and_print_logs(&log_bytes, job_name)?;
        println!();
    }

    Ok(())
}

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
