use anyhow::{Result, bail};
use clap::Subcommand;
use serde_json::Value;

use crate::client::Client;

const PAGE_SIZE: usize = 100;

#[derive(Subcommand)]
pub enum WatchCommands {
    /// List repositories you watch (receive notifications for)
    List {
        /// Only repositories owned by this user or organization
        #[arg(short, long)]
        owner: Option<String>,
    },
    /// Stop watching repositories
    Remove {
        /// Repositories (owner/repo)
        repos: Vec<String>,
        /// Stop watching every watched repository owned by this user or organization
        #[arg(short, long, conflicts_with = "repos")]
        owner: Option<String>,
        /// Also unsubscribe from existing issue/PR notification threads in these repositories
        #[arg(long)]
        threads: bool,
    },
}

pub async fn handle(client: &Client, command: WatchCommands) -> Result<()> {
    match command {
        WatchCommands::List { owner } => {
            for repo in watched_repos(client, owner.as_deref()).await? {
                println!("{repo}");
            }
        }
        WatchCommands::Remove {
            repos,
            owner,
            threads,
        } => {
            let selection = Selection::new(repos, owner)?;
            unwatch_repos(client, &selection).await?;
            if threads {
                unsubscribe_threads(client, &selection).await?;
            }
        }
    }
    Ok(())
}

enum Selection {
    Owner(String),
    Repos(Vec<String>),
}

impl Selection {
    fn new(repos: Vec<String>, owner: Option<String>) -> Result<Self> {
        match owner {
            Some(owner) => Ok(Self::Owner(owner)),
            None if repos.is_empty() => bail!("Pass repositories or --owner"),
            None => Ok(Self::Repos(repos)),
        }
    }

    fn matches(&self, full_name: &str) -> bool {
        match self {
            Self::Owner(owner) => owner_matches(full_name, Some(owner)),
            Self::Repos(repos) => repos
                .iter()
                .any(|repo| repo.eq_ignore_ascii_case(full_name)),
        }
    }
}

async fn unwatch_repos(client: &Client, selection: &Selection) -> Result<()> {
    let targets = match selection {
        Selection::Owner(owner) => watched_repos(client, Some(owner)).await?,
        Selection::Repos(repos) => repos.clone(),
    };
    if targets.is_empty() {
        println!("No watched repositories matched");
    }
    for repo in targets {
        client
            .delete(&format!("/repos/{repo}/subscription"))
            .await?;
        println!("Unwatched {repo}");
    }
    Ok(())
}

/// Thread subscriptions are only discoverable through notification threads, so
/// threads that never produced a notification cannot be found here.
async fn unsubscribe_threads(client: &Client, selection: &Selection) -> Result<()> {
    let threads = all_pages(client, "/notifications?all=true").await?;
    let mut count = 0;
    for thread in &threads {
        let repo = thread["repository"]["full_name"]
            .as_str()
            .unwrap_or_default();
        let Some(id) = thread["id"].as_str() else {
            continue;
        };
        if selection.matches(repo) {
            client
                .delete(&format!("/notifications/threads/{id}/subscription"))
                .await?;
            count += 1;
        }
    }
    println!("Unsubscribed from {count} notification threads");
    Ok(())
}

async fn watched_repos(client: &Client, owner: Option<&str>) -> Result<Vec<String>> {
    Ok(all_pages(client, "/user/subscriptions")
        .await?
        .iter()
        .filter_map(|repo| repo["full_name"].as_str())
        .filter(|name| owner_matches(name, owner))
        .map(str::to_owned)
        .collect())
}

async fn all_pages(client: &Client, path: &str) -> Result<Vec<Value>> {
    let separator = if path.contains('?') { '&' } else { '?' };
    let mut items = Vec::new();
    for page in 1.. {
        let value = client
            .get(&format!(
                "{path}{separator}per_page={PAGE_SIZE}&page={page}"
            ))
            .await?;
        // Endpoints may cap per_page below PAGE_SIZE (notifications caps at 50),
        // so only an empty page marks the end.
        let batch = value.as_array().cloned().unwrap_or_default();
        if batch.is_empty() {
            break;
        }
        items.extend(batch);
    }
    Ok(items)
}

