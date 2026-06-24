#![cfg_attr(coverage_nightly, coverage(off))]

use anyhow::{Context, Result, bail};
use base64::Engine;
use clap::Subcommand;
use crypto_box::{PublicKey, aead::OsRng};

use crate::client::Client;

#[derive(Subcommand)]
pub enum SecretCommands {
    /// List repository secrets
    List {
        /// Repository (owner/repo)
        repo: String,
    },
    /// Create or update a repository secret
    Set {
        /// Repository (owner/repo)
        repo: String,
        /// Secret name
        name: String,
        /// Secret value
        value: String,
    },
    /// Delete a repository secret
    Delete {
        /// Repository (owner/repo)
        repo: String,
        /// Secret name
        name: String,
    },
}

pub async fn handle(client: &Client, command: SecretCommands) -> Result<()> {
    match command {
        SecretCommands::List { repo } => {
            let result = client
                .get(&format!("/repos/{repo}/actions/secrets"))
                .await?;
            print_secrets(&result);
        }
        SecretCommands::Set { repo, name, value } => {
            let encrypted = encrypt_secret(client, &repo, &value).await?;
            client
                .put(
                    &format!("/repos/{repo}/actions/secrets/{name}"),
                    &serde_json::json!({
                        "encrypted_value": encrypted.ciphertext_b64,
                        "key_id": encrypted.key_id,
                    }),
                )
                .await?;
            println!("Set secret '{name}' in {repo}");
        }
        SecretCommands::Delete { repo, name } => {
            client
                .delete(&format!("/repos/{repo}/actions/secrets/{name}"))
                .await?;
            println!("Deleted secret '{name}' from {repo}");
        }
    }
    Ok(())
}

struct EncryptedSecret {
    ciphertext_b64: String,
    key_id: String,
}

async fn encrypt_secret(client: &Client, repo: &str, value: &str) -> Result<EncryptedSecret> {
    let key_resp = client
        .get(&format!("/repos/{repo}/actions/secrets/public-key"))
        .await
        .context("Failed to fetch repo public key")?;

    let key_b64 = key_resp["key"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'key' in public key response"))?;
    let key_id = key_resp["key_id"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("Missing 'key_id' in public key response"))?
        .to_string();

    let key_bytes = base64::engine::general_purpose::STANDARD
        .decode(key_b64)
        .context("Failed to decode public key from base64")?;

    if key_bytes.len() != 32 {
        bail!("Expected 32-byte public key, got {}", key_bytes.len());
    }

    let public_key = PublicKey::from_slice(&key_bytes).unwrap();
    let ciphertext = public_key
        .seal(&mut OsRng, value.as_bytes())
        .map_err(|e| anyhow::anyhow!("Encryption failed: {e}"))?;
    let ciphertext_b64 = base64::engine::general_purpose::STANDARD.encode(ciphertext);

    Ok(EncryptedSecret {
        ciphertext_b64,
        key_id,
    })
}

fn print_secrets(value: &serde_json::Value) {
    let secrets = value["secrets"].as_array();
    let total = value["total_count"].as_u64().unwrap_or(0);

    let secrets = match secrets {
        None => {
            println!("No secrets found");
            return;
        }
        Some(s) if s.is_empty() => {
            println!("No secrets found");
            return;
        }
        Some(s) => s,
    };

    println!("Total: {total}");
    println!("{:<50} {:<12} {:<12}", "Name", "Created", "Updated");
    println!("{}", "-".repeat(76));
    for secret in secrets {
        let name = secret["name"].as_str().unwrap_or("");
        let created = date_part(secret["created_at"].as_str().unwrap_or(""));
        let updated = date_part(secret["updated_at"].as_str().unwrap_or(""));
        println!("{name:<50} {created:<12} {updated:<12}");
    }
}

fn date_part(ts: &str) -> &str {
    ts.split('T').next().unwrap_or(ts)
}
