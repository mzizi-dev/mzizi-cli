//! Port of the vitest cases for auth, explore, context-gather, describe-skill, plan-steps,
//! tools and the MCP tool name. Each test names the vitest case it replaces
//! (the full table is in docs/porting/fundi.md).
#![allow(missing_docs)]

mod fundi_support;

use fundi_support::*;
use mzizi_cli::Env;
use mzizi_cli::fundi::auth::*;
use mzizi_cli::fundi::context::{describe_skill, gather_context};
use mzizi_cli::fundi::explore::{explore_project, resolve};
use mzizi_cli::fundi::plan::{Step, extract_steps_from_text};
use mzizi_cli::fundi::tools::*;
use serde_json::json;

fn env(pairs: &[(&str, &str)]) -> Env {
    Env(pairs
        .iter()
        .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
        .collect())
}
fn sink() -> Vec<u8> {
    Vec::new()
}

// ---- auth.test.ts: resolveToken ----

#[test]
fn resolve_token_prefers_explicit_over_both_env_vars() {
    let e = env(&[
        ("WORKOS_ACCESS_TOKEN", "from-workos-env"),
        ("MZ_AUTH_TOKEN", "from-mzizi-env"),
    ]);
    assert_eq!(
        resolve_token(Some("explicit"), &e, &mut sink()).as_deref(),
        Some("explicit")
    );
}

#[test]
fn resolve_token_falls_back_to_workos_access_token() {
    let e = env(&[
        ("WORKOS_ACCESS_TOKEN", "from-workos-env"),
        ("MZ_AUTH_TOKEN", "from-mzizi-env"),
    ]);
    assert_eq!(
        resolve_token(None, &e, &mut sink()).as_deref(),
        Some("from-workos-env")
    );
}

#[test]
fn resolve_token_falls_back_to_auth_token_alias() {
    let e = env(&[("MZ_AUTH_TOKEN", "from-mzizi-env")]);
    let mut warn = sink();
    assert_eq!(
        resolve_token(None, &e, &mut warn).as_deref(),
        Some("from-mzizi-env")
    );
    assert!(warn.is_empty(), "the new name never warns");
}

#[test]
fn resolve_token_deprecated_mzizi_auth_token_still_works_and_warns_once() {
    let e = env(&[("MZIZI_AUTH_TOKEN", "old")]);
    let mut warn = sink();
    assert_eq!(resolve_token(None, &e, &mut warn).as_deref(), Some("old"));
    assert_eq!(
        String::from_utf8(warn).unwrap(),
        "warning: MZIZI_AUTH_TOKEN is deprecated, use MZ_AUTH_TOKEN\n"
    );
}

#[test]
fn resolve_token_new_name_wins_over_deprecated_and_does_not_warn() {
    let e = env(&[("MZ_AUTH_TOKEN", "new"), ("MZIZI_AUTH_TOKEN", "old")]);
    let mut warn = sink();
    assert_eq!(resolve_token(None, &e, &mut warn).as_deref(), Some("new"));
    assert!(warn.is_empty());
}

#[test]
fn resolve_token_returns_none_when_no_token_is_available() {
    assert_eq!(resolve_token(None, &env(&[]), &mut sink()), None);
}

#[test]
fn resolve_token_ignores_whitespace_only_tokens() {
    assert_eq!(resolve_token(Some("   "), &env(&[]), &mut sink()), None);
    assert_eq!(
        resolve_token(None, &env(&[("WORKOS_ACCESS_TOKEN", "   ")]), &mut sink()),
        None
    );
}

// ---- auth.test.ts: assertToken ----

#[test]
fn assert_token_returns_the_resolved_token() {
    assert_eq!(
        assert_token(Some("abc"), &env(&[]), "mzizi-sdk", &mut sink()).unwrap(),
        "abc"
    );
}

