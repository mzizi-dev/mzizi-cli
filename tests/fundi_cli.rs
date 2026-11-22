//! CLI-level tests for explore, plan, chat and sign-in: port of cli-plan-add-output.test.ts
//! (the plan half), plus the token store, the MZ_/MZIZI_ fallbacks and an end-to-end planner
//! loop against fixture servers. No live network, no real keychain.
#![allow(missing_docs)]

mod fundi_support;

use std::cell::RefCell;
use std::rc::Rc;

use fundi_support::*;
use mzizi_cli::Env;
use mzizi_cli::fundi::explore::explore_project;
use mzizi_cli::fundi::iso_from_epoch_secs;
use mzizi_cli::fundi::plan::{Plan, Step};
use mzizi_cli::fundi::token_store::{Keychain, Saved, StoredAuth, TokenStore};
use serde_json::{Value, json};

fn app_dir() -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    std::fs::write(d.path().join("package.json"), r#"{"name":"app"}"#).unwrap();
    d
}

// ---- cli-plan-add-output.test.ts: `mzizi plan`, no ANTHROPIC_API_KEY (#281) ----

#[test]
fn plan_without_a_key_prints_a_summary_not_skill_bodies() {
    let d = app_dir();
    let o = run_cli(
        &["plan", "improve", "discoverability", "for", "AI", "agents"],
        &[],
        d.path(),
    );
    o.result.unwrap();
    let ctx: Value = serde_json::from_str(&o.out).unwrap();
    let skills = ctx["relevantSkills"].as_array().unwrap();
    assert!(!skills.is_empty());
    for s in skills {
        assert!(s["name"].is_string());
        let desc = s["description"].as_str().unwrap();
        assert!(!desc.is_empty() && desc.chars().count() <= 160 && !desc.contains('\n'));
        assert!(s.get("body").is_none());
    }
    assert!(
        ctx["howToFetch"]
            .as_str()
            .unwrap()
            .contains("--skill <name>")
    );
    assert!(ctx["howToFetch"].as_str().unwrap().contains("--full"));
    assert!(o.out.len() < 20_000);
}

#[test]
fn plan_full_prints_matching_skills_in_full() {
    let d = app_dir();
    let o = run_cli(
        &["plan", "improve", "discoverability", "--full"],
        &[],
        d.path(),
    );
    o.result.unwrap();
    let ctx: Value = serde_json::from_str(&o.out).unwrap();
    let s = ctx["relevantSkills"]
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["name"] == "discoverability")
        .unwrap();
    assert!(s["body"].as_str().unwrap().len() > 1000);
}

#[test]
fn plan_skill_prints_one_skill_without_a_goal() {
    let d = app_dir();
    let o = run_cli(&["plan", "--skill", "discoverability"], &[], d.path());
    o.result.unwrap();
    assert!(o.out.starts_with("---\nname: discoverability"));
    assert!(o.out.len() > 1000);
}

#[test]
fn plan_skill_with_an_unknown_name_lists_the_available_ones() {
    let d = app_dir();
    let e = run_cli(&["plan", "--skill", "nope"], &[], d.path())
        .result
        .unwrap_err();
    let m = e.to_string();
    assert!(
        m.starts_with("no skill named \"nope\". Available: ") && m.contains("discoverability"),
        "{m}"
    );
}

#[test]
fn plan_skill_without_a_name_and_plan_without_a_goal_are_usage_errors() {
    let d = app_dir();
    assert_eq!(
        run_cli(&["plan", "--skill"], &[], d.path())
            .result
            .unwrap_err()
            .to_string(),
        "--skill needs a skill name, e.g. `mz plan --skill mzizi-design`"
    );
    assert!(
        run_cli(&["plan"], &[], d.path())
            .result
            .unwrap_err()
            .to_string()
            .starts_with("plan requires a goal")
    );
}

// ---- explore ----

#[test]
fn explore_prints_the_snapshot_as_json_in_the_ts_key_order() {
    let o = run_cli(&["explore"], &[], &fixture_root());
    o.result.unwrap();
    let v: Value = serde_json::from_str(&o.out).unwrap();
    let keys: Vec<&str> = v.as_object().unwrap().keys().map(String::as_str).collect();
    assert_eq!(
        keys,
        [
            "root",
            "packageJson",
            "hasTsconfig",
            "hasGlobalsCss",
            "hasComponentsJson",
            "notes"
        ]
    );
    assert!(o.out.ends_with("}\n") && o.out.contains("\n  \"root\": "));
}

