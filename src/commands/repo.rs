use anyhow::Result;
use clap::Subcommand;

use crate::client::Client;

#[derive(Subcommand)]
pub enum RepoCommands {
    /// Manage deploy keys
    Keys {
        #[command(subcommand)]
        command: KeyCommands,
    },
    /// Manage webhooks
    Hooks {
        #[command(subcommand)]
        command: HookCommands,
    },
}

#[derive(Subcommand)]
pub enum KeyCommands {
    /// List deploy keys for a repository
    List {
        /// Repository (owner/repo)
        repo: String,
    },
    /// Add a deploy key to a repository
    Add {
        /// Repository (owner/repo)
        repo: String,
        /// Key title
        #[arg(short, long)]
        title: String,
        /// Public key content or path to .pub file
        key: String,
        /// Allow write access (default: read-only)
        #[arg(short, long)]
        write: bool,
    },
    /// Remove a deploy key from a repository
    Remove {
        /// Repository (owner/repo)
        repo: String,
        /// Key ID
        key_id: u64,
    },
}

#[derive(Subcommand)]
pub enum HookCommands {
    /// List webhooks for a repository
    List {
        /// Repository (owner/repo)
        repo: String,
    },
}

pub async fn handle(client: &Client, command: RepoCommands) -> Result<()> {
    match command {
        RepoCommands::Keys { command } => match command {
            KeyCommands::List { repo } => {
                let result = client.get(&format!("/repos/{repo}/keys")).await?;
                print_deploy_keys(&result);
            }
            KeyCommands::Add { repo, title, key, write } => {
                let key_content = if std::path::Path::new(&key).exists() {
                    std::fs::read_to_string(&key)?.trim().to_string()
                } else {
                    key
                };
                let result = client
                    .post(
                        &format!("/repos/{repo}/keys"),
                        &serde_json::json!({ "title": title, "key": key_content, "read_only": !write }),
                    )
                    .await?;
                let id = result["id"].as_u64().unwrap_or(0);
                let access = if write { "read-write" } else { "read-only" };
                println!("Added deploy key '{title}' (id: {id}) to {repo} [{access}]");
            }
            KeyCommands::Remove { repo, key_id } => {
                client.delete(&format!("/repos/{repo}/keys/{key_id}")).await?;
                println!("Removed deploy key {key_id} from {repo}");
            }
        },
        RepoCommands::Hooks { command } => match command {
            HookCommands::List { repo } => {
                let result = client.get(&format!("/repos/{repo}/hooks")).await?;
                print_hooks(&result);
            }
        },
    }
    Ok(())
}

fn print_deploy_keys(value: &serde_json::Value) {
    if let Some(keys) = value.as_array() {
        if keys.is_empty() {
            println!("No deploy keys found");
            return;
        }
        for key in keys {
            let id = key["id"].as_u64().unwrap_or(0);
            let title = key["title"].as_str().unwrap_or("");
            let read_only = key["read_only"].as_bool().unwrap_or(true);
            let created = key["created_at"].as_str().unwrap_or("").split('T').next().unwrap_or("");
            let access = if read_only { "read-only" } else { "read-write" };
            println!("{id:<10} {title:<30} [{access}] {created}");
        }
    }
}

fn print_hooks(value: &serde_json::Value) {
    if let Some(hooks) = value.as_array() {
        if hooks.is_empty() {
            println!("No webhooks found");
            return;
        }
        for hook in hooks {
            let id = hook["id"].as_u64().unwrap_or(0);
            let name = hook["name"].as_str().unwrap_or("");
            let active = hook["active"].as_bool().unwrap_or(false);
            let url = hook["config"]["url"].as_str().unwrap_or("");
            let events: Vec<&str> = hook["events"]
                .as_array()
                .map(|arr| arr.iter().filter_map(|e| e.as_str()).collect())
                .unwrap_or_default();
            let status = if active { "active" } else { "inactive" };
            println!("{id:<10} {name:<15} [{status}] {url}");
            if !events.is_empty() {
                println!("           Events: {}", events.join(", "));
            }
        }
    }
}
