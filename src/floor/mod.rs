//! The Bundu floor check (TS `check`): spacing and leading. Port of `check.ts`,
//! `check-spacing.ts` and `spacing-floor.ts` with no behaviour change.
//!
//! Walks the given paths (default: the current directory) for CSS, TSX and the other files
//! that carry classes or styles, scans each, and reports `file:line:column`. Offline and
//! deterministic. Any error not excused by `mzizi-spacing-exempt: <reason>` fails the command.

pub mod scan;
pub mod tokens;

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde_json::{Map, Value};

use crate::{CliError, Io, Result, VERSION};
use scan::{Exempted, Finding, ScanOptions, scan_text};
use tokens::{EXEMPT_MARKER, HIT_DESKTOP_PX, HIT_TOUCH_PX, Kind, Layer, floor_of, js_num};

/// Files that can hold a class, a style or a stylesheet. Anything else is not read.
pub const CHECK_EXTENSIONS: &[&str] = &[
    ".css", ".pcss", ".scss", ".tsx", ".jsx", ".ts", ".js", ".mjs", ".cjs", ".astro", ".html",
    ".vue", ".svelte", ".mdx", ".rs",
];

/// Directories that are build output, dependencies or VCS state, never source.
pub const CHECK_IGNORED_DIRS: &[&str] = &[
    "node_modules",
    ".git",
    "dist",
    "build",
    "out",
    "coverage",
    "target",
    ".next",
    ".astro",
    ".turbo",
    ".wrangler",
    ".svelte-kit",
    ".output",
    ".vercel",
    ".cache",
];

/// `//` is a comment in these; in CSS and HTML it is a protocol-relative URL.
const LINE_COMMENT_EXTENSIONS: &[&str] = &[
    ".tsx", ".jsx", ".ts", ".js", ".mjs", ".cjs", ".astro", ".vue", ".svelte", ".mdx", ".rs",
    ".scss",
];

const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

/// Options of [`check_project`].
#[derive(Debug, Clone, Default)]
pub struct CheckOptions {
    /// Files or directories to scan. Default: `["."]`.
    pub paths: Vec<String>,
    /// Project root that reported paths are relative to.
    pub cwd: PathBuf,
    /// Path prefixes (relative to `cwd`) to leave out.
    pub exclude: Vec<String>,
    /// Pixels per Tailwind spacing step.
    pub spacing_unit_px: Option<f64>,
    /// Pixels per rem and em.
    pub rem_px: Option<f64>,
}

/// A finding with the file it came from.
#[derive(Debug, Clone)]
pub struct FileFinding {
    /// Posix path relative to the root.
    pub file: String,
    /// The finding.
    pub finding: Finding,
}

/// An exempted finding with its file.
#[derive(Debug, Clone)]
pub struct FileExempted {
    /// Posix path relative to the root.
    pub file: String,
    /// The finding and its reason.
    pub exempted: Exempted,
}

/// A notice: never a failure.
#[derive(Debug, Clone)]
pub struct Notice {
    /// File the notice is about.
    pub file: String,
    /// 1-based line, when the notice is about one.
    pub line: Option<usize>,
    /// The text.
    pub message: String,
}

/// The report `check` prints.
#[derive(Debug, Clone)]
pub struct CheckReport {
    /// Absolute project root.
    pub root: String,
    /// Files read.
    pub files_scanned: usize,
    /// Errors. The check fails when there is one.
    pub violations: Vec<FileFinding>,
    /// Warnings: reported, never failing.
    pub warnings: Vec<FileFinding>,
    /// Excused findings.
    pub exempted: Vec<FileExempted>,
    /// Legacy opt-outs and files too large to read.
    pub notices: Vec<Notice>,
}

impl CheckReport {
    /// True when nothing is below the floor.
    pub fn ok(&self) -> bool {
        self.violations.is_empty()
    }

