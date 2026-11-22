//! Argument dispatch. Hand-rolled on purpose: the TypeScript CLI's flag quirks
//! (`--type=v`, `--exclude dir`, unknown `--x` as bare flags) and its exact error
//! strings are the parity target, and a derive parser would change both.

use crate::{CliError, Io, Result, VERSION, add, floor, fundi};

/// The `--help` text, byte for byte the TypeScript CLI's.
pub const HELP: &str = include_str!("help.txt");

/// Run one invocation. `argv` excludes the program name.
pub fn run(argv: &[String], io: &mut Io<'_>) -> Result<()> {
    let Some((command, rest)) = argv.split_first() else {
        io.out.write_all(HELP.as_bytes())?;
        return Ok(());
    };
    match command.as_str() {
        "help" | "--help" | "-h" => io.out.write_all(HELP.as_bytes())?,
        "version" | "--version" | "-v" => writeln!(io.out, "{VERSION}")?,
        "add" => add::cmd_add(rest, io)?,
        "check" | "floor" => floor::cmd_check(rest, io)?,
        "explore" => fundi::cmd_explore(rest, io)?,
        "plan" => fundi::cmd_plan(rest, io)?,
        "chat" => fundi::cmd_chat(rest, io)?,
        "login" => fundi::cmd_login(rest, io)?,
        "logout" => fundi::cmd_logout(rest, io)?,
        "whoami" => fundi::cmd_whoami(rest, io)?,
        other => {
            return Err(CliError::msg(format!(
                "unknown command \"{other}\". Run `fundi --help`."
            )));
        }
    }
    Ok(())
}

/// The process entry point shared by every bin. Returns the exit code.
pub fn main_with(argv: Vec<String>) -> i32 {
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    let (mut out, mut err) = (stdout.lock(), stderr.lock());
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut io = Io { out: &mut out, err: &mut err, cwd, env: crate::Env::from_process() };
    match run(&argv, &mut io) {
        Ok(()) => 0,
        Err(CliError::CheckFailed) => 1,
        Err(CliError::Msg(m)) => {
            use std::io::Write;
            let _ = writeln!(io.err, "error: {m}");
            1
        }
    }
}
