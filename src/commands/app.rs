#![cfg_attr(coverage_nightly, coverage(off))]

use anyhow::Result;
use clap::Subcommand;

use crate::client::Client;

#[derive(Subcommand)]
pub enum AppCommands {
    /// List GitHub App installations for an organization
    List {
        /// Organization name
        org: String,
    },
    /// List repositories accessible to a GitHub App installation
    Repos {
        /// Installation ID
        installation_id: u64,
    },
}

pub async fn handle(client: &Client, command: AppCommands) -> Result<()> {
    match command {
        AppCommands::List { org } => {
            let result = client.get(&format!("/orgs/{org}/installations")).await?;
            print_org_installations(&result);
        }
        AppCommands::Repos { installation_id } => {
            let result = client
                .get(&format!(
                    "/user/installations/{installation_id}/repositories"
                ))
                .await?;
            print_installation_repos(&result);
        }
    }
    Ok(())
}

fn print_org_installations(value: &serde_json::Value) {
    let installations = value["installations"].as_array();
    let total = value["total_count"].as_u64().unwrap_or(0);

    if let Some(installations) = installations {
        if installations.is_empty() {
            println!("No GitHub App installations found");
            return;
        }
        println!("Total: {total}");
        for app in installations {
            let id = app["id"].as_u64().unwrap_or(0);
            let app_id = app["app_id"].as_u64().unwrap_or(0);
            let app_slug = app["app_slug"].as_str().unwrap_or("");
            let target_type = app["target_type"].as_str().unwrap_or("");
            let repository_selection = app["repository_selection"].as_str().unwrap_or("");
            let permissions: Vec<String> = app["permissions"]
                .as_object()
                .map(|obj| {
                    obj.iter()
                        .map(|(k, v)| format!("{}:{}", k, v.as_str().unwrap_or("")))
                        .collect()
                })
                .unwrap_or_default();
            println!(
                "{id:<10} {app_slug:<30} (app_id: {app_id}) target: {target_type} repos: {repository_selection}"
            );
            if !permissions.is_empty() {
                println!("           Permissions: {}", permissions.join(", "));
            }
        }
    } else {
        println!("No installations found or insufficient permissions");
    }
}

fn print_installation_repos(value: &serde_json::Value) {
    let repos = value["repositories"].as_array();
    let total = value["total_count"].as_u64().unwrap_or(0);
    let repository_selection = value["repository_selection"].as_str().unwrap_or("");

    if let Some(repos) = repos {
        if repos.is_empty() {
            println!("No repositories found for installation");
            return;
        }
        println!("Total: {total}");
        if !repository_selection.is_empty() {
            println!("Repository selection: {repository_selection}");
        }
        for repo in repos {
            let full_name = repo["full_name"].as_str().unwrap_or("");
            let private = repo["private"].as_bool().unwrap_or(false);
            println!("{}{}", full_name, if private { " (private)" } else { "" });
        }
    } else {
        println!("No installation repositories found or insufficient permissions");
    }
}
