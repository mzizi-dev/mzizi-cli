//! The Mzizi CLI: `mz add`, the Bundu floor check, `explore`, and the fundi plan agent.
//!
//! One binary, `mz`. `mzizi` and `fundi` (the TypeScript CLI's old bin names) are the
//! same program under their old names. Everything here is a port of
//! `@nyuchi/mzizi-cli` 0.13.0 with no behaviour change; the TypeScript source is the
//! specification and each Rust test names the vitest case it replaces.
#![forbid(unsafe_code)]

pub mod add;
pub mod cli;
pub mod floor;
pub mod fundi;

use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

/// The CLI's version, read from Cargo at compile time (never restated).
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A process environment, injectable so tests never touch the real one.
#[derive(Debug, Clone, Default)]
pub struct Env(pub HashMap<String, String>);

impl Env {
    /// The real process environment.
    pub fn from_process() -> Self {
        Env(std::env::vars().collect())
    }
    /// A variable, or `None` when it is unset.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }
    /// A variable trimmed, or `None` when unset or blank.
    pub fn get_trimmed(&self, key: &str) -> Option<&str> {
        self.get(key).map(str::trim).filter(|v| !v.is_empty())
    }
}

/// Everything a command touches outside its arguments: streams, working directory, environment.
pub struct Io<'a> {
    /// Standard output.
    pub out: &'a mut dyn Write,
    /// Standard error.
    pub err: &'a mut dyn Write,
    /// The working directory (the project root).
    pub cwd: PathBuf,
    /// The environment.
    pub env: Env,
}

/// Why a command stopped.
#[derive(Debug)]
pub enum CliError {
    /// A user-facing error: printed as `error: <message>` on stderr, exit 1.
    Msg(String),
    /// The floor check found violations. The report is already on stdout; exit 1, print nothing.
    CheckFailed,
}

impl CliError {
    /// Build a [`CliError::Msg`].
    pub fn msg(m: impl Into<String>) -> Self {
        CliError::Msg(m.into())
    }
}

impl std::fmt::Display for CliError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CliError::Msg(m) => f.write_str(m),
            CliError::CheckFailed => f.write_str("check failed"),
        }
    }
}

impl std::error::Error for CliError {}

impl From<std::io::Error> for CliError {
    fn from(e: std::io::Error) -> Self {
        CliError::Msg(e.to_string())
    }
}

/// A command result.
pub type Result<T> = std::result::Result<T, CliError>;
