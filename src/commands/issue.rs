#![cfg_attr(coverage_nightly, coverage(off))]

use anyhow::{Result, bail};
use clap::{Args, Subcommand};
use serde_json::{Value, json};

use crate::client::Client;
use crate::commands::issue_fields::MetadataArgs;

#[derive(Args)]
pub struct CreateIssueArgs {
    /// Issue title
    #[arg(short, long)]
    title: String,
    /// Issue body
    #[arg(short, long)]
    body: Option<String>,
    #[command(flatten)]
    metadata: MetadataArgs,
}

#[derive(Args)]
pub struct EditIssueArgs {
    /// Replacement title
    #[arg(short, long)]
    title: Option<String>,
    /// Replacement body (empty string clears it)
    #[arg(short, long)]
    body: Option<String>,
    #[command(flatten)]
    metadata: MetadataArgs,
}

#[derive(Subcommand)]
pub enum IssueCommands {
    /// Create an issue
    Create {
        /// Repository (owner/repo)
        repo: String,
        #[command(flatten)]
        fields: CreateIssueArgs,
    },
    /// Edit an issue; omitted fields remain unchanged
    Edit {
        /// Repository (owner/repo)
        repo: String,
        /// Issue number
        number: u64,
        #[command(flatten)]
        fields: EditIssueArgs,
    },
    /// Close an issue
    Close {
        /// Repository (owner/repo)
        repo: String,
        /// Issue number
        number: u64,
    },
    /// Reopen an issue
    Reopen {
        /// Repository (owner/repo)
        repo: String,
        /// Issue number
        number: u64,
    },
    /// Post a comment on an issue
    Comment {
        /// Repository (owner/repo)
        repo: String,
        /// Issue number
        number: u64,
        /// Comment body
        #[arg(short, long)]
        message: String,
    },
    /// List issues
    List {
        /// Repository (owner/repo)
        repo: String,
        /// Search query
        #[arg(short = 'S', long)]
        search: Option<String>,
        /// Limit results
        #[arg(short, long, default_value = "10")]
        limit: u32,
    },
    /// View issue details
    View {
        /// Repository (owner/repo)
        repo: String,
        /// Issue number
        number: u64,
    },
    /// List comments on an issue
    Comments {
        /// Repository (owner/repo)
        repo: String,
        /// Issue number
        number: u64,
    },
}

pub async fn handle(client: &Client, command: IssueCommands) -> Result<()> {
    match command {
        IssueCommands::Create { repo, fields } => create_issue(client, &repo, fields).await?,
        IssueCommands::Edit {
            repo,
            number,
            fields,
        } => edit_issue(client, &repo, number, fields).await?,
        IssueCommands::Close { repo, number } => {
            set_issue_state(client, &repo, number, "closed").await?
        }
        IssueCommands::Reopen { repo, number } => {
            set_issue_state(client, &repo, number, "open").await?
        }
        IssueCommands::Comment {
            repo,
            number,
            message,
        } => post_issue_comment(client, &repo, number, &message).await?,
        IssueCommands::List {
            repo,
            search,
            limit,
        } => {
            let result = if let Some(query) = search {
                search_issues(client, &repo, &query, limit).await?
            } else {
                list_issues(client, &repo, limit).await?
            };
            print_issues(&result);
        }
        IssueCommands::View { repo, number } => {
            let result = get_issue(client, &repo, number).await?;
            print_issue_detail(&result);
        }
        IssueCommands::Comments { repo, number } => {
            let result = list_issue_comments(client, &repo, number).await?;
            print_issue_comments(&result);
        }
    }
    Ok(())
}

async fn create_issue(client: &Client, repo: &str, fields: CreateIssueArgs) -> Result<()> {
    let body = issue_body(
        Some(&fields.title),
        fields.body.as_deref(),
        &fields.metadata,
    );
    let issue = client.post(&format!("/repos/{repo}/issues"), &body).await?;
    print_issue_detail(&issue);
    Ok(())
}

async fn edit_issue(client: &Client, repo: &str, number: u64, fields: EditIssueArgs) -> Result<()> {
    let body = issue_body(
        fields.title.as_deref(),
        fields.body.as_deref(),
        &fields.metadata,
    );
    if body == json!({}) {
        bail!("No issue changes supplied; use --title, --body, --labels or --assignees");
    }
    let issue = client
        .patch(&format!("/repos/{repo}/issues/{number}"), &body)
        .await?;
    print_issue_detail(&issue);
    Ok(())
}

