//! The fundi agent: `explore`, `plan`, `chat` and sign-in. Port of explore.ts,
//! context-gather.ts, prompts.ts, plan.ts, client.ts, tools.ts, auth.ts and
//! cli-token-store.ts, and the matching cases of cli.ts.
//!
//! Environment: `MZ_MCP_URL`, `MZ_MODEL`, `MZ_AUTH_TOKEN`; the TypeScript CLI's
//! `MZIZI_*` names still work and warn once on stderr. `WORKOS_ACCESS_TOKEN`,
//! `ANTHROPIC_API_KEY` and `ANTHROPIC_BASE_URL` are external names and unchanged.

pub mod anthropic;
pub mod auth;
pub mod client;
pub mod context;
pub mod explore;
pub mod mcp;
pub mod plan;
pub mod prompts;
pub mod skills;
mod skills_data;
pub mod token_store;
pub mod tools;

use serde_json::Value;

use crate::{CliError, Io, Result};
use auth::{env_with_fallback, get_identity};
use token_store::{Saved, StoredAuth, TokenStore};

fn fail<T>(m: impl Into<String>) -> Result<T> {
    Err(CliError::msg(m))
}

fn pretty(v: &Value) -> String {
    format!(
        "{}\n",
        serde_json::to_string_pretty(v).expect("a Value always serialises")
    )
}

/// `new Date(secs * 1000).toISOString()`.
pub fn iso_from_epoch_secs(secs: f64) -> String {
    let ms_total = (secs * 1000.0).round() as i64;
    let (days, ms_of_day) = (
        ms_total.div_euclid(86_400_000),
        ms_total.rem_euclid(86_400_000),
    );
    // Howard Hinnant's civil_from_days.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    let (h, mi, s, ms) = (
        ms_of_day / 3_600_000,
        ms_of_day / 60_000 % 60,
        ms_of_day / 1000 % 60,
        ms_of_day % 1000,
    );
    format!("{y:04}-{m:02}-{d:02}T{h:02}:{mi:02}:{s:02}.{ms:03}Z")
}

/// `mz explore`: print the project snapshot (offline).
pub fn cmd_explore(_args: &[String], io: &mut Io<'_>) -> Result<()> {
    let snap = explore::explore_project(&io.cwd, ".");
    io.out.write_all(pretty(&snap.to_json()).as_bytes())?;
    Ok(())
}

fn api_key(io: &Io<'_>) -> Option<String> {
    io.env
        .get("ANTHROPIC_API_KEY")
        .filter(|k| !k.is_empty())
        .map(str::to_string)
}

fn client_options(io: &mut Io<'_>, key: &str) -> client::FundiOptions {
    let mut o = client::FundiOptions::new(&io.cwd, key);
    if let Some(u) = io.env.get("ANTHROPIC_BASE_URL").filter(|u| !u.is_empty()) {
        o.anthropic_base = u.to_string();
    }
    if let Some(u) = env_with_fallback(&io.env, "MZ_MCP_URL", "MZIZI_MCP_URL", io.err) {
        o.mcp_url = u;
    }
    if let Some(m) = env_with_fallback(&io.env, "MZ_MODEL", "MZIZI_MODEL", io.err) {
        o.model = m;
    }
    o
}

