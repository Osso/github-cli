use anyhow::Result;
use clap::Subcommand;

use crate::client::Client;

#[derive(Subcommand)]
pub enum TeamCommands {
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

pub async fn handle(client: &Client, command: TeamCommands) -> Result<()> {
    match command {
        TeamCommands::List { org, limit } => handle_list(client, &org, limit).await,
        TeamCommands::Create {
            org,
            name,
            description,
            privacy,
            parent_team_id,
        } => handle_create_command(client, org, name, description, privacy, parent_team_id).await,
        TeamCommands::View { org, team } => handle_view(client, &org, &team).await,
        TeamCommands::Members { org, team, limit } => {
            handle_members(client, &org, &team, limit).await
        }
        TeamCommands::AddMember {
            org,
            team,
            username,
            role,
        } => handle_add_member_command(client, org, team, username, role).await,
        TeamCommands::RemoveMember {
            org,
            team,
            username,
        } => handle_remove_member(client, &org, &team, &username).await,
        TeamCommands::AddRepo {
            org,
            team,
            repo,
            permission,
        } => handle_add_repo_command(client, org, team, repo, permission).await,
        TeamCommands::Repos { org, team, limit } => handle_repos(client, &org, &team, limit).await,
    }
}

async fn handle_create_command(
    client: &Client,
    org: String,
    name: String,
    description: Option<String>,
    privacy: String,
    parent_team_id: Option<u64>,
) -> Result<()> {
    handle_create(
        client,
        &org,
        &name,
        description.as_deref(),
        &privacy,
        parent_team_id,
    )
    .await
}

async fn handle_add_member_command(
    client: &Client,
    org: String,
    team: String,
    username: String,
    role: String,
) -> Result<()> {
    handle_add_member(client, &org, &team, &username, &role).await
}

async fn handle_add_repo_command(
    client: &Client,
    org: String,
    team: String,
    repo: String,
    permission: String,
) -> Result<()> {
    handle_add_repo(client, &org, &team, &repo, &permission).await
}

async fn handle_list(client: &Client, org: &str, limit: u32) -> Result<()> {
    let result = client
        .get(&format!("/orgs/{org}/teams?per_page={limit}"))
        .await?;
    print_teams(&result);
    Ok(())
}

async fn handle_create(
    client: &Client,
    org: &str,
    name: &str,
    description: Option<&str>,
    privacy: &str,
    parent_team_id: Option<u64>,
) -> Result<()> {
    let result = create_team(client, org, name, description, privacy, parent_team_id).await?;
    let slug = result["slug"].as_str().unwrap_or("");
    let id = result["id"].as_u64().unwrap_or(0);
    println!("Created team '{name}' (slug: {slug}, id: {id})");
    Ok(())
}

async fn handle_view(client: &Client, org: &str, team: &str) -> Result<()> {
    let result = client.get(&format!("/orgs/{org}/teams/{team}")).await?;
    print_team_detail(&result);
    Ok(())
}

async fn handle_members(client: &Client, org: &str, team: &str, limit: u32) -> Result<()> {
    let result = client
        .get(&format!(
            "/orgs/{org}/teams/{team}/members?per_page={limit}"
        ))
        .await?;
    print_team_members(&result);
    Ok(())
}

async fn handle_add_member(
    client: &Client,
    org: &str,
    team: &str,
    username: &str,
    role: &str,
) -> Result<()> {
    let path = format!("/orgs/{org}/teams/{team}/memberships/{username}");
    let result = client
        .put(&path, &serde_json::json!({ "role": role }))
        .await?;
    let state = result["state"].as_str().unwrap_or("added");
    println!("User '{username}' {state} to team '{team}' as {role}");
    Ok(())
}

async fn handle_remove_member(
    client: &Client,
    org: &str,
    team: &str,
    username: &str,
) -> Result<()> {
    client
        .delete(&format!("/orgs/{org}/teams/{team}/memberships/{username}"))
        .await?;
    println!("Removed '{username}' from team '{team}'");
    Ok(())
}

async fn handle_add_repo(
    client: &Client,
    org: &str,
    team: &str,
    repo: &str,
    permission: &str,
) -> Result<()> {
    let path = format!("/orgs/{org}/teams/{team}/repos/{repo}");
    client
        .put(&path, &serde_json::json!({ "permission": permission }))
        .await?;
    println!("Added repo '{repo}' to team '{team}' with {permission} permission");
    Ok(())
}

async fn handle_repos(client: &Client, org: &str, team: &str, limit: u32) -> Result<()> {
    let result = client
        .get(&format!("/orgs/{org}/teams/{team}/repos?per_page={limit}"))
        .await?;
    print_team_repos(&result);
    Ok(())
}

async fn create_team(
    client: &Client,
    org: &str,
    name: &str,
    description: Option<&str>,
    privacy: &str,
    parent_team_id: Option<u64>,
) -> Result<serde_json::Value> {
    let path = format!("/orgs/{org}/teams");
    let mut body = serde_json::json!({ "name": name, "privacy": privacy });
    if let Some(desc) = description {
        body["description"] = serde_json::json!(desc);
    }
    if let Some(parent_id) = parent_team_id {
        body["parent_team_id"] = serde_json::json!(parent_id);
    }
    client.post(&path, &body).await
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
