#![cfg_attr(coverage_nightly, coverage(off))]

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand};

use crate::client::Client;
use crate::commands::issue::{print_issue_comments, print_issue_detail};
use crate::commands::issue_fields::MetadataArgs;

#[cfg(test)]
#[path = "pr_write_tests.rs"]
mod pr_write_tests;

#[derive(Args)]
pub struct CreatePrArgs {
    /// Pull request title
    #[arg(short, long)]
    title: String,
    /// Pull request body
    #[arg(short, long)]
    body: Option<String>,
    /// Existing head branch (or owner:branch for a fork)
    #[arg(long)]
    head: String,
    /// Target branch
    #[arg(long)]
    base: String,
    /// Create as a draft
    #[arg(long)]
    draft: bool,
    #[command(flatten)]
    metadata: MetadataArgs,
}

#[derive(Args)]
pub struct EditPrArgs {
    /// Replacement title
    #[arg(short, long)]
    title: Option<String>,
    /// Replacement body (empty string clears it)
    #[arg(short, long)]
    body: Option<String>,
    /// Replacement target branch
    #[arg(long)]
    base: Option<String>,
    #[command(flatten)]
    metadata: MetadataArgs,
}

#[derive(Subcommand)]
pub enum PrCommands {
    /// Create a pull request
    Create {
        /// Repository (owner/repo)
        repo: String,
        #[command(flatten)]
        fields: CreatePrArgs,
    },
    /// Edit a pull request; omitted fields remain unchanged
    Edit {
        /// Repository (owner/repo)
        repo: String,
        /// Pull request number
        number: u64,
        #[command(flatten)]
        fields: EditPrArgs,
    },
    /// Close a pull request
    Close { repo: String, number: u64 },
    /// Reopen a pull request
    Reopen { repo: String, number: u64 },
    /// List pull requests
    List {
        /// Repository (owner/repo)
        repo: String,
        /// State (open, closed, all)
        #[arg(short, long, default_value = "open")]
        state: String,
        /// Limit results
        #[arg(short, long, default_value = "10")]
        limit: u32,
    },
    /// Show the pull request for a local branch, in the repository the branch is pushed to
    Branch {
        /// Local branch name
        branch: String,
        /// Print JSON ({number, url, state, checks} or null)
        #[arg(long)]
        json: bool,
    },
    /// View PR details
    View {
        /// Repository (owner/repo)
        repo: String,
        /// PR number
        number: u64,
    },
    /// Post a comment on a PR
    Comment {
        /// Repository (owner/repo)
        repo: String,
        /// PR number
        number: u64,
        /// Comment body
        #[arg(short, long)]
        message: String,
    },
    /// List comments on a PR
    Comments {
        /// Repository (owner/repo)
        repo: String,
        /// PR number
        number: u64,
    },
    /// Approve a PR
    Approve {
        /// Repository (owner/repo)
        repo: String,
        /// PR number
        number: u64,
    },
    /// Merge a pull request
    Merge {
        /// Repository (owner/repo)
        repo: String,
        /// PR number
        number: u64,
        /// Create a merge commit (default)
        #[arg(long, conflicts_with_all = ["squash", "rebase"])]
        merge: bool,
        /// Squash commits
        #[arg(long, conflicts_with_all = ["merge", "rebase"])]
        squash: bool,
        /// Rebase commits
        #[arg(long, conflicts_with_all = ["merge", "squash"])]
        rebase: bool,
        /// Expected head SHA
        #[arg(long)]
        sha: Option<String>,
        /// Delete a same-repository head branch after merging
        #[arg(long)]
        delete_branch: bool,
    },
    /// List review comments (discussions) on a PR
    Discussions {
        /// Repository (owner/repo)
        repo: String,
        /// PR number
        number: u64,
        /// Show only unresolved threads
        #[arg(long)]
        unresolved: bool,
    },
    /// Reply to a review comment
    Reply {
        /// Repository (owner/repo)
        repo: String,
        /// PR number
        number: u64,
        /// Comment ID to reply to
        #[arg(long)]
        comment: u64,
        /// Reply body
        #[arg(short, long)]
        message: String,
    },
    /// Submit a review with inline comments
    Review {
        /// Repository (owner/repo)
        repo: String,
        /// PR number
        number: u64,
        /// Review summary body
        #[arg(short = 'b', long)]
        body: Option<String>,
        /// Review event: COMMENT, APPROVE, REQUEST_CHANGES
        #[arg(short, long, default_value = "COMMENT")]
        event: String,
        /// Inline comment (repeatable): path:line:body
        #[arg(long = "comment", short = 'c')]
        comments: Vec<String>,
    },
}

