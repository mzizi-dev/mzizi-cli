//! The Anthropic Messages API (replaces `@anthropic-ai/sdk`). The base URL comes from
//! `ANTHROPIC_BASE_URL`, as the SDK's does.

use reqwest::blocking::Client;
use serde_json::Value;

/// A Messages API client.
pub struct Anthropic {
    base: String,
    key: String,
    http: Client,
}

impl Anthropic {
    /// A client for `base` (no trailing slash needed).
    pub fn new(base: &str, key: &str, version: &str) -> Result<Self, String> {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(600))
            .user_agent(format!("mzizi-cli/{version}"))
            .build()
            .map_err(|e| e.to_string())?;
        Ok(Self {
            base: base.trim_end_matches('/').to_string(),
            key: key.into(),
            http,
        })
    }

    /// `POST /v1/messages`, retrying 408/409/429/5xx and connection errors twice.
    pub fn messages_create(&self, body: &Value) -> Result<Value, String> {
        let mut attempt = 0;
        loop {
            let sent = self
                .http
                .post(format!("{}/v1/messages", self.base))
                .header("x-api-key", &self.key)
                .header("anthropic-version", "2023-06-01")
                .json(body)
                .send();
            match sent {
                Ok(r) if r.status().is_success() => return r.json().map_err(|e| e.to_string()),
                Ok(r) => {
                    let status = r.status().as_u16();
                    let text = r.text().unwrap_or_default();
                    let retry = matches!(status, 408 | 409 | 429) || status >= 500;
                    if retry && attempt < 2 {
                        attempt += 1;
                        backoff(attempt);
                        continue;
                    }
                    return Err(format!("{status} {text}"));
                }
                Err(e) => {
                    if attempt < 2 {
                        attempt += 1;
                        backoff(attempt);
                        continue;
                    }
                    return Err(format!("Connection error. ({e})"));
                }
            }
        }
    }
}

fn backoff(attempt: u32) {
    std::thread::sleep(std::time::Duration::from_millis(250 * u64::from(attempt)));
}
