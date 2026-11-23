//! Per-command `--help`. The text comes from `help.txt` (one source of truth for the
//! registry, floor and fundi commands) and from the language harness's command table (for
//! `check`, `fix`, `contract` and the rest), so no usage is written twice.

use crate::cli::HELP;
use mzizi_lang_compiler::harness;

/// Whether `args` ask for help: `-h` or `--help` before any `--` terminator.
pub fn wants_help(args: &[String]) -> bool {
    args.iter()
        .take_while(|a| a.as_str() != "--")
        .any(|a| a == "-h" || a == "--help")
}

/// Every command `mz` knows, in the order the help lists them.
pub const COMMANDS: &[&str] = &[
    "add", "floor", "explore", "plan", "chat", "login", "logout", "whoami", "help", "version",
];

/// The usage text for one command, or `None` when `command` is not one we know.
pub fn for_command(command: &str) -> Option<String> {
    if let Some(text) = own(command) {
        return Some(text);
    }
    if command == "harness" {
        return Some(
            "usage: mz harness version | definition [--agent] | entry <name> [--agent]\n".into(),
        );
    }
    let c = harness::COMMANDS.iter().find(|c| c.word == command)?;
    Some(format!("usage: {}\n\n{}\n", c.usage, c.teach))
}

/// The commands the language toolchain owns (they print their own usage).
pub fn language_words() -> Vec<&'static str> {
    let mut v: Vec<&'static str> = harness::COMMANDS.iter().map(|c| c.word).collect();
    v.push("harness");
    v
}

/// A command from `help.txt`: its line in COMMANDS (with continuation lines) and its
/// `OPTIONS FOR` section, if any.
fn own(command: &str) -> Option<String> {
    let name = match command {
        "help" | "--help" | "-h" => "help,",
        "version" | "--version" | "-v" => "version,",
        c => c,
    };
    let lines: Vec<&str> = HELP.lines().collect();
    let start = lines.iter().position(|l| {
        l.strip_prefix("  ").is_some_and(|r| {
            !r.starts_with(' ') && r.starts_with(name) && {
                let rest = &r[name.len()..];
                rest.is_empty() || rest.starts_with([' ', ',', '\t'])
            }
        })
    })?;
    let mut out = String::from("USAGE\n");
    let mut i = start;
    out.push_str(&format!("  mz {}\n\n", usage_line(lines[start])));
    // The description: the entry's own line, then its continuation lines.
    out.push_str(lines[i]);
    out.push('\n');
    i += 1;
    while i < lines.len() && lines[i].starts_with("                           ") {
        out.push_str(lines[i]);
        out.push('\n');
        i += 1;
    }
    let header = format!("OPTIONS FOR `{command}`");
    if let Some(s) = lines.iter().position(|l| *l == header) {
        out.push('\n');
        let mut j = s;
        while j < lines.len() && !(j > s && lines[j].is_empty()) {
            out.push_str(lines[j]);
            out.push('\n');
            j += 1;
        }
    }
    Some(out)
}

/// `add <name...>` from `  add <name...>            Install ...`: the command and its
/// arguments, before the run of spaces that starts the description.
fn usage_line(entry: &str) -> String {
    let body = entry.trim_start();
    let head = match body.find("  ") {
        Some(i) => body[..i].trim_end(),
        None => body.trim_end(),
    };
    head.replace(", ", " | ")
}
