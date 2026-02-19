use anyhow::Result;
use clap::Subcommand;

use crate::client::Client;
use crate::commands::issue::{print_issue_comments, print_issue_detail};

#[derive(Subcommand)]
pub enum PrCommands {
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
        PrCommands::List { repo, state, limit } => {
            let result = list_prs(client, &repo, &state, limit).await?;
            print_prs(&result);
        }
        PrCommands::View { repo, number } => {
            let result = get_pr(client, &repo, number).await?;
            print_issue_detail(&result);
        }
        PrCommands::Comment { repo, number, message } => {
            let result = comment_on_issue(client, &repo, number, &message).await?;
            let id = result["id"].as_u64().unwrap_or(0);
            println!("Posted comment (id: {id}) on {repo}#{number}");
        }
        PrCommands::Comments { repo, number } => {
            let result = list_issue_comments(client, &repo, number).await?;
            print_issue_comments(&result);
        }
        PrCommands::Approve { repo, number } => {
            approve_pr(client, &repo, number).await?;
            println!("Approved {repo}#{number}");
        }
        PrCommands::Discussions { repo, number, unresolved: _ } => {
            let result = list_review_comments(client, &repo, number).await?;
            print_discussions(&result);
        }
        PrCommands::Reply { repo, number, comment, message } => {
            let result = reply_to_review_comment(client, &repo, number, comment, &message).await?;
            let id = result["id"].as_u64().unwrap_or(0);
            println!("Posted reply (id: {id}) to comment {comment}");
        }
        PrCommands::Review { repo, number, body, event, comments } => {
            handle_review(client, &repo, number, body, event, comments).await?;
        }
    }
    Ok(())
}

async fn list_prs(client: &Client, repo: &str, state: &str, limit: u32) -> Result<serde_json::Value> {
    let path = format!("/repos/{repo}/pulls?per_page={limit}&state={state}");
    client.get(&path).await
}

async fn get_pr(client: &Client, repo: &str, number: u64) -> Result<serde_json::Value> {
    let path = format!("/repos/{repo}/pulls/{number}");
    client.get(&path).await
}

async fn comment_on_issue(client: &Client, repo: &str, number: u64, body: &str) -> Result<serde_json::Value> {
    let path = format!("/repos/{repo}/issues/{number}/comments");
    client.post(&path, &serde_json::json!({ "body": body })).await
}

async fn list_issue_comments(client: &Client, repo: &str, number: u64) -> Result<serde_json::Value> {
    let path = format!("/repos/{repo}/issues/{number}/comments?per_page=100");
    client.get(&path).await
}

async fn approve_pr(client: &Client, repo: &str, number: u64) -> Result<serde_json::Value> {
    let path = format!("/repos/{repo}/pulls/{number}/reviews");
    client.post(&path, &serde_json::json!({ "event": "APPROVE" })).await
}

async fn list_review_comments(client: &Client, repo: &str, number: u64) -> Result<serde_json::Value> {
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
    client.post(&path, &serde_json::json!({ "body": body })).await
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
            let comment_body = parts.next().unwrap_or("").replace("\\n", "\n").replace("\\t", "\t");
            serde_json::json!({ "path": path, "line": line, "side": "RIGHT", "body": comment_body })
        })
        .collect();
    let event_upper = event.to_uppercase();
    let body_interpreted = body.map(|b| b.replace("\\n", "\n").replace("\\t", "\t"));
    let result = create_review(client, repo, number, &event_upper, body_interpreted.as_deref(), inline_comments).await?;
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
