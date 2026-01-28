use anyhow::{anyhow, bail, Context, Result};
use clap::{Parser, Subcommand};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Parser)]
#[command(name = "github", about = "GitHub CLI")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// React to an issue or PR
    React {
        /// Repository (owner/repo)
        repo: String,
        /// Issue or PR number
        number: u64,
        /// Reaction type (+1, -1, laugh, confused, heart, hooray, rocket, eyes)
        #[arg(default_value = "+1")]
        reaction: String,
    },
    /// List issues
    Issue {
        #[command(subcommand)]
        command: IssueCommands,
    },
    /// List pull requests
    Pr {
        #[command(subcommand)]
        command: PrCommands,
    },
    /// Manage teams
    Team {
        #[command(subcommand)]
        command: TeamCommands,
    },
    /// Organization management
    Org {
        #[command(subcommand)]
        command: OrgCommands,
    },
    /// Repository management
    Repo {
        #[command(subcommand)]
        command: RepoCommands,
    },
    /// Manage GitHub App installations
    App {
        #[command(subcommand)]
        command: AppCommands,
    },
    /// Manage Actions runners
    Runner {
        #[command(subcommand)]
        command: RunnerCommands,
    },
    /// Configure token
    Config {
        /// GitHub personal access token
        #[arg(long)]
        token: Option<String>,
    },
}

#[derive(Subcommand)]
enum IssueCommands {
    /// List issues
    List {
        /// Repository (owner/repo)
        repo: String,
        /// Search query
        #[arg(short = 'S', long)]
        search: Option<String>,
        /// Limit results
        #[arg(short, long, default_value = "10")]
        limit: u32,
    },
    /// View issue details
    View {
        /// Repository (owner/repo)
        repo: String,
        /// Issue number
        number: u64,
    },
}

#[derive(Subcommand)]
enum PrCommands {
    /// List pull requests
    List {
        /// Repository (owner/repo)
        repo: String,
        /// State (open, closed, all)
        #[arg(short, long, default_value = "open")]
        state: String,
        /// Limit results
        #[arg(short, long, default_value = "10")]
        limit: u32,
    },
    /// View PR details
    View {
        /// Repository (owner/repo)
        repo: String,
        /// PR number
        number: u64,
    },
    /// Post a comment on a PR
    Comment {
        /// Repository (owner/repo)
        repo: String,
        /// PR number
        number: u64,
        /// Comment body
        #[arg(short, long)]
        message: String,
    },
    /// Approve a PR
    Approve {
        /// Repository (owner/repo)
        repo: String,
        /// PR number
        number: u64,
    },
    /// List review comments (discussions) on a PR
    Discussions {
        /// Repository (owner/repo)
        repo: String,
        /// PR number
        number: u64,
        /// Show only unresolved threads
        #[arg(long)]
        unresolved: bool,
    },
    /// Reply to a review comment
    Reply {
        /// Repository (owner/repo)
        repo: String,
        /// PR number
        number: u64,
        /// Comment ID to reply to
        #[arg(long)]
        comment: u64,
        /// Reply body
        #[arg(short, long)]
        message: String,
    },
    /// Submit a review with inline comments
    Review {
        /// Repository (owner/repo)
        repo: String,
        /// PR number
        number: u64,
        /// Review summary body
        #[arg(short = 'b', long)]
        body: Option<String>,
        /// Review event: COMMENT, APPROVE, REQUEST_CHANGES
        #[arg(short, long, default_value = "COMMENT")]
        event: String,
        /// Inline comment (repeatable): path:line:body
        #[arg(long = "comment", short = 'c')]
        comments: Vec<String>,
    },
}

