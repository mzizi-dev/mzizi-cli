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
        "floor" => floor::cmd_check(rest, io)?,
        "check" if matches!(io.env.get("MZ_INVOKED_AS"), Some("mzizi" | "fundi")) => {
            floor::cmd_check(rest, io)?
        }
        "check" => {
            return Err(CliError::msg(
                "`mz check` is the language compiler's command and arrives with it. \
                 For the Bundu spacing floor use `mz floor`.",
            ));
        }
        "explore" => fundi::cmd_explore(rest, io)?,
        "plan" => fundi::cmd_plan(rest, io)?,
        "chat" => fundi::cmd_chat(rest, io)?,
        "login" => fundi::cmd_login(rest, io)?,
        "logout" => fundi::cmd_logout(rest, io)?,
        "whoami" => fundi::cmd_whoami(rest, io)?,
        other => {
            return Err(CliError::msg(format!(
                "unknown command \"{other}\". Run `mz --help`."
            )));
        }
    }
    Ok(())
}

/// The process entry point shared by every bin. Returns the exit code.
pub fn main_with(invoked_as: &str, argv: Vec<String>) -> i32 {
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    let (mut out, mut err) = (stdout.lock(), stderr.lock());
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut io = Io {
        out: &mut out,
        err: &mut err,
        cwd,
        env: crate::Env::from_process(),
    };
    // The npm launcher runs `mz` for every bin name and says which one was typed.
    let invoked_as = match io.env.get("MZ_INVOKED_AS") {
        Some(n) if invoked_as == "mz" && matches!(n, "mzizi" | "fundi") => n.to_string(),
        _ => invoked_as.to_string(),
    };
    io.env.0.insert("MZ_INVOKED_AS".into(), invoked_as.clone());
    if invoked_as != "mz" {
        let _ = writeln!(
            io.err,
            "note: `{invoked_as}` is now `mz`; this name stays for one release."
        );
    }
    match run(&argv, &mut io) {
        Ok(()) => 0,
        Err(CliError::CheckFailed) => 1,
        Err(CliError::Msg(m)) => {
            let _ = writeln!(io.err, "error: {m}");
            1
        }
    }
}