// ---- chat ----

#[test]
fn chat_without_a_key_points_at_the_running_agent() {
    let d = app_dir();
    let o = run_cli(&["chat", "hello"], &[], d.path());
    o.result.unwrap();
    assert!(
        o.out.starts_with(
            "No ANTHROPIC_API_KEY set.\n\nmz chat is a standalone conversational client"
        )
    );
    assert!(run_cli(&["chat"], &[], d.path()).result.is_err());
}

fn anthropic_text(text: &str) -> Resp {
    Resp::json(
        &json!({"content": [{"type": "text", "text": text}], "stop_reason": "end_turn"})
            .to_string(),
    )
}

#[test]
fn chat_with_a_key_calls_the_messages_api_with_the_model_and_system_prompt() {
    let d = app_dir();
    let api = serve(|_| anthropic_text("  Hello there  "));
    let o = run_cli(
        &["chat", "hi"],
        &[("ANTHROPIC_API_KEY", "k"), ("ANTHROPIC_BASE_URL", &api.url)],
        d.path(),
    );
    o.result.unwrap();
    assert_eq!(o.out, "Hello there\n");
    let req = &api.seen()[0];
    assert_eq!(req.path, "/v1/messages");
    assert_eq!(req.headers["x-api-key"], "k");
    assert_eq!(req.headers["anthropic-version"], "2023-06-01");
    let b: Value = serde_json::from_str(&req.body).unwrap();
    assert_eq!(b["model"], "claude-sonnet-5");
    assert_eq!(b["max_tokens"], 16000);
    assert_eq!(b["messages"], json!([{"role": "user", "content": "hi"}]));
    assert_eq!(
        b["system"][0]["cache_control"],
        json!({"type": "ephemeral"})
    );
    assert!(
        b["system"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("You are Fundi")
    );
}

// ---- env: MZ_ names, deprecated MZIZI_ fallbacks ----

#[test]
fn mz_model_is_used_and_does_not_warn() {
    let d = app_dir();
    let api = serve(|_| anthropic_text("ok"));
    let o = run_cli(
        &["chat", "hi"],
        &[
            ("ANTHROPIC_API_KEY", "k"),
            ("ANTHROPIC_BASE_URL", &api.url),
            ("MZ_MODEL", "model-new"),
        ],
        d.path(),
    );
    o.result.unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&api.seen()[0].body).unwrap()["model"],
        "model-new"
    );
    assert_eq!(o.err, "");
}

#[test]
fn deprecated_mzizi_model_still_works_with_a_one_line_warning() {
    let d = app_dir();
    let api = serve(|_| anthropic_text("ok"));
    let o = run_cli(
        &["chat", "hi"],
        &[
            ("ANTHROPIC_API_KEY", "k"),
            ("ANTHROPIC_BASE_URL", &api.url),
            ("MZIZI_MODEL", "model-old"),
        ],
        d.path(),
    );
    o.result.unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&api.seen()[0].body).unwrap()["model"],
        "model-old"
    );
    assert_eq!(o.err, "warning: MZIZI_MODEL is deprecated, use MZ_MODEL\n");
}

#[test]
fn new_model_name_wins_over_the_deprecated_one() {
    let d = app_dir();
    let api = serve(|_| anthropic_text("ok"));
    let o = run_cli(
        &["chat", "hi"],
        &[
            ("ANTHROPIC_API_KEY", "k"),
            ("ANTHROPIC_BASE_URL", &api.url),
            ("MZ_MODEL", "new"),
            ("MZIZI_MODEL", "old"),
        ],
        d.path(),
    );
    o.result.unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&api.seen()[0].body).unwrap()["model"],
        "new"
    );
    assert_eq!(o.err, "");
}

