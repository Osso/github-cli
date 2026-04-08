use anyhow::Result;
use clap::Subcommand;

use crate::client::Client;

#[derive(Subcommand)]
pub enum WebhookCommands {
    /// List webhooks for a repository
    List {
        /// Repository (owner/repo)
        repo: String,
    },
    /// Create a webhook for a repository
    Create {
        /// Repository (owner/repo)
        repo: String,
        /// Payload URL
        #[arg(long)]
        url: String,
        /// Webhook secret
        #[arg(long)]
        secret: Option<String>,
        /// Comma-separated list of events (default: push)
        #[arg(long, default_value = "push")]
        events: String,
        /// Content type (json or form)
        #[arg(long, default_value = "json")]
        content_type: String,
    },
    /// Delete a webhook from a repository
    Delete {
        /// Repository (owner/repo)
        repo: String,
        /// Hook ID
        hook_id: u64,
    },
    /// Ping a webhook
    Ping {
        /// Repository (owner/repo)
        repo: String,
        /// Hook ID
        hook_id: u64,
    },
    /// List recent deliveries for a webhook
    Deliveries {
        /// Repository (owner/repo)
        repo: String,
        /// Hook ID
        hook_id: u64,
    },
}

pub async fn handle(client: &Client, command: WebhookCommands) -> Result<()> {
    match command {
        WebhookCommands::List { repo } => handle_list(client, &repo).await?,
        WebhookCommands::Create { repo, url, secret, events, content_type } => {
            handle_create(client, &repo, &url, secret.as_deref(), &events, &content_type).await?
        }
        WebhookCommands::Delete { repo, hook_id } => handle_delete(client, &repo, hook_id).await?,
        WebhookCommands::Ping { repo, hook_id } => handle_ping(client, &repo, hook_id).await?,
        WebhookCommands::Deliveries { repo, hook_id } => {
            handle_deliveries(client, &repo, hook_id).await?
        }
    }
    Ok(())
}

async fn handle_list(client: &Client, repo: &str) -> Result<()> {
    let result = client.get(&format!("/repos/{repo}/hooks")).await?;
    print_hooks(&result);
    Ok(())
}

async fn handle_create(
    client: &Client,
    repo: &str,
    url: &str,
    secret: Option<&str>,
    events: &str,
    content_type: &str,
) -> Result<()> {
    let events: Vec<&str> = events.split(',').map(str::trim).collect();
    let mut config = serde_json::json!({ "url": url, "content_type": content_type });
    if let Some(s) = secret {
        config["secret"] = serde_json::Value::String(s.to_owned());
    }
    let body = serde_json::json!({
        "name": "web",
        "active": true,
        "events": events,
        "config": config,
    });
    let result = client.post(&format!("/repos/{repo}/hooks"), &body).await?;
    let id = result["id"].as_u64().unwrap_or(0);
    println!("Created webhook {id} on {repo} -> {url}");
    Ok(())
}

async fn handle_delete(client: &Client, repo: &str, hook_id: u64) -> Result<()> {
    client.delete(&format!("/repos/{repo}/hooks/{hook_id}")).await?;
    println!("Deleted webhook {hook_id} from {repo}");
    Ok(())
}

async fn handle_ping(client: &Client, repo: &str, hook_id: u64) -> Result<()> {
    client.post_empty(&format!("/repos/{repo}/hooks/{hook_id}/pings")).await?;
    println!("Pinged webhook {hook_id} on {repo}");
    Ok(())
}

async fn handle_deliveries(client: &Client, repo: &str, hook_id: u64) -> Result<()> {
    let result = client.get(&format!("/repos/{repo}/hooks/{hook_id}/deliveries")).await?;
    print_deliveries(&result);
    Ok(())
}

fn print_hooks(value: &serde_json::Value) {
    let Some(hooks) = value.as_array() else {
        return;
    };
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

fn print_deliveries(value: &serde_json::Value) {
    let Some(deliveries) = value.as_array() else {
        return;
    };
    if deliveries.is_empty() {
        println!("No deliveries found");
        return;
    }
    for delivery in deliveries {
        let id = delivery["id"].as_u64().unwrap_or(0);
        let event = delivery["event"].as_str().unwrap_or("");
        let delivered_at = delivery["delivered_at"]
            .as_str()
            .unwrap_or("")
            .split('T')
            .next()
            .unwrap_or("");
        let status = delivery["status"].as_str().unwrap_or("");
        let status_code = delivery["status_code"].as_u64().unwrap_or(0);
        println!("{id:<12} {event:<20} {status:<10} {status_code:<5} {delivered_at}");
    }
}
