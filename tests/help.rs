//! `--help` and `-h` work for every subcommand, before any positional validation, and print
//! that subcommand's usage on stdout with exit 0 (not the global help, not an error, not a
//! registry lookup). Spawns the real binaries, so the old names are covered too.

use std::process::Command;

const OWN: &[&str] = &[
    "add", "floor", "explore", "plan", "chat", "login", "logout", "whoami",
];
const LANGUAGE: &[&str] = &[
    "check", "fix", "contract", "outline", "hash", "ir", "build", "run", "harness",
];

fn run(bin: &str, args: &[&str]) -> (i32, String, String) {
    let dir = std::env::temp_dir().join("mz-help-test");
    std::fs::create_dir_all(&dir).unwrap();
    let out = Command::new(bin)
        .args(args)
        .current_dir(&dir)
        // A registry lookup or a sign-in must never be reached; point everything nowhere.
        .env("MZ_MCP_URL", "http://127.0.0.1:9/mcp")
        .env_remove("ANTHROPIC_API_KEY")
        .env_remove("WORKOS_ACCESS_TOKEN")
        .output()
        .unwrap();
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).into_owned(),
        String::from_utf8_lossy(&out.stderr).into_owned(),
    )
}

fn global_help() -> String {
    run(env!("CARGO_BIN_EXE_mz"), &["--help"]).1
}

#[test]
fn every_subcommand_prints_its_own_usage_for_both_flags() {
    let global = global_help();
    assert!(global.contains("COMMANDS"));
    for cmd in OWN.iter().chain(LANGUAGE) {
        for flag in ["--help", "-h"] {
            let (code, out, err) = run(env!("CARGO_BIN_EXE_mz"), &[cmd, flag]);
            assert_eq!(code, 0, "mz {cmd} {flag}: exit, stderr: {err}");
            assert!(
                out.contains("usage") || out.contains("USAGE"),
                "mz {cmd} {flag}: {out}"
            );
            assert!(out.contains(cmd), "mz {cmd} {flag} names itself: {out}");
            assert_ne!(out, global, "mz {cmd} {flag} is not the global help");
            assert!(err.is_empty(), "mz {cmd} {flag}: stderr {err}");
        }
    }
}

#[test]
fn help_wins_over_positional_validation_and_flags_before_it() {
    // `add` with no component name used to be an error; with a component name and `-h` it
    // used to look the name up.
    let (code, out, _) = run(env!("CARGO_BIN_EXE_mz"), &["add", "--help"]);
    assert_eq!(code, 0);
    assert!(out.contains("OPTIONS FOR `add`"));
    let (code, out, _) = run(
        env!("CARGO_BIN_EXE_mz"),
        &["add", "button", "--dry-run", "-h"],
    );
    assert_eq!(code, 0);
    assert!(out.contains("mz add <name...>"));
    let (code, out, _) = run(
        env!("CARGO_BIN_EXE_mz"),
        &["floor", "--json", "src", "--help"],
    );
    assert_eq!(code, 0);
    assert!(out.contains("OPTIONS FOR `floor`"));
    // After `--`, `-h` is an operand, not a flag: it must not turn into help.
    let (_, out, _) = run(env!("CARGO_BIN_EXE_mz"), &["floor", "--", "-h"]);
    assert!(!out.contains("USAGE"));
}

#[test]
fn the_old_names_behave_the_same() {
    let (code, out, err) = run(env!("CARGO_BIN_EXE_mzizi"), &["floor", "--help"]);
    assert_eq!(code, 0);
    assert!(out.contains("OPTIONS FOR `floor`"));
    assert!(
        err.contains("`mzizi` is now `mz`"),
        "the deprecation note still prints: {err}"
    );
    // Under the old names `check` still means `floor`.
    let (code, out, _) = run(env!("CARGO_BIN_EXE_mzizi"), &["check", "-h"]);
    assert_eq!(code, 0);
    assert!(out.contains("OPTIONS FOR `floor`"));
    let (code, out, _) = run(env!("CARGO_BIN_EXE_fundi"), &["plan", "--help"]);
    assert_eq!(code, 0);
    assert!(out.contains("mz plan <goal...>"));
    let (code, out, _) = run(env!("CARGO_BIN_EXE_fundi"), &["add", "-h"]);
    assert_eq!(code, 0);
    assert!(out.contains("mz add <name...>"));
}

#[test]
fn bare_help_and_version_are_unchanged() {
    let (code, out, _) = run(env!("CARGO_BIN_EXE_mz"), &["help"]);
    assert_eq!(code, 0);
    assert!(out.contains("COMMANDS"));
    let (code, out, _) = run(env!("CARGO_BIN_EXE_mz"), &["-v"]);
    assert_eq!(code, 0);
    assert_eq!(out.trim(), env!("CARGO_PKG_VERSION"));
}