#[test]
fn an_invalid_workos_token_is_ignored_with_the_ts_warning() {
    let d = app_dir();
    let api = serve(|_| anthropic_text("ok"));
    let o = run_cli(
        &["chat", "hi"],
        &[
            ("ANTHROPIC_API_KEY", "k"),
            ("ANTHROPIC_BASE_URL", &api.url),
            ("WORKOS_ACCESS_TOKEN", "garbage"),
        ],
        d.path(),
    );
    o.result.unwrap();
    assert_eq!(
        o.err,
        "mzizi-sdk: ignoring invalid WorkOS token (invalid WorkOS access token: Invalid JWT)\n"
    );
}

// ---- end to end: the planner loop against fixture servers ----

fn planner_api(dir_listing_seen: std::sync::Arc<std::sync::Mutex<Vec<String>>>) -> Server {
    serve(move |req| {
        let b: Value = serde_json::from_str(&req.body).unwrap();
        let msgs = b["messages"].as_array().unwrap();
        if msgs.len() == 1 {
            return Resp::json(&json!({
                "stop_reason": "tool_use",
                "content": [{"type": "tool_use", "id": "tu_1", "name": "list_dir", "input": {"path": "."}}],
            }).to_string());
        }
        let result = &msgs[2]["content"][0];
        dir_listing_seen
            .lock()
            .unwrap()
            .push(result["content"].as_str().unwrap().to_string());
        assert_eq!(result["type"], "tool_result");
        assert_eq!(result["tool_use_id"], "tu_1");
        let plan = json!([
            {"kind": "install-component", "description": "add Button", "params": {"components": ["button"]}},
            {"kind": "create-file", "description": "tokens", "params": {"path": "app/tokens.css", "contents": "é"}},
            {"kind": "run-command", "description": "lint", "params": {"argv": ["pnpm", "lint"], "cwd": "web"}},
        ]);
        anthropic_text(&format!("```plan\n{plan}\n```"))
    })
}

#[test]
fn plan_with_a_key_runs_the_tool_loop_and_prints_a_dry_run() {
    let d = app_dir();
    let seen = Default::default();
    let api = planner_api(std::sync::Arc::clone(&seen));
    let o = run_cli(
        &["plan", "add", "a", "button"],
        &[("ANTHROPIC_API_KEY", "k"), ("ANTHROPIC_BASE_URL", &api.url)],
        d.path(),
    );
    o.result.unwrap();
    assert_eq!(
        o.out,
        "Goal: add a button\n\n\
- [install-component] add Button\n    [dry-run] would run: npx shadcn@latest add https://api.mzizi.dev/v1/ui/button\n\
- [create-file] tokens\n    [dry-run] would create-file app/tokens.css (1 bytes)\n\
- [run-command] lint\n    [dry-run] would run: pnpm lint (cwd=web)\n\
\n3 step(s) — dry run, nothing was written.\n"
    );
    assert_eq!(seen.lock().unwrap()[0], "package.json");
    assert!(!d.path().join("app").exists(), "a dry run writes nothing");
    let first: Value = serde_json::from_str(&api.seen()[0].body).unwrap();
    assert_eq!(first["tools"].as_array().unwrap().len(), 6);
    assert!(
        first["messages"][0]["content"][0]["text"]
            .as_str()
            .unwrap()
            .starts_with("Goal: add a button\n\nCurrent project snapshot:\nProject root: ")
    );
}

#[test]
fn plan_errors_when_the_planner_never_stops_calling_tools() {
    let d = app_dir();
    let api = serve(|_| {
        Resp::json(&json!({"stop_reason": "tool_use", "content": [{"type": "tool_use", "id": "t", "name": "list_dir", "input": {"path": "."}}]}).to_string())
    });
    let e = run_cli(
        &["plan", "x"],
        &[("ANTHROPIC_API_KEY", "k"), ("ANTHROPIC_BASE_URL", &api.url)],
        d.path(),
    )
    .result
    .unwrap_err();
    assert_eq!(
        e.to_string(),
        "Fundi planner exhausted iterations without producing a plan"
    );
    assert_eq!(api.seen().len(), 10);
}

