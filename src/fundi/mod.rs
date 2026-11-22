//! The fundi agent: explore, plan, chat, sign-in. Port of explore.ts, context-gather.ts, prompts.ts, plan.ts, client.ts, tools.ts, auth.ts, cli-token-store.ts.
use crate::{CliError, Io, Result};

macro_rules! stub {
    ($($n:ident),*) => {$(
        /// Command entry point.
        pub fn $n(_args: &[String], _io: &mut Io<'_>) -> Result<()> {
            Err(CliError::msg(concat!(stringify!($n), " is not ported yet")))
        }
    )*};
}
stub!(cmd_explore, cmd_plan, cmd_chat, cmd_login, cmd_logout, cmd_whoami);