pub async fn handle(client: &Client, command: PrCommands) -> Result<()> {
    match command {
        PrCommands::Create { repo, fields } => create_pr(client, &repo, fields).await?,
        PrCommands::Edit {
            repo,
            number,
            fields,
        } => edit_pr(client, &repo, number, fields).await?,
        PrCommands::Close { repo, number } => set_state(client, &repo, number, "closed").await?,
        PrCommands::Reopen { repo, number } => set_state(client, &repo, number, "open").await?,
        PrCommands::List { repo, state, limit } => {
            handle_list(client, &repo, &state, limit).await?
        }
        PrCommands::Branch { branch, json } => {
            crate::commands::pr_branch::handle_branch(client, &branch, json).await?
        }
        PrCommands::View { repo, number } => handle_view(client, &repo, number).await?,
        PrCommands::Comment {
            repo,
            number,
            message,
        } => handle_comment(client, &repo, number, &message).await?,
        PrCommands::Comments { repo, number } => handle_comments(client, &repo, number).await?,
        PrCommands::Approve { repo, number } => handle_approve(client, &repo, number).await?,
        PrCommands::Merge {
            repo,
            number,
            merge: _,
            squash,
            rebase,
            sha,
            delete_branch,
        } => {
            let method = if squash {
                "squash"
            } else if rebase {
                "rebase"
            } else {
                "merge"
            };
            merge_pr(client, &repo, number, method, sha.as_deref(), delete_branch).await?;
        }
        PrCommands::Discussions {
            repo,
            number,
            unresolved: _,
        } => handle_discussions(client, &repo, number).await?,
        PrCommands::Reply {
            repo,
            number,
            comment,
            message,
        } => handle_reply(client, &repo, number, comment, &message).await?,
        PrCommands::Review {
            repo,
            number,
            body,
            event,
            comments,
        } => handle_review(client, &repo, number, body, event, comments).await?,
    }
    Ok(())
}

async fn create_pr(client: &Client, repo: &str, fields: CreatePrArgs) -> Result<()> {
    let mut payload = serde_json::json!({"title": fields.title, "head": fields.head, "base": fields.base, "draft": fields.draft});
    if let Some(body) = fields.body {
        payload["body"] = body.into();
    }
    let result = client
        .post(&format!("/repos/{repo}/pulls"), &payload)
        .await?;
    let number = result["number"]
        .as_u64()
        .context("Pull request created, but response has no number")?;
    if !fields.metadata.is_empty() {
        update_metadata(client, repo, number, &fields.metadata)
            .await
            .with_context(|| format!("Created {repo}#{number}, but failed to update metadata"))?;
    }
    println!("Created {repo}#{number}");
    Ok(())
}

async fn edit_pr(client: &Client, repo: &str, number: u64, fields: EditPrArgs) -> Result<()> {
    let payload = pr_edit_payload(&fields);
    let has_pull_changes = payload != serde_json::json!({});
    if !has_pull_changes && fields.metadata.is_empty() {
        bail!("No changes specified for {repo}#{number}");
    }
    if has_pull_changes {
        client
            .patch(&format!("/repos/{repo}/pulls/{number}"), &payload)
            .await?;
    }
    if !fields.metadata.is_empty() {
        let status = if has_pull_changes {
            "Updated"
        } else {
            "No pull fields changed for"
        };
        update_metadata(client, repo, number, &fields.metadata)
            .await
            .with_context(|| format!("{status} {repo}#{number}, but failed to update metadata"))?;
    }
    println!("Updated {repo}#{number}");
    Ok(())
}

fn pr_edit_payload(fields: &EditPrArgs) -> serde_json::Value {
    let mut payload = serde_json::json!({});
    if let Some(title) = &fields.title {
        payload["title"] = title.clone().into();
    }
    if let Some(body) = &fields.body {
        payload["body"] = body.clone().into();
    }
    if let Some(base) = &fields.base {
        payload["base"] = base.clone().into();
    }
    payload
}

async fn update_metadata(
    client: &Client,
    repo: &str,
    number: u64,
    metadata: &MetadataArgs,
) -> Result<()> {
    let mut payload = serde_json::json!({});
    metadata.append_to(&mut payload);
    client
        .patch(&format!("/repos/{repo}/issues/{number}"), &payload)
        .await?;
    Ok(())
}