#[derive(Subcommand)]
enum TeamCommands {
    /// List teams in an organization
    List {
        /// Organization name
        org: String,
        /// Limit results
        #[arg(short, long, default_value = "30")]
        limit: u32,
    },
    /// Create a new team
    Create {
        /// Organization name
        org: String,
        /// Team name
        name: String,
        /// Team description
        #[arg(short, long)]
        description: Option<String>,
        /// Privacy level (secret or closed)
        #[arg(short, long, default_value = "closed")]
        privacy: String,
        /// Parent team ID (for nested teams)
        #[arg(long)]
        parent_team_id: Option<u64>,
    },
    /// Get team details
    View {
        /// Organization name
        org: String,
        /// Team slug
        team: String,
    },
    /// List team members
    Members {
        /// Organization name
        org: String,
        /// Team slug
        team: String,
        /// Limit results
        #[arg(short, long, default_value = "30")]
        limit: u32,
    },
    /// Add a member to a team
    AddMember {
        /// Organization name
        org: String,
        /// Team slug
        team: String,
        /// Username to add
        username: String,
        /// Role (member or maintainer)
        #[arg(short, long, default_value = "member")]
        role: String,
    },
    /// Remove a member from a team
    RemoveMember {
        /// Organization name
        org: String,
        /// Team slug
        team: String,
        /// Username to remove
        username: String,
    },
    /// Add a repository to a team
    AddRepo {
        /// Organization name
        org: String,
        /// Team slug
        team: String,
        /// Repository (owner/repo)
        repo: String,
        /// Permission level (pull, triage, push, maintain, admin)
        #[arg(short, long, default_value = "push")]
        permission: String,
    },
    /// List team repositories
    Repos {
        /// Organization name
        org: String,
        /// Team slug
        team: String,
        /// Limit results
        #[arg(short, long, default_value = "30")]
        limit: u32,
    },
}

#[derive(Subcommand)]
enum OrgCommands {
    /// Invite a user to the organization by email
    Invite {
        /// Organization name
        org: String,
        /// Email address to invite
        email: String,
        /// Role (admin, direct_member, billing_manager)
        #[arg(short, long, default_value = "direct_member")]
        role: String,
        /// Team IDs to add the user to (comma-separated)
        #[arg(short, long)]
        teams: Option<String>,
    },
    /// List pending invitations
    Invitations {
        /// Organization name
        org: String,
        /// Limit results
        #[arg(short, long, default_value = "30")]
        limit: u32,
    },
    /// List organization members
    Members {
        /// Organization name
        org: String,
        /// Limit results
        #[arg(short, long, default_value = "100")]
        limit: u32,
    },
}

#[derive(Subcommand)]
enum RepoCommands {
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
enum KeyCommands {
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
enum HookCommands {
    /// List webhooks for a repository
    List {
        /// Repository (owner/repo)
        repo: String,
    },
}

#[derive(Subcommand)]
enum AppCommands {
    /// List GitHub App installations for an organization
    List {
        /// Organization name
        org: String,
    },
}

#[derive(Subcommand)]
enum RunnerCommands {
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

#[derive(Debug, Serialize, Deserialize, Default)]
struct Config {
    token: Option<String>,
}

impl Config {
    fn path() -> Result<PathBuf> {
        let dir = dirs::config_dir()
            .ok_or_else(|| anyhow!("No config directory"))?
            .join("github-cli");
        Ok(dir.join("config.json"))
    }

    fn load() -> Result<Self> {
        let path = Self::path()?;
        if path.exists() {
            let contents = std::fs::read_to_string(&path)?;
            Ok(serde_json::from_str(&contents)?)
        } else {
            Ok(Self::default())
        }
    }

    fn save(&self) -> Result<()> {
        let path = Self::path()?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&path, serde_json::to_string_pretty(self)?)?;
        Ok(())
    }

    fn get_token(&self) -> Option<String> {
        std::env::var("GITHUB_TOKEN")
            .ok()
            .or_else(|| self.token.clone())
            .or_else(|| {
                std::process::Command::new("gh")
                    .args(["auth", "token"])
                    .output()
                    .ok()
                    .filter(|o| o.status.success())
                    .and_then(|o| String::from_utf8(o.stdout).ok())
                    .map(|s| s.trim().to_string())
            })
    }
}

struct Client {
    http: reqwest::Client,
}

impl Client {
    fn new(token: &str) -> Result<Self> {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {token}").parse()?,
        );
        headers.insert(
            reqwest::header::ACCEPT,
            "application/vnd.github+json".parse()?,
        );
        headers.insert("X-GitHub-Api-Version", "2022-11-28".parse()?);
        headers.insert(
            reqwest::header::USER_AGENT,
            "github-cli/0.1.0".parse()?,
        );

        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;

        Ok(Self { http })
    }

