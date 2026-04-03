// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// License: MIT

use serde_json::json;
use serde_json::Value;

pub struct McpClient {
    base_url: String,
    http: reqwest::Client,
    token: Option<String>,
}

impl McpClient {
    pub fn new(base_url: &str, token: Option<String>, timeout_secs: Option<u64>) -> Self {
        let mut builder = reqwest::Client::builder();
        if let Some(secs) = timeout_secs {
            builder = builder.timeout(std::time::Duration::from_secs(secs));
        }
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http: builder.build().expect("failed to build HTTP client"),
            token,
        }
    }

    /// Send a JSON-RPC request to the MCP endpoint.
    /// Returns (response_body, session_id from Mcp-Session-Id header).
    async fn rpc(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> anyhow::Result<(Value, Option<String>)> {
        let url = format!("{}/mcp", self.base_url);

        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": method,
            "params": params,
        });

        let mut req = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json, text/event-stream");

        if let Some(token) = &self.token {
            req = req.header("x-server-token", token);
        }
        if let Some(sid) = session_id {
            req = req.header("Mcp-Session-Id", sid);
        }

        let resp = req.json(&body).send().await?;

        let sid = resp
            .headers()
            .get("Mcp-Session-Id")
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string());

        if !resp.status().is_success() {
            let status = resp.status();
            let text = resp.text().await.unwrap_or_default();
            anyhow::bail!("MCP call failed (HTTP {status}): {text}");
        }

        // Check content type — SSE or JSON
        let content_type = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_string();

        if content_type.contains("text/event-stream") {
            let text = resp.text().await?;
            let result = parse_sse_response(&text)?;
            Ok((result, sid))
        } else {
            let result: Value = resp.json().await?;
            Ok((result, sid))
        }
    }

    /// Send a notification (no id, no response expected).
    async fn notify(&self, method: &str, session_id: &str) -> anyhow::Result<()> {
        let url = format!("{}/mcp", self.base_url);
        let body = json!({
            "jsonrpc": "2.0",
            "method": method,
        });

        let mut req = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .header("Mcp-Session-Id", session_id);

        if let Some(token) = &self.token {
            req = req.header("x-server-token", token);
        }

        let _ = req.json(&body).send().await;
        Ok(())
    }

    /// Initialize an MCP session. Returns session ID.
    async fn initialize(&self) -> anyhow::Result<String> {
        let params = json!({
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {
                "name": "gosh-cli",
                "version": "0.1.0"
            }
        });

        let (resp, sid) = self.rpc("initialize", params, None).await?;

        let session_id =
            sid.ok_or_else(|| anyhow::anyhow!("server did not return Mcp-Session-Id header"))?;

        if let Some(error) = resp.get("error") {
            anyhow::bail!("MCP initialize error: {error}");
        }

        self.notify("notifications/initialized", &session_id).await?;

        Ok(session_id)
    }

    /// Call an MCP tool: initialize session, call tool, return parsed result.
    pub async fn call_tool(&self, tool_name: &str, args: Value) -> anyhow::Result<Value> {
        let session_id = self.initialize().await?;

        let params = json!({
            "name": tool_name,
            "arguments": args,
        });

        let (result, _) = self.rpc("tools/call", params, Some(&session_id)).await?;

        if let Some(error) = result.get("error") {
            anyhow::bail!("MCP error: {error}");
        }

        // Extract text content from MCP response
        let mcp_result = result.get("result");

        // Check MCP-level isError flag
        let is_error =
            mcp_result.and_then(|r| r.get("isError")).and_then(|v| v.as_bool()).unwrap_or(false);

        if let Some(content) = mcp_result.and_then(|r| r.get("content")).and_then(|c| c.as_array())
        {
            for item in content {
                if item.get("type").and_then(|t| t.as_str()) == Some("text") {
                    if let Some(text) = item.get("text").and_then(|t| t.as_str()) {
                        if is_error {
                            anyhow::bail!("{tool_name}: {text}");
                        }
                        if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                            return Ok(parsed);
                        }
                        return Ok(Value::String(text.to_string()));
                    }
                }
            }
        }

        Ok(result.get("result").cloned().unwrap_or(Value::Null))
    }
}

/// Parse an SSE response body to extract the JSON-RPC message.
/// SSE format: "event: message\ndata: {json}\n\n"
fn parse_sse_response(body: &str) -> anyhow::Result<Value> {
    for line in body.lines() {
        let line = line.trim();
        if let Some(data) = line.strip_prefix("data: ") {
            if let Ok(parsed) = serde_json::from_str::<Value>(data) {
                if parsed.get("id").is_some() {
                    return Ok(parsed);
                }
            }
        }
    }
    anyhow::bail!("no JSON-RPC response found in SSE stream")
}