    /// The `--json` report, with the TypeScript key order.
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("tool".into(), "mzizi check".into());
        m.insert("rule".into(), "spacing-floor".into());
        m.insert("version".into(), VERSION.into());
        m.insert("floor".into(), tokens::spacing_floor_json());
        m.insert("exemptMarker".into(), EXEMPT_MARKER.into());
        m.insert("root".into(), self.root.clone().into());
        m.insert("filesScanned".into(), self.files_scanned.into());
        let ff = |v: &[FileFinding]| {
            Value::Array(
                v.iter()
                    .map(|f| f.finding.to_json(Some(&f.file), None))
                    .collect(),
            )
        };
        m.insert("violations".into(), ff(&self.violations));
        m.insert("warnings".into(), ff(&self.warnings));
        m.insert(
            "exempted".into(),
            Value::Array(
                self.exempted
                    .iter()
                    .map(|e| {
                        e.exempted
                            .finding
                            .to_json(Some(&e.file), Some(&e.exempted.reason))
                    })
                    .collect(),
            ),
        );
        m.insert(
            "notices".into(),
            Value::Array(
                self.notices
                    .iter()
                    .map(|n| {
                        let mut o = Map::new();
                        o.insert("file".into(), n.file.clone().into());
                        if let Some(l) = n.line {
                            o.insert("line".into(), l.into());
                        }
                        o.insert("message".into(), n.message.clone().into());
                        Value::Object(o)
                    })
                    .collect(),
            ),
        );
        m.insert("ok".into(), self.ok().into());
        Value::Object(m)
    }
}

/// Why [`check_project`] stopped.
#[derive(Debug)]
pub enum CheckError {
    /// A usage error (the TypeScript `CheckUsageError`).
    Usage(String),
    /// Anything else (an I/O failure).
    Io(std::io::Error),
}

impl From<std::io::Error> for CheckError {
    fn from(e: std::io::Error) -> Self {
        CheckError::Io(e)
    }
}