#[test]
fn assert_token_errors_when_no_token_is_available() {
    let e = assert_token(None, &env(&[]), "mzizi-sdk", &mut sink()).unwrap_err();
    assert!(
        e.0.contains("no WorkOS token")
            && e.0.contains("WORKOS_ACCESS_TOKEN")
            && e.0.contains("options.auth.accessToken")
    );
}

#[test]
fn assert_token_includes_the_label_in_the_message() {
    let e = assert_token(None, &env(&[]), "fetchComponent", &mut sink()).unwrap_err();
    assert!(e.0.starts_with("fetchComponent: no WorkOS token"));
}

// ---- auth.test.ts: getIdentity ----

const ONE_HOUR: i64 = 3600;

#[test]
fn get_identity_decodes_a_well_formed_workos_token() {
    let exp = now() + ONE_HOUR;
    let t = make_jwt(
        json!({"sub": "user_01HZX", "email": "user@example.com", "organization_id": "org_01HZY",
        "permissions": ["mzizi:read", "fundi:plan"], "exp": exp}),
    );
    let i = get_identity(&t).unwrap();
    assert_eq!(
        i,
        Identity {
            sub: "user_01HZX".into(),
            email: Some("user@example.com".into()),
            organization_id: Some("org_01HZY".into()),
            permissions: vec!["mzizi:read".into(), "fundi:plan".into()],
            exp: exp as f64
        }
    );
}

#[test]
fn get_identity_accepts_organization_id_as_camel_case() {
    let t = make_jwt(
        json!({"sub": "user_x", "organizationId": "org_camel", "permissions": [], "exp": now() + ONE_HOUR}),
    );
    assert_eq!(
        get_identity(&t).unwrap().organization_id.as_deref(),
        Some("org_camel")
    );
}

#[test]
fn get_identity_defaults_permissions_to_empty() {
    let t = make_jwt(json!({"sub": "user_y", "exp": now() + ONE_HOUR}));
    assert!(get_identity(&t).unwrap().permissions.is_empty());
}

#[test]
fn get_identity_filters_non_string_permission_entries() {
    let t = make_jwt(
        json!({"sub": "user_z", "permissions": ["valid", 42, null, "also-valid"], "exp": now() + ONE_HOUR}),
    );
    assert_eq!(
        get_identity(&t).unwrap().permissions,
        vec!["valid", "also-valid"]
    );
}

#[test]
fn get_identity_errors_when_the_token_is_expired() {
    let t = make_jwt(json!({"sub": "user_expired", "exp": now() - 10}));
    let e = get_identity(&t).unwrap_err();
    assert!(
        regex::Regex::new(r"expired \d+s ago")
            .unwrap()
            .is_match(&e.0),
        "{e}"
    );
}

#[test]
fn get_identity_errors_when_sub_is_missing() {
    let t = make_jwt(json!({"email": "no-sub@example.com", "exp": now() + ONE_HOUR}));
    assert!(
        get_identity(&t)
            .unwrap_err()
            .0
            .contains("missing 'sub' claim")
    );
}

#[test]
fn get_identity_errors_when_exp_is_missing() {
    let t = make_jwt(json!({"sub": "user_no_exp"}));
    assert!(
        get_identity(&t)
            .unwrap_err()
            .0
            .contains("missing 'exp' claim")
    );
}

#[test]
fn get_identity_errors_when_the_token_is_malformed() {
    assert!(get_identity("not.a.valid.jwt.at.all").is_err());
    assert!(
        get_identity("garbage")
            .unwrap_err()
            .0
            .contains("invalid WorkOS access token")
    );
}

// ---- auth.test.ts: bearerHeader ----

#[test]
fn bearer_header_returns_authorization_when_a_token_is_provided() {
    assert_eq!(
        bearer_header(Some("abc")),
        vec![("Authorization".to_string(), "Bearer abc".to_string())]
    );
}

#[test]
fn bearer_header_is_empty_without_a_token() {
    assert!(bearer_header(None).is_empty());
}

// ---- explore.test.ts ----

