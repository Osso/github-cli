use anyhow::{Context, Result, anyhow};
use clap::{Parser, Subcommand};

mod client;
mod commands;
mod config;

use client::Client;
use config::Config;

use commands::app::AppCommands;
use commands::code::CodeCommands;
use commands::issue::IssueCommands;
use commands::org::OrgCommands;
use commands::pr::PrCommands;
use commands::repo::RepoCommands;
use commands::run::RunCommands;
use commands::runner::RunnerCommands;
use commands::secret::SecretCommands;
use commands::team::TeamCommands;
use commands::webhook::WebhookCommands;

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
    /// Manage workflow runs
    Run {
        #[command(subcommand)]
        command: RunCommands,
    },
    /// Search code across GitHub
    Code {
        #[command(subcommand)]
        command: CodeCommands,
    },
    /// Manage Actions secrets
    Secret {
        #[command(subcommand)]
        command: SecretCommands,
    },
    /// Manage repository webhooks
    Webhook {
        #[command(subcommand)]
        command: WebhookCommands,
    },
    /// Configure token
    Config {
        /// GitHub personal access token
        #[arg(long)]
        token: Option<String>,
    },
}

fn get_client(config: &Config) -> Result<Client> {
    let token = config.get_token().ok_or_else(|| {
        anyhow!("No token. Set GITHUB_TOKEN or run: github config --token <token>")
    })?;
    Client::new(&token)
}

fn handle_config(config: Config, token: Option<String>) -> Result<()> {
    let mut config = config;
    if let Some(t) = token {
        config.token = Some(t);
        config.save()?;
        println!("Token saved");
        return Ok(());
    }
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
    Ok(())
}

async fn handle_react(client: &Client, repo: &str, number: u64, reaction: &str) -> Result<()> {
    let path = format!("/repos/{repo}/issues/{number}/reactions");
    let result = client
        .post(&path, &serde_json::json!({ "content": reaction }))
        .await?;
    let content = result["content"].as_str().unwrap_or(reaction);
    println!("Added {content} reaction to {repo}#{number}");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let config = Config::load().context("Failed to load config")?;

    match cli.command {
        Commands::Config { token } => handle_config(config, token)?,
        Commands::React {
            repo,
            number,
            reaction,
        } => handle_react(&get_client(&config)?, &repo, number, &reaction).await?,
        Commands::Issue { command } => {
            commands::issue::handle(&get_client(&config)?, command).await?;
        }
        Commands::Pr { command } => {
            commands::pr::handle(&get_client(&config)?, command).await?;
        }
        Commands::Team { command } => {
            commands::team::handle(&get_client(&config)?, command).await?;
        }
        Commands::Org { command } => {
            commands::org::handle(&get_client(&config)?, command).await?;
        }
        Commands::Repo { command } => {
            commands::repo::handle(&get_client(&config)?, command).await?;
        }
        Commands::App { command } => {
            commands::app::handle(&get_client(&config)?, command).await?;
        }
        Commands::Runner { command } => {
            commands::runner::handle(&get_client(&config)?, command).await?;
        }
        Commands::Run { command } => {
            commands::run::handle(&get_client(&config)?, command).await?;
        }
        Commands::Code { command } => {
            commands::code::handle(&get_client(&config)?, command).await?;
        }
        Commands::Secret { command } => {
            commands::secret::handle(&get_client(&config)?, command).await?;
        }
        Commands::Webhook { command } => {
            commands::webhook::handle(&get_client(&config)?, command).await?;
        }
    }

    Ok(())
}