async fn set_state(client: &Client, repo: &str, number: u64, state: &str) -> Result<()> {
    client
        .patch(
            &format!("/repos/{repo}/pulls/{number}"),
            &serde_json::json!({"state": state}),
        )
        .await?;
    println!("Set {repo}#{number} state to {state}");
    Ok(())
}

async fn handle_list(client: &Client, repo: &str, state: &str, limit: u32) -> Result<()> {
    let result = list_prs(client, repo, state, limit).await?;
    print_prs(&result);
    Ok(())
}

async fn handle_view(client: &Client, repo: &str, number: u64) -> Result<()> {
    let result = get_pr(client, repo, number).await?;
    print_issue_detail(&result);
    Ok(())
}

async fn handle_comment(client: &Client, repo: &str, number: u64, message: &str) -> Result<()> {
    let result = comment_on_issue(client, repo, number, message).await?;
    let id = result["id"].as_u64().unwrap_or(0);
    println!("Posted comment (id: {id}) on {repo}#{number}");
    Ok(())
}

async fn handle_comments(client: &Client, repo: &str, number: u64) -> Result<()> {
    let result = list_issue_comments(client, repo, number).await?;
    print_issue_comments(&result);
    Ok(())
}

async fn handle_approve(client: &Client, repo: &str, number: u64) -> Result<()> {
    approve_pr(client, repo, number).await?;
    println!("Approved {repo}#{number}");
    Ok(())
}

async fn merge_pr(
    client: &Client,
    repo: &str,
    number: u64,
    method: &str,
    sha: Option<&str>,
    delete_branch: bool,
) -> Result<()> {
    let mut body = serde_json::json!({ "merge_method": method });
    if let Some(sha) = sha {
        body["sha"] = serde_json::json!(sha);
    }
    let result = client
        .put(&format!("/repos/{repo}/pulls/{number}/merge"), &body)
        .await?;
    if result["merged"].as_bool() != Some(true) {
        bail!(
            "Merge failed: {}",
            result["message"]
                .as_str()
                .unwrap_or("missing merge confirmation")
        );
    }
    let merge_sha = result["sha"]
        .as_str()
        .context("Merge succeeded but response has no SHA")?;
    println!("Merged {repo}#{number}: {merge_sha}");
    if delete_branch {
        delete_head_branch(client, repo, number).await?;
    }
    Ok(())
}

async fn delete_head_branch(client: &Client, repo: &str, number: u64) -> Result<()> {
    let pr = get_pr(client, repo, number)
        .await
        .context("Merged, but failed to inspect head branch")?;
    let head_repo = pr["head"]["repo"]["full_name"]
        .as_str()
        .context("Merged, but head repository is unavailable; branch not deleted")?;
    if !head_repo.eq_ignore_ascii_case(repo) {
        println!("Skipped branch deletion: head is in {head_repo}");
        return Ok(());
    }
    let branch = pr["head"]["ref"]
        .as_str()
        .filter(|name| !name.is_empty())
        .context("Merged, but head branch is unavailable; branch not deleted")?;
    let path = format!(
        "/repos/{repo}/git/refs/heads/{}",
        urlencoding::encode(branch)
    );
    client
        .delete(&path)
        .await
        .with_context(|| format!("Merged, but failed to delete branch {branch}"))?;
    println!("Deleted branch {branch}");
    Ok(())
}

async fn handle_discussions(client: &Client, repo: &str, number: u64) -> Result<()> {
    let result = list_review_comments(client, repo, number).await?;
    print_discussions(&result);
    Ok(())
}

async fn handle_reply(
    client: &Client,
    repo: &str,
    number: u64,
    comment: u64,
    message: &str,
) -> Result<()> {
    let result = reply_to_review_comment(client, repo, number, comment, message).await?;
    let id = result["id"].as_u64().unwrap_or(0);
    println!("Posted reply (id: {id}) to comment {comment}");
    Ok(())
}

async fn list_prs(
    client: &Client,
    repo: &str,
    state: &str,
    limit: u32,
) -> Result<serde_json::Value> {
    let path = format!("/repos/{repo}/pulls?per_page={limit}&state={state}");
    client.get(&path).await
}

async fn get_pr(client: &Client, repo: &str, number: u64) -> Result<serde_json::Value> {
    let path = format!("/repos/{repo}/pulls/{number}");
    client.get(&path).await
}

