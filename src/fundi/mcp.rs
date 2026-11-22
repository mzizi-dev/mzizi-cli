//! A minimal MCP client over Streamable HTTP (JSON-RPC 2.0), enough for `tools/call`.
//! Replaces `@modelcontextprotocol/sdk` in the TypeScript CLI.

use reqwest::blocking::{Client, Response};
use serde_json::{Value, json};

use super::auth::bearer_header;

const PROTOCOL_VERSION: &str = "2025-06-18";

/// A connected MCP session.
pub struct McpClient {
    url: String,
    token: Option<String>,
    http: Client,
    session: Option<String>,
    next_id: u64,
}

impl McpClient {
    /// `initialize` + `notifications/initialized`.
    pub fn connect(url: &str, token: Option<&str>, version: &str) -> Result<Self, String> {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(60))
            .user_agent(format!("mzizi-cli/{version}"))
            .build()
            .map_err(|e| e.to_string())?;
        let mut c = Self {
            url: url.to_string(),
            token: token.map(str::to_string),
            http,
            session: None,
            next_id: 0,
        };
        c.request(
            "initialize",
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": {},
                "clientInfo": {"name": "mzizi-cli", "version": version},
            }),
        )?;
        c.notify("notifications/initialized")?;
        Ok(c)
    }

    fn post(&mut self, body: &Value) -> Result<Response, String> {
        let mut req = self
            .http
            .post(&self.url)
            .header("content-type", "application/json")
            .header("accept", "application/json, text/event-stream")
            .json(body);
        for (k, v) in bearer_header(self.token.as_deref()) {
            req = req.header(k, v);
        }
        if let Some(s) = &self.session {
            req = req
                .header("mcp-session-id", s)
                .header("mcp-protocol-version", PROTOCOL_VERSION);
        }
        let resp = req.send().map_err(|e| e.to_string())?;
        if let Some(s) = resp
            .headers()
            .get("mcp-session-id")
            .and_then(|v| v.to_str().ok())
        {
            self.session = Some(s.to_string());
        }
        if !resp.status().is_success() {
            return Err(format!("HTTP {}", resp.status().as_u16()));
        }
        Ok(resp)
    }

    fn notify(&mut self, method: &str) -> Result<(), String> {
        self.post(&json!({"jsonrpc": "2.0", "method": method}))
            .map(|_| ())
    }

    fn request(&mut self, method: &str, params: Value) -> Result<Value, String> {
        self.next_id += 1;
        let id = self.next_id;
        let resp =
            self.post(&json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params}))?;
        let is_sse = resp
            .headers()
            .get("content-type")
            .and_then(|v| v.to_str().ok())
            .is_some_and(|v| v.starts_with("text/event-stream"));
        let text = resp.text().map_err(|e| e.to_string())?;
        let msg = if is_sse {
            find_sse_message(&text, id)
        } else {
            serde_json::from_str(&text).ok()
        }
        .ok_or_else(|| "no JSON-RPC response".to_string())?;
        if let Some(err) = msg.get("error") {
            return Err(err
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("MCP error")
                .to_string());
        }
        Ok(msg.get("result").cloned().unwrap_or(Value::Null))
    }

    /// `tools/call`; returns the result object (`content`, `isError`).
    pub fn call_tool(&mut self, name: &str, arguments: Value) -> Result<Value, String> {
        self.request("tools/call", json!({"name": name, "arguments": arguments}))
    }
}

fn find_sse_message(body: &str, id: u64) -> Option<Value> {
    let mut data = String::new();
    let mut found = None;
    let mut flush = |data: &mut String| {
        if let Ok(v) = serde_json::from_str::<Value>(data)
            && v.get("id").and_then(Value::as_u64) == Some(id)
        {
            found = Some(v);
        }
        data.clear();
    };
    for line in body.lines() {
        if let Some(rest) = line.strip_prefix("data:") {
            if !data.is_empty() {
                data.push('\n');
            }
            data.push_str(rest.strip_prefix(' ').unwrap_or(rest));
        } else if line.is_empty() {
            flush(&mut data);
        }
    }
    flush(&mut data);
    found
}
