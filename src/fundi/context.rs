//! The no-key `plan` fallback (port of context-gather.ts).

use regex::Regex;
use serde_json::{Map, Value, json};

use super::explore::ProjectSnapshot;
use super::skills::{Skill, load_mzizi_skills};

/// The note every context carries.
pub const NOTE: &str = "No ANTHROPIC_API_KEY set — this is raw context, not a plan. If you are a coding agent, use this (and your own model access) to plan the change yourself; fundi did not call any model to produce it.";

const STOPWORDS: [&str; 11] = [
    "the", "a", "an", "and", "or", "to", "of", "for", "in", "on", "with",
];

/// Goal, snapshot and the skills that share a word with the goal.
pub struct ContextGatherResult {
    /// The goal.
    pub goal: String,
    /// The snapshot.
    pub snapshot: ProjectSnapshot,
    /// Matching skills.
    pub relevant_skills: Vec<Skill>,
    /// The note.
    pub note: &'static str,
}

/// Match the goal's words against `skills`.
pub fn gather_context_with(
    goal: &str,
    snapshot: ProjectSnapshot,
    skills: Vec<Skill>,
) -> ContextGatherResult {
    let lower = goal.to_lowercase();
    let words: Vec<&str> = lower
        .split(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
        .filter(|w| w.len() > 2 && *w != "add" && !STOPWORDS.contains(w))
        .collect();
    let relevant_skills = skills
        .into_iter()
        .filter(|s| {
            let hay = format!("{} {}", s.name, s.body).to_lowercase();
            words.iter().any(|w| hay.contains(w))
        })
        .collect();
    ContextGatherResult {
        goal: goal.into(),
        snapshot,
        relevant_skills,
        note: NOTE,
    }
}

/// [`gather_context_with`] over the embedded corpus.
pub fn gather_context(goal: &str, snapshot: ProjectSnapshot) -> ContextGatherResult {
    gather_context_with(goal, snapshot, load_mzizi_skills())
}

const MAX_DESCRIPTION: usize = 160;

/// One line describing a skill: frontmatter `description`, else the first prose line;
/// cut at the first sentence and capped at 160 characters.
pub fn describe_skill(body: &str) -> String {
    let fm = Regex::new(r"^---\r?\n((?s:.*?))\r?\n---").expect("static regex");
    let desc = Regex::new(r"(?mR)^description:[ \t]*(\S.*)$").expect("static regex");
    let fm_description = fm.captures(body).and_then(|c| {
        desc.captures(c.get(1).map_or("", |m| m.as_str()))
            .map(|d| d[1].to_string())
    });
    let stripped = fm.replace(body, "");
    let prose = stripped
        .split('\n')
        .map(|l| l.trim_end_matches('\r').trim())
        .find(|l| !l.is_empty() && !l.starts_with('#'))
        .map(str::to_string);
    let mut text = fm_description.or(prose).unwrap_or_default();
    if text.starts_with(['"', '\'']) {
        text.remove(0);
    }
    if text.ends_with(['"', '\'']) {
        text.pop();
    }
    let text = text.trim().to_string();
    let sentence = Regex::new(r"^(.+?[.!?])(\s|$)").expect("static regex");
    let line = sentence
        .captures(&text)
        .map_or(text.clone(), |c| c[1].to_string());
    if line.chars().count() > MAX_DESCRIPTION {
        let cut: String = line.chars().take(MAX_DESCRIPTION - 1).collect();
        format!("{}…", cut.trim_end())
    } else {
        line
    }
}

/// The default no-key `plan` output: names, one-line descriptions, how to fetch.
pub fn summarise_context(r: &ContextGatherResult) -> Value {
    let skills: Vec<Value> = r
        .relevant_skills
        .iter()
        .map(|s| {
            json!({"name": s.name, "description": describe_skill(&s.body), "bytes": s.body.len()})
        })
        .collect();
    let mut m = Map::new();
    m.insert("goal".into(), r.goal.clone().into());
    m.insert("snapshot".into(), r.snapshot.to_json());
    m.insert("relevantSkills".into(), Value::Array(skills));
    m.insert(
        "howToFetch".into(),
        "Print one skill in full with `mz plan --skill <name>`, or every matching skill in full with `mz plan <goal> --full`. The same skills are on mcp.mzizi.dev (the `get_skills` operation of `mzizi_execute`).".into(),
    );
    m.insert("note".into(), r.note.into());
    Value::Object(m)
}

/// The `--full` output: every matching skill with its body.
pub fn full_context(r: &ContextGatherResult) -> Value {
    let skills: Vec<Value> = r
        .relevant_skills
        .iter()
        .map(|s| json!({"name": s.name, "body": s.body}))
        .collect();
    let mut m = Map::new();
    m.insert("goal".into(), r.goal.clone().into());
    m.insert("snapshot".into(), r.snapshot.to_json());
    m.insert("relevantSkills".into(), Value::Array(skills));
    m.insert("note".into(), r.note.into());
    Value::Object(m)
}
