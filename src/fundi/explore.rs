//! `explore`: read a small, well-known set of markers off disk (port of explore.ts).
//! Deterministic: no model calls, no shelling out, no globbing.

use std::path::{Component, Path, PathBuf};

use serde_json::{Map, Value};

/// Something `explore` tried to read but could not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Note {
    /// The file.
    pub path: String,
    /// Why.
    pub reason: String,
}

/// The project as fundi sees it.
#[derive(Debug, Clone, PartialEq)]
pub struct ProjectSnapshot {
    /// Absolute project root.
    pub root: String,
    /// Parsed package.json, when present and valid.
    pub package_json: Option<Value>,
    /// tsconfig.json present.
    pub has_tsconfig: bool,
    /// app/globals.css present.
    pub has_globals_css: bool,
    /// First tailwind.config.* found.
    pub tailwind_config_path: Option<String>,
    /// components.json present.
    pub has_components_json: bool,
    /// Read failures.
    pub notes: Vec<Note>,
}

impl ProjectSnapshot {
    /// The JSON the TS CLI printed (same keys and order; `undefined` fields omitted).
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("root".into(), Value::String(self.root.clone()));
        if let Some(p) = &self.package_json {
            m.insert("packageJson".into(), p.clone());
        }
        m.insert("hasTsconfig".into(), self.has_tsconfig.into());
        m.insert("hasGlobalsCss".into(), self.has_globals_css.into());
        if let Some(t) = &self.tailwind_config_path {
            m.insert("tailwindConfigPath".into(), Value::String(t.clone()));
        }
        m.insert("hasComponentsJson".into(), self.has_components_json.into());
        let notes = self
            .notes
            .iter()
            .map(|n| {
                let mut o = Map::new();
                o.insert("path".into(), Value::String(n.path.clone()));
                o.insert("reason".into(), Value::String(n.reason.clone()));
                Value::Object(o)
            })
            .collect();
        m.insert("notes".into(), Value::Array(notes));
        Value::Object(m)
    }
}

const TAILWIND: [&str; 4] = [
    "tailwind.config.ts",
    "tailwind.config.js",
    "tailwind.config.mjs",
    "tailwind.config.cjs",
];

/// Node's `path.resolve(base, p)`, lexically (no filesystem access, no symlinks).
pub fn resolve(base: &Path, p: &str) -> PathBuf {
    let p = Path::new(p);
    let joined = if p.is_absolute() {
        p.to_path_buf()
    } else {
        base.join(p)
    };
    let mut out = PathBuf::new();
    for c in joined.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !matches!(
                    out.components().next_back(),
                    None | Some(Component::RootDir | Component::Prefix(_))
                ) {
                    out.pop();
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Explore `root` (resolved against `cwd` when relative).
pub fn explore_project(cwd: &Path, root: &str) -> ProjectSnapshot {
    let abs = resolve(cwd, root);
    let mut notes = Vec::new();
    let package_json = read_json(&abs.join("package.json"), &mut notes);
    let tailwind = TAILWIND
        .iter()
        .find(|c| abs.join(c).exists())
        .map(|c| (*c).to_string());
    ProjectSnapshot {
        root: abs.to_string_lossy().into_owned(),
        package_json,
        has_tsconfig: abs.join("tsconfig.json").exists(),
        has_globals_css: abs.join("app").join("globals.css").exists(),
        tailwind_config_path: tailwind,
        has_components_json: abs.join("components.json").exists(),
        notes,
    }
}

fn read_json(p: &Path, notes: &mut Vec<Note>) -> Option<Value> {
    let raw = match std::fs::read(p) {
        Ok(b) => String::from_utf8_lossy(&b).into_owned(),
        Err(e) => {
            // Missing is by design not a note; real failures are.
            if e.kind() != std::io::ErrorKind::NotFound {
                notes.push(Note {
                    path: p.to_string_lossy().into_owned(),
                    reason: node_message(&e, "open", p),
                });
            }
            return None;
        }
    };
    match serde_json::from_str::<Value>(&raw) {
        Ok(v) => Some(v),
        Err(e) => {
            notes.push(Note {
                path: p.to_string_lossy().into_owned(),
                reason: format!("invalid JSON: {e}"),
            });
            None
        }
    }
}

/// An I/O error worded the way Node words it (`ENOENT: no such file or directory, open '...'`).
pub fn node_message(e: &std::io::Error, syscall: &str, p: &Path) -> String {
    use std::io::ErrorKind::*;
    let (code, text) = match e.kind() {
        NotFound => ("ENOENT", "no such file or directory"),
        PermissionDenied => ("EACCES", "permission denied"),
        AlreadyExists => ("EEXIST", "file already exists"),
        NotADirectory => ("ENOTDIR", "not a directory"),
        IsADirectory => ("EISDIR", "illegal operation on a directory"),
        _ => return e.to_string(),
    };
    format!("{code}: {text}, {syscall} '{}'", p.display())
}
