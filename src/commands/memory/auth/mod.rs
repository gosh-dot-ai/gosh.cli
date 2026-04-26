// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

pub mod membership;
pub mod principal;
pub mod provision_cli;
pub mod status;
pub mod swarm;
pub mod token;

use anyhow::Result;
use clap::Args;
use clap::Subcommand;

use crate::context::CliContext;

#[derive(Args)]
pub struct AuthArgs {
    #[command(subcommand)]
    pub command: AuthCommand,
}

#[derive(Subcommand)]
pub enum AuthCommand {
    /// Show current auth status
    Status(status::StatusArgs),
    /// Manage principals
    Principal(principal::PrincipalArgs),
    /// Manage tokens
    Token(token::TokenArgs),
    /// Manage swarms
    Swarm(swarm::SwarmArgs),
    /// Manage memberships
    Membership(membership::MembershipArgs),
    /// Provision a CLI agent for data operations (store, recall, etc.)
    ProvisionCli(provision_cli::ProvisionCliArgs),
}

pub async fn dispatch(args: AuthArgs, ctx: &CliContext) -> Result<()> {
    match args.command {
        AuthCommand::Status(a) => status::run(a, ctx).await,
        AuthCommand::Principal(a) => principal::dispatch(a, ctx).await,
        AuthCommand::Token(a) => token::dispatch(a, ctx).await,
        AuthCommand::Swarm(a) => swarm::dispatch(a, ctx).await,
        AuthCommand::Membership(a) => membership::dispatch(a, ctx).await,
        AuthCommand::ProvisionCli(a) => provision_cli::run(a, ctx).await,
    }
}
