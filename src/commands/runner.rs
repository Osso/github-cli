use anyhow::Result;
use clap::Subcommand;

use crate::client::Client;

#[derive(Subcommand)]
pub enum RunnerCommands {
    /// List self-hosted runners for a repository
    List {
        /// Repository (owner/repo) or organization name
        target: String,
        /// Treat target as organization instead of repository
        #[arg(long)]
        org: bool,
    },
    /// Get details of a specific runner
    View {
        /// Repository (owner/repo) or organization name
        target: String,
        /// Runner ID
        runner_id: u64,
        /// Treat target as organization instead of repository
        #[arg(long)]
        org: bool,
    },
    /// Delete a self-hosted runner
    Delete {
        /// Repository (owner/repo) or organization name
        target: String,
        /// Runner ID
        runner_id: u64,
        /// Treat target as organization instead of repository
        #[arg(long)]
        org: bool,
    },
}

pub async fn handle(client: &Client, command: RunnerCommands) -> Result<()> {
    match command {
        RunnerCommands::List { target, org } => {
            let result = if org {
                client
                    .get(&format!("/orgs/{target}/actions/runners"))
                    .await?
            } else {
                client
                    .get(&format!("/repos/{target}/actions/runners"))
                    .await?
            };
            print_runners(&result);
        }
        RunnerCommands::View {
            target,
            runner_id,
            org,
        } => {
            let result = if org {
                client
                    .get(&format!("/orgs/{target}/actions/runners/{runner_id}"))
                    .await?
            } else {
                client
                    .get(&format!("/repos/{target}/actions/runners/{runner_id}"))
                    .await?
            };
            print_runner_detail(&result);
        }
        RunnerCommands::Delete {
            target,
            runner_id,
            org,
        } => {
            if org {
                client
                    .delete(&format!("/orgs/{target}/actions/runners/{runner_id}"))
                    .await?;
            } else {
                client
                    .delete(&format!("/repos/{target}/actions/runners/{runner_id}"))
                    .await?;
            }
            println!("Deleted runner {runner_id}");
        }
    }
    Ok(())
}

fn print_runners(value: &serde_json::Value) {
    let runners = value["runners"].as_array();
    let total = value["total_count"].as_u64().unwrap_or(0);

    if let Some(runners) = runners {
        if runners.is_empty() {
            println!("No runners found");
            return;
        }
        println!("Total: {total}");
        println!(
            "{:<8} {:<40} {:<10} {:<6} Labels",
            "ID", "Name", "Status", "Busy"
        );
        println!("{}", "-".repeat(80));
        for runner in runners {
            let id = runner["id"].as_u64().unwrap_or(0);
            let name = runner["name"].as_str().unwrap_or("");
            let status = runner["status"].as_str().unwrap_or("");
            let busy = runner["busy"].as_bool().unwrap_or(false);
            let busy_str = if busy { "yes" } else { "no" };
            let labels: Vec<&str> = runner["labels"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|l| l["name"].as_str()).collect())
                .unwrap_or_default();
            let labels_str = labels.join(", ");
            println!("{id:<8} {name:<40} {status:<10} {busy_str:<6} {labels_str}");
        }
    }
}

fn print_runner_detail(value: &serde_json::Value) {
    let id = value["id"].as_u64().unwrap_or(0);
    let name = value["name"].as_str().unwrap_or("");
    let os = value["os"].as_str().unwrap_or("");
    let status = value["status"].as_str().unwrap_or("");
    let busy = value["busy"].as_bool().unwrap_or(false);
    let labels: Vec<&str> = value["labels"]
        .as_array()
        .map(|arr| arr.iter().filter_map(|l| l["name"].as_str()).collect())
        .unwrap_or_default();
    println!("Runner: {name}");
    println!("ID: {id}");
    println!("OS: {os}");
    println!("Status: {status}");
    println!("Busy: {}", if busy { "yes" } else { "no" });
    println!(
        "Labels: {}",
        if labels.is_empty() {
            "(none)".to_string()
        } else {
            labels.join(", ")
        }
    );
}
