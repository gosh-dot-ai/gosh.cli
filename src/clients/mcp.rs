// Copyright 2026 (c) Mitja Goroshevsky and GOSH Technology Ltd.
// SPDX-License-Identifier: MIT

use anyhow::bail;
use anyhow::Result;
use serde_json::json;
use serde_json::Value;

/// MCP client that communicates with a gosh.memory or gosh-agent server.
#[derive(Debug)]
pub struct McpClient {
    base_url: String,
    http: reqwest::Client,
    transport_token: Option<String>,
    principal_token: Option<String>,
}

impl McpClient {
    /// Create a new MCP client with default HTTP settings.
    pub fn new(
        base_url: &str,
        transport_token: Option<String>,
        principal_token: Option<String>,
        timeout_secs: Option<u64>,
    ) -> Self {
        let mut builder = reqwest::Client::builder();
        if let Some(secs) = timeout_secs {
            builder = builder.timeout(std::time::Duration::from_secs(secs));
        }
        Self::with_client(
            base_url,
            transport_token,
            principal_token,
            builder.build().expect("failed to build HTTP client"),
        )
    }

    /// Create a new MCP client with a custom reqwest::Client.
    pub fn with_client(
        base_url: &str,
        transport_token: Option<String>,
        principal_token: Option<String>,
        http: reqwest::Client,
    ) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            http,
            transport_token,
            principal_token,
        }
    }

    /// Send a JSON-RPC request. Returns (response_body, session_id).
    async fn rpc(
        &self,
        method: &str,
        params: Value,
        session_id: Option<&str>,
    ) -> Result<(Value, Option<String>)> {
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

        if let Some(token) = &self.transport_token {
            req = req.header("x-server-token", token);
        }
        if let Some(token) = &self.principal_token {
            req = req.header("Authorization", format!("Bearer {token}"));
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
            bail!("MCP call failed (HTTP {status}): {text}");
        }

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

    /// Send a notification (fire-and-forget).
    async fn notify(&self, method: &str, session_id: &str) -> Result<()> {
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

        if let Some(token) = &self.transport_token {
            req = req.header("x-server-token", token);
        }
        if let Some(token) = &self.principal_token {
            req = req.header("Authorization", format!("Bearer {token}"));
        }

        let _ = req.json(&body).send().await;
        Ok(())
    }

    /// Initialize an MCP session. Returns session ID.
    async fn initialize(&self) -> Result<String> {
        let params = json!({
            "protocolVersion": "2025-03-26",
            "capabilities": {},
            "clientInfo": {
                "name": "gosh-cli",
                "version": env!("CARGO_PKG_VERSION")
            }
        });

        let (resp, sid) = self.rpc("initialize", params, None).await?;

        let session_id =
            sid.ok_or_else(|| anyhow::anyhow!("server did not return Mcp-Session-Id header"))?;

        if let Some(error) = resp.get("error") {
            bail!("MCP initialize error: {error}");
        }

        self.notify("notifications/initialized", &session_id).await?;
        Ok(session_id)
    }

    /// Call an MCP tool: initialize session, invoke tool, return parsed result.
    pub async fn call_tool(&self, tool_name: &str, args: Value) -> Result<Value> {
        let session_id = self.initialize().await?;

        let params = json!({
            "name": tool_name,
            "arguments": args,
        });

        let (result, _) = self.rpc("tools/call", params, Some(&session_id)).await?;

        if let Some(error) = result.get("error") {
            bail!("MCP error: {error}");
        }

        extract_tool_result(tool_name, &result)
    }
}

