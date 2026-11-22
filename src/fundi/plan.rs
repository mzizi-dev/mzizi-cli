//! `plan`: a read-only Claude tool-use loop that produces a [`Plan`] (port of plan.ts).

use std::path::Path;

use regex::Regex;
use serde_json::{Value, json};

use super::anthropic::Anthropic;
use super::explore::ProjectSnapshot;
use super::prompts::render_snapshot;
use super::tools::{
    ToolCtx, dispatch_tool, fundi_tool_definitions, js_len, registry_install_argv, resolve_within,
    spawn_message,
};

const MAX_PLANNER_ITERATIONS: usize = 10;

/// One executable unit the planner proposes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Install registry components.
    InstallComponent {
        /// Why.
        description: String,
        /// Bare registry names.
        components: Vec<String>,
    },
    /// Create a file.
    CreateFile {
        /// Why.
        description: String,
        /// Path relative to the root.
        path: String,
        /// Contents.
        contents: String,
    },
    /// Replace a file.
    ModifyFile {
        /// Why.
        description: String,
        /// Path relative to the root.
        path: String,
        /// Contents.
        contents: String,
    },
    /// Run a command.
    RunCommand {
        /// Why.
        description: String,
        /// argv.
        argv: Vec<String>,
        /// Working directory relative to the root.
        cwd: Option<String>,
    },
}

impl Step {
    /// The `kind` string.
    pub fn kind(&self) -> &'static str {
        match self {
            Step::InstallComponent { .. } => "install-component",
            Step::CreateFile { .. } => "create-file",
            Step::ModifyFile { .. } => "modify-file",
            Step::RunCommand { .. } => "run-command",
        }
    }
    /// The description.
    pub fn description(&self) -> &str {
        match self {
            Step::InstallComponent { description, .. }
            | Step::CreateFile { description, .. }
            | Step::ModifyFile { description, .. }
            | Step::RunCommand { description, .. } => description,
        }
    }
}

/// The result of applying one step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StepResult {
    /// The step.
    pub step: Step,
    /// Whether it applied (or would, on a dry run).
    pub applied: bool,
    /// What happened.
    pub detail: String,
    /// Failure tag.
    pub error: Option<String>,
}

/// A goal and its steps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Plan {
    /// The goal.
    pub goal: String,
    /// The steps.
    pub steps: Vec<Step>,
    project_root: std::path::PathBuf,
}

/// Everything the planner loop needs.
pub struct PlanContext<'a> {
    /// The Anthropic client.
    pub anthropic: &'a Anthropic,
    /// Model id.
    pub model: String,
    /// Max tokens.
    pub max_tokens: u64,
    /// Tools context (read-only).
    pub tools: &'a mut ToolCtx,
    /// The cacheable system prompt.
    pub system_prompt: String,
}

fn text_of(content: &Value) -> String {
    content
        .as_array()
        .map(|a| {
            a.iter()
                .filter(|b| b.get("type").and_then(Value::as_str) == Some("text"))
                .filter_map(|b| b.get("text").and_then(Value::as_str))
                .collect::<Vec<_>>()
                .join("\n")
        })
        .unwrap_or_default()
        .trim()
        .to_string()
}

/// Drive the planner loop and return a plan bound to the project root.
pub fn build_plan(
    goal: &str,
    snapshot: &ProjectSnapshot,
    ctx: &mut PlanContext<'_>,
) -> Result<Plan, String> {
    let mut messages = vec![json!({
        "role": "user",
        "content": [{"type": "text", "text": plan_user_prompt(goal, snapshot)}],
    })];
    let tools = fundi_tool_definitions();
    let mut final_text = None;
    for _ in 0..MAX_PLANNER_ITERATIONS {
        let response = ctx.anthropic.messages_create(&json!({
            "model": ctx.model,
            "max_tokens": ctx.max_tokens,
            "system": [{"type": "text", "text": ctx.system_prompt, "cache_control": {"type": "ephemeral"}}],
            "tools": tools,
            "messages": messages,
        }))?;
        let content = response.get("content").cloned().unwrap_or(json!([]));
        messages.push(json!({"role": "assistant", "content": content}));
        if response.get("stop_reason").and_then(Value::as_str) == Some("tool_use") {
            let mut results = Vec::new();
            for b in content.as_array().into_iter().flatten() {
                if b.get("type").and_then(Value::as_str) != Some("tool_use") {
                    continue;
                }
                let name = b.get("name").and_then(Value::as_str).unwrap_or("");
                let input = b.get("input").cloned().unwrap_or(Value::Null);
                let r = dispatch_tool(name, &input, ctx.tools);
                results.push(json!({
                    "type": "tool_result",
                    "tool_use_id": b.get("id").cloned().unwrap_or(Value::Null),
                    "content": r.content,
                    "is_error": r.is_error,
                }));
            }
            messages.push(json!({"role": "user", "content": results}));
            continue;
        }
        final_text = Some(text_of(&content));
        break;
    }
    let text = final_text
        .filter(|t| !t.is_empty())
        .ok_or("Fundi planner exhausted iterations without producing a plan")?;
    let steps = extract_steps_from_text(&text)?;
    Ok(Plan {
        goal: goal.into(),
        steps,
        project_root: ctx.tools.project_root.clone(),
    })
}