    async fn get(&self, path: &str) -> Result<serde_json::Value> {
        let url = format!("https://api.github.com{path}");
        let resp = self.http.get(&url).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await?;
            bail!("GET {path} failed ({status}): {body}");
        }
        Ok(resp.json().await?)
    }

    async fn post(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        let url = format!("https://api.github.com{path}");
        let resp = self.http.post(&url).json(body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await?;
            bail!("POST {path} failed ({status}): {body}");
        }
        Ok(resp.json().await?)
    }

    async fn react(&self, repo: &str, number: u64, reaction: &str) -> Result<serde_json::Value> {
        let path = format!("/repos/{repo}/issues/{number}/reactions");
        self.post(&path, &serde_json::json!({ "content": reaction }))
            .await
    }

    async fn list_issues(
        &self,
        repo: &str,
        limit: u32,
    ) -> Result<serde_json::Value> {
        let path = format!("/repos/{repo}/issues?per_page={limit}&state=open");
        self.get(&path).await
    }

    async fn get_issue(&self, repo: &str, number: u64) -> Result<serde_json::Value> {
        let path = format!("/repos/{repo}/issues/{number}");
        self.get(&path).await
    }

    async fn search_issues(&self, repo: &str, query: &str, limit: u32) -> Result<serde_json::Value> {
        let search_query = format!("repo:{repo} {query}");
        let q = urlencoding::encode(&search_query);
        let path = format!("/search/issues?q={q}&per_page={limit}");
        self.get(&path).await
    }

    async fn list_prs(
        &self,
        repo: &str,
        state: &str,
        limit: u32,
    ) -> Result<serde_json::Value> {
        let path = format!("/repos/{repo}/pulls?per_page={limit}&state={state}");
        self.get(&path).await
    }

    async fn get_pr(&self, repo: &str, number: u64) -> Result<serde_json::Value> {
        let path = format!("/repos/{repo}/pulls/{number}");
        self.get(&path).await
    }

    async fn comment_on_issue(&self, repo: &str, number: u64, body: &str) -> Result<serde_json::Value> {
        let path = format!("/repos/{repo}/issues/{number}/comments");
        self.post(&path, &serde_json::json!({ "body": body })).await
    }

    async fn approve_pr(&self, repo: &str, number: u64) -> Result<serde_json::Value> {
        let path = format!("/repos/{repo}/pulls/{number}/reviews");
        self.post(&path, &serde_json::json!({ "event": "APPROVE" })).await
    }

    async fn list_review_comments(&self, repo: &str, number: u64) -> Result<serde_json::Value> {
        let path = format!("/repos/{repo}/pulls/{number}/comments?per_page=100");
        self.get(&path).await
    }

    async fn create_review(
        &self,
        repo: &str,
        number: u64,
        event: &str,
        body: Option<&str>,
        comments: Vec<serde_json::Value>,
    ) -> Result<serde_json::Value> {
        let path = format!("/repos/{repo}/pulls/{number}/reviews");
        let mut payload = serde_json::json!({ "event": event });
        if let Some(b) = body {
            payload["body"] = serde_json::json!(b);
        }
        if !comments.is_empty() {
            payload["comments"] = serde_json::json!(comments);
        }
        self.post(&path, &payload).await
    }

    async fn reply_to_review_comment(&self, repo: &str, _number: u64, comment_id: u64, body: &str) -> Result<serde_json::Value> {
        let path = format!("/repos/{repo}/pulls/comments/{comment_id}/replies");
        self.post(&path, &serde_json::json!({ "body": body })).await
    }