fn normalize(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                if !matches!(
                    out.components().next_back(),
                    Some(Component::RootDir) | None
                ) {
                    out.pop();
                }
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// `path.resolve(base, p)`.
fn resolve(base: &Path, p: &str) -> PathBuf {
    normalize(&base.join(p))
}

fn relative(from: &Path, to: &Path) -> String {
    let f: Vec<_> = from.components().collect();
    let t: Vec<_> = to.components().collect();
    let common = f.iter().zip(&t).take_while(|(a, b)| a == b).count();
    let mut parts: Vec<String> = Vec::new();
    for _ in common..f.len() {
        parts.push("..".into());
    }
    for c in &t[common..] {
        parts.push(c.as_os_str().to_string_lossy().into_owned());
    }
    parts.join("/")
}

/// `path.extname(...).toLowerCase()`.
fn ext_of(abs: &Path) -> String {
    let name = abs
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    match name.rfind('.') {
        Some(i) if i > 0 => name[i..].to_lowercase(),
        _ => String::new(),
    }
}

fn excluded(abs: &Path, cwd: &Path, exclude: &[String]) -> bool {
    let a = abs.to_string_lossy();
    exclude.iter().any(|raw| {
        let e = resolve(cwd, raw);
        let e = e.to_string_lossy();
        *a == *e || a.starts_with(&format!("{e}/"))
    })
}

fn walk(
    abs: &Path,
    cwd: &Path,
    exclude: &[String],
    explicit: bool,
    files: &mut Vec<PathBuf>,
) -> std::io::Result<()> {
    if excluded(abs, cwd, exclude) {
        return Ok(());
    }
    // A path the user named is followed even if it is a symlink; a symlink met while walking is not.
    let meta = if explicit {
        fs::metadata(abs)?
    } else {
        fs::symlink_metadata(abs)?
    };
    if meta.file_type().is_symlink() {
        return Ok(());
    }
    if meta.is_dir() {
        let base = abs
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        if !explicit && CHECK_IGNORED_DIRS.contains(&base.as_str()) {
            return Ok(());
        }
        let mut names: Vec<String> = fs::read_dir(abs)?
            .map(|e| e.map(|e| e.file_name().to_string_lossy().into_owned()))
            .collect::<std::io::Result<_>>()?;
        names.sort();
        for name in names {
            walk(&abs.join(name), cwd, exclude, false, files)?;
        }
        return Ok(());
    }
    if !meta.is_file() {
        return Ok(());
    }
    let ext = ext_of(abs);
    let name = abs
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    if !explicit
        && (!CHECK_EXTENSIONS.contains(&ext.as_str())
            || name.ends_with(".min.css")
            || name.ends_with(".d.ts"))
    {
        return Ok(());
    }
    files.push(abs.to_path_buf());
    Ok(())
}

/// Scan a project.
pub fn check_project(options: &CheckOptions) -> std::result::Result<CheckReport, CheckError> {
    let cwd = normalize(&options.cwd);
    let named = !options.paths.is_empty();
    let paths: Vec<String> = if named {
        options.paths.clone()
    } else {
        vec![".".to_string()]
    };
    let mut files: Vec<PathBuf> = Vec::new();
    for p in &paths {
        let abs = resolve(&cwd, p);
        if fs::symlink_metadata(&abs).is_err() {
            return Err(CheckError::Usage(format!("no such file or directory: {p}")));
        }
        walk(&abs, &cwd, &options.exclude, true, &mut files)?;
    }

    let mut violations = Vec::new();
    let mut warnings = Vec::new();
    let mut exempted = Vec::new();
    let mut notices = Vec::new();
    let mut seen = BTreeSet::new();
    let mut files_scanned = 0;
    files.sort_by_key(|p| p.to_string_lossy().into_owned());
    for abs in files {
        if !seen.insert(abs.clone()) {
            continue;
        }
        let file = relative(&cwd, &abs);
        if fs::metadata(&abs)?.len() > MAX_FILE_BYTES {
            notices.push(Notice {
                file,
                line: None,
                message: format!("not scanned: larger than {MAX_FILE_BYTES} bytes"),
            });
            continue;
        }
        let text = String::from_utf8_lossy(&fs::read(&abs)?).into_owned();
        files_scanned += 1;
        let ext = ext_of(&abs);
        let result = scan_text(
            &text,
            &ScanOptions {
                line_comments: Some(LINE_COMMENT_EXTENSIONS.contains(&ext.as_str())),
                spacing_unit_px: options.spacing_unit_px,
                rem_px: options.rem_px,
            },
        );
        for f in result.findings {
            violations.push(FileFinding {
                file: file.clone(),
                finding: f,
            });
        }
        for f in result.warnings {
            warnings.push(FileFinding {
                file: file.clone(),
                finding: f,
            });
        }
        for e in result.exempted {
            exempted.push(FileExempted {
                file: file.clone(),
                exempted: e,
            });
        }
        for line in result.legacy {
            notices.push(Notice {
                file: file.clone(),
                line: Some(line),
                message: "data-mzizi-sizes=\"legacy\" steps out of the Bundu floor at runtime; the source is still checked".into(),
            });
        }
    }

    if files_scanned == 0 {
        // A gate that read nothing must not look like a pass.
        if named {
            return Err(CheckError::Usage(format!(
                "no files to check under {} (reads {})",
                paths.join(", "),
                CHECK_EXTENSIONS.join(" ")
            )));
        }
        notices.push(Notice {
            file: ".".into(),
            line: None,
            message: "no files to check in this directory".into(),
        });
    }

    Ok(CheckReport {
        root: cwd.to_string_lossy().into_owned(),
        files_scanned,
        violations,
        warnings,
        exempted,
        notices,
    })
}

/// The human report.
pub fn render_report(report: &CheckReport) -> String {
    let mut lines: Vec<String> = Vec::new();
    for v in &report.violations {
        let f = &v.finding;
        lines.push(format!(
            "{}:{}:{}  {}  {}",
            v.file, f.line, f.column, f.rule, f.message
        ));
    }
    for w in &report.warnings {
        let f = &w.finding;
        lines.push(format!(
            "{}:{}:{}  warning  {}  {}",
            w.file, f.line, f.column, f.rule, f.message
        ));
    }
    for n in &report.notices {
        let at = match n.line {
            Some(l) if l > 0 => format!(":{l}"),
            _ => String::new(),
        };
        lines.push(format!("{}{}  notice  {}", n.file, at, n.message));
    }
    let fl = |layer: Layer| -> String {
        let cell = |k| {
            floor_of(layer, k).map_or("-".to_string(), |f| {
                format!("{}/{}", js_num(f.default), js_num(f.dense))
            })
        };
        format!(
            "{} {} {}",
            layer.as_str(),
            cell(Kind::Padding),
            cell(Kind::Gap)
        )
    };
    lines.push(String::new());
    lines.push(format!(
        "spacing floors, default/dense px (padding, gap): {}; unplaced {} padding, {} gap; leading {}",
        Layer::ALL.iter().map(|l| fl(*l)).collect::<Vec<_>>().join("; "),
        js_num(tokens::UNPLACED_PADDING_PX),
        js_num(tokens::UNPLACED_GAP_PX),
        js_num(tokens::LEADING_BODY)
    ));
    lines.push(format!(
        "hit region floor (px): {} touch, {} desktop",
        js_num(HIT_TOUCH_PX),
        js_num(HIT_DESKTOP_PX)
    ));
    lines.push(format!(
        "{} file(s) scanned, {} violation(s), {} warning(s), {} exempted",
        report.files_scanned,
        report.violations.len(),
        report.warnings.len(),
        report.exempted.len()
    ));
    if !report.violations.is_empty() {
        lines.push(format!(
            "Use the components and the layer tokens (--space-control-*, --space-card-*, --space-container-*, --space-page-x, --space-section, --leading-floor). A value below the floor needs `{EXEMPT_MARKER}: <reason>` in a comment on the same or the previous line."
        ));
    }
    format!("{}\n", lines.join("\n"))
}

/// `JSON.stringify(value, null, 2)`.
pub fn json_pretty(v: &Value) -> String {
    fn go(v: &Value, depth: usize, out: &mut String) {
        let pad = |n: usize| "  ".repeat(n);
        match v {
            Value::Null => out.push_str("null"),
            Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
            Value::Number(n) => out.push_str(&js_num(n.as_f64().unwrap_or(f64::NAN))),
            Value::String(s) => out.push_str(&serde_json::to_string(s).unwrap_or_default()),
            Value::Array(a) if a.is_empty() => out.push_str("[]"),
            Value::Array(a) => {
                out.push_str("[\n");
                for (i, x) in a.iter().enumerate() {
                    out.push_str(&pad(depth + 1));
                    go(x, depth + 1, out);
                    out.push_str(if i + 1 < a.len() { ",\n" } else { "\n" });
                }
                out.push_str(&pad(depth));
                out.push(']');
            }
            Value::Object(o) if o.is_empty() => out.push_str("{}"),
            Value::Object(o) => {
                out.push_str("{\n");
                let n = o.len();
                for (i, (k, x)) in o.iter().enumerate() {
                    out.push_str(&pad(depth + 1));
                    out.push_str(&serde_json::to_string(k).unwrap_or_default());
                    out.push_str(": ");
                    go(x, depth + 1, out);
                    out.push_str(if i + 1 < n { ",\n" } else { "\n" });
                }
                out.push_str(&pad(depth));
                out.push('}');
            }
        }
    }
    let mut s = String::new();
    go(v, 0, &mut s);
    s
}

/// JavaScript's `Number(string)` for the values `--spacing-unit` and `--rem` take.
fn js_number(s: &str) -> f64 {
    let t = s.trim();
    if t.is_empty() {
        return 0.0;
    }
    for (prefix, radix) in [
        ("0x", 16),
        ("0X", 16),
        ("0o", 8),
        ("0O", 8),
        ("0b", 2),
        ("0B", 2),
    ] {
        if let Some(rest) = t.strip_prefix(prefix) {
            return u64::from_str_radix(rest, radix).map_or(f64::NAN, |n| n as f64);
        }
    }
    if t == "Infinity" || t == "+Infinity" {
        return f64::INFINITY;
    }
    if t == "-Infinity" {
        return f64::NEG_INFINITY;
    }
    // Rust also accepts `inf`, `nan` and `infinity` spellings that JS does not.
    if t.chars()
        .any(|c| c.is_ascii_alphabetic() && c != 'e' && c != 'E')
    {
        return f64::NAN;
    }
    t.parse::<f64>().unwrap_or(f64::NAN)
}

fn fail<T>(m: impl Into<String>) -> Result<T> {
    Err(CliError::msg(m))
}

/// `mz floor [path...]` (`check` under the old bin names).
pub fn cmd_check(args: &[String], io: &mut Io<'_>) -> Result<()> {
    let mut paths: Vec<String> = Vec::new();
    let mut exclude: Vec<String> = Vec::new();
    let mut spacing_unit_px = None;
    let mut rem_px = None;
    let mut json = false;
    let mut i = 0;
    while i < args.len() {
        let mut arg = args[i].as_str();
        if arg == "-h" || arg == "--help" {
            io.out.write_all(crate::cli::HELP.as_bytes())?;
            return Ok(());
        }
        if arg == "--json" {
            json = true;
            i += 1;
            continue;
        }
        // `--exclude=dir` and `--exclude dir` are the same.
        let mut inline: Option<&str> = None;
        if arg.starts_with("--") {
            if let Some(eq) = arg.find('=') {
                inline = Some(&arg[eq + 1..]);
                arg = &arg[..eq];
            }
        }
        if arg == "--exclude" || arg == "--spacing-unit" || arg == "--rem" {
            let value = inline.or_else(|| args.get(i + 1).map(String::as_str));
            let value = match value {
                Some(v) if !v.is_empty() && !v.starts_with("--") => v,
                _ => return fail(format!("{arg} needs a value")),
            };
            if inline.is_none() {
                i += 1;
            }
            if arg == "--exclude" {
                exclude.push(value.to_string());
            } else {
                let n = js_number(value);
                if !n.is_finite() || n <= 0.0 {
                    return fail(format!("{arg} must be a positive number (got {value})"));
                }
                if arg == "--rem" {
                    rem_px = Some(n);
                } else {
                    spacing_unit_px = Some(n);
                }
            }
            i += 1;
            continue;
        }
        if arg.starts_with('-') {
            return fail(format!(
                "unknown option \"{arg}\" for check. Run `fundi --help`."
            ));
        }
        paths.push(arg.to_string());
        i += 1;
    }
    let report = match check_project(&CheckOptions {
        paths,
        cwd: io.cwd.clone(),
        exclude,
        spacing_unit_px,
        rem_px,
    }) {
        Ok(r) => r,
        Err(CheckError::Usage(m)) => return fail(m),
        Err(CheckError::Io(e)) => return fail(e.to_string()),
    };
    if json {
        writeln!(io.out, "{}", json_pretty(&report.to_json()))?;
    } else {
        io.out.write_all(render_report(&report).as_bytes())?;
    }
    if !report.ok() {
        return Err(CliError::CheckFailed);
    }
    Ok(())
}

// Re-exports used by tests and other modules.
pub use tokens::{SPACING_LAYERS, is_mzizi_number, next_mzizi_number};
