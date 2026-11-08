use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use serde::Serialize;

use crate::client::Client;

#[cfg(test)]
#[path = "pr_branch_tests.rs"]
mod tests;

/// Check run conclusions and commit status states share these names.
const PASSING_OUTCOMES: &[&str] = &["success", "neutral", "skipped"];
const FAILING_OUTCOMES: &[&str] = &[
    "failure",
    "error",
    "timed_out",
    "cancelled",
    "action_required",
    "startup_failure",
];

#[derive(Debug, Default, PartialEq, Serialize)]
pub(crate) struct Checks {
    passed: u32,
    running: u32,
    failed: u32,
}

#[derive(Debug, PartialEq, Serialize)]
pub(crate) struct BranchPullRequest {
    number: u64,
    url: String,
    state: String,
    checks: Checks,
}

pub async fn handle_branch(client: &Client, branch: &str, json: bool) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let pull_request = branch_pull_request(client, &cwd, branch).await?;
    if json {
        println!("{}", serde_json::to_string(&pull_request)?);
        return Ok(());
    }
    match pull_request {
        Some(pr) => println!(
            "#{} {} passed {} running {} failed {} {}",
            pr.number, pr.state, pr.checks.passed, pr.checks.running, pr.checks.failed, pr.url
        ),
        None => println!("No pull request for {branch}"),
    }
    Ok(())
}

/// The branch's pull request lives in the repository it is pushed to; an unpushed branch has none.
pub(crate) async fn branch_pull_request(
    client: &Client,
    cwd: &Path,
    branch: &str,
) -> Result<Option<BranchPullRequest>> {
    let Some(repo) = push_repository(cwd, branch)? else {
        return Ok(None);
    };
    let owner = repo
        .split_once('/')
        .map_or(repo.as_str(), |(owner, _)| owner);
    // `owner:branch` matches only heads in that owner's repository, so fork branches with the same name stay out.
    let head = urlencoding::encode(&format!("{owner}:{branch}")).into_owned();
    let pulls = client
        .get(&format!(
            "/repos/{repo}/pulls?head={head}&state=all&per_page=1"
        ))
        .await?;
    let Some(pull) = pulls.as_array().and_then(|pulls| pulls.first()) else {
        return Ok(None);
    };
    let sha = pull["head"]["sha"]
        .as_str()
        .context("Pull request has no head SHA")?;
    Ok(Some(BranchPullRequest {
        number: pull["number"]
            .as_u64()
            .context("Pull request has no number")?,
        url: pull["html_url"]
            .as_str()
            .context("Pull request has no URL")?
            .to_owned(),
        state: pull_state(pull)?,
        checks: commit_checks(client, &repo, sha).await?,
    }))
}

fn pull_state(pull: &serde_json::Value) -> Result<String> {
    if !pull["merged_at"].is_null() {
        return Ok("merged".to_owned());
    }
    match pull["state"].as_str() {
        Some(state @ ("open" | "closed")) => Ok(state.to_owned()),
        other => bail!("Unknown pull request state: {other:?}"),
    }
}

/// GitHub reports Actions results as check runs and external CI results as commit statuses.
async fn commit_checks(client: &Client, repo: &str, sha: &str) -> Result<Checks> {
    let runs = client
        .get(&format!(
            "/repos/{repo}/commits/{sha}/check-runs?per_page=100"
        ))
        .await?;
    let statuses = client
        .get(&format!("/repos/{repo}/commits/{sha}/status"))
        .await?;
    let mut checks = Checks::default();
    let run_outcomes = runs["check_runs"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|run| &run["conclusion"]);
    let status_outcomes = statuses["statuses"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|status| &status["state"]);
    for outcome in run_outcomes.chain(status_outcomes) {
        count_outcome(&mut checks, outcome.as_str().unwrap_or(""));
    }
    Ok(checks)
}

/// Queued, in-progress, and unknown results (no conclusion yet) must never look green.
fn count_outcome(checks: &mut Checks, outcome: &str) {
    if FAILING_OUTCOMES.contains(&outcome) {
        checks.failed += 1;
    } else if PASSING_OUTCOMES.contains(&outcome) {
        checks.passed += 1;
    } else {
        checks.running += 1;
    }
}

/// `None` when the branch has no push remote or that remote is not a GitHub repository.
pub(crate) fn push_repository(cwd: &Path, branch: &str) -> Result<Option<String>> {
    let remote = git(
        cwd,
        &[
            "for-each-ref",
            "--format=%(push:remotename)",
            &format!("refs/heads/{branch}"),
        ],
    )?;
    if remote.is_empty() {
        return Ok(None);
    }
    let url = git(cwd, &["remote", "get-url", &remote])?;
    Ok(github_repository(&url))
}

fn git(cwd: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .args(args)
        .current_dir(cwd)
        .output()
        .context("Failed to run git")?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

/// `owner/repo` from scp-style, SSH, and HTTPS github.com remote URLs.
pub(crate) fn github_repository(url: &str) -> Option<String> {
    let (before_host, after_host) = url.split_once("github.com")?;
    let host_starts_url =
        before_host.is_empty() || before_host.ends_with('@') || before_host.ends_with("//");
    let path = after_host.strip_prefix([':', '/'])?.trim_end_matches('/');
    let path = path.strip_suffix(".git").unwrap_or(path);
    let (owner, repo) = path.split_once('/')?;
    let valid = host_starts_url && !owner.is_empty() && !repo.is_empty() && !repo.contains('/');
    valid.then(|| format!("{owner}/{repo}"))
}