fn owner_matches(full_name: &str, owner: Option<&str>) -> bool {
    owner.is_none_or(|owner| {
        full_name
            .split_once('/')
            .is_some_and(|(repo_owner, _)| repo_owner.eq_ignore_ascii_case(owner))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::write_test_fixture::fixture;
    use clap::Parser;

    fn command(args: &[&str]) -> WatchCommands {
        let mut argv = vec!["github", "watch"];
        argv.extend_from_slice(args);
        match crate::Cli::try_parse_from(argv).unwrap().command {
            crate::Commands::Watch { command } => command,
            _ => unreachable!(),
        }
    }

    #[tokio::test]
    async fn remove_by_owner_pages_subscriptions_and_deletes_only_matching_repos() {
        let (client, server) = fixture(vec![
            ("200 OK", r#"[{"full_name":"other/a"}]"#),
            (
                "200 OK",
                r#"[{"full_name":"InvenioSoftware/invenio-app"},{"full_name":"other/x"}]"#,
            ),
            ("200 OK", "[]"),
            ("204 No Content", ""),
        ]);
        handle(&client, command(&["remove", "--owner", "inveniosoftware"]))
            .await
            .unwrap();
        let requests = server.join().unwrap();
        assert!(requests[0].starts_with("GET /user/subscriptions?per_page=100&page=1 "));
        assert!(requests[1].starts_with("GET /user/subscriptions?per_page=100&page=2 "));
        assert!(requests[2].starts_with("GET /user/subscriptions?per_page=100&page=3 "));
        assert!(
            requests[3].starts_with("DELETE /repos/InvenioSoftware/invenio-app/subscription "),
            "{}",
            requests[3]
        );
        assert_eq!(requests.len(), 4);
    }

    #[tokio::test]
    async fn remove_threads_reads_capped_pages_and_unsubscribes_only_selected_repos() {
        // GitHub caps notifications at 50 per page, below the requested 100.
        let (client, server) = fixture(vec![
            ("204 No Content", ""),
            (
                "200 OK",
                r#"[{"id":"11","repository":{"full_name":"Owner/Repo"}},{"id":"22","repository":{"full_name":"owner/other"}}]"#,
            ),
            (
                "200 OK",
                r#"[{"id":"33","repository":{"full_name":"owner/repo"}}]"#,
            ),
            ("200 OK", "[]"),
            ("204 No Content", ""),
            ("204 No Content", ""),
        ]);
        handle(&client, command(&["remove", "owner/repo", "--threads"]))
            .await
            .unwrap();
        let requests = server.join().unwrap();
        assert!(requests[0].starts_with("DELETE /repos/owner/repo/subscription "));
        assert!(requests[1].starts_with("GET /notifications?all=true&per_page=100&page=1 "));
        assert!(requests[2].starts_with("GET /notifications?all=true&per_page=100&page=2 "));
        assert!(requests[3].starts_with("GET /notifications?all=true&per_page=100&page=3 "));
        assert!(
            requests[4].starts_with("DELETE /notifications/threads/11/subscription "),
            "{}",
            requests[4]
        );
        assert!(requests[5].starts_with("DELETE /notifications/threads/33/subscription "));
        assert_eq!(requests.len(), 6);
    }

    #[tokio::test]
    async fn remove_reports_api_failure() {
        let (client, server) = fixture(vec![("404 Not Found", r#"{"message":"Not Found"}"#)]);
        let error = handle(&client, command(&["remove", "owner/missing"]))
            .await
            .unwrap_err();
        assert!(error.to_string().contains("404"), "{error}");
        assert_eq!(server.join().unwrap().len(), 1);
    }

    #[test]
    fn remove_rejects_repos_with_owner() {
        let argv = ["github", "watch", "remove", "a/b", "--owner", "a"];
        assert!(crate::Cli::try_parse_from(argv).is_err());
    }
}
