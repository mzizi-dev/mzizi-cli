//! `add`: registry resolution, target detection, install plans. Port of registry.ts, target.ts, install.ts, types.ts.
use crate::{CliError, Io, Result};

/// `mz add <name...>`.
pub fn cmd_add(_args: &[String], _io: &mut Io<'_>) -> Result<()> {
    Err(CliError::msg("add is not ported yet"))
}
