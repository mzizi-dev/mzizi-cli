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
    // `-h` and `--help` before anything else is read: positional checks never see them.
    if crate::usage::wants_help(rest) && !matches!(command.as_str(), "help" | "version") {
        let name = match command.as_str() {
            "check" if matches!(io.env.get("MZ_INVOKED_AS"), Some("mzizi" | "fundi")) => "floor",
            c => c,
        };
        if let Some(text) = crate::usage::for_command(name) {
            io.out.write_all(text.as_bytes())?;
            return Ok(());
        }
    }
    match command.as_str() {
        "help" | "--help" | "-h" => io.out.write_all(HELP.as_bytes())?,
        "version" | "--version" | "-v" => writeln!(io.out, "{VERSION}")?,
        "add" => add::cmd_add(rest, io)?,
        "floor" => floor::cmd_check(rest, io)?,
        "check" if matches!(io.env.get("MZ_INVOKED_AS"), Some("mzizi" | "fundi")) => {
            floor::cmd_check(rest, io)?
        }
        "explore" => fundi::cmd_explore(rest, io)?,
        "plan" => fundi::cmd_plan(rest, io)?,
        "chat" => fundi::cmd_chat(rest, io)?,
        "login" => fundi::cmd_login(rest, io)?,
        "logout" => fundi::cmd_logout(rest, io)?,
        "whoami" => fundi::cmd_whoami(rest, io)?,
        // Everything else is the language toolchain's: check, fix, contract, outline, hash,
        // ir, build, run, harness, or a bare path (which means check).
        _ => return lang_command(argv),
    }
    Ok(())
}

/// Hand a command to the language driver and carry its exit status. The driver prints its
/// own output and returns a process exit code.
fn lang_command(argv: &[String]) -> Result<()> {
    let code = crate::lang::run_command(argv.to_vec());
    // `ExitCode` is opaque: its Debug form is `ExitCode(unix_exit_status(N))` (or a platform
    // variant), so read the status back from that.
    let dbg = format!("{code:?}");
    let n: u8 = dbg
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .unwrap_or(0);
    if n == 0 {
        Ok(())
    } else {
        Err(CliError::Exit(n))
    }
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
        Err(CliError::Exit(c)) => i32::from(c),
        Err(CliError::Msg(m)) => {
            let _ = writeln!(io.err, "error: {m}");
            1
        }
    }
}
