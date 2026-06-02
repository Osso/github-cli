use anyhow::{Result, bail};
use base64::Engine;
use clap::Subcommand;

use crate::client::Client;

#[derive(Subcommand)]
pub enum RepoCommands {
    /// List repositories for an organization
    List {
        /// Organization name
        #[arg(short, long)]
        org: String,
    },
    /// List branches for a repository
    Branches {
        /// Repository (owner/repo)
        repo: String,
    },
    /// Print a file from a repository
    Content {
        /// Repository (owner/repo)
        repo: String,
        /// File path in the repository
        path: String,
        /// Git ref (branch, tag, or SHA)
        #[arg(long = "ref")]
        git_ref: Option<String>,
    },
    /// Manage branch protection rules
    Protect {
        #[command(subcommand)]
        command: ProtectCommands,
    },
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
    /// Transfer a repository to a new owner
    Transfer {
        /// Repository (owner/repo)
        repo: String,
        /// New owner (user or organization)
        new_owner: String,
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

#[derive(Subcommand)]
pub enum ProtectCommands {
    /// Set branch protection (enforce admins, no force push, no deletion)
    Set {
        /// Repository (owner/repo)
        repo: String,
        /// Branch name
        #[arg(short, long)]
        branch: String,
        /// Required status check contexts (can be repeated)
        #[arg(short, long)]
        check: Vec<String>,
    },
    /// Show branch protection status
    Get {
        /// Repository (owner/repo)
        repo: String,
        /// Branch name
        #[arg(short, long)]
        branch: String,
    },
}

pub async fn handle(client: &Client, command: RepoCommands) -> Result<()> {
    match command {
        RepoCommands::List { org } => list_repos(client, &org).await,
        RepoCommands::Branches { repo } => list_branches(client, &repo).await,
        RepoCommands::Content {
            repo,
            path,
            git_ref,
        } => print_content(client, &repo, &path, git_ref.as_deref()).await,
        RepoCommands::Protect { command } => handle_protect(client, command).await,
        RepoCommands::Keys { command } => handle_keys(client, command).await,
        RepoCommands::Hooks { command } => handle_hooks(client, command).await,
        RepoCommands::Transfer { repo, new_owner } => {
            transfer_repo(client, &repo, &new_owner).await
        }
    }
}

async fn list_repos(client: &Client, org: &str) -> Result<()> {
    let result = client
        .get(&format!("/orgs/{org}/repos?per_page=100&sort=full_name"))
        .await?;
    print_repos(&result);
    Ok(())
}

async fn list_branches(client: &Client, repo: &str) -> Result<()> {
    let result = client
        .get(&format!("/repos/{repo}/branches?per_page=100"))
        .await?;
    print_branches(&result);
    Ok(())
}

async fn print_content(
    client: &Client,
    repo: &str,
    path: &str,
    git_ref: Option<&str>,
) -> Result<()> {
    let encoded_path = encode_repo_path(path);
    let mut api_path = format!("/repos/{repo}/contents/{encoded_path}");
    if let Some(git_ref) = git_ref {
        api_path.push_str("?ref=");
        api_path.push_str(&urlencoding::encode(git_ref));
    }

    let result = client.get(&api_path).await?;
    if result.is_array() {
        bail!("{repo}:{path} is a directory, not a file");
    }

    let encoding = result["encoding"].as_str().unwrap_or("");
    if encoding != "base64" {
        bail!("{repo}:{path} has unsupported encoding '{encoding}'");
    }

    let Some(content) = result["content"].as_str() else {
        bail!("{repo}:{path} response did not include file content");
    };
    let normalized = content.lines().collect::<String>();
    let decoded = base64::engine::general_purpose::STANDARD.decode(normalized)?;
    print!("{}", String::from_utf8_lossy(&decoded));
    Ok(())
}

fn encode_repo_path(path: &str) -> String {
    path.split('/')
        .filter(|segment| !segment.is_empty())
        .map(urlencoding::encode)
        .collect::<Vec<_>>()
        .join("/")
}

async fn handle_protect(client: &Client, command: ProtectCommands) -> Result<()> {
    match command {
        ProtectCommands::Set {
            repo,
            branch,
            check,
        } => set_protection(client, &repo, &branch, &check).await,
        ProtectCommands::Get { repo, branch } => get_protection(client, &repo, &branch).await,
    }
}

async fn set_protection(
    client: &Client,
    repo: &str,
    branch: &str,
    checks: &[String],
) -> Result<()> {
    let status_checks = if checks.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::json!({ "strict": true, "contexts": checks })
    };
    let body = serde_json::json!({
        "required_status_checks": status_checks,
        "enforce_admins": true,
        "required_pull_request_reviews": null,
        "restrictions": null,
        "allow_force_pushes": false,
        "allow_deletions": false,
        "block_creations": false,
        "lock_branch": false,
    });
    client
        .put(
            &format!("/repos/{repo}/branches/{branch}/protection"),
            &body,
        )
        .await?;
    println!("Protected {repo}:{branch} (enforce admins, no force push, no deletion)");
    if !checks.is_empty() {
        println!("Required checks: {}", checks.join(", "));
    }
    Ok(())
}

async fn get_protection(client: &Client, repo: &str, branch: &str) -> Result<()> {
    match client
        .get(&format!("/repos/{repo}/branches/{branch}/protection"))
        .await
    {
        Ok(result) => print_protection(&result),
        Err(e) if e.to_string().contains("404") => {
            println!("No branch protection rules for {repo}:{branch}");
        }
        Err(e) => return Err(e),
    }
    Ok(())
}

async fn handle_keys(client: &Client, command: KeyCommands) -> Result<()> {
    match command {
        KeyCommands::List { repo } => {
            let result = client.get(&format!("/repos/{repo}/keys")).await?;
            print_deploy_keys(&result);
        }
        KeyCommands::Add {
            repo,
            title,
            key,
            write,
        } => {
            add_deploy_key(client, &repo, &title, &key, write).await?;
        }
        KeyCommands::Remove { repo, key_id } => {
            client
                .delete(&format!("/repos/{repo}/keys/{key_id}"))
                .await?;
            println!("Removed deploy key {key_id} from {repo}");
        }
    }
    Ok(())
}

async fn add_deploy_key(
    client: &Client,
    repo: &str,
    title: &str,
    key: &str,
    write: bool,
) -> Result<()> {
    let key_content = if std::path::Path::new(key).exists() {
        std::fs::read_to_string(key)?.trim().to_string()
    } else {
        key.to_string()
    };
    let body = serde_json::json!({ "title": title, "key": key_content, "read_only": !write });
    let result = client.post(&format!("/repos/{repo}/keys"), &body).await?;
    let id = result["id"].as_u64().unwrap_or(0);
    let access = if write { "read-write" } else { "read-only" };
    println!("Added deploy key '{title}' (id: {id}) to {repo} [{access}]");
    Ok(())
}

async fn handle_hooks(client: &Client, command: HookCommands) -> Result<()> {
    match command {
        HookCommands::List { repo } => {
            let result = client.get(&format!("/repos/{repo}/hooks")).await?;
            print_hooks(&result);
        }
    }
    Ok(())
}

async fn transfer_repo(client: &Client, repo: &str, new_owner: &str) -> Result<()> {
    let body = serde_json::json!({ "new_owner": new_owner });
    let result = client
        .post(&format!("/repos/{repo}/transfer"), &body)
        .await?;
    let full_name = result["full_name"].as_str().unwrap_or(repo);
    println!("Transferred to {full_name}");
    Ok(())
}

fn print_repos(value: &serde_json::Value) {
    let Some(repos) = value.as_array() else {
        return;
    };
    if repos.is_empty() {
        println!("No repositories found");
        return;
    }
    for repo in repos {
        let name = repo["name"].as_str().unwrap_or("");
        let archived = if repo["archived"].as_bool().unwrap_or(false) {
            " [archived]"
        } else {
            ""
        };
        let default_branch = repo["default_branch"].as_str().unwrap_or("");
        let private = if repo["private"].as_bool().unwrap_or(false) {
            "private"
        } else {
            "public"
        };
        println!("{name:<30} {default_branch:<10} [{private}]{archived}");
    }
}

fn print_branches(value: &serde_json::Value) {
    let Some(branches) = value.as_array() else {
        return;
    };
    if branches.is_empty() {
        println!("No branches found");
        return;
    }
    for branch in branches {
        let name = branch["name"].as_str().unwrap_or("");
        let protected = if branch["protected"].as_bool().unwrap_or(false) {
            " [protected]"
        } else {
            ""
        };
        println!("{name}{protected}");
    }
}

fn print_protection(value: &serde_json::Value) {
    let enforce_admins = value["enforce_admins"]["enabled"]
        .as_bool()
        .unwrap_or(false);
    let force_pushes = value["allow_force_pushes"]["enabled"]
        .as_bool()
        .unwrap_or(false);
    let deletions = value["allow_deletions"]["enabled"]
        .as_bool()
        .unwrap_or(false);
    println!("Enforce admins: {enforce_admins}");
    println!("Allow force pushes: {force_pushes}");
    println!("Allow deletions: {deletions}");
    if let Some(checks) = value["required_status_checks"].as_object() {
        let strict = checks
            .get("strict")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        println!("Required status checks (strict: {strict})");
    }
    if value["required_pull_request_reviews"].is_object() {
        println!("Required PR reviews: yes");
    }
}

fn print_deploy_keys(value: &serde_json::Value) {
    let Some(keys) = value.as_array() else { return };
    if keys.is_empty() {
        println!("No deploy keys found");
        return;
    }
    for key in keys {
        let id = key["id"].as_u64().unwrap_or(0);
        let title = key["title"].as_str().unwrap_or("");
        let read_only = key["read_only"].as_bool().unwrap_or(true);
        let created = key["created_at"]
            .as_str()
            .unwrap_or("")
            .split('T')
            .next()
            .unwrap_or("");
        let access = if read_only { "read-only" } else { "read-write" };
        println!("{id:<10} {title:<30} [{access}] {created}");
    }
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