fn plan_user_prompt(goal: &str, snapshot: &ProjectSnapshot) -> String {
    [
        format!("Goal: {goal}"),
        String::new(),
        "Current project snapshot:".into(),
        render_snapshot(snapshot),
        String::new(),
        "Use the read-only tools (read_file, list_dir, fetch_mzizi_skill,".into(),
        "fetch_mzizi_component) to gather what you need. Do not call write_file".into(),
        "or run_shell; those are reserved for the apply phase.".into(),
        String::new(),
        "When you are ready, respond with a single fenced JSON block tagged".into(),
        "`plan` whose payload is an array of Step objects. Each Step is one of:".into(),
        r#"  { "kind": "install-component", "description": "...", "params": { "components": ["button", "card"] } }"#.into(),
        r#"  { "kind": "create-file",       "description": "...", "params": { "path": "app/foo.tsx", "contents": "..." } }"#.into(),
        r#"  { "kind": "modify-file",       "description": "...", "params": { "path": "app/foo.tsx", "contents": "..." } }"#.into(),
        r#"  { "kind": "run-command",       "description": "...", "params": { "argv": ["pnpm", "lint"], "cwd": "." } }"#.into(),
        String::new(),
        "Example response shell:".into(),
        "```plan".into(),
        r#"[ { "kind": "install-component", "description": "...", "params": { "components": ["button"] } } ]"#.into(),
        "```".into(),
        String::new(),
        "Emit the fenced block as the entire response; no prose around it.".into(),
    ]
    .join("\n")
}

/// Pull the first ```plan fenced block (or a bare JSON array) out of the text and validate it.
pub fn extract_steps_from_text(text: &str) -> Result<Vec<Step>, String> {
    let fence = Regex::new(r"(?s)```plan\s*\n(.*?)```").expect("static regex");
    let payload: Option<String> = match fence.captures(text) {
        Some(c) => Some(c[1].to_string()),
        None => {
            let t = text.trim();
            (t.starts_with('[') && t.ends_with(']')).then(|| t.to_string())
        }
    };
    let payload = payload
        .filter(|p| !p.is_empty())
        .ok_or("Planner response did not contain a `plan` JSON block")?;
    let parsed: Value =
        serde_json::from_str(&payload).map_err(|e| format!("Planner JSON parse failed: {e}"))?;
    let Value::Array(entries) = parsed else {
        return Err("Planner JSON must be an array of Step objects".into());
    };
    entries
        .iter()
        .enumerate()
        .map(|(i, e)| validate_step(e, i))
        .collect()
}

fn js_string(v: Option<&Value>) -> String {
    match v {
        None => "undefined".into(),
        Some(Value::String(s)) => s.clone(),
        Some(Value::Null) => "null".into(),
        Some(Value::Object(_)) => "[object Object]".into(),
        Some(Value::Array(a)) => a
            .iter()
            .map(|x| js_string(Some(x)))
            .collect::<Vec<_>>()
            .join(","),
        Some(other) => other.to_string(),
    }
}

fn string_array(v: Option<&Value>) -> Option<Vec<String>> {
    v?.as_array()?
        .iter()
        .map(|x| x.as_str().map(str::to_string))
        .collect()
}

fn validate_step(entry: &Value, idx: usize) -> Result<Step, String> {
    if !entry.is_object() && !entry.is_array() {
        return Err(format!("Step {idx}: not an object"));
    }
    let kind = entry.get("kind");
    let description = entry
        .get("description")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let params = entry.get("params").filter(|p| !p.is_null());
    let p = |k: &str| params.and_then(|p| p.get(k));
    match kind.and_then(Value::as_str) {
        Some("install-component") => {
            let components = string_array(p("components")).ok_or_else(|| {
                format!("Step {idx}: install-component requires params.components: string[]")
            })?;
            Ok(Step::InstallComponent {
                description,
                components,
            })
        }
        Some(k @ ("create-file" | "modify-file")) => {
            let (Some(path), Some(contents)) = (
                p("path").and_then(Value::as_str),
                p("contents").and_then(Value::as_str),
            ) else {
                return Err(format!(
                    "Step {idx}: {k} requires params.path: string and params.contents: string"
                ));
            };
            let (path, contents) = (path.to_string(), contents.to_string());
            Ok(if k == "create-file" {
                Step::CreateFile {
                    description,
                    path,
                    contents,
                }
            } else {
                Step::ModifyFile {
                    description,
                    path,
                    contents,
                }
            })
        }
        Some("run-command") => {
            let argv = string_array(p("argv"))
                .filter(|a| !a.is_empty())
                .ok_or_else(|| {
                    format!("Step {idx}: run-command requires non-empty params.argv: string[]")
                })?;
            let cwd = p("cwd").and_then(Value::as_str).map(str::to_string);
            Ok(Step::RunCommand {
                description,
                argv,
                cwd,
            })
        }
        _ => Err(format!("Step {idx}: unknown kind '{}'", js_string(kind))),
    }
}

