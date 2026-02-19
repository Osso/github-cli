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
}

pub async fn handle(client: &Client, command: AppCommands) -> Result<()> {
    match command {
        AppCommands::List { org } => {
            let result = client.get(&format!("/orgs/{org}/installations")).await?;
            print_org_installations(&result);
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
            let permissions: Vec<String> = app["permissions"]
                .as_object()
                .map(|obj| {
                    obj.iter()
                        .map(|(k, v)| format!("{}:{}", k, v.as_str().unwrap_or("")))
                        .collect()
                })
                .unwrap_or_default();
            println!("{id:<10} {app_slug:<30} (app_id: {app_id}) target: {target_type}");
            if !permissions.is_empty() {
                println!("           Permissions: {}", permissions.join(", "));
            }
        }
    } else {
        println!("No installations found or insufficient permissions");
    }
}
