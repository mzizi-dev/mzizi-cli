//! The Bundu floor check (TS `check`): spacing and leading. Port of check.ts, check-spacing.ts, spacing-floor.ts.
use crate::{CliError, Io, Result};

/// `mz floor [path...]` (and `check` under the old bin names).
pub fn cmd_check(_args: &[String], _io: &mut Io<'_>) -> Result<()> {
    Err(CliError::msg("floor is not ported yet"))
}