fn issue_body(title: Option<&str>, body: Option<&str>, metadata: &MetadataArgs) -> Value {
    let mut result = json!({});
    if let Some(title) = title {
        result["title"] = json!(title);
    }
    if let Some(body) = body {
        result["body"] = json!(body);
    }
    metadata.append_to(&mut result);
    result
}

async fn set_issue_state(client: &Client, repo: &str, number: u64, state: &str) -> Result<()> {
    let issue = client
        .patch(
            &format!("/repos/{repo}/issues/{number}"),
            &json!({"state": state}),
        )
        .await?;
    print_issue_detail(&issue);
    Ok(())
}

async fn post_issue_comment(client: &Client, repo: &str, number: u64, message: &str) -> Result<()> {
    let comment = client
        .post(
            &format!("/repos/{repo}/issues/{number}/comments"),
            &json!({"body": message}),
        )
        .await?;
    println!("Posted comment (id: {}) on {repo}#{number}", comment["id"]);
    Ok(())
}

async fn list_issues(client: &Client, repo: &str, limit: u32) -> Result<serde_json::Value> {
    let path = format!("/repos/{repo}/issues?per_page={limit}&state=open");
    client.get(&path).await
}

async fn get_issue(client: &Client, repo: &str, number: u64) -> Result<serde_json::Value> {
    let path = format!("/repos/{repo}/issues/{number}");
    client.get(&path).await
}

async fn list_issue_comments(
    client: &Client,
    repo: &str,
    number: u64,
) -> Result<serde_json::Value> {
    let path = format!("/repos/{repo}/issues/{number}/comments?per_page=100");
    client.get(&path).await
}

async fn search_issues(
    client: &Client,
    repo: &str,
    query: &str,
    limit: u32,
) -> Result<serde_json::Value> {
    let search_query = format!("repo:{repo} {query}");
    let q = urlencoding::encode(&search_query);
    let path = format!("/search/issues?q={q}&per_page={limit}");
    client.get(&path).await
}

fn print_issues(value: &serde_json::Value) {
    let items = if let Some(arr) = value.as_array() {
        arr.clone()
    } else if let Some(obj) = value.get("items").and_then(|v| v.as_array()) {
        obj.clone()
    } else {
        return;
    };

    for issue in items {
        let number = issue["number"].as_u64().unwrap_or(0);
        let title = issue["title"].as_str().unwrap_or("");
        let state = issue["state"].as_str().unwrap_or("");
        let labels: Vec<&str> = issue["labels"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|l| l["name"].as_str()).collect())
            .unwrap_or_default();
        let labels_str = if labels.is_empty() {
            String::new()
        } else {
            format!(" [{}]", labels.join(", "))
        };
        println!("#{number:<5} {state:<6} {title}{labels_str}");
    }
}

pub fn print_issue_comments(value: &serde_json::Value) {
    if let Some(comments) = value.as_array() {
        if comments.is_empty() {
            println!("No comments");
            return;
        }
        for c in comments {
            let id = c["id"].as_u64().unwrap_or(0);
            let author = c["user"]["login"].as_str().unwrap_or("?");
            let created = c["created_at"]
                .as_str()
                .unwrap_or("")
                .split('T')
                .next()
                .unwrap_or("");
            let body = c["body"].as_str().unwrap_or("");
            let url = c["html_url"].as_str().unwrap_or("");
            println!("#{id} @{author} ({created})");
            println!("{url}");
            println!("{body}");
            println!("---");
        }
    }
}

#[cfg(test)]
#[path = "issue_write_tests.rs"]
mod write_tests;

pub fn print_issue_detail(value: &serde_json::Value) {
    let number = value["number"].as_u64().unwrap_or(0);
    let title = value["title"].as_str().unwrap_or("");
    let state = value["state"].as_str().unwrap_or("");
    let author = value["user"]["login"].as_str().unwrap_or("");
    let body = value["body"].as_str().unwrap_or("");
    let url = value["html_url"].as_str().unwrap_or("");

    println!("#{number} {title}");
    println!("State: {state}  Author: {author}");
    println!("URL: {url}");
    if !body.is_empty() {
        println!("\n{body}");
    }
}
