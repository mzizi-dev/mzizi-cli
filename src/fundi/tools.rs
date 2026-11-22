//! The tool registry and dispatcher for the planner loop (port of tools.ts).

use std::path::Path;

use serde_json::{Value, json};

use super::explore::{node_message, resolve};
use super::mcp::McpClient;
use super::skills::load_mzizi_skills;

/// The registry's HTTP base. `GET {base}/{name}` returns the component document.
pub const REGISTRY_HTTP_BASE: &str = "https://api.mzizi.dev/v1/ui";
/// The MCP tool `fetch_mzizi_component` calls first.
pub const MCP_EXECUTE_TOOL: &str = "mzizi_execute";
/// The operation it runs.
pub const MCP_GET_COMPONENT_OPERATION: &str = "get_component";

/// A tool result: the string sent back as `tool_result` content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolResult {
    /// The content.
    pub content: String,
    /// Whether it is an error.
    pub is_error: bool,
}

fn ok(content: String) -> ToolResult {
    ToolResult {
        content,
        is_error: false,
    }
}
fn err(content: String) -> ToolResult {
    ToolResult {
        content,
        is_error: true,
    }
}

/// JS `String.prototype.length` (UTF-16 code units).
pub fn js_len(s: &str) -> usize {
    s.encode_utf16().count()
}

/// Everything the dispatcher needs.
pub struct ToolCtx {
    /// Project root (absolute).
    pub project_root: std::path::PathBuf,
    /// The MCP endpoint.
    pub mcp_url: String,
    /// Console token forwarded to MCP.
    pub mcp_auth_token: Option<String>,
    /// Reject side-effectful tools.
    pub read_only: bool,
    /// Registry HTTP fallback base.
    pub registry_http_base: String,
    mcp: Option<McpClient>,
}

impl ToolCtx {
    /// A context with the production registry fallback.
    pub fn new(project_root: &Path, mcp_url: &str, token: Option<&str>, read_only: bool) -> Self {
        Self {
            project_root: project_root.to_path_buf(),
            mcp_url: mcp_url.into(),
            mcp_auth_token: token.map(str::to_string),
            read_only,
            registry_http_base: REGISTRY_HTTP_BASE.into(),
            mcp: None,
        }
    }
}

