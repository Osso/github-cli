#![cfg_attr(coverage_nightly, coverage(off))]

use anyhow::{Result, bail};
use clap::Subcommand;
use std::io::{self, Read};

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
        /// Legacy inline payload URL; use --url-stdin for secret-bearing URLs
        #[arg(
            long,
            conflicts_with = "url_stdin",
            required_unless_present = "url_stdin"
        )]
        url: Option<String>,
        /// Read exactly one payload URL from stdin without exposing it in argv
        #[arg(long, conflicts_with = "url", required_unless_present = "url")]
        url_stdin: bool,
        /// Legacy webhook secret (exposed in argv; do not use for secret-bearing URLs)
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
        WebhookCommands::Create {
            repo,
            url,
            url_stdin,
            secret,
            events,
            content_type,
        } => {
            let url = match (url, url_stdin) {
                (Some(url), false) => {
                    warn_for_inline_secret_url(&url);
                    url
                }
                (None, true) => read_url_from_stdin(&mut io::stdin().lock())?,
                _ => unreachable!("clap enforces webhook URL input exclusivity"),
            };
            handle_create(
                client,
                &repo,
                &url,
                secret.as_deref(),
                &events,
                &content_type,
            )
            .await?
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
    println!("Created webhook {id} on {repo}");
    Ok(())
}

async fn handle_delete(client: &Client, repo: &str, hook_id: u64) -> Result<()> {
    client
        .delete(&format!("/repos/{repo}/hooks/{hook_id}"))
        .await?;
    println!("Deleted webhook {hook_id} from {repo}");
    Ok(())
}

async fn handle_ping(client: &Client, repo: &str, hook_id: u64) -> Result<()> {
    client
        .post_empty(&format!("/repos/{repo}/hooks/{hook_id}/pings"))
        .await?;
    println!("Pinged webhook {hook_id} on {repo}");
    Ok(())
}

async fn handle_deliveries(client: &Client, repo: &str, hook_id: u64) -> Result<()> {
    let result = client
        .get(&format!("/repos/{repo}/hooks/{hook_id}/deliveries"))
        .await?;
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
        let url = redact_webhook_url(hook["config"]["url"].as_str().unwrap_or(""));
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

fn read_url_from_stdin<R: Read>(mut reader: R) -> Result<String> {
    let mut input = String::new();
    reader.read_to_string(&mut input)?;
    let url = input.trim();
    if url.is_empty() || url.chars().any(char::is_whitespace) {
        bail!("stdin must contain exactly one non-empty URL");
    }
    Ok(url.to_owned())
}

fn warn_for_inline_secret_url(url: &str) {
    if url.contains('?') || url.contains('#') {
        eprintln!(
            "warning: URLs with query strings or fragments should be supplied with --url-stdin"
        );
    }
}

pub(crate) fn redact_webhook_url(raw_url: &str) -> String {
    let Ok(mut url) = reqwest::Url::parse(raw_url) else {
        return "[redacted webhook URL]".to_owned();
    };
    let contains_query_or_fragment = url.query().is_some() || url.fragment().is_some();
    if !contains_query_or_fragment {
        return url.to_string();
    }
    url.set_query(None);
    url.set_fragment(None);
    format!("{url} [query redacted]")
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::{read_url_from_stdin, redact_webhook_url};

    #[test]
    fn reads_one_url_and_ignores_terminal_whitespace() {
        let url =
            read_url_from_stdin(Cursor::new("https://example.test/hook?token=secret\n\n")).unwrap();

        assert_eq!("https://example.test/hook?token=secret", url);
    }

    #[test]
    fn rejects_extra_non_whitespace_input() {
        let error =
            read_url_from_stdin(Cursor::new("https://example.test/hook\nextra")).unwrap_err();

        assert!(error.to_string().contains("one non-empty URL"));
    }

    #[test]
    fn rejects_empty_input() {
        let error = read_url_from_stdin(Cursor::new("  \n\t")).unwrap_err();

        assert!(error.to_string().contains("one non-empty URL"));
    }

    #[test]
    fn redacts_query_and_fragment_from_valid_urls() {
        assert_eq!(
            "https://example.test/hook [query redacted]",
            redact_webhook_url("https://example.test/hook?token=query#fragment")
        );
    }

    #[test]
    fn leaves_valid_urls_without_query_or_fragment_unchanged() {
        assert_eq!(
            "https://example.test/hook",
            redact_webhook_url("https://example.test/hook")
        );
    }

    #[test]
    fn replaces_invalid_urls_without_echoing_input() {
        let rendered = redact_webhook_url("not a URL with secret");

        assert_eq!("[redacted webhook URL]", rendered);
        assert!(!rendered.contains("secret"));
    }
}