    async fn put(&self, path: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        let url = format!("https://api.github.com{path}");
        let resp = self.http.put(&url).json(body).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await?;
            bail!("PUT {path} failed ({status}): {body}");
        }
        // Some PUT endpoints return empty body
        let text = resp.text().await?;
        if text.is_empty() {
            Ok(serde_json::json!({}))
        } else {
            Ok(serde_json::from_str(&text)?)
        }
    }

    async fn delete(&self, path: &str) -> Result<()> {
        let url = format!("https://api.github.com{path}");
        let resp = self.http.delete(&url).send().await?;
        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await?;
            bail!("DELETE {path} failed ({status}): {body}");
        }
        Ok(())
    }

    // Team operations
    async fn list_teams(&self, org: &str, limit: u32) -> Result<serde_json::Value> {
        let path = format!("/orgs/{org}/teams?per_page={limit}");
        self.get(&path).await
    }

    async fn create_team(
        &self,
        org: &str,
        name: &str,
        description: Option<&str>,
        privacy: &str,
        parent_team_id: Option<u64>,
    ) -> Result<serde_json::Value> {
        let path = format!("/orgs/{org}/teams");
        let mut body = serde_json::json!({
            "name": name,
            "privacy": privacy,
        });
        if let Some(desc) = description {
            body["description"] = serde_json::json!(desc);
        }
        if let Some(parent_id) = parent_team_id {
            body["parent_team_id"] = serde_json::json!(parent_id);
        }
        self.post(&path, &body).await
    }

    async fn get_team(&self, org: &str, team_slug: &str) -> Result<serde_json::Value> {
        let path = format!("/orgs/{org}/teams/{team_slug}");
        self.get(&path).await
    }

    async fn list_team_members(&self, org: &str, team_slug: &str, limit: u32) -> Result<serde_json::Value> {
        let path = format!("/orgs/{org}/teams/{team_slug}/members?per_page={limit}");
        self.get(&path).await
    }

    async fn add_team_member(&self, org: &str, team_slug: &str, username: &str, role: &str) -> Result<serde_json::Value> {
        let path = format!("/orgs/{org}/teams/{team_slug}/memberships/{username}");
        self.put(&path, &serde_json::json!({ "role": role })).await
    }

    async fn remove_team_member(&self, org: &str, team_slug: &str, username: &str) -> Result<()> {
        let path = format!("/orgs/{org}/teams/{team_slug}/memberships/{username}");
        self.delete(&path).await
    }

    async fn add_team_repo(&self, org: &str, team_slug: &str, repo: &str, permission: &str) -> Result<serde_json::Value> {
        let path = format!("/orgs/{org}/teams/{team_slug}/repos/{repo}");
        self.put(&path, &serde_json::json!({ "permission": permission })).await
    }

    async fn list_team_repos(&self, org: &str, team_slug: &str, limit: u32) -> Result<serde_json::Value> {
        let path = format!("/orgs/{org}/teams/{team_slug}/repos?per_page={limit}");
        self.get(&path).await
    }

    // Organization operations
    async fn invite_to_org(&self, org: &str, email: &str, role: &str, team_ids: Option<Vec<u64>>) -> Result<serde_json::Value> {
        let path = format!("/orgs/{org}/invitations");
        let mut body = serde_json::json!({
            "email": email,
            "role": role,
        });
        if let Some(ids) = team_ids {
            body["team_ids"] = serde_json::json!(ids);
        }
        self.post(&path, &body).await
    }

    async fn list_org_invitations(&self, org: &str, limit: u32) -> Result<serde_json::Value> {
        let path = format!("/orgs/{org}/invitations?per_page={limit}");
        self.get(&path).await
    }

    async fn list_org_members(&self, org: &str, limit: u32) -> Result<serde_json::Value> {
        let path = format!("/orgs/{org}/members?per_page={limit}");
        self.get(&path).await
    }

    // Deploy key operations
    async fn add_deploy_key(&self, repo: &str, title: &str, key: &str, read_only: bool) -> Result<serde_json::Value> {
        let path = format!("/repos/{repo}/keys");
        self.post(&path, &serde_json::json!({
            "title": title,
            "key": key,
            "read_only": read_only
        })).await
    }

    async fn list_deploy_keys(&self, repo: &str) -> Result<serde_json::Value> {
        let path = format!("/repos/{repo}/keys");
        self.get(&path).await
    }

    async fn remove_deploy_key(&self, repo: &str, key_id: u64) -> Result<()> {
        let path = format!("/repos/{repo}/keys/{key_id}");
        self.delete(&path).await
    }

    async fn list_repo_hooks(&self, repo: &str) -> Result<serde_json::Value> {
        let path = format!("/repos/{repo}/hooks");
        self.get(&path).await
    }

    async fn list_org_installations(&self, org: &str) -> Result<serde_json::Value> {
        let path = format!("/orgs/{org}/installations");
        self.get(&path).await
    }

    // Runner operations
    async fn list_repo_runners(&self, repo: &str) -> Result<serde_json::Value> {
        let path = format!("/repos/{repo}/actions/runners");
        self.get(&path).await
    }

    async fn list_org_runners(&self, org: &str) -> Result<serde_json::Value> {
        let path = format!("/orgs/{org}/actions/runners");
        self.get(&path).await
    }

    async fn get_repo_runner(&self, repo: &str, runner_id: u64) -> Result<serde_json::Value> {
        let path = format!("/repos/{repo}/actions/runners/{runner_id}");
        self.get(&path).await
    }

    async fn get_org_runner(&self, org: &str, runner_id: u64) -> Result<serde_json::Value> {
        let path = format!("/orgs/{org}/actions/runners/{runner_id}");
        self.get(&path).await
    }

    async fn delete_repo_runner(&self, repo: &str, runner_id: u64) -> Result<()> {
        let path = format!("/repos/{repo}/actions/runners/{runner_id}");
        self.delete(&path).await
    }

    async fn delete_org_runner(&self, org: &str, runner_id: u64) -> Result<()> {
        let path = format!("/orgs/{org}/actions/runners/{runner_id}");
        self.delete(&path).await
    }
}