#[test]
fn apply_writes_for_real_only_inside_the_project_root() {
    let d = app_dir();
    let snap = explore_project(d.path(), ".");
    let api = serve(|req| {
        let b: Value = serde_json::from_str(&req.body).unwrap();
        if !b["messages"].as_array().unwrap().is_empty() {
            let plan = json!([
                {"kind": "create-file", "description": "ok", "params": {"path": "a/b.txt", "contents": "hi"}},
                {"kind": "create-file", "description": "escape", "params": {"path": "../x.txt", "contents": "no"}},
            ]);
            return anthropic_text(&format!("```plan\n{plan}\n```"));
        }
        unreachable!()
    });
    let mut opts = mzizi_cli::fundi::client::FundiOptions::new(d.path(), "k");
    opts.anthropic_base = api.url.clone();
    let mut sink = Vec::new();
    let mut c =
        mzizi_cli::fundi::client::FundiClient::create(opts, &Env::default(), &mut sink).unwrap();
    let plan: Plan = c.plan("g", &snap).unwrap();
    let r = plan.apply(false);
    assert!(r.results[0].applied && r.results[0].detail == "wrote a/b.txt (2 bytes)");
    assert_eq!(
        std::fs::read_to_string(d.path().join("a/b.txt")).unwrap(),
        "hi"
    );
    assert!(!r.results[1].applied && r.results[1].error.as_deref() == Some("path-escape"));
    assert!(matches!(plan.steps[1], Step::CreateFile { .. }));
}

// ---- login / logout / whoami and the token store ----

fn token(extra: Value) -> String {
    let mut c = json!({"sub": "user_1", "email": "a@b.example", "exp": now() + 3600});
    c.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    make_jwt(c)
}

fn cfg_env(cfg: &str) -> Vec<(&str, &str)> {
    vec![("XDG_CONFIG_HOME", cfg), ("MZ_NO_KEYCHAIN", "1")]
}

#[test]
fn login_whoami_logout_round_trip_through_the_file_store() {
    let (d, cfg) = (app_dir(), tempfile::tempdir().unwrap());
    let c = cfg.path().to_str().unwrap();
    let t = token(json!({}));
    let o = run_cli(&["login", "--token", &t], &cfg_env(c), d.path());
    o.result.unwrap();
    let file = cfg.path().join("mz").join("auth.json");
    assert!(
        o.out.starts_with(&format!(
            "Saved token for a@b.example to {} (mode 0600).\nExpires ",
            file.display()
        )),
        "{}",
        o.out
    );
    let saved: Value = serde_json::from_str(&std::fs::read_to_string(&file).unwrap()).unwrap();
    assert_eq!(saved["accessToken"], t.as_str());
    assert!(saved["savedAt"].as_str().unwrap().ends_with('Z'));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(&file).unwrap().permissions().mode() & 0o777,
            0o600
        );
    }
    let who = run_cli(&["whoami"], &cfg_env(c), d.path());
    who.result.unwrap();
    let v: Value = serde_json::from_str(&who.out).unwrap();
    assert_eq!(v["sub"], "user_1");
    assert_eq!(v["email"], "a@b.example");
    assert_eq!(v["permissions"], json!([]));
    assert!(v.get("organizationId").is_none());
    let out = run_cli(&["logout"], &cfg_env(c), d.path());
    out.result.unwrap();
    assert_eq!(out.out, format!("Removed {}.\n", file.display()));
    let again = run_cli(&["logout"], &cfg_env(c), d.path());
    assert_eq!(
        again.out,
        format!(
            "No saved token at {} — nothing to remove.\n",
            file.display()
        )
    );
    let e = run_cli(&["whoami"], &cfg_env(c), d.path())
        .result
        .unwrap_err()
        .to_string();
    assert!(
        e.starts_with("not signed in — no token at ")
            && e.ends_with("neither WORKOS_ACCESS_TOKEN nor MZ_AUTH_TOKEN is set. Run `mz login`."),
        "{e}"
    );
}

#[test]
fn login_needs_a_token_and_rejects_an_unusable_one() {
    let (d, cfg) = (app_dir(), tempfile::tempdir().unwrap());
    let c = cfg.path().to_str().unwrap();
    let e = run_cli(&["login"], &cfg_env(c), d.path())
        .result
        .unwrap_err()
        .to_string();
    assert!(
        e.starts_with("login needs a token: `mz login --token <jwt>`, or set WORKOS_ACCESS_TOKEN.")
    );
    let e = run_cli(&["login", "--token", "garbage"], &cfg_env(c), d.path())
        .result
        .unwrap_err()
        .to_string();
    assert_eq!(
        e,
        "that token is not usable: invalid WorkOS access token: Invalid JWT"
    );
    assert!(!cfg.path().join("mz").exists());
}

