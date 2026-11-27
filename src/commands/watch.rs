use anyhow::{Result, bail};
use clap::Subcommand;

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
    },
}

pub async fn handle(client: &Client, command: WatchCommands) -> Result<()> {
    match command {
        WatchCommands::List { owner } => {
            for repo in watched_repos(client, owner.as_deref()).await? {
                println!("{repo}");
            }
        }
        WatchCommands::Remove { repos, owner } => {
            let targets = match owner {
                Some(owner) => watched_repos(client, Some(&owner)).await?,
                None if repos.is_empty() => bail!("Pass repositories or --owner"),
                None => repos,
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
        }
    }
    Ok(())
}

async fn watched_repos(client: &Client, owner: Option<&str>) -> Result<Vec<String>> {
    let mut names = Vec::new();
    for page in 1.. {
        let value = client
            .get(&format!(
                "/user/subscriptions?per_page={PAGE_SIZE}&page={page}"
            ))
            .await?;
        let repos = value.as_array().map(Vec::as_slice).unwrap_or_default();
        names.extend(
            repos
                .iter()
                .filter_map(|repo| repo["full_name"].as_str())
                .filter(|name| owner_matches(name, owner))
                .map(str::to_owned),
        );
        if repos.len() < PAGE_SIZE {
            break;
        }
    }
    Ok(names)
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

    fn full_page(owner: &str) -> &'static str {
        let repos: Vec<String> = (0..PAGE_SIZE)
            .map(|i| format!(r#"{{"full_name":"{owner}/repo{i}"}}"#))
            .collect();
        Box::leak(format!("[{}]", repos.join(",")).into_boxed_str())
    }

    #[tokio::test]
    async fn remove_by_owner_pages_subscriptions_and_deletes_only_matching_repos() {
        let (client, server) = fixture(vec![
            ("200 OK", full_page("other")),
            (
                "200 OK",
                r#"[{"full_name":"InvenioSoftware/invenio-app"},{"full_name":"other/x"}]"#,
            ),
            ("204 No Content", ""),
        ]);
        handle(&client, command(&["remove", "--owner", "inveniosoftware"]))
            .await
            .unwrap();
        let requests = server.join().unwrap();
        assert!(requests[0].starts_with("GET /user/subscriptions?per_page=100&page=1 "));
        assert!(requests[1].starts_with("GET /user/subscriptions?per_page=100&page=2 "));
        assert!(
            requests[2].starts_with("DELETE /repos/InvenioSoftware/invenio-app/subscription "),
            "{}",
            requests[2]
        );
        assert_eq!(requests.len(), 3);
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