#[test]
fn explore_reads_the_well_known_markers_and_normalises_them() {
    let s = explore_project(&fixture_root(), ".");
    assert_eq!(s.root, resolve(&fixture_root(), ".").to_string_lossy());
    assert!(s.has_tsconfig && s.has_globals_css && !s.has_components_json);
    assert_eq!(s.tailwind_config_path, None);
    assert_eq!(s.package_json.as_ref().unwrap()["name"], "empty-app");
    assert!(s.notes.is_empty());
}

#[test]
fn explore_surfaces_a_missing_root_as_empty_not_an_error() {
    let missing = fixture_root().join("..").join("does-not-exist");
    let s = explore_project(&fixture_root(), missing.to_str().unwrap());
    assert!(s.package_json.is_none() && !s.has_tsconfig && !s.has_globals_css);
    assert!(
        s.notes.is_empty(),
        "package.json ENOENT is suppressed by design"
    );
}

// ---- context-gather.test.ts ----

#[test]
fn gather_context_returns_snapshot_and_goal_untouched() {
    let snap = explore_project(&fixture_root(), ".");
    let r = gather_context("add dark mode toggle", snap.clone());
    assert_eq!(r.goal, "add dark mode toggle");
    assert_eq!(r.snapshot, snap);
    assert!(r.note.contains("No ANTHROPIC_API_KEY set"));
}

#[test]
fn gather_context_matches_skills_sharing_a_word_with_the_goal() {
    let r = gather_context(
        "improve discoverability for AI agents",
        explore_project(&fixture_root(), "."),
    );
    assert!(
        r.relevant_skills
            .iter()
            .any(|s| s.name == "discoverability")
    );
}

#[test]
fn gather_context_matches_nothing_for_unrelated_words() {
    assert!(
        gather_context("xyzzy plugh qux", explore_project(&fixture_root(), "."))
            .relevant_skills
            .is_empty()
    );
}

#[test]
fn gather_context_ignores_stopwords_and_short_words() {
    assert!(
        gather_context("add a", explore_project(&fixture_root(), "."))
            .relevant_skills
            .is_empty()
    );
}

// ---- describe-skill.test.ts ----

#[test]
fn describe_skill_reads_the_frontmatter_description_cut_at_first_sentence() {
    assert_eq!(
        describe_skill("---\nname: x\ndescription: Does a thing. Then more.\n---\n\n# X\n"),
        "Does a thing."
    );
}

#[test]
fn describe_skill_falls_back_to_first_prose_line_when_description_is_empty() {
    assert_eq!(
        describe_skill("---\nname: x\ndescription:   \n---\n\nFirst prose line. More.\n"),
        "First prose line."
    );
}

#[test]
fn describe_skill_stays_fast_on_a_long_run_of_whitespace() {
    let body = format!(
        "---\ndescription:{}\n---\n\nFallback line.\n",
        " ".repeat(200_000)
    );
    let t = std::time::Instant::now();
    assert_eq!(describe_skill(&body), "Fallback line.");
    assert!(t.elapsed().as_millis() < 500);
}

#[test]
fn describe_skill_caps_at_160_characters() {
    let long = format!("---\ndescription: {}\n---\n", "word ".repeat(80));
    let d = describe_skill(&long);
    assert!(d.chars().count() <= 160 && d.ends_with('…'));
}

// ---- plan-steps.test.ts ----

fn sample() -> Vec<Step> {
    vec![
        Step::InstallComponent {
            description: "add Button".into(),
            components: vec!["button".into()],
        },
        Step::CreateFile {
            description: "seed globals.css".into(),
            path: "app/globals.css".into(),
            contents: "/* tokens */".into(),
        },
        Step::ModifyFile {
            description: "patch layout".into(),
            path: "app/layout.tsx".into(),
            contents: "// ...".into(),
        },
        Step::RunCommand {
            description: "lint".into(),
            argv: vec!["pnpm".into(), "lint".into()],
            cwd: None,
        },
    ]
}