/// The plan's results, as `apply` returns them.
pub struct PlanResult {
    /// The goal.
    pub goal: String,
    /// Whether this was a dry run.
    pub dry_run: bool,
    /// Per-step results.
    pub results: Vec<StepResult>,
}

impl Plan {
    /// Walk the steps; a dry run touches nothing.
    pub fn apply(&self, dry_run: bool) -> PlanResult {
        let results = self
            .steps
            .iter()
            .map(|s| apply_step(s, &self.project_root, dry_run))
            .collect();
        PlanResult {
            goal: self.goal.clone(),
            dry_run,
            results,
        }
    }
}

fn applied(step: &Step, detail: String) -> StepResult {
    StepResult {
        step: step.clone(),
        applied: true,
        detail,
        error: None,
    }
}

fn apply_step(step: &Step, root: &Path, dry_run: bool) -> StepResult {
    match step {
        Step::InstallComponent { components, .. } => {
            let argv = registry_install_argv(components);
            if dry_run {
                return applied(step, format!("[dry-run] would run: {}", argv.join(" ")));
            }
            run_argv(step, &argv, root, None)
        }
        Step::CreateFile { path, contents, .. } | Step::ModifyFile { path, contents, .. } => {
            let Some(_abs) = resolve_within(root, path) else {
                return StepResult {
                    step: step.clone(),
                    applied: false,
                    detail: format!("refusing to write outside project root: {path}"),
                    error: Some("path-escape".into()),
                };
            };
            if dry_run {
                return applied(
                    step,
                    format!(
                        "[dry-run] would {} {path} ({} bytes)",
                        step.kind(),
                        js_len(contents)
                    ),
                );
            }
            let written = crate::pathsafe::safe_write(root, path, contents.as_bytes());
            match written {
                Ok(()) => applied(step, format!("wrote {path} ({} bytes)", js_len(contents))),
                Err(e) => {
                    let reason = e;
                    StepResult {
                        step: step.clone(),
                        applied: false,
                        detail: format!("write failed: {reason}"),
                        error: Some(reason),
                    }
                }
            }
        }
        Step::RunCommand { argv, cwd, .. } => {
            if dry_run {
                let where_ = cwd
                    .as_ref()
                    .map(|c| format!(" (cwd={c})"))
                    .unwrap_or_default();
                return applied(
                    step,
                    format!("[dry-run] would run: {}{where_}", argv.join(" ")),
                );
            }
            run_argv(step, argv, root, cwd.as_deref())
        }
    }
}

fn run_argv(step: &Step, argv: &[String], root: &Path, cwd_rel: Option<&str>) -> StepResult {
    let cwd = cwd_rel
        .filter(|c| !c.is_empty())
        .and_then(|c| resolve_within(root, c))
        .unwrap_or_else(|| root.to_path_buf());
    let Some((cmd, rest)) = argv.split_first().filter(|(c, _)| !c.is_empty()) else {
        return StepResult {
            step: step.clone(),
            applied: false,
            detail: "empty argv".into(),
            error: Some("empty-argv".into()),
        };
    };
    match std::process::Command::new(cmd)
        .args(rest)
        .current_dir(cwd)
        .output()
    {
        Err(e) => {
            let m = spawn_message(&e, cmd);
            StepResult {
                step: step.clone(),
                applied: false,
                detail: format!("spawn failed: {m}"),
                error: Some(m),
            }
        }
        Ok(o) => {
            let code = o.status.code();
            let (so, se) = (
                String::from_utf8_lossy(&o.stdout).trim().to_string(),
                String::from_utf8_lossy(&o.stderr).trim().to_string(),
            );
            let mut parts = vec![format!(
                "exit {}",
                code.map_or("null".to_string(), |c| c.to_string())
            )];
            if !so.is_empty() {
                parts.push(format!("stdout:\n{so}"));
            }
            if !se.is_empty() {
                parts.push(format!("stderr:\n{se}"));
            }
            StepResult {
                step: step.clone(),
                applied: code == Some(0),
                detail: parts.join("\n"),
                error: (code != Some(0)).then(|| {
                    format!(
                        "exit-{}",
                        code.map_or("null".to_string(), |c| c.to_string())
                    )
                }),
            }
        }
    }
}