/// Anthropic tool definitions, in the TS order.
pub fn fundi_tool_definitions() -> Value {
    json!([
        {"name": "read_file", "description": "Read a UTF-8 file from the project. Path is relative to the project root.",
         "input_schema": {"type": "object", "properties": {"path": {"type": "string", "description": "Path relative to project root"}}, "required": ["path"]}},
        {"name": "write_file", "description": "Write a UTF-8 file. Used during the apply phase only; planner should not call this.",
         "input_schema": {"type": "object", "properties": {"path": {"type": "string"}, "contents": {"type": "string"}}, "required": ["path", "contents"]}},
        {"name": "list_dir", "description": "List entries in a directory relative to the project root. Returns one entry per line, suffixed with '/' for directories.",
         "input_schema": {"type": "object", "properties": {"path": {"type": "string"}}, "required": ["path"]}},
        {"name": "run_shell", "description": "Run a shell command. Argv-style (no shell interpolation). Used during the apply phase only.",
         "input_schema": {"type": "object", "properties": {"argv": {"type": "array", "items": {"type": "string"}}, "cwd": {"type": "string"}}, "required": ["argv"]}},
        {"name": "fetch_mzizi_skill", "description": "Fetch the body of a Mzizi skill by name. Returns the markdown source.",
         "input_schema": {"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]}},
        {"name": "fetch_mzizi_component", "description": "Fetch a Mzizi component's full JSON document (metadata, sources, files, source code) by name from the live Mzizi MCP registry.",
         "input_schema": {"type": "object", "properties": {"name": {"type": "string"}}, "required": ["name"]}},
    ])
}

/// Resolve `rel` inside `root`, or `None` when it escapes.
pub fn resolve_within(root: &Path, rel: &str) -> Option<std::path::PathBuf> {
    let resolved = resolve(root, rel);
    let norm = resolve(root, ".");
    resolved.starts_with(&norm).then_some(resolved)
}

/// JS `encodeURIComponent`.
pub fn encode_uri_component(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// `{base}/{name}`.
pub fn registry_component_url(base: &str, name: &str) -> String {
    format!("{base}/{}", encode_uri_component(name))
}

/// The install argv for missing primitives: full registry URLs, never bare names.
pub fn registry_install_argv(components: &[String]) -> Vec<String> {
    ["npx", "shadcn@latest", "add"]
        .iter()
        .map(|s| (*s).to_string())
        .chain(
            components
                .iter()
                .map(|c| registry_component_url(REGISTRY_HTTP_BASE, c)),
        )
        .collect()
}

fn str_field<'a>(input: &'a Value, key: &str) -> Option<&'a str> {
    input.get(key).and_then(Value::as_str)
}

/// Dispatch one tool call.
pub fn dispatch_tool(name: &str, input: &Value, ctx: &mut ToolCtx) -> ToolResult {
    if ctx.read_only && matches!(name, "write_file" | "run_shell") {
        return err(format!("Tool '{name}' is not available in read-only mode."));
    }
    let root = ctx.project_root.clone();
    match name {
        "read_file" => {
            let Some(p) = str_field(input, "path") else {
                return err("read_file requires a 'path' string".into());
            };
            let Some(abs) = resolve_within(&root, p) else {
                return err(format!("Refusing to read outside project root: {p}"));
            };
            match std::fs::read(&abs) {
                Ok(b) => ok(String::from_utf8_lossy(&b).into_owned()),
                Err(e) => failed(name, &node_message(&e, "open", &abs)),
            }
        }
        "write_file" => {
            let (Some(p), Some(c)) = (str_field(input, "path"), str_field(input, "contents"))
            else {
                return err("write_file requires 'path' and 'contents' strings".into());
            };
            let Some(abs) = resolve_within(&root, p) else {
                return err(format!("Refusing to write outside project root: {p}"));
            };
            match write_with_dirs(&abs, c) {
                Ok(()) => ok(format!("wrote {p} ({} bytes)", js_len(c))),
                Err(e) => failed(name, &node_message(&e, "open", &abs)),
            }
        }
        "list_dir" => {
            let Some(p) = str_field(input, "path") else {
                return err("list_dir requires a 'path' string".into());
            };
            let Some(abs) = resolve_within(&root, p) else {
                return err(format!("Refusing to list outside project root: {p}"));
            };
            match std::fs::read_dir(&abs) {
                Ok(rd) => {
                    let mut lines: Vec<String> = rd
                        .filter_map(Result::ok)
                        .map(|e| {
                            let n = e.file_name().to_string_lossy().into_owned();
                            if e.file_type().is_ok_and(|t| t.is_dir()) {
                                format!("{n}/")
                            } else {
                                n
                            }
                        })
                        .collect();
                    lines.sort_by(|a, b| a.encode_utf16().cmp(b.encode_utf16()));
                    ok(lines.join("\n"))
                }
                Err(e) => failed(name, &node_message(&e, "scandir", &abs)),
            }
        }
        "run_shell" => run_shell(&root, input),
        "fetch_mzizi_skill" => {
            let Some(n) = str_field(input, "name") else {
                return err("fetch_mzizi_skill requires a 'name' string".into());
            };
            let skills = load_mzizi_skills();
            match skills.iter().find(|s| s.name == n) {
                Some(s) => ok(s.body.clone()),
                None => {
                    let known = skills
                        .iter()
                        .map(|s| s.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let known = if known.is_empty() {
                        "(none)".to_string()
                    } else {
                        known
                    };
                    err(format!("Unknown skill '{n}'. Known: {known}"))
                }
            }
        }
        "fetch_mzizi_component" => {
            let Some(n) = str_field(input, "name") else {
                return err("fetch_mzizi_component requires a 'name' string".into());
            };
            fetch_component(ctx, n)
        }
        other => err(format!("Unknown tool: {other}")),
    }
}

fn failed(tool: &str, msg: &str) -> ToolResult {
    err(format!("Tool '{tool}' failed: {msg}"))
}

fn write_with_dirs(abs: &Path, contents: &str) -> std::io::Result<()> {
    if let Some(d) = abs.parent() {
        std::fs::create_dir_all(d)?;
    }
    std::fs::write(abs, contents)
}

fn run_shell(root: &Path, input: &Value) -> ToolResult {
    let argv: Vec<String> = match input.get("argv") {
        Some(Value::Array(a)) if !a.is_empty() => a
            .iter()
            .map(|v| v.as_str().unwrap_or("").to_string())
            .collect(),
        _ => return err("run_shell requires a non-empty 'argv' array".into()),
    };
    let cwd = match str_field(input, "cwd").filter(|c| !c.is_empty()) {
        Some(c) => resolve_within(root, c).unwrap_or_else(|| root.to_path_buf()),
        None => root.to_path_buf(),
    };
    if argv[0].is_empty() {
        return err("run_shell argv[0] is empty".into());
    }
    match std::process::Command::new(&argv[0])
        .args(&argv[1..])
        .current_dir(cwd)
        .output()
    {
        Err(e) => err(format!("spawn failed: {}", spawn_message(&e, &argv[0]))),
        Ok(o) => {
            let code = o
                .status
                .code()
                .map_or("null".to_string(), |c| c.to_string());
            let (so, se) = (
                String::from_utf8_lossy(&o.stdout).into_owned(),
                String::from_utf8_lossy(&o.stderr).into_owned(),
            );
            let mut parts = vec![format!("exit code: {code}")];
            if !so.is_empty() {
                parts.push(format!("stdout:\n{so}"));
            }
            if !se.is_empty() {
                parts.push(format!("stderr:\n{se}"));
            }
            ToolResult {
                content: parts.join("\n"),
                is_error: o.status.code() != Some(0),
            }
        }
    }
}

/// Node words a failed spawn `spawn <cmd> ENOENT`.
pub fn spawn_message(e: &std::io::Error, cmd: &str) -> String {
    if e.kind() == std::io::ErrorKind::NotFound {
        format!("{cmd} ENOENT")
    } else {
        e.to_string()
    }
}

fn fetch_component(ctx: &mut ToolCtx, name: &str) -> ToolResult {
    // Primary: mzizi-mcp's `get_component` operation, through `mzizi_execute`.
    let mcp_try = (|| -> Result<Option<String>, String> {
        if ctx.mcp.is_none() {
            ctx.mcp = Some(McpClient::connect(
                &ctx.mcp_url,
                ctx.mcp_auth_token.as_deref(),
                crate::VERSION,
            )?);
        }
        let client = ctx.mcp.as_mut().expect("just set");
        let result = client.call_tool(
            MCP_EXECUTE_TOOL,
            json!({"operation": MCP_GET_COMPONENT_OPERATION, "arguments": {"name": name}}),
        )?;
        let text = result
            .get("content")
            .and_then(Value::as_array)
            .map(|blocks| {
                blocks
                    .iter()
                    .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                    .filter_map(|b| b.get("text").and_then(Value::as_str))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();
        let is_error = result.get("isError").and_then(Value::as_bool) == Some(true);
        Ok((!is_error && !text.is_empty()).then_some(text))
    })();
    match mcp_try {
        Ok(Some(text)) => return ok(text),
        Ok(None) => {}
        Err(_) => ctx.mcp = None, // a failed connection is retried next call
    }
    // Fallback: the public registry HTTP API.
    let fallback = (|| -> Result<String, String> {
        let r = reqwest::blocking::get(registry_component_url(&ctx.registry_http_base, name))
            .map_err(|e| e.to_string())?;
        if !r.status().is_success() {
            return Err(format!("HTTP {}", r.status().as_u16()));
        }
        let v: Value = r.json().map_err(|e| e.to_string())?;
        Ok(v.to_string())
    })();
    match fallback {
        Ok(s) => ok(s),
        Err(m) => err(format!(
            "fetch_mzizi_component('{name}') failed — MCP {MCP_EXECUTE_TOOL} {MCP_GET_COMPONENT_OPERATION} against {} and the HTTP fallback both errored: {m}",
            ctx.mcp_url
        )),
    }
}