#[test]
fn step_kinds_narrow_correctly() {
    // Step type guards: the enum's variants are the guards.
    let kinds: Vec<_> = sample().iter().map(Step::kind).collect();
    assert_eq!(
        kinds,
        [
            "install-component",
            "create-file",
            "modify-file",
            "run-command"
        ]
    );
}

#[test]
fn step_params_stay_typed_after_narrowing() {
    let s = sample();
    assert!(
        matches!(&s[0], Step::InstallComponent { components, .. } if components == &["button"])
    );
    assert!(matches!(&s[1], Step::CreateFile { path, .. } if path == "app/globals.css"));
    assert!(matches!(&s[3], Step::RunCommand { argv, .. } if argv == &["pnpm", "lint"]));
}

#[test]
fn extract_steps_parses_a_fenced_plan_block() {
    let body = json!([
        {"kind": "install-component", "description": "add Button", "params": {"components": ["button"]}},
        {"kind": "create-file", "description": "create file", "params": {"path": "x.ts", "contents": "export {}"}},
    ]);
    let steps = extract_steps_from_text(&format!("```plan\n{body}\n```")).unwrap();
    assert_eq!(steps.len(), 2);
    assert_eq!(steps[0].kind(), "install-component");
    assert_eq!(steps[1].kind(), "create-file");
}

#[test]
fn extract_steps_falls_back_to_a_bare_json_array() {
    let t = json!([{"kind": "run-command", "description": "test", "params": {"argv": ["pnpm", "test"]}}]).to_string();
    let steps = extract_steps_from_text(&t).unwrap();
    assert_eq!(steps.len(), 1);
    assert_eq!(steps[0].kind(), "run-command");
}

#[test]
fn extract_steps_rejects_unknown_kinds() {
    let e = extract_steps_from_text(&json!([{"kind": "nuke-database", "params": {}}]).to_string())
        .unwrap_err();
    assert_eq!(e, "Step 0: unknown kind 'nuke-database'");
}

#[test]
fn extract_steps_rejects_install_component_without_components() {
    let e = extract_steps_from_text(
        &json!([{"kind": "install-component", "description": "x", "params": {}}]).to_string(),
    )
    .unwrap_err();
    assert_eq!(
        e,
        "Step 0: install-component requires params.components: string[]"
    );
}

#[test]
fn extract_steps_errors_without_a_fence_or_array() {
    assert_eq!(
        extract_steps_from_text("just some prose").unwrap_err(),
        "Planner response did not contain a `plan` JSON block"
    );
}

// ---- tools.test.ts ----

#[test]
fn tool_registry_exposes_all_six_expected_tools() {
    let mut names: Vec<String> = fundi_tool_definitions()
        .as_array()
        .unwrap()
        .iter()
        .map(|t| t["name"].as_str().unwrap().to_string())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "fetch_mzizi_component",
            "fetch_mzizi_skill",
            "list_dir",
            "read_file",
            "run_shell",
            "write_file"
        ]
    );
}

#[test]
fn resolve_within_permits_paths_inside_the_root() {
    let root = resolve(&fixture_root(), ".");
    assert_eq!(
        resolve_within(&root, "app/globals.css"),
        Some(root.join("app").join("globals.css"))
    );
    assert_eq!(resolve_within(&root, "."), Some(root.clone()));
}

#[test]
fn resolve_within_refuses_paths_that_escape_the_root() {
    let root = resolve(&fixture_root(), ".");
    assert_eq!(resolve_within(&root, "../escape.txt"), None);
    assert_eq!(resolve_within(&root, "../../etc/passwd"), None);
}

fn ctx(read_only: bool) -> ToolCtx {
    ToolCtx::new(
        &resolve(&fixture_root(), "."),
        "https://mcp.example",
        None,
        read_only,
    )
}

