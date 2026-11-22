//! `FundiClient` (port of client.ts): the Anthropic client, the cached system prompt, and
//! the three verbs `explore`, `plan` and `chat`.

use std::path::{Path, PathBuf};

use serde_json::{Value, json};

use super::anthropic::Anthropic;
use super::auth::{AuthError, Identity, assert_token, get_identity, resolve_token};
use super::explore::{ProjectSnapshot, explore_project};
use super::plan::{Plan, PlanContext, build_plan};
use super::prompts::build_system_prompt;
use super::tools::ToolCtx;
use crate::Env;

/// Default model.
pub const DEFAULT_MODEL: &str = "claude-sonnet-5";
/// Default MCP endpoint.
pub const DEFAULT_MCP_URL: &str = "https://mcp.mzizi.dev/mcp";
/// Default Anthropic API base.
pub const DEFAULT_ANTHROPIC_BASE: &str = "https://api.anthropic.com";
/// Thinking tokens count against max_tokens, so 16000 leaves room for both.
pub const DEFAULT_MAX_TOKENS: u64 = 16000;

/// What `createFundi` took.
pub struct FundiOptions {
    /// Project root.
    pub project_root: PathBuf,
    /// Anthropic API key.
    pub anthropic_api_key: String,
    /// Anthropic base URL (from `ANTHROPIC_BASE_URL`).
    pub anthropic_base: String,
    /// MCP endpoint.
    pub mcp_url: String,
    /// Model.
    pub model: String,
    /// Max tokens.
    pub max_tokens: u64,
    /// Registry HTTP fallback base for component fetches.
    pub registry_http_base: String,
    /// Fail when no valid token is resolved.
    pub require_auth: bool,
}

impl FundiOptions {
    /// Options with the defaults.
    pub fn new(project_root: &Path, key: &str) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
            anthropic_api_key: key.into(),
            anthropic_base: DEFAULT_ANTHROPIC_BASE.into(),
            mcp_url: DEFAULT_MCP_URL.into(),
            model: DEFAULT_MODEL.into(),
            max_tokens: DEFAULT_MAX_TOKENS,
            registry_http_base: super::tools::REGISTRY_HTTP_BASE.into(),
            require_auth: false,
        }
    }
}

/// The agent.
pub struct FundiClient {
    opts: FundiOptions,
    anthropic: Anthropic,
    /// Decoded identity when a valid token was resolved.
    pub identity: Option<Identity>,
    access_token: Option<String>,
    system_prompt: Option<String>,
}

impl FundiClient {
    /// Construct. A bad token is ignored with a stderr warning unless `require_auth`.
    pub fn create(
        opts: FundiOptions,
        env: &Env,
        err: &mut dyn std::io::Write,
    ) -> Result<Self, String> {
        if opts.anthropic_api_key.is_empty() {
            return Err(
                "mzizi-sdk: anthropicApiKey is required (or pass a pre-built `anthropic` client)"
                    .into(),
            );
        }
        let anthropic = Anthropic::new(
            &opts.anthropic_base,
            &opts.anthropic_api_key,
            crate::VERSION,
        )?;
        let mut token = if opts.require_auth {
            Some(assert_token(None, env, "createFundi", err).map_err(|e| e.0)?)
        } else {
            resolve_token(None, env, err)
        };
        let mut identity = None;
        if let Some(t) = &token {
            match get_identity(t) {
                Ok(i) => identity = Some(i),
                Err(AuthError(reason)) => {
                    if opts.require_auth {
                        return Err(reason);
                    }
                    let _ = writeln!(err, "mzizi-sdk: ignoring invalid WorkOS token ({reason})");
                    token = None;
                }
            }
        }
        Ok(Self {
            opts,
            anthropic,
            identity,
            access_token: token,
            system_prompt: None,
        })
    }

    /// Read project markers.
    pub fn explore(&self) -> ProjectSnapshot {
        explore_project(&self.opts.project_root, ".")
    }

    fn system_prompt(&mut self) -> String {
        self.system_prompt
            .get_or_insert_with(build_system_prompt)
            .clone()
    }

    /// Plan a goal.
    pub fn plan(&mut self, goal: &str, snapshot: &ProjectSnapshot) -> Result<Plan, String> {
        let system_prompt = self.system_prompt();
        let mut tools = ToolCtx::new(
            &self.opts.project_root,
            &self.opts.mcp_url,
            self.access_token.as_deref(),
            true,
        );
        tools.registry_http_base = self.opts.registry_http_base.clone();
        let mut ctx = PlanContext {
            anthropic: &self.anthropic,
            model: self.opts.model.clone(),
            max_tokens: self.opts.max_tokens,
            tools: &mut tools,
            system_prompt,
        };
        build_plan(goal, snapshot, &mut ctx)
    }

    /// Single-turn chat.
    pub fn chat(&mut self, message: &str) -> Result<String, String> {
        let system_prompt = self.system_prompt();
        let response = self.anthropic.messages_create(&json!({
            "model": self.opts.model,
            "max_tokens": self.opts.max_tokens,
            "system": [{"type": "text", "text": system_prompt, "cache_control": {"type": "ephemeral"}}],
            "messages": [{"role": "user", "content": message}],
        }))?;
        Ok(response
            .get("content")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                    .filter_map(|b| b.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default()
            .trim()
            .to_string())
    }
}
