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
    /// List self-hosted runner groups for an organization
    Groups {
        /// Organization name
        org: String,
    },
    /// Get details for an organization runner group
    Group {
        /// Organization name
        org: String,
        /// Runner group ID
        group_id: u64,
    },
    /// List repositories allowed to use an organization runner group
    GroupRepos {
        /// Organization name
        org: String,
        /// Runner group ID
        group_id: u64,
    },
    /// List selected workflow refs allowed to use an organization runner group
    GroupWorkflows {
        /// Organization name
        org: String,
        /// Runner group ID
        group_id: u64,
    },
}

pub async fn handle(client: &Client, command: RunnerCommands) -> Result<()> {
    match command {
        RunnerCommands::List { target, org } => handle_list(client, &target, org).await?,
        RunnerCommands::View {
            target,
            runner_id,
            org,
        } => handle_view(client, &target, runner_id, org).await?,
        RunnerCommands::Delete {
            target,
            runner_id,
            org,
        } => handle_delete(client, &target, runner_id, org).await?,
        RunnerCommands::Groups { org } => handle_groups(client, &org).await?,
        RunnerCommands::Group { org, group_id } => handle_group(client, &org, group_id).await?,
        RunnerCommands::GroupRepos { org, group_id } => {
            handle_group_repos(client, &org, group_id).await?
        }
        RunnerCommands::GroupWorkflows { org, group_id } => {
            handle_group_workflows(client, &org, group_id).await?
        }
    }
    Ok(())
}

async fn handle_list(client: &Client, target: &str, org: bool) -> Result<()> {
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
    Ok(())
}

async fn handle_view(client: &Client, target: &str, runner_id: u64, org: bool) -> Result<()> {
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
    Ok(())
}

async fn handle_delete(client: &Client, target: &str, runner_id: u64, org: bool) -> Result<()> {
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
    Ok(())
}

async fn handle_groups(client: &Client, org: &str) -> Result<()> {
    let result = client
        .get(&format!("/orgs/{org}/actions/runner-groups"))
        .await?;
    print_runner_groups(&result);
    Ok(())
}

async fn handle_group(client: &Client, org: &str, group_id: u64) -> Result<()> {
    let result = client
        .get(&format!("/orgs/{org}/actions/runner-groups/{group_id}"))
        .await?;
    print_runner_group_detail(&result);
    Ok(())
}

async fn handle_group_repos(client: &Client, org: &str, group_id: u64) -> Result<()> {
    let result = client
        .get(&format!(
            "/orgs/{org}/actions/runner-groups/{group_id}/repositories"
        ))
        .await?;
    print_runner_group_repos(&result);
    Ok(())
}

async fn handle_group_workflows(client: &Client, org: &str, group_id: u64) -> Result<()> {
    let result = client
        .get(&format!(
            "/orgs/{org}/actions/runner-groups/{group_id}/selected_workflows"
        ))
        .await?;
    print_selected_workflows(&result);
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

fn print_runner_groups(value: &serde_json::Value) {
    let groups = value["runner_groups"].as_array();
    let total = value["total_count"].as_u64().unwrap_or(0);

    if let Some(groups) = groups {
        if groups.is_empty() {
            println!("No runner groups found");
            return;
        }
        println!("Total: {total}");
        println!(
            "{:<8} {:<30} {:<14} {:<16} {:<14} {:<8}",
            "ID", "Name", "Visibility", "Default", "Runners", "Repos"
        );
        println!("{}", "-".repeat(96));
        for group in groups {
            let id = group["id"].as_u64().unwrap_or(0);
            let name = group["name"].as_str().unwrap_or("");
            let visibility = group["visibility"].as_str().unwrap_or("");
            let default = group["default"].as_bool().unwrap_or(false);
            let runners = group["runners_url"].as_str().unwrap_or("");
            let repos = group["selected_repositories_url"].as_str().unwrap_or("");
            println!(
                "{id:<8} {name:<30} {visibility:<14} {:<16} {:<14} {:<8}",
                if default { "yes" } else { "no" },
                if runners.is_empty() { "n/a" } else { "url" },
                if repos.is_empty() { "n/a" } else { "url" },
            );
        }
    } else {
        println!("No runner groups found or insufficient permissions");
    }
}

fn print_runner_group_detail(value: &serde_json::Value) {
    let id = value["id"].as_u64().unwrap_or(0);
    let name = value["name"].as_str().unwrap_or("");
    let visibility = value["visibility"].as_str().unwrap_or("");
    let default = value["default"].as_bool().unwrap_or(false);
    let inherited = value["inherited"].as_bool().unwrap_or(false);
    let restricted = value["restricted_to_workflows"].as_bool().unwrap_or(false);
    let workflow_count = value["selected_workflows"]
        .as_array()
        .map_or(0, |a| a.len());

    println!("Runner group: {name}");
    println!("ID: {id}");
    println!("Visibility: {visibility}");
    println!("Default: {}", if default { "yes" } else { "no" });
    println!("Inherited: {}", if inherited { "yes" } else { "no" });
    println!(
        "Restricted to workflows: {}",
        if restricted { "yes" } else { "no" }
    );
    println!("Inline selected workflows: {workflow_count}");
}

fn print_runner_group_repos(value: &serde_json::Value) {
    let repos = value["repositories"].as_array();
    let total = value["total_count"].as_u64().unwrap_or(0);

    if let Some(repos) = repos {
        if repos.is_empty() {
            println!("No selected repositories found");
            return;
        }
        println!("Total: {total}");
        for repo in repos {
            let full_name = repo["full_name"].as_str().unwrap_or("");
            let private = repo["private"].as_bool().unwrap_or(false);
            println!("{}{}", full_name, if private { " (private)" } else { "" });
        }
    } else {
        println!("No repositories found or insufficient permissions");
    }
}

fn print_selected_workflows(value: &serde_json::Value) {
    let workflows = value["selected_workflows"].as_array();
    let total = value["total_count"].as_u64().unwrap_or(0);

    if let Some(workflows) = workflows {
        if workflows.is_empty() {
            println!("No selected workflow restrictions found");
            return;
        }
        println!("Total: {total}");
        for workflow in workflows {
            if let Some(path) = workflow.as_str() {
                println!("{path}");
            } else {
                println!("{workflow}");
            }
        }
    } else {
        println!("No selected workflows found or insufficient permissions");
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