#[test]
fn dispatch_read_file_returns_file_contents() {
    let r = dispatch_tool(
        "read_file",
        &json!({"path": "package.json"}),
        &mut ctx(false),
    );
    assert!(!r.is_error);
    assert!(r.content.contains("\"name\": \"empty-app\""));
}

#[test]
fn dispatch_list_dir_returns_sorted_entries_with_slash_on_dirs() {
    let r = dispatch_tool("list_dir", &json!({"path": "."}), &mut ctx(false));
    assert!(!r.is_error);
    let lines: Vec<&str> = r.content.split('\n').collect();
    assert!(
        lines.contains(&"app/")
            && lines.contains(&"package.json")
            && lines.contains(&"tsconfig.json")
    );
    let mut sorted = lines.clone();
    sorted.sort();
    assert_eq!(lines, sorted);
}

#[test]
fn dispatch_refuses_unknown_tools() {
    let r = dispatch_tool("nope", &json!({}), &mut ctx(false));
    assert!(r.is_error && r.content.to_lowercase().contains("unknown tool"));
}

#[test]
fn dispatch_blocks_write_file_in_read_only_mode() {
    let r = dispatch_tool(
        "write_file",
        &json!({"path": "x.txt", "contents": "hi"}),
        &mut ctx(true),
    );
    assert!(r.is_error && r.content.contains("read-only"));
}

#[test]
fn dispatch_blocks_run_shell_in_read_only_mode() {
    let r = dispatch_tool("run_shell", &json!({"argv": ["true"]}), &mut ctx(true));
    assert!(r.is_error && r.content.contains("read-only"));
}

#[test]
fn dispatch_refuses_paths_outside_the_project_root() {
    let r = dispatch_tool(
        "read_file",
        &json!({"path": "../../../etc/passwd"}),
        &mut ctx(false),
    );
    assert!(r.is_error && r.content.to_lowercase().contains("outside project root"));
}

#[test]
fn dispatch_fetch_mzizi_skill_returns_body_for_a_known_skill() {
    let r = dispatch_tool(
        "fetch_mzizi_skill",
        &json!({"name": "mzizi-design"}),
        &mut ctx(false),
    );
    assert!(!r.is_error);
    assert!(r.content.to_lowercase().contains("african minerals"));
}

#[test]
fn dispatch_fetch_mzizi_skill_errors_on_unknown_name() {
    let r = dispatch_tool(
        "fetch_mzizi_skill",
        &json!({"name": "no-such-skill"}),
        &mut ctx(false),
    );
    assert!(r.is_error && r.content.to_lowercase().contains("unknown skill"));
}

// ---- mcp-tool-name.test.ts ----
// The vitest case read mzizi-mcp's TypeScript source from the monorepo, which the standalone
// repo cannot. The same contract is asserted on the wire instead: the request the CLI sends.