/// `mz plan <goal...> [--full] | --skill <name>`.
pub fn cmd_plan(args: &[String], io: &mut Io<'_>) -> Result<()> {
    let skill_idx = args.iter().position(|a| a == "--skill");
    let skill_name = skill_idx.and_then(|i| args.get(i + 1));
    if skill_idx.is_some() && skill_name.is_none_or(|n| n.starts_with("--")) {
        return fail("--skill needs a skill name, e.g. `mz plan --skill mzizi-design`");
    }
    let full = args.iter().any(|a| a == "--full");
    let goal = args
        .iter()
        .enumerate()
        .filter(|(i, a)| {
            *a != "--full"
                && *a != "--skill"
                && !(skill_idx.is_some() && Some(*i) == skill_idx.map(|s| s + 1))
        })
        .map(|(_, a)| a.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let goal = goal.trim();

    // A named skill is a lookup, not planning: no goal, key or snapshot needed.
    if let Some(name) = skill_name {
        let skills = skills::load_mzizi_skills();
        let Some(found) = skills.iter().find(|s| &s.name == name) else {
            let names = skills
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(", ");
            return fail(format!("no skill named \"{name}\". Available: {names}"));
        };
        writeln!(io.out, "{}", found.body.trim_end())?;
        return Ok(());
    }
    if goal.is_empty() {
        return fail("plan requires a goal, e.g. mz plan \"add the token layer\"");
    }
    let Some(key) = api_key(io) else {
        let snap = explore::explore_project(&io.cwd, ".");
        let ctx = context::gather_context(goal, snap);
        let v = if full {
            context::full_context(&ctx)
        } else {
            context::summarise_context(&ctx)
        };
        io.out.write_all(pretty(&v).as_bytes())?;
        return Ok(());
    };
    let opts = client_options(io, &key);
    let mut fundi = client::FundiClient::create(opts, &io.env, io.err).map_err(CliError::Msg)?;
    let snap = fundi.explore();
    let plan = fundi.plan(goal, &snap).map_err(CliError::Msg)?;
    let preview = plan.apply(true);
    write!(io.out, "Goal: {}\n\n", plan.goal)?;
    for r in &preview.results {
        write!(
            io.out,
            "- [{}] {}\n    {}\n",
            r.step.kind(),
            r.step.description(),
            r.detail
        )?;
    }
    write!(
        io.out,
        "\n{} step(s) — dry run, nothing was written.\n",
        preview.results.len()
    )?;
    Ok(())
}

/// `mz chat <message...>`.
pub fn cmd_chat(args: &[String], io: &mut Io<'_>) -> Result<()> {
    let message = args.join(" ");
    let message = message.trim();
    if message.is_empty() {
        return fail("chat requires a message, e.g. mz chat \"…\"");
    }
    let Some(key) = api_key(io) else {
        io.out.write_all(
            "No ANTHROPIC_API_KEY set.\n\n\
mz chat is a standalone conversational client — if a coding agent\n\
is already running this command, ask it directly instead; it already\n\
has model access and the context of what you're working on.\n\n\
To use mz chat standalone, set ANTHROPIC_API_KEY.\n"
                .as_bytes(),
        )?;
        return Ok(());
    };
    let opts = client_options(io, &key);
    let mut fundi = client::FundiClient::create(opts, &io.env, io.err).map_err(CliError::Msg)?;
    let reply = fundi.chat(message).map_err(CliError::Msg)?;
    writeln!(io.out, "{reply}")?;
    Ok(())
}

/// `mz login [--token <jwt>]`.
pub fn cmd_login(args: &[String], io: &mut Io<'_>) -> Result<()> {
    let mut token = io
        .env
        .get_trimmed("WORKOS_ACCESS_TOKEN")
        .map(str::to_string);
    if let Some(i) = args.iter().position(|a| a == "--token") {
        token = args
            .get(i + 1)
            .map(|t| t.trim().to_string())
            .filter(|t| !t.is_empty());
    }
    let Some(token) = token else {
        return fail(
            "login needs a token: `mz login --token <jwt>`, or set WORKOS_ACCESS_TOKEN.\n\
Only the Fundi tools need one; everything else in the CLI and mzizi-mcp is free\n\
with no sign-in. Get a token by signing in to the Mzizi console at\n\
https://app.mzizi.dev.",
        );
    };
    let identity = get_identity(&token)
        .map_err(|e| CliError::msg(format!("that token is not usable: {e}")))?;
    let store = TokenStore::from_env(&io.env);
    let saved_at = iso_from_epoch_secs(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0.0, |d| d.as_secs_f64()),
    );
    let saved = store
        .write(&StoredAuth {
            access_token: token,
            refresh_token: None,
            saved_at,
        })
        .map_err(|e| CliError::msg(e.to_string()))?;
    let who = identity
        .email
        .clone()
        .unwrap_or_else(|| identity.sub.clone());
    let where_ = match saved {
        Saved::Keychain => "the OS keychain (service \"mz\") ".to_string(),
        Saved::File(p) => format!("{} (mode 0600)", p.display()),
    };
    write!(
        io.out,
        "Saved token for {who} to {}.\nExpires {}.\n",
        where_.trim_end(),
        iso_from_epoch_secs(identity.exp)
    )?;
    Ok(())
}

/// `mz logout`.
pub fn cmd_logout(_args: &[String], io: &mut Io<'_>) -> Result<()> {
    let store = TokenStore::from_env(&io.env);
    let (keychain, file, legacy) = store.clear();
    if file || legacy {
        let p = if file {
            store.path()
        } else {
            store.legacy_path()
        };
        writeln!(io.out, "Removed {}.", p.display())?;
    } else if keychain {
        writeln!(io.out, "Removed the saved token from the OS keychain.")?;
    } else {
        writeln!(
            io.out,
            "No saved token at {} — nothing to remove.",
            store.path().display()
        )?;
    }
    Ok(())
}

/// `mz whoami`.
pub fn cmd_whoami(_args: &[String], io: &mut Io<'_>) -> Result<()> {
    let store = TokenStore::from_env(&io.env);
    let token = match store.read(io.err) {
        Some(a) => Some(a.access_token),
        None => io
            .env
            .get_trimmed("WORKOS_ACCESS_TOKEN")
            .map(str::to_string)
            .or_else(|| env_with_fallback(&io.env, "MZ_AUTH_TOKEN", "MZIZI_AUTH_TOKEN", io.err)),
    };
    let Some(token) = token else {
        return fail(format!(
            "not signed in — no token at {} and neither WORKOS_ACCESS_TOKEN nor MZ_AUTH_TOKEN is set. Run `mz login`.",
            store.path().display()
        ));
    };
    let identity =
        get_identity(&token).map_err(|e| CliError::msg(format!("{e} — run `mz login` again.")))?;
    io.out.write_all(pretty(&identity.to_json()).as_bytes())?;
    Ok(())
}