fn get_client(config: &Config) -> Result<Client> {
    let token = config
        .get_token()
        .ok_or_else(|| anyhow!("No token. Set GITHUB_TOKEN or run: github config --token <token>"))?;
    Client::new(&token)
}

fn print_issues(value: &serde_json::Value) {
    let items = if let Some(arr) = value.as_array() {
        arr.clone()
    } else if let Some(obj) = value.get("items").and_then(|v| v.as_array()) {
        obj.clone()
    } else {
        return;
    };

    for issue in items {
        let number = issue["number"].as_u64().unwrap_or(0);
        let title = issue["title"].as_str().unwrap_or("");
        let state = issue["state"].as_str().unwrap_or("");
        let labels: Vec<&str> = issue["labels"]
            .as_array()
            .map(|arr| arr.iter().filter_map(|l| l["name"].as_str()).collect())
            .unwrap_or_default();
        let labels_str = if labels.is_empty() {
            String::new()
        } else {
            format!(" [{}]", labels.join(", "))
        };
        println!("#{number:<5} {state:<6} {title}{labels_str}");
    }
}

fn print_prs(value: &serde_json::Value) {
    if let Some(prs) = value.as_array() {
        for pr in prs {
            let number = pr["number"].as_u64().unwrap_or(0);
            let title = pr["title"].as_str().unwrap_or("");
            let state = pr["state"].as_str().unwrap_or("");
            let draft = pr["draft"].as_bool().unwrap_or(false);
            let draft_str = if draft { " [draft]" } else { "" };
            println!("#{number:<5} {state:<6} {title}{draft_str}");
        }
    }
}

fn print_issue_detail(value: &serde_json::Value) {
    let number = value["number"].as_u64().unwrap_or(0);
    let title = value["title"].as_str().unwrap_or("");
    let state = value["state"].as_str().unwrap_or("");
    let author = value["user"]["login"].as_str().unwrap_or("");
    let body = value["body"].as_str().unwrap_or("");
    let url = value["html_url"].as_str().unwrap_or("");

    println!("#{number} {title}");
    println!("State: {state}  Author: {author}");
    println!("URL: {url}");
    if !body.is_empty() {
        println!("\n{body}");
    }
}

fn print_teams(value: &serde_json::Value) {
    if let Some(teams) = value.as_array() {
        for team in teams {
            let id = team["id"].as_u64().unwrap_or(0);
            let name = team["name"].as_str().unwrap_or("");
            let slug = team["slug"].as_str().unwrap_or("");
            let privacy = team["privacy"].as_str().unwrap_or("");
            let members = team["members_count"].as_u64().unwrap_or(0);
            println!("{id:<10} {slug:<25} {privacy:<8} {members:>3} members  {name}");
        }
    }
}

