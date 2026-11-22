//! `add`: registry resolution, target detection, install plans. Port of `registry.ts`,
//! `target.ts`, `install.ts` and the `add` case of `cli.ts`.

pub mod install;
pub mod registry;
pub mod target;

use crate::{CliError, Io, Result};
use install::{apply_plan, build_install_plan, render_plan};
use registry::{Fetch, HttpFetch, available_types, crate_exists, item_url_for, resolve_closure};
use target::{
    Choice, Target, TargetDetection, choose_target, detect_target, parse_target, targets_joined,
};

fn fail<T>(m: impl Into<String>) -> Result<T> {
    Err(CliError::Msg(m.into()))
}
fn e(m: String) -> CliError {
    CliError::Msg(m)
}

/// `mz add <name...>` against the real network.
pub fn cmd_add(args: &[String], io: &mut Io<'_>) -> Result<()> {
    let fetch = HttpFetch::new(&io.env);
    cmd_add_with(args, io, &fetch)
}

/// `mz add <name...>` over any transport.
pub fn cmd_add_with(args: &[String], io: &mut Io<'_>, fetch: &dyn Fetch) -> Result<()> {
    // `--type <v>` / `--target <v>` (or `=v`) takes a value; every other `--x` is a bare
    // flag, and what is left is the component names.
    let mut flags: Vec<&str> = Vec::new();
    let mut names: Vec<&str> = Vec::new();
    let mut requested_raw: Option<&str> = None;
    let mut type_flag_seen = false;
    let mut i = 0;
    while i < args.len() {
        let arg = args[i].as_str();
        let value_flag = ["--type", "--target"].iter().find_map(|f| {
            if arg == *f {
                Some(None)
            } else {
                arg.strip_prefix(f)
                    .and_then(|r| r.strip_prefix('='))
                    .map(Some)
            }
        });
        match value_flag {
            Some(inline) => {
                type_flag_seen = true;
                requested_raw = match inline {
                    Some(v) => Some(v),
                    None => {
                        i += 1;
                        args.get(i).map(String::as_str)
                    }
                };
            }
            None if arg.starts_with("--") => flags.push(arg),
            None => names.push(arg),
        }
        i += 1;
    }
    if names.is_empty() {
        return fail("add requires at least one component name, e.g. `mz add button`");
    }
    let (dry_run, overwrite) = (flags.contains(&"--dry-run"), flags.contains(&"--overwrite"));

    // An explicit wrong value is a usage error, not a silent fallback.
    let mut requested: Option<Target> = None;
    if type_flag_seen {
        requested = parse_target(requested_raw);
        if requested.is_none() {
            let retired = requested_raw.is_some_and(|r| r.eq_ignore_ascii_case("dioxus"));
            return fail(format!(
                "--type must be one of: {} (got {}){}",
                targets_joined(", "),
                requested_raw.unwrap_or("nothing"),
                if retired {
                    ". dioxus is not a type: Mzizi components are Mzizi first, then TypeScript, with native Swift and Kotlin where they exist."
                } else {
                    ""
                }
            ));
        }
    }
    let detected: Option<TargetDetection> = requested.is_none().then(|| detect_target(&io.cwd));

    for name in names {
        let available = available_types(fetch, name).map_err(e)?;
        let (target, reason) = match choose_target(name, &available, requested, detected.as_ref()) {
            Choice::Error(m) => return fail(m),
            Choice::Chosen { target, reason } => (target, reason),
        };
        writeln!(
            io.err,
            "{name}: type {target} ({reason}); available: {}",
            available
                .iter()
                .map(|t| t.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )?;

        // Resolve the whole closure before writing anything.
        let items = resolve_closure(fetch, &item_url_for(name, target), 200).map_err(e)?;
        let plan = build_install_plan(&items, target, &io.cwd).map_err(e)?;
        writeln!(io.out, "{}", render_plan(&plan, name, overwrite))?;

        if dry_run {
            if plan.writes.is_empty() {
                writeln!(
                    io.out,
                    "\nDry run — nothing would be written: this target writes no files."
                )?;
                for c in &plan.crates {
                    writeln!(io.out, "Would tell you to run:\n  cargo add {}", c.name)?;
                }
            } else {
                writeln!(io.out, "\nDry run — nothing was written.")?;
            }
            continue;
        }

        let result = apply_plan(&plan, &io.cwd, overwrite).map_err(e)?;
        writeln!(io.out, "\nWrote {} file(s).", result.written.len())?;
        if !result.skipped.is_empty() {
            write!(
                io.out,
                "Skipped {} existing file(s) — pass --overwrite to replace them:\n{}",
                result.skipped.len(),
                result
                    .skipped
                    .iter()
                    .map(|p| format!("  {p}\n"))
                    .collect::<String>()
            )?;
        }
        if !plan.npm_dependencies.is_empty() {
            writeln!(
                io.out,
                "\nStill to do — add the npm dependencies:\n  npm install {}",
                plan.npm_dependencies.join(" ")
            )?;
        }
        for c in &plan.crates {
            let published = crate_exists(fetch, &c.name);
            if published == Some(false) {
                return fail(format!(
                    "{name} is served as a Rust component that depends on the `{0}` crate, but {0} has never been published to {1}.\n`cargo add {0}` would fail on a name that does not resolve, so there is nothing this tool can honestly tell you to run.\nThis is a publishing gap, not an installer gap: the {2} Rust document(s) are correct and the crate they name does not exist yet. Publish {0} and this command works unchanged.",
                    c.name,
                    c.registry,
                    plan.items.len()
                ));
            }
            write!(
                io.out,
                "\nStill to do — add the crate dependency:\n  cargo add {}\n",
                c.name
            )?;
            if published.is_none() {
                writeln!(
                    io.out,
                    "  (could not reach {} to confirm {} is published)",
                    c.registry, c.name
                )?;
            }
        }
    }
    Ok(())
}