#[test]
fn login_reads_workos_access_token_from_the_environment() {
    let (d, cfg) = (app_dir(), tempfile::tempdir().unwrap());
    let c = cfg.path().to_str().unwrap();
    let t = token(json!({}));
    let mut env = cfg_env(c);
    env.push(("WORKOS_ACCESS_TOKEN", &t));
    run_cli(&["login"], &env, d.path()).result.unwrap();
    assert!(cfg.path().join("mz").join("auth.json").exists());
}

#[test]
fn whoami_falls_back_to_env_tokens_and_warns_for_the_deprecated_name() {
    let (d, cfg) = (app_dir(), tempfile::tempdir().unwrap());
    let c = cfg.path().to_str().unwrap();
    let t = token(json!({"organization_id": "org_9", "permissions": ["a"]}));
    let mut env = cfg_env(c);
    env.push(("MZIZI_AUTH_TOKEN", &t));
    let o = run_cli(&["whoami"], &env, d.path());
    o.result.unwrap();
    assert_eq!(
        o.err,
        "warning: MZIZI_AUTH_TOKEN is deprecated, use MZ_AUTH_TOKEN\n"
    );
    assert_eq!(
        serde_json::from_str::<Value>(&o.out).unwrap()["organizationId"],
        "org_9"
    );
    let mut env = cfg_env(c);
    env.push(("MZ_AUTH_TOKEN", &t));
    assert_eq!(run_cli(&["whoami"], &env, d.path()).err, "");
}

#[test]
fn whoami_with_an_expired_saved_token_says_to_log_in_again() {
    let (d, cfg) = (app_dir(), tempfile::tempdir().unwrap());
    std::fs::create_dir_all(cfg.path().join("mz")).unwrap();
    let t = make_jwt(json!({"sub": "u", "exp": now() - 100}));
    std::fs::write(
        cfg.path().join("mz").join("auth.json"),
        json!({"accessToken": t, "savedAt": "x"}).to_string(),
    )
    .unwrap();
    let e = run_cli(
        &["whoami"],
        &cfg_env(cfg.path().to_str().unwrap()),
        d.path(),
    )
    .result
    .unwrap_err()
    .to_string();
    assert!(
        e.contains("expired") && e.ends_with("— run `mz login` again."),
        "{e}"
    );
}

#[test]
fn the_ts_auth_file_format_is_read_unchanged() {
    // What @nyuchi/mzizi-cli 0.13 wrote: pretty JSON with camelCase keys, optional refreshToken.
    let cfg = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(cfg.path().join("mz")).unwrap();
    std::fs::write(cfg.path().join("mz").join("auth.json"), "{\n  \"accessToken\": \"abc\",\n  \"refreshToken\": \"r\",\n  \"savedAt\": \"2026-01-01T00:00:00.000Z\"\n}\n").unwrap();
    let store = TokenStore::from_env(&Env([
        (
            "XDG_CONFIG_HOME".into(),
            cfg.path().to_str().unwrap().into(),
        ),
        ("MZ_NO_KEYCHAIN".into(), "1".into()),
    ]
    .into()));
    let a = store.read(&mut Vec::new()).unwrap();
    assert_eq!(
        (a.access_token.as_str(), a.refresh_token.as_deref()),
        ("abc", Some("r"))
    );
}

// ---- keychain: preferred when it works, file when it does not ----

#[derive(Clone)]
struct FakeKc {
    slot: Rc<RefCell<Option<String>>>,
    usable: bool,
}
impl Keychain for FakeKc {
    fn get(&self) -> Option<String> {
        self.slot.borrow().clone()
    }
    fn set(&self, s: &str) -> bool {
        if self.usable {
            *self.slot.borrow_mut() = Some(s.into());
        }
        self.usable
    }
    fn delete(&self) -> bool {
        self.slot.borrow_mut().take().is_some()
    }
}

