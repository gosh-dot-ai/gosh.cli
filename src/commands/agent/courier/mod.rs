// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

mod subscribe;
mod unsubscribe;

use clap::Subcommand;

use crate::context::AppContext;

#[derive(Subcommand)]
pub enum CourierCommands {
    /// Subscribe agent to courier (listen for new tasks)
    Subscribe(subscribe::SubscribeArgs),
    /// Unsubscribe agent from courier
    Unsubscribe(unsubscribe::UnsubscribeArgs),
}

pub async fn run(agent_name: &str, cmd: &CourierCommands, ctx: &AppContext) -> anyhow::Result<()> {
    match cmd {
        CourierCommands::Subscribe(args) => subscribe::run(agent_name, args, ctx).await,
        CourierCommands::Unsubscribe(args) => unsubscribe::run(agent_name, args, ctx).await,
    }
}
