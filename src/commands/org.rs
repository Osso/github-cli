use anyhow::Result;
use clap::Subcommand;

use crate::client::Client;

#[derive(Subcommand)]
pub enum OrgCommands {
    /// Create a new GitHub organization
    Create {
        /// Organization login name
        name: String,
        /// Billing email address (required)
        #[arg(long)]
        billing_email: String,
    },
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

pub async fn handle(client: &Client, command: OrgCommands) -> Result<()> {
    match command {
        OrgCommands::Create {
            name,
            billing_email,
        } => create_org(client, &name, &billing_email).await,
        OrgCommands::Invite {
            org,
            email,
            role,
            teams,
        } => invite_member(client, &org, &email, &role, teams).await,
        OrgCommands::Invitations { org, limit } => list_invitations(client, &org, limit).await,
        OrgCommands::Members { org, limit } => list_members(client, &org, limit).await,
    }
}

async fn create_org(client: &Client, name: &str, billing_email: &str) -> Result<()> {
    let body = serde_json::json!({
        "login": name,
        "name": name,
        "billing_email": billing_email,
    });
    let result = client.post("/user/orgs", &body).await?;
    let login = result["login"].as_str().unwrap_or(name);
    let id = result["id"].as_u64().unwrap_or(0);
    println!("Organization created: {login} (id: {id})");
    Ok(())
}

async fn invite_member(
    client: &Client,
    org: &str,
    email: &str,
    role: &str,
    teams: Option<String>,
) -> Result<()> {
    let team_ids = teams.map(|t| {
        t.split(',')
            .filter_map(|s| s.trim().parse::<u64>().ok())
            .collect::<Vec<_>>()
    });
    let mut body = serde_json::json!({ "email": email, "role": role });
    if let Some(ids) = team_ids {
        body["team_ids"] = serde_json::json!(ids);
    }
    let result = client
        .post(&format!("/orgs/{org}/invitations"), &body)
        .await?;
    let id = result["id"].as_u64().unwrap_or(0);
    println!("Invitation sent to {email} (id: {id})");
    Ok(())
}

async fn list_invitations(client: &Client, org: &str, limit: u32) -> Result<()> {
    let result = client
        .get(&format!("/orgs/{org}/invitations?per_page={limit}"))
        .await?;
    print_org_invitations(&result);
    Ok(())
}

async fn list_members(client: &Client, org: &str, limit: u32) -> Result<()> {
    let result = client
        .get(&format!("/orgs/{org}/members?per_page={limit}"))
        .await?;
    print_org_members(&result);
    Ok(())
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
            let created = inv["created_at"]
                .as_str()
                .unwrap_or("")
                .split('T')
                .next()
                .unwrap_or("");
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
