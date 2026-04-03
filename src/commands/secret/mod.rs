// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use clap::Subcommand;

use crate::context::AppContext;

#[derive(Args)]
pub struct SetArgs {
    /// Secret name (e.g. ANTHROPIC_API_KEY)
    pub name: String,
    /// Secret value
    pub value: String,
}

#[derive(Args)]
pub struct DeleteArgs {
    /// Secret name
    pub name: String,
}

#[derive(Subcommand)]
pub enum SecretCommands {
    /// Store a secret
    Set(SetArgs),

    /// List stored secret names (values are never printed)
    List,

    /// Delete a secret
    Delete(DeleteArgs),
}

pub fn run(cmd: &SecretCommands, ctx: &AppContext) -> anyhow::Result<()> {
    let mut secrets = crate::stores::secret::SecretStore::load(&ctx.state_dir);

    match cmd {
        SecretCommands::Set(args) => {
            secrets.set(&args.name, &args.value);
            secrets.save()?;
            println!("  Stored: {}", args.name);
        }
        SecretCommands::List => {
            let keys = secrets.list_keys();
            if keys.is_empty() {
                println!("  No secrets stored.");
            } else {
                for key in keys {
                    println!("  {key}");
                }
            }
        }
        SecretCommands::Delete(args) => {
            if secrets.delete(&args.name) {
                secrets.save()?;
                println!("  Deleted: {}", args.name);
            } else {
                println!("  Not found: {}", args.name);
            }
        }
    }
    Ok(())
}