fn print_team_detail(value: &serde_json::Value) {
    let id = value["id"].as_u64().unwrap_or(0);
    let name = value["name"].as_str().unwrap_or("");
    let slug = value["slug"].as_str().unwrap_or("");
    let description = value["description"].as_str().unwrap_or("");
    let privacy = value["privacy"].as_str().unwrap_or("");
    let members = value["members_count"].as_u64().unwrap_or(0);
    let repos = value["repos_count"].as_u64().unwrap_or(0);
    let url = value["html_url"].as_str().unwrap_or("");

    println!("{name} ({slug})");
    println!("ID: {id}  Privacy: {privacy}  Members: {members}  Repos: {repos}");
    if !description.is_empty() {
        println!("Description: {description}");
    }
    println!("URL: {url}");
}

fn print_team_members(value: &serde_json::Value) {
    if let Some(members) = value.as_array() {
        for member in members {
            let login = member["login"].as_str().unwrap_or("");
            let id = member["id"].as_u64().unwrap_or(0);
            let role = member["role"].as_str().unwrap_or("member");
            println!("{login:<25} {role:<12} (id: {id})");
        }
    }
}

fn print_team_repos(value: &serde_json::Value) {
    if let Some(repos) = value.as_array() {
        for repo in repos {
            let full_name = repo["full_name"].as_str().unwrap_or("");
            let permission = repo["role_name"].as_str().unwrap_or("");
            let private = repo["private"].as_bool().unwrap_or(false);
            let visibility = if private { "private" } else { "public" };
            println!("{full_name:<40} {permission:<10} {visibility}");
        }
    }
}

fn print_org_invitations(value: &serde_json::Value) {
    if let Some(invitations) = value.as_array() {
        if invitations.is_empty() {
            println!("No pending invitations");
            return;
        }
        for inv in invitations {
            let email = inv["email"].as_str().unwrap_or("");
            let login = inv["login"].as_str().unwrap_or("-");
            let role = inv["role"].as_str().unwrap_or("");
            let created = inv["created_at"].as_str().unwrap_or("").split('T').next().unwrap_or("");
            let teams: Vec<&str> = inv["team_count"]
                .as_u64()
                .map(|n| format!("{} teams", n))
                .unwrap_or_default()
                .split(' ')
                .collect();
            println!("{email:<40} {login:<20} {role:<15} {created}");
        }
    }
}

