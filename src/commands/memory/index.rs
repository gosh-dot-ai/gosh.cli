// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use clap::Args;
use serde_json::json;

use crate::clients::mcp::McpClient;

#[derive(Args)]
pub struct BuildIndexArgs {
    /// Memory namespace key
    #[arg(long, default_value = "default")]
    pub key: String,
    /// Agent identity used for instance-owned keys
    #[arg(long, default_value = "default")]
    pub agent_id: String,
}

fn build_call_args(args: &BuildIndexArgs) -> serde_json::Value {
    json!({
        "key": args.key,
        "agent_id": args.agent_id,
    })
}

pub async fn run(client: &McpClient, args: &BuildIndexArgs) -> anyhow::Result<()> {
    println!("Building index for key '{}'...", args.key);

    let result = client.call_tool("memory_build_index", build_call_args(args)).await?;

    if let Some(err) = result.get("error") {
        anyhow::bail!("memory_build_index error: {err}");
    }

    let granular = result.get("granular").and_then(|v| v.as_i64()).unwrap_or(0);
    let cons = result.get("consolidated").and_then(|v| v.as_i64()).unwrap_or(0);
    let cross = result.get("cross_session").and_then(|v| v.as_i64()).unwrap_or(0);

    println!("Index built: {granular} granular, {cons} consolidated, {cross} cross-session");
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::build_call_args;
    use super::BuildIndexArgs;

    #[test]
    fn build_index_passes_agent_id_for_instance_acl() {
        let args = BuildIndexArgs { key: "proj".to_string(), agent_id: "planner".to_string() };

        assert_eq!(
            build_call_args(&args),
            json!({
                "key": "proj",
                "agent_id": "planner",
            })
        );
    }
}