async fn comment_on_issue(
    client: &Client,
    repo: &str,
    number: u64,
    body: &str,
) -> Result<serde_json::Value> {
    let path = format!("/repos/{repo}/issues/{number}/comments");
    client
        .post(&path, &serde_json::json!({ "body": body }))
        .await
}

async fn list_issue_comments(
    client: &Client,
    repo: &str,
    number: u64,
) -> Result<serde_json::Value> {
    let path = format!("/repos/{repo}/issues/{number}/comments?per_page=100");
    client.get(&path).await
}

async fn approve_pr(client: &Client, repo: &str, number: u64) -> Result<serde_json::Value> {
    let path = format!("/repos/{repo}/pulls/{number}/reviews");
    client
        .post(&path, &serde_json::json!({ "event": "APPROVE" }))
        .await
}

async fn list_review_comments(
    client: &Client,
    repo: &str,
    number: u64,
) -> Result<serde_json::Value> {
    let path = format!("/repos/{repo}/pulls/{number}/comments?per_page=100");
    client.get(&path).await
}

async fn reply_to_review_comment(
    client: &Client,
    repo: &str,
    _number: u64,
    comment_id: u64,
    body: &str,
) -> Result<serde_json::Value> {
    let path = format!("/repos/{repo}/pulls/comments/{comment_id}/replies");
    client
        .post(&path, &serde_json::json!({ "body": body }))
        .await
}

async fn handle_review(
    client: &Client,
    repo: &str,
    number: u64,
    body: Option<String>,
    event: String,
    comments: Vec<String>,
) -> Result<()> {
    let inline_comments: Vec<serde_json::Value> = comments
        .iter()
        .map(|c| {
            let mut parts = c.splitn(3, ':');
            let path = parts.next().unwrap_or("");
            let line: u64 = parts.next().unwrap_or("0").parse().unwrap_or(0);
            let comment_body = parts
                .next()
                .unwrap_or("")
                .replace("\\n", "\n")
                .replace("\\t", "\t");
            serde_json::json!({ "path": path, "line": line, "side": "RIGHT", "body": comment_body })
        })
        .collect();
    let event_upper = event.to_uppercase();
    let body_interpreted = body.map(|b| b.replace("\\n", "\n").replace("\\t", "\t"));
    let result = create_review(
        client,
        repo,
        number,
        &event_upper,
        body_interpreted.as_deref(),
        inline_comments,
    )
    .await?;
    let review_id = result["id"].as_u64().unwrap_or(0);
    println!("Submitted review (id: {review_id}) on {repo}#{number} [{event_upper}]");
    Ok(())
}

async fn create_review(
    client: &Client,
    repo: &str,
    number: u64,
    event: &str,
    body: Option<&str>,
    comments: Vec<serde_json::Value>,
) -> Result<serde_json::Value> {
    let path = format!("/repos/{repo}/pulls/{number}/reviews");
    let mut payload = serde_json::json!({ "event": event });
    if let Some(b) = body {
        payload["body"] = serde_json::json!(b);
    }
    if !comments.is_empty() {
        payload["comments"] = serde_json::json!(comments);
    }
    client.post(&path, &payload).await
}

#[cfg(test)]
#[path = "pr_merge_tests.rs"]
mod merge_tests;

fn print_prs(value: &serde_json::Value) {
    if let Some(prs) = value.as_array() {
        for pr in prs {
            let number = pr["number"].as_u64().unwrap_or(0);
            let title = pr["title"].as_str().unwrap_or("");
            let state = pr["state"].as_str().unwrap_or("");
            let draft = pr["draft"].as_bool().unwrap_or(false);
            let draft_str = if draft { " [draft]" } else { "" };
            println!("#{number:<5} {state:<6} {title}{draft_str}");
        }
    }
}

fn print_discussions(value: &serde_json::Value) {
    if let Some(comments) = value.as_array() {
        if comments.is_empty() {
            println!("No review comments");
            return;
        }
        for c in comments {
            let id = c["id"].as_u64().unwrap_or(0);
            let path = c["path"].as_str().unwrap_or("?");
            let line = c["line"].as_u64().unwrap_or(0);
            let author = c["user"]["login"].as_str().unwrap_or("?");
            let body = c["body"].as_str().unwrap_or("");
            println!("{id:<10} {path}:{line} @{author}");
            println!("  {body}");
            println!();
        }
    }
}
