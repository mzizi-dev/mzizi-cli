//! System prompt and snapshot rendering (port of prompts.ts).

use serde_json::Value;

use super::explore::ProjectSnapshot;
use super::skills::load_mzizi_skills;

/// Frozen identity block. Never interpolate per-request data into it (prompt caching).
pub const FUNDI_IDENTITY: &str = include_str!("identity.txt");

/// Identity + skills as one cacheable text block.
pub fn build_system_prompt() -> String {
    let section = load_mzizi_skills()
        .iter()
        .map(|s| {
            format!(
                "<<skill name=\"{}\">>\n{}\n<<end-skill>>",
                s.name,
                s.body.trim()
            )
        })
        .collect::<Vec<_>>()
        .join("\n\n");
    format!("{FUNDI_IDENTITY}\n\n# Mzizi skill corpus\n\n{section}\n")
}

fn keys(v: Option<&Value>) -> Vec<String> {
    match v {
        Some(Value::Object(m)) => m.keys().cloned().collect(),
        _ => Vec::new(),
    }
}

/// Render a snapshot for a user-turn message (per-request data, never in the system prompt).
pub fn render_snapshot(s: &ProjectSnapshot) -> String {
    let mut l = vec![
        format!("Project root: {}", s.root),
        format!("Has tsconfig.json: {}", s.has_tsconfig),
        format!("Has app/globals.css: {}", s.has_globals_css),
        format!("Has components.json (shadcn): {}", s.has_components_json),
        format!(
            "Tailwind config: {}",
            s.tailwind_config_path
                .as_deref()
                .unwrap_or("(none detected)")
        ),
    ];
    if let Some(pj) = &s.package_json {
        let name = match pj.get("name") {
            Some(Value::String(n)) => n.clone(),
            Some(Value::Number(n)) => n.to_string(),
            _ => "(unnamed)".to_string(),
        };
        let join = |v: Vec<String>| {
            if v.is_empty() {
                "(none)".to_string()
            } else {
                v.join(", ")
            }
        };
        l.push(format!("package.json: name={name}"));
        l.push(format!(
            "dependencies: {}",
            join(keys(pj.get("dependencies")))
        ));
        l.push(format!(
            "devDependencies: {}",
            join(keys(pj.get("devDependencies")))
        ));
    } else {
        l.push("package.json: (missing)".into());
    }
    if !s.notes.is_empty() {
        l.push("Notes:".into());
        for n in &s.notes {
            l.push(format!("  - {}: {}", n.path, n.reason));
        }
    }
    l.join("\n")
}