fn auth(t: &str) -> StoredAuth {
    StoredAuth {
        access_token: t.into(),
        refresh_token: None,
        saved_at: "now".into(),
    }
}

#[test]
fn the_keychain_is_preferred_and_a_stale_file_is_removed() {
    let cfg = tempfile::tempdir().unwrap();
    let kc = FakeKc {
        slot: Rc::default(),
        usable: true,
    };
    let store = TokenStore::new(cfg.path().to_path_buf(), Box::new(kc.clone()));
    std::fs::create_dir_all(cfg.path().join("mz")).unwrap();
    std::fs::write(store.path(), "{\"accessToken\":\"old\",\"savedAt\":\"x\"}").unwrap();
    assert_eq!(store.write(&auth("new")).unwrap(), Saved::Keychain);
    assert!(!store.path().exists());
    assert_eq!(store.read(&mut Vec::new()).unwrap().access_token, "new");
}

#[test]
fn an_unusable_keychain_falls_back_to_the_file() {
    let cfg = tempfile::tempdir().unwrap();
    let store = TokenStore::new(
        cfg.path().to_path_buf(),
        Box::new(FakeKc {
            slot: Rc::default(),
            usable: false,
        }),
    );
    assert_eq!(
        store.write(&auth("t")).unwrap(),
        Saved::File(cfg.path().join("mz").join("auth.json"))
    );
    assert_eq!(store.read(&mut Vec::new()).unwrap().access_token, "t");
}

#[test]
fn logout_removes_the_keychain_entry_the_file_and_the_legacy_file() {
    let cfg = tempfile::tempdir().unwrap();
    let kc = FakeKc {
        slot: Rc::default(),
        usable: true,
    };
    let store = TokenStore::new(cfg.path().to_path_buf(), Box::new(kc.clone()));
    store.write(&auth("t")).unwrap();
    for p in [store.path(), store.legacy_path()] {
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(p, "{\"accessToken\":\"x\",\"savedAt\":\"x\"}").unwrap();
    }
    assert_eq!(store.clear(), (true, true, true));
    assert!(store.read(&mut Vec::new()).is_none());
    assert_eq!(store.clear(), (false, false, false));
}

// ---- migration of the TypeScript CLI's ~/.config/mzizi/auth.json ----

#[test]
fn the_legacy_file_is_migrated_to_the_file_store_on_first_read() {
    let cfg = tempfile::tempdir().unwrap();
    let store = TokenStore::from_env(&Env([
        (
            "XDG_CONFIG_HOME".into(),
            cfg.path().to_str().unwrap().into(),
        ),
        ("MZ_NO_KEYCHAIN".into(), "1".into()),
    ]
    .into()));
    std::fs::create_dir_all(store.legacy_path().parent().unwrap()).unwrap();
    std::fs::write(
        store.legacy_path(),
        "{\"accessToken\":\"old-tok\",\"savedAt\":\"2026-01-01\"}",
    )
    .unwrap();
    let mut note = Vec::new();
    assert_eq!(store.read(&mut note).unwrap().access_token, "old-tok");
    assert!(!store.legacy_path().exists(), "the old file is removed");
    assert_eq!(store.read(&mut Vec::new()).unwrap().access_token, "old-tok");
    assert!(store.path().exists());
    let note = String::from_utf8(note).unwrap();
    assert!(
        note.starts_with("note: moved your saved token from ") && note.contains("auth.json"),
        "{note}"
    );
    assert_eq!(note.lines().count(), 1);
}

#[test]
fn the_legacy_file_is_migrated_into_the_keychain_when_there_is_one() {
    let cfg = tempfile::tempdir().unwrap();
    let kc = FakeKc {
        slot: Rc::default(),
        usable: true,
    };
    let store = TokenStore::new(cfg.path().to_path_buf(), Box::new(kc.clone()));
    std::fs::create_dir_all(store.legacy_path().parent().unwrap()).unwrap();
    std::fs::write(
        store.legacy_path(),
        "{\"accessToken\":\"old-tok\",\"savedAt\":\"s\"}",
    )
    .unwrap();
    let mut note = Vec::new();
    store.read(&mut note).unwrap();
    assert!(kc.slot.borrow().as_ref().unwrap().contains("old-tok"));
    assert!(!store.legacy_path().exists() && !store.path().exists());
    assert!(String::from_utf8(note).unwrap().contains("the OS keychain"));
}