fn mcp_handler(result: &'static str, sse: bool) -> impl Fn(&Req) -> Resp + Send + Sync + 'static {
    move |req| {
        let v: serde_json::Value = serde_json::from_str(&req.body).unwrap_or(json!({}));
        match v["method"].as_str() {
            Some("initialize") => Resp {
                status: 200,
                content_type: "application/json",
                headers: vec![("mcp-session-id", "sess-1".into())],
                body: json!({"jsonrpc": "2.0", "id": v["id"], "result": {"protocolVersion": "2025-06-18", "capabilities": {}, "serverInfo": {"name": "t", "version": "0"}}}).to_string(),
            },
            Some("tools/call") => {
                let payload = json!({"jsonrpc": "2.0", "id": v["id"], "result": serde_json::from_str::<serde_json::Value>(result).unwrap()}).to_string();
                if sse {
                    Resp { status: 200, content_type: "text/event-stream", headers: vec![], body: format!("event: message\ndata: {payload}\n\n") }
                } else {
                    Resp::json(&payload)
                }
            }
            _ => Resp::status(202),
        }
    }
}

#[test]
fn fetch_component_calls_the_registered_tool_and_operation_with_name() {
    assert_eq!(MCP_EXECUTE_TOOL, "mzizi_execute");
    assert_eq!(MCP_GET_COMPONENT_OPERATION, "get_component");
    let srv = serve(mcp_handler(
        r#"{"content":[{"type":"text","text":"{\"name\":\"button\"}"}]}"#,
        false,
    ));
    let mut c = ToolCtx::new(&fixture_root(), &srv.url, Some("tok"), true);
    let r = dispatch_tool("fetch_mzizi_component", &json!({"name": "button"}), &mut c);
    assert_eq!(
        r,
        ToolResult {
            content: "{\"name\":\"button\"}".into(),
            is_error: false
        }
    );
    let seen = srv.seen();
    let call: serde_json::Value = serde_json::from_str(&seen.last().unwrap().body).unwrap();
    assert_eq!(call["method"], "tools/call");
    assert_eq!(call["params"]["name"], "mzizi_execute");
    assert_eq!(
        call["params"]["arguments"],
        json!({"operation": "get_component", "arguments": {"name": "button"}})
    );
    assert!(
        call["params"]["arguments"]["arguments"]
            .get("p_name")
            .is_none(),
        "takes `name`, not the retired `p_name`"
    );
    assert_eq!(
        seen.last()
            .unwrap()
            .headers
            .get("authorization")
            .map(String::as_str),
        Some("Bearer tok")
    );
    assert_eq!(
        seen.last()
            .unwrap()
            .headers
            .get("mcp-session-id")
            .map(String::as_str),
        Some("sess-1")
    );
}

#[test]
fn fetch_component_reads_a_server_sent_event_response() {
    let srv = serve(mcp_handler(
        r#"{"content":[{"type":"text","text":"sse-doc"}]}"#,
        true,
    ));
    let mut c = ToolCtx::new(&fixture_root(), &srv.url, None, true);
    let r = dispatch_tool("fetch_mzizi_component", &json!({"name": "card"}), &mut c);
    assert_eq!(r.content, "sse-doc");
}

#[test]
fn fetch_component_falls_back_to_the_http_registry_when_mcp_reports_an_error() {
    let mcp = serve(mcp_handler(
        r#"{"isError":true,"content":[{"type":"text","text":"nope"}]}"#,
        false,
    ));
    let reg = serve(|req| {
        if req.path == "/v1/ui/but%20ton" {
            Resp::json(r#"{ "name": "but ton" }"#)
        } else {
            Resp::status(404)
        }
    });
    let mut c = ToolCtx::new(&fixture_root(), &mcp.url, None, true);
    c.registry_http_base = format!("{}/v1/ui", reg.url);
    let r = dispatch_tool("fetch_mzizi_component", &json!({"name": "but ton"}), &mut c);
    assert_eq!(
        r,
        ToolResult {
            content: r#"{"name":"but ton"}"#.into(),
            is_error: false
        }
    );
}

#[test]
fn fetch_component_reports_both_failures() {
    let mcp = serve(|_| Resp::status(500));
    let reg = serve(|_| Resp::status(404));
    let mut c = ToolCtx::new(&fixture_root(), &mcp.url, None, true);
    c.registry_http_base = format!("{}/v1/ui", reg.url);
    let r = dispatch_tool("fetch_mzizi_component", &json!({"name": "x"}), &mut c);
    assert!(r.is_error);
    assert!(
        r.content.starts_with(
            "fetch_mzizi_component('x') failed — MCP mzizi_execute get_component against "
        ),
        "{}",
        r.content
    );
    assert!(
        r.content
            .ends_with("the HTTP fallback both errored: HTTP 404")
    );
}

#[test]
fn registry_install_argv_uses_full_urls_never_bare_names() {
    assert_eq!(
        registry_install_argv(&["button".into()]),
        [
            "npx",
            "shadcn@latest",
            "add",
            "https://api.mzizi.dev/v1/ui/button"
        ]
    );
}
