use anyhow::Result;
use clap::Subcommand;

use crate::client::Client;

#[derive(Subcommand)]
pub enum CodeCommands {
    /// Search code across repositories
    Search {
        /// Search query (supports GitHub qualifiers inline)
        query: String,
        /// Filter by language
        #[arg(short = 'L', long)]
        language: Option<String>,
        /// Filter by repository (owner/repo)
        #[arg(short, long)]
        repo: Option<String>,
        /// Filter by organization
        #[arg(short, long)]
        org: Option<String>,
        /// Filter by file path
        #[arg(short, long)]
        path: Option<String>,
        /// Filter by filename
        #[arg(short, long)]
        filename: Option<String>,
        /// Results per page (max 100)
        #[arg(short, long, default_value = "10")]
        limit: u32,
        /// Page number
        #[arg(long, default_value = "1")]
        page: u32,
    },
}

pub async fn handle(client: &Client, command: CodeCommands) -> Result<()> {
    match command {
        CodeCommands::Search {
            query,
            language,
            repo,
            org,
            path,
            filename,
            limit,
            page,
        } => {
            let q = build_query(query, language, repo, org, path, filename);
            let result = client.search_code(&q, limit, page).await?;
            print_code_results(&result);
        }
    }
    Ok(())
}

fn build_query(
    base: String,
    language: Option<String>,
    repo: Option<String>,
    org: Option<String>,
    path: Option<String>,
    filename: Option<String>,
) -> String {
    let mut q = base;
    if let Some(lang) = language {
        q = format!("{q} language:{lang}");
    }
    if let Some(r) = repo {
        q = format!("{q} repo:{r}");
    }
    if let Some(o) = org {
        q = format!("{q} org:{o}");
    }
    if let Some(p) = path {
        q = format!("{q} path:{p}");
    }
    if let Some(f) = filename {
        q = format!("{q} filename:{f}");
    }
    q
}

fn print_code_results(value: &serde_json::Value) {
    let total = value["total_count"].as_u64().unwrap_or(0);
    let items = match value["items"].as_array() {
        Some(arr) => arr,
        None => {
            println!("No results");
            return;
        }
    };
    if items.is_empty() {
        println!("No results (total: {total})");
        return;
    }
    println!("Total: {total}");
    println!();
    for item in items {
        print_code_result_item(item);
    }
}

fn print_code_result_item(item: &serde_json::Value) {
    let repo = item["repository"]["full_name"].as_str().unwrap_or("");
    let path = item["path"].as_str().unwrap_or("");
    let url = item["html_url"].as_str().unwrap_or("");
    println!("{repo}  {path}");
    print_text_match_fragments(item["text_matches"].as_array());
    println!("  {url}");
    println!();
}

fn print_text_match_fragments(matches: Option<&Vec<serde_json::Value>>) {
    let Some(matches) = matches else {
        return;
    };
    for text_match in matches {
        print_text_match_fragment(text_match);
    }
}

fn print_text_match_fragment(text_match: &serde_json::Value) {
    let Some(fragment) = text_match["fragment"].as_str() else {
        return;
    };
    for line in fragment.lines().take(3) {
        println!("  {line}");
    }
}