fn print_org_members(value: &serde_json::Value) {
    if let Some(members) = value.as_array() {
        if members.is_empty() {
            println!("No members found");
            return;
        }
        for member in members {
            let login = member["login"].as_str().unwrap_or("");
            let id = member["id"].as_u64().unwrap_or(0);
            let site_admin = member["site_admin"].as_bool().unwrap_or(false);
            let admin_str = if site_admin { " [site-admin]" } else { "" };
            println!("{login:<30} (id: {id}){admin_str}");
        }
    }
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
                .map(|obj| obj.iter().map(|(k, v)| format!("{}:{}", k, v.as_str().unwrap_or(""))).collect())
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

fn print_runners(value: &serde_json::Value) {
    let runners = value["runners"].as_array();
    let total = value["total_count"].as_u64().unwrap_or(0);

    if let Some(runners) = runners {
        if runners.is_empty() {
            println!("No runners found");
            return;
        }
        println!("Total: {total}");
        println!("{:<8} {:<40} {:<10} {:<6} Labels", "ID", "Name", "Status", "Busy");
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
    println!("Labels: {}", if labels.is_empty() { "(none)".to_string() } else { labels.join(", ") });
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load().context("Failed to load config")?;

    match cli.command {
        Commands::Config { token } => {
            let mut config = config;
            if let Some(t) = token {
                config.token = Some(t);
                config.save()?;
                println!("Token saved");
            } else {
                let path = Config::path()?;
                println!("Config path: {}", path.display());
                if config.token.is_some() {
                    println!("Token: configured");
                } else if std::env::var("GITHUB_TOKEN").is_ok() {
                    println!("Token: from GITHUB_TOKEN env");
                } else if std::process::Command::new("gh")
                    .args(["auth", "token"])
                    .output()
                    .is_ok_and(|o| o.status.success())
                {
                    println!("Token: from gh auth");
                } else {
                    println!("Token: not configured");
                }
            }
        }
        Commands::React { repo, number, reaction } => {
            let client = get_client(&config)?;
            let result = client.react(&repo, number, &reaction).await?;
            let content = result["content"].as_str().unwrap_or(&reaction);
            println!("Added {content} reaction to {repo}#{number}");
        }
        Commands::Issue { command } => {
            let client = get_client(&config)?;
            match command {
                IssueCommands::List { repo, search, limit } => {
                    let result = if let Some(query) = search {
                        client.search_issues(&repo, &query, limit).await?
                    } else {
                        client.list_issues(&repo, limit).await?
                    };
                    print_issues(&result);
                }
                IssueCommands::View { repo, number } => {
                    let result = client.get_issue(&repo, number).await?;
                    print_issue_detail(&result);
                }
            }
        }
        Commands::Pr { command } => {
            let client = get_client(&config)?;
            match command {
                PrCommands::List { repo, state, limit } => {
                    let result = client.list_prs(&repo, &state, limit).await?;
                    print_prs(&result);
                }
                PrCommands::View { repo, number } => {
                    let result = client.get_pr(&repo, number).await?;
                    print_issue_detail(&result);
                }
                PrCommands::Comment { repo, number, message } => {
                    let result = client.comment_on_issue(&repo, number, &message).await?;
                    let id = result["id"].as_u64().unwrap_or(0);
                    println!("Posted comment (id: {id}) on {repo}#{number}");
                }
                PrCommands::Approve { repo, number } => {
                    client.approve_pr(&repo, number).await?;
                    println!("Approved {repo}#{number}");
                }
                PrCommands::Discussions { repo, number, unresolved: _ } => {
                    let result = client.list_review_comments(&repo, number).await?;
                    if let Some(comments) = result.as_array() {
                        if comments.is_empty() {
                            println!("No review comments");
                        } else {
                            for c in comments {
                                let id = c["id"].as_u64().unwrap_or(0);
                                let path = c["path"].as_str().unwrap_or("?");
                                let line = c["line"].as_u64().unwrap_or(0);
                                let author = c["user"]["login"].as_str().unwrap_or("?");
                                let body = c["body"].as_str().unwrap_or("");
                                println!("{id:<10} {path}:{line} @{author}");
                                println!("  {body}");
                                println!();
                            }
                        }
                    }
                }
                PrCommands::Reply { repo, number, comment, message } => {
                    let result = client.reply_to_review_comment(&repo, number, comment, &message).await?;
                    let id = result["id"].as_u64().unwrap_or(0);
                    println!("Posted reply (id: {id}) to comment {comment}");
                }
                PrCommands::Review { repo, number, body, event, comments } => {
                    let inline_comments: Vec<serde_json::Value> = comments
                        .iter()
                        .map(|c| {
                            // Format: path:line:body
                            let mut parts = c.splitn(3, ':');
                            let path = parts.next().unwrap_or("");
                            let line: u64 = parts.next().unwrap_or("0").parse().unwrap_or(0);
                            let comment_body = parts.next().unwrap_or("");
                            serde_json::json!({
                                "path": path,
                                "line": line,
                                "side": "RIGHT",
                                "body": comment_body
                            })
                        })
                        .collect();
                    let event_upper = event.to_uppercase();
                    let result = client.create_review(
                        &repo,
                        number,
                        &event_upper,
                        body.as_deref(),
                        inline_comments,
                    ).await?;
                    let review_id = result["id"].as_u64().unwrap_or(0);
                    println!("Submitted review (id: {review_id}) on {repo}#{number} [{event_upper}]");
                }
            }
        }
        Commands::Team { command } => {
            let client = get_client(&config)?;
            match command {
                TeamCommands::List { org, limit } => {
                    let result = client.list_teams(&org, limit).await?;
                    print_teams(&result);
                }
                TeamCommands::Create { org, name, description, privacy, parent_team_id } => {
                    let result = client.create_team(&org, &name, description.as_deref(), &privacy, parent_team_id).await?;
                    let slug = result["slug"].as_str().unwrap_or("");
                    let id = result["id"].as_u64().unwrap_or(0);
                    println!("Created team '{name}' (slug: {slug}, id: {id})");
                }
                TeamCommands::View { org, team } => {
                    let result = client.get_team(&org, &team).await?;
                    print_team_detail(&result);
                }
                TeamCommands::Members { org, team, limit } => {
                    let result = client.list_team_members(&org, &team, limit).await?;
                    print_team_members(&result);
                }
                TeamCommands::AddMember { org, team, username, role } => {
                    let result = client.add_team_member(&org, &team, &username, &role).await?;
                    let state = result["state"].as_str().unwrap_or("added");
                    println!("User '{username}' {state} to team '{team}' as {role}");
                }
                TeamCommands::RemoveMember { org, team, username } => {
                    client.remove_team_member(&org, &team, &username).await?;
                    println!("Removed '{username}' from team '{team}'");
                }
                TeamCommands::AddRepo { org, team, repo, permission } => {
                    client.add_team_repo(&org, &team, &repo, &permission).await?;
                    println!("Added repo '{repo}' to team '{team}' with {permission} permission");
                }
                TeamCommands::Repos { org, team, limit } => {
                    let result = client.list_team_repos(&org, &team, limit).await?;
                    print_team_repos(&result);
                }
            }
        }
        Commands::Org { command } => {
            let client = get_client(&config)?;
            match command {
                OrgCommands::Invite { org, email, role, teams } => {
                    let team_ids = teams.map(|t| {
                        t.split(',')
                            .filter_map(|s| s.trim().parse::<u64>().ok())
                            .collect()
                    });
                    let result = client.invite_to_org(&org, &email, &role, team_ids).await?;
                    let id = result["id"].as_u64().unwrap_or(0);
                    println!("Invitation sent to {email} (id: {id})");
                }
                OrgCommands::Invitations { org, limit } => {
                    let result = client.list_org_invitations(&org, limit).await?;
                    print_org_invitations(&result);
                }
                OrgCommands::Members { org, limit } => {
                    let result = client.list_org_members(&org, limit).await?;
                    print_org_members(&result);
                }
            }
        }
        Commands::Repo { command } => {
            let client = get_client(&config)?;
            match command {
                RepoCommands::Keys { command } => match command {
                    KeyCommands::List { repo } => {
                        let result = client.list_deploy_keys(&repo).await?;
                        print_deploy_keys(&result);
                    }
                    KeyCommands::Add { repo, title, key, write } => {
                        let key_content = if std::path::Path::new(&key).exists() {
                            std::fs::read_to_string(&key)?.trim().to_string()
                        } else {
                            key
                        };
                        let result = client.add_deploy_key(&repo, &title, &key_content, !write).await?;
                        let id = result["id"].as_u64().unwrap_or(0);
                        let access = if write { "read-write" } else { "read-only" };
                        println!("Added deploy key '{}' (id: {}) to {} [{}]", title, id, repo, access);
                    }
                    KeyCommands::Remove { repo, key_id } => {
                        client.remove_deploy_key(&repo, key_id).await?;
                        println!("Removed deploy key {} from {}", key_id, repo);
                    }
                }
                RepoCommands::Hooks { command } => match command {
                    HookCommands::List { repo } => {
                        let result = client.list_repo_hooks(&repo).await?;
                        print_hooks(&result);
                    }
                }
            }
        }
        Commands::App { command } => {
            let client = get_client(&config)?;
            match command {
                AppCommands::List { org } => {
                    let result = client.list_org_installations(&org).await?;
                    print_org_installations(&result);
                }
            }
        }
        Commands::Runner { command } => {
            let client = get_client(&config)?;
            match command {
                RunnerCommands::List { target, org } => {
                    let result = if org {
                        client.list_org_runners(&target).await?
                    } else {
                        client.list_repo_runners(&target).await?
                    };
                    print_runners(&result);
                }
                RunnerCommands::View { target, runner_id, org } => {
                    let result = if org {
                        client.get_org_runner(&target, runner_id).await?
                    } else {
                        client.get_repo_runner(&target, runner_id).await?
                    };
                    print_runner_detail(&result);
                }
                RunnerCommands::Delete { target, runner_id, org } => {
                    if org {
                        client.delete_org_runner(&target, runner_id).await?;
                    } else {
                        client.delete_repo_runner(&target, runner_id).await?;
                    }
                    println!("Deleted runner {runner_id}");
                }
            }
        }
    }

    Ok(())
}