/// Extract text content from an MCP tool call response.
fn extract_tool_result(tool_name: &str, result: &Value) -> Result<Value> {
    let mcp_result = result.get("result");

    let is_error =
        mcp_result.and_then(|r| r.get("isError")).and_then(|v| v.as_bool()).unwrap_or(false);

    if let Some(content) = mcp_result.and_then(|r| r.get("content")).and_then(|c| c.as_array()) {
        for item in content {
            if item.get("type").and_then(|t| t.as_str()) == Some("text")
                && let Some(text) = item.get("text").and_then(|t| t.as_str())
            {
                if is_error {
                    bail!("{tool_name}: {text}");
                }
                if let Ok(parsed) = serde_json::from_str::<Value>(text) {
                    return Ok(parsed);
                }
                return Ok(Value::String(text.to_string()));
            }
        }
    }

    Ok(result.get("result").cloned().unwrap_or(Value::Null))
}

/// Parse SSE response to extract JSON-RPC message.
fn parse_sse_response(body: &str) -> Result<Value> {
    for line in body.lines() {
        let line = line.trim();
        if let Some(data) = line.strip_prefix("data: ")
            && let Ok(parsed) = serde_json::from_str::<Value>(data)
            && parsed.get("id").is_some()
        {
            return Ok(parsed);
        }
    }
    bail!("no JSON-RPC response found in SSE stream")
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    // ── parse_sse_response ─────────────────────────────────────────

    #[test]
    fn parse_sse_valid_response() {
        let body = "event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n\n";
        let result = parse_sse_response(body).unwrap();
        assert_eq!(result["id"], 1);
    }

    #[test]
    fn parse_sse_skips_notifications() {
        // Notifications have no "id" field
        let body = "data: {\"jsonrpc\":\"2.0\",\"method\":\"notify\"}\n\
                    data: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{\"ok\":true}}\n\n";
        let result = parse_sse_response(body).unwrap();
        assert_eq!(result["result"]["ok"], true);
    }

    #[test]
    fn parse_sse_empty_body_fails() {
        let result = parse_sse_response("");
        assert!(result.is_err());
    }

    #[test]
    fn parse_sse_no_id_fails() {
        let body = "data: {\"jsonrpc\":\"2.0\",\"method\":\"notify\"}\n\n";
        assert!(parse_sse_response(body).is_err());
    }

    // ── extract_tool_result ────────────────────────────────────────

    #[test]
    fn extract_json_text_content() {
        let result = json!({
            "result": {
                "content": [{"type": "text", "text": "{\"status\":\"ok\"}"}],
                "isError": false
            }
        });
        let extracted = extract_tool_result("test_tool", &result).unwrap();
        assert_eq!(extracted["status"], "ok");
    }

    #[test]
    fn extract_plain_text_content() {
        let result = json!({
            "result": {
                "content": [{"type": "text", "text": "hello world"}],
                "isError": false
            }
        });
        let extracted = extract_tool_result("test_tool", &result).unwrap();
        assert_eq!(extracted, Value::String("hello world".into()));
    }

    #[test]
    fn extract_error_content_fails() {
        let result = json!({
            "result": {
                "content": [{"type": "text", "text": "something went wrong"}],
                "isError": true
            }
        });
        let err = extract_tool_result("my_tool", &result).unwrap_err();
        assert!(err.to_string().contains("my_tool"));
        assert!(err.to_string().contains("something went wrong"));
    }

    #[test]
    fn extract_empty_content_returns_null() {
        let result = json!({ "result": { "content": [], "isError": false } });
        let extracted = extract_tool_result("test_tool", &result).unwrap();
        assert_eq!(extracted, json!({ "content": [], "isError": false }));
    }

    #[test]
    fn extract_no_result_returns_null() {
        let result = json!({});
        let extracted = extract_tool_result("test_tool", &result).unwrap();
        assert_eq!(extracted, Value::Null);
    }

    #[test]
    fn extract_skips_non_text_items() {
        let result = json!({
            "result": {
                "content": [
                    {"type": "image", "data": "..."},
                    {"type": "text", "text": "{\"found\":true}"}
                ],
                "isError": false
            }
        });
        let extracted = extract_tool_result("test_tool", &result).unwrap();
        assert_eq!(extracted["found"], true);
    }
}
