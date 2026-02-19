use anyhow::Result;
use clap::Subcommand;

use crate::client::Client;

#[derive(Subcommand)]
pub enum IssueCommands {
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
        IssueCommands::List { repo, search, limit } => {
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

async fn list_issues(client: &Client, repo: &str, limit: u32) -> Result<serde_json::Value> {
    let path = format!("/repos/{repo}/issues?per_page={limit}&state=open");
    client.get(&path).await
}

async fn get_issue(client: &Client, repo: &str, number: u64) -> Result<serde_json::Value> {
    let path = format!("/repos/{repo}/issues/{number}");
    client.get(&path).await
}

async fn list_issue_comments(client: &Client, repo: &str, number: u64) -> Result<serde_json::Value> {
    let path = format!("/repos/{repo}/issues/{number}/comments?per_page=100");
    client.get(&path).await
}

async fn search_issues(client: &Client, repo: &str, query: &str, limit: u32) -> Result<serde_json::Value> {
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
            let created = c["created_at"].as_str().unwrap_or("").split('T').next().unwrap_or("");
            let body = c["body"].as_str().unwrap_or("");
            let url = c["html_url"].as_str().unwrap_or("");
            println!("#{id} @{author} ({created})");
            println!("{url}");
            println!("{body}");
            println!("---");
        }
    }
}

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