#[test]
fn whoami_migrates_and_reports_through_the_cli() {
    let (d, cfg) = (app_dir(), tempfile::tempdir().unwrap());
    let legacy = cfg.path().join("mzizi").join("auth.json");
    std::fs::create_dir_all(legacy.parent().unwrap()).unwrap();
    std::fs::write(
        &legacy,
        json!({"accessToken": token(json!({})), "savedAt": "s"}).to_string(),
    )
    .unwrap();
    let o = run_cli(
        &["whoami"],
        &cfg_env(cfg.path().to_str().unwrap()),
        d.path(),
    );
    o.result.unwrap();
    assert!(o.err.starts_with("note: moved your saved token from "));
    assert!(!legacy.exists() && cfg.path().join("mz").join("auth.json").exists());
}

#[test]
fn xdg_config_home_defaults_to_home_dot_config() {
    let store = TokenStore::from_env(&Env([
        ("HOME".into(), "/home/u".into()),
        ("MZ_NO_KEYCHAIN".into(), "1".into()),
    ]
    .into()));
    assert_eq!(
        store.path(),
        std::path::PathBuf::from("/home/u/.config/mz/auth.json")
    );
    assert_eq!(
        store.legacy_path(),
        std::path::PathBuf::from("/home/u/.config/mzizi/auth.json")
    );
}

// ---- small helpers ----

#[test]
fn iso_dates_match_js_to_iso_string() {
    assert_eq!(iso_from_epoch_secs(0.0), "1970-01-01T00:00:00.000Z");
    assert_eq!(
        iso_from_epoch_secs(1_700_000_000.0),
        "2023-11-14T22:13:20.000Z"
    );
    assert_eq!(
        iso_from_epoch_secs(1_709_164_800.5),
        "2024-02-29T00:00:00.500Z"
    );
}

// ---- MZ_MCP_URL / deprecated MZIZI_MCP_URL reach the planner's MCP call ----

fn mcp_url_run(var: &str) -> (Out, Server) {
    let d = app_dir();
    let mcp = serve(|req| {
        let v: Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
        match v["method"].as_str() {
            Some("initialize") => Resp::json(&json!({"jsonrpc": "2.0", "id": v["id"], "result": {}}).to_string()),
            Some("tools/call") => Resp::json(&json!({"jsonrpc": "2.0", "id": v["id"], "result": {"content": [{"type": "text", "text": "doc"}]}}).to_string()),
            _ => Resp::status(202),
        }
    });
    let api = serve(|req| {
        let b: Value = serde_json::from_str(&req.body).unwrap();
        if b["messages"].as_array().unwrap().len() == 1 {
            Resp::json(&json!({"stop_reason": "tool_use", "content": [{"type": "tool_use", "id": "t", "name": "fetch_mzizi_component", "input": {"name": "button"}}]}).to_string())
        } else {
            anthropic_text("```plan\n[]\n```")
        }
    });
    let o = run_cli(
        &["plan", "g"],
        &[
            ("ANTHROPIC_API_KEY", "k"),
            ("ANTHROPIC_BASE_URL", &api.url),
            (var, &mcp.url),
        ],
        d.path(),
    );
    (o, mcp)
}

#[test]
fn mz_mcp_url_is_used_without_a_warning() {
    let (o, mcp) = mcp_url_run("MZ_MCP_URL");
    o.result.unwrap();
    assert_eq!(o.err, "");
    assert!(mcp.seen().iter().any(|r| r.body.contains("tools/call")));
}

#[test]
fn deprecated_mzizi_mcp_url_still_works_with_a_one_line_warning() {
    let (o, mcp) = mcp_url_run("MZIZI_MCP_URL");
    o.result.unwrap();
    assert_eq!(
        o.err,
        "warning: MZIZI_MCP_URL is deprecated, use MZ_MCP_URL\n"
    );
    assert!(mcp.seen().iter().any(|r| r.body.contains("tools/call")));
}
