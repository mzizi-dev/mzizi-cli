//! `add` against recorded registry responses (tests/fixtures/recorded). No live network:
//! every request is answered from the fixtures, or by a loopback server in one test.
//!
//! Snapshots under tests/fixtures/expected/add/ are the Rust output. They were checked against
//! the TypeScript CLI by `scripts/parity-add.sh`, which diffs the two on these same fixtures;
//! regenerate with `UPDATE_GOLDEN=1 cargo test --test add_golden`, then re-run the parity script.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::time::Duration;

use mzizi_cli::add::cmd_add_with;
use mzizi_cli::add::registry::{Fetch, HttpFetch, Response};
use mzizi_cli::{CliError, Env, Io};

fn recorded() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/recorded")
}

/// Answers from the recorded files; an unknown URL is a 404.
struct Fixtures {
    extra: HashMap<String, (u16, String)>,
    /// Only `extra` answers; every other URL is a 404 (the TS tests' in-memory registry).
    strict: bool,
}
impl Fixtures {
    fn new() -> Self {
        Fixtures {
            extra: HashMap::new(),
            strict: false,
        }
    }
    fn only() -> Self {
        Fixtures {
            extra: HashMap::new(),
            strict: true,
        }
    }
}
impl Fetch for Fixtures {
    fn get(&self, url: &str, _ua: &str, _t: Duration) -> Result<Response, String> {
        if let Some((s, b)) = self.extra.get(url) {
            return Ok(Response {
                status: *s,
                body: b.clone(),
            });
        }
        if self.strict {
            return Ok(Response {
                status: 404,
                body: "{}".into(),
            });
        }
        let rel = url
            .strip_prefix("https://api.mzizi.dev/")
            .map(|p| format!("{p}.json"))
            .or_else(|| {
                url.strip_prefix("https://crates.io/api/v1/")
                    .map(|p| format!("{p}.json"))
            });
        let file = rel.map(|r| recorded().join(r));
        Ok(match file.and_then(|f| std::fs::read_to_string(f).ok()) {
            Some(body) => Response { status: 200, body },
            None => Response {
                status: 404,
                body: "{\"error\":\"not found\"}".into(),
            },
        })
    }
}

struct Run {
    out: String,
    err: String,
    result: Result<(), String>,
}

fn run_with(fetch: &dyn Fetch, dir: &Path, args: &[&str]) -> Run {
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let args: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let result = {
        let mut io = Io {
            out: &mut out,
            err: &mut err,
            cwd: dir.to_path_buf(),
            env: Env::default(),
        };
        cmd_add_with(&args, &mut io, fetch)
    };
    Run {
        out: String::from_utf8(out).unwrap(),
        err: String::from_utf8(err).unwrap(),
        result: result.map_err(|e| match e {
            CliError::Msg(m) => m,
            CliError::CheckFailed => "check failed".into(),
        }),
    }
}

fn run(dir: &Path, args: &[&str]) -> Run {
    run_with(&Fixtures::new(), dir, args)
}

fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
    let d = tempfile::tempdir().unwrap();
    for (n, b) in files {
        std::fs::write(d.path().join(n), b).unwrap();
    }
    d
}

fn golden(name: &str, actual: &str) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/expected/add")
        .join(name);
    if std::env::var_os("UPDATE_GOLDEN").is_some() {
        std::fs::write(&path, actual).unwrap();
    }
    let want = std::fs::read_to_string(&path).unwrap_or_else(|_| panic!("missing snapshot {name}"));
    assert_eq!(actual, want, "snapshot {name} differs");
}

/// The `content` of a file in a recorded document, for byte-for-byte comparison with disk.
fn recorded_content(doc: &str, index: usize) -> String {
    let v: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(recorded().join(doc)).unwrap()).unwrap();
    v["files"][index]["content"].as_str().unwrap().to_string()
}

#[test]
fn button_dry_run_matches_snapshot_and_writes_nothing() {
    let d = project(&[("components.json", "{}")]);
    let r = run(d.path(), &["button", "--dry-run"]);
    r.result.unwrap();
    golden("button-dry-run.stdout", &r.out);
    golden("button-dry-run.stderr", &r.err);
    assert!(!d.path().join("components/ui/button.tsx").exists());
}

#[test]
fn button_write_resolves_the_closure_and_writes_the_recorded_bytes() {
    let d = project(&[("components.json", "{}")]);
    let r = run(d.path(), &["button"]);
    r.result.unwrap();
    golden("button-write.stdout", &r.out);
    assert_eq!(
        std::fs::read_to_string(d.path().join("components/ui/button.tsx")).unwrap(),
        recorded_content("v1/ui/button.json", 0)
    );
    // ui-variants was pulled in as a dependency and landed too.
    let v: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(recorded().join("v1/ui/ui-variants.json")).unwrap(),
    )
    .unwrap();
    let dest = v["files"][0]["target"].as_str().unwrap();
    assert!(d.path().join(dest).exists(), "{dest}");
}

#[test]
fn address_input_closure_lists_its_dependency_first() {
    let d = project(&[("components.json", "{}")]);
    let r = run(d.path(), &["address-input", "--dry-run"]);
    r.result.unwrap();
    golden("address-input-dry-run.stdout", &r.out);
    assert!(r.out.contains("- input  (required by address-input)"));
}

#[test]
fn rust_type_dry_run_names_the_crate_and_writes_no_files() {
    let d = project(&[("Cargo.toml", "[package]\nname = \"x\"\n")]);
    let r = run(d.path(), &["button", "--type", "rust", "--dry-run"]);
    r.result.unwrap();
    golden("button-mzizi-dry-run.stdout", &r.out);
    assert!(r.out.contains("nothing would be written") && r.out.contains("cargo add mzizi-ui"));
    assert!(!r.out.contains("nothing was written"));
}

#[test]
fn rust_type_write_prints_the_cargo_add_line_when_the_crate_is_published() {
    let d = project(&[("Cargo.toml", "[package]\nname = \"x\"\n")]);
    let r = run(d.path(), &["button", "--type=mzizi"]);
    r.result.unwrap();
    golden("button-mzizi-write.stdout", &r.out);
}

#[test]
fn an_unpublished_crate_is_a_publishing_gap_not_an_install_hint() {
    // crates.io answers 404 for the crate: the CLI refuses to print a `cargo add` that fails.
    let d = project(&[("Cargo.toml", "[package]\nname = \"x\"\n")]);
    let mut f = Fixtures::new();
    f.extra.insert(
        "https://crates.io/api/v1/crates/mzizi-ui".into(),
        (404, "{}".into()),
    );
    let r = run_with(&f, d.path(), &["button", "--type", "mzizi"]);
    let e = r.result.unwrap_err();
    assert!(e.contains("has never been published to crates.io"), "{e}");
}

#[test]
fn an_unreachable_crates_io_is_unknown_not_missing() {
    struct Down;
    impl Fetch for Down {
        fn get(&self, url: &str, ua: &str, t: Duration) -> Result<Response, String> {
            if url.starts_with("https://crates.io") {
                Err("offline".into())
            } else {
                Fixtures::new().get(url, ua, t)
            }
        }
    }
    let d = project(&[("Cargo.toml", "[package]\nname = \"x\"\n")]);
    let r = run_with(&Down, d.path(), &["button", "--type", "mzizi"]);
    r.result.unwrap();
    assert!(
        r.out
            .contains("could not reach crates.io to confirm mzizi-ui is published")
    );
}

#[test]
fn astro_project_installs_pure_astro_into_the_astro_dir() {
    let d = project(&[
        ("astro.config.mjs", "export default {}"),
        ("package.json", "{}"),
    ]);
    let r = run(d.path(), &["button"]);
    r.result.unwrap();
    golden("button-astro.stdout", &r.out);
    assert_eq!(
        std::fs::read_to_string(d.path().join("src/components/mzizi/button.astro")).unwrap(),
        recorded_content("v1/astro/button.json", 0)
    );
}

#[test]
fn swift_tokens_install_with_no_flag() {
    let d = project(&[]);
    let r = run(d.path(), &["mzizi-tokens-swift"]);
    r.result.unwrap();
    assert!(r.out.contains("target swift"));
    assert_eq!(
        std::fs::read_to_string(d.path().join("lib/tokens/mzizi-tokens-swift.swift")).unwrap(),
        recorded_content("v1/ui/mzizi-tokens-swift.json", 0)
    );
}

#[test]
fn rewriting_skips_the_existing_file_unless_overwrite() {
    let d = project(&[("components.json", "{}")]);
    std::fs::create_dir_all(d.path().join("components/ui")).unwrap();
    std::fs::write(d.path().join("components/ui/button.tsx"), "MINE").unwrap();
    let r = run(d.path(), &["button"]);
    r.result.unwrap();
    assert!(r.out.contains("Skipped 1 existing file(s) — pass --overwrite to replace them:\n  components/ui/button.tsx\n"));
    assert_eq!(
        std::fs::read_to_string(d.path().join("components/ui/button.tsx")).unwrap(),
        "MINE"
    );
    run(d.path(), &["button", "--overwrite"]).result.unwrap();
    assert_ne!(
        std::fs::read_to_string(d.path().join("components/ui/button.tsx")).unwrap(),
        "MINE"
    );
}

// ---- cli-plan-add-output.test.ts: "mzizi add --dry-run on the mzizi type (#282)" and
// ---- "mzizi add type selection" -------------------------------------------------------------

fn doc(body: serde_json::Value) -> (u16, String) {
    (200, body.to_string())
}

#[test]
fn dry_run_on_the_mzizi_type_says_nothing_would_be_written_and_shows_cargo_add() {
    // TS stub: every URL answers the same Rust document.
    struct Same;
    impl Fetch for Same {
        fn get(&self, _: &str, _: &str, _: Duration) -> Result<Response, String> {
            let (status, body) = doc(serde_json::json!({
                "name": "button", "type": "registry:ui",
                "crate": {"name": "mzizi-ui", "registry": "crates.io"},
                "files": [{"path": "button.rs", "type": "registry:rust", "content": "// rs"}]
            }));
            Ok(Response { status, body })
        }
    }
    let d = project(&[("package.json", r#"{"name":"app"}"#)]);
    let r = run_with(&Same, d.path(), &["button", "--type", "rust", "--dry-run"]);
    r.result.unwrap();
    assert!(r.out.contains("nothing would be written"));
    assert!(r.out.contains("cargo add mzizi-ui"));
    assert!(!r.out.contains("nothing was written"));
}

#[test]
fn refuses_type_dioxus_listing_the_types_without_fetching() {
    struct Never;
    impl Fetch for Never {
        fn get(&self, _: &str, _: &str, _: Duration) -> Result<Response, String> {
            panic!("must not fetch")
        }
    }
    let d = project(&[]);
    let e = run_with(&Never, d.path(), &["button", "--type", "dioxus"])
        .result
        .unwrap_err();
    assert!(
        e.contains("--type must be one of: mzizi, tsx, astro, swift, kotlin (got dioxus). dioxus is not a type"),
        "{e}"
    );
}

#[test]
fn refuses_a_component_with_several_types_and_no_signal_listing_them() {
    struct TwoTypes;
    impl Fetch for TwoTypes {
        fn get(&self, url: &str, _: &str, _: Duration) -> Result<Response, String> {
            if url.contains("/astro/") {
                return Ok(Response {
                    status: 404,
                    body: "{}".into(),
                });
            }
            let (status, body) = doc(serde_json::json!({
                "name": "button", "type": "registry:ui",
                "files": [{"path": "button.tsx", "type": "registry:ui", "content": "// x"}],
                "crate": {"name": "mzizi-ui", "registry": "crates.io"}
            }));
            Ok(Response { status, body })
        }
    }
    let d = project(&[("package.json", r#"{"name":"app"}"#)]);
    let e = run_with(&TwoTypes, d.path(), &["button", "--dry-run"])
        .result
        .unwrap_err();
    assert!(
        e.contains("comes in 2 types (mzizi, tsx)") && e.contains("--type mzizi|tsx"),
        "{e}"
    );
}

#[test]
fn installs_the_only_type_a_native_item_has_with_no_flag() {
    let mut f = Fixtures::only();
    f.extra.insert(
        "https://api.mzizi.dev/v1/ui/mzizi-tokens-swift".into(),
        doc(serde_json::json!({
            "name": "mzizi-tokens-swift", "type": "registry:lib",
            "files": [{"path": "components/registry/n1-tokens/mzizi-tokens-swift.swift", "type": "registry:lib",
                       "target": "lib/tokens/mzizi-tokens-swift.swift", "content": "// swift"}]
        })),
    );
    let d = project(&[("package.json", r#"{"name":"app"}"#)]);
    let r = run_with(&f, d.path(), &["mzizi-tokens-swift", "--dry-run"]);
    r.result.unwrap();
    assert!(r.out.contains("target swift"));
    assert!(r.out.contains("lib/tokens/mzizi-tokens-swift.swift"));
}

#[test]
fn names_the_components_types_when_type_asks_for_one_it_lacks() {
    let mut f = Fixtures::only();
    f.extra.insert(
        "https://api.mzizi.dev/v1/ui/button".into(),
        doc(serde_json::json!({"name": "button", "type": "registry:ui",
            "files": [{"path": "button.tsx", "type": "registry:ui", "content": "x"}]})),
    );
    let d = project(&[("package.json", r#"{"name":"app"}"#)]);
    let e = run_with(&f, d.path(), &["button", "--type", "kotlin"])
        .result
        .unwrap_err();
    assert!(
        e.contains("button has no kotlin build. Available types: tsx"),
        "{e}"
    );
}

#[test]
fn a_missing_type_value_is_a_usage_error_naming_nothing() {
    let d = project(&[]);
    let e = run_with(&Fixtures::new(), d.path(), &["button", "--type"])
        .result
        .unwrap_err();
    assert!(e.contains("(got nothing)"), "{e}");
}

#[test]
fn add_requires_a_component_name() {
    let d = project(&[]);
    let e = run(d.path(), &["--dry-run"]).result.unwrap_err();
    assert_eq!(
        e,
        "add requires at least one component name, e.g. `mz add button`"
    );
}

// ---- The real transport, against a loopback server ------------------------------------------

#[test]
fn the_http_transport_reaches_a_local_server_through_the_base_override() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = std::thread::spawn(move || {
        // button's three type probes, its closure (button, ui-variants), nothing else is fetched
        // before the first write; serve until the test drops the listener's accept loop.
        listener.set_nonblocking(false).unwrap();
        for stream in listener.incoming().take(5) {
            let mut s = stream.unwrap();
            let mut buf = [0u8; 4096];
            let n = s.read(&mut buf).unwrap();
            let req = String::from_utf8_lossy(&buf[..n]);
            let path = req.split_whitespace().nth(1).unwrap_or("/").to_string();
            let file = recorded().join(path.trim_start_matches('/').to_string() + ".json");
            let (status, body) = match std::fs::read_to_string(&file) {
                Ok(b) => ("200 OK", b),
                Err(_) => ("404 Not Found", "{}".to_string()),
            };
            write!(
                s,
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            )
            .unwrap();
        }
    });
    let env = Env([(
        "MZ_REGISTRY_BASE".to_string(),
        format!("http://127.0.0.1:{port}"),
    )]
    .into_iter()
    .collect());
    let fetch = HttpFetch::new(&env);
    let d = project(&[("components.json", "{}")]);
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let mut io = Io {
        out: &mut out,
        err: &mut err,
        cwd: d.path().to_path_buf(),
        env,
    };
    // 3 probes + 2 closure documents = 5 requests; --dry-run stops before crates.io.
    cmd_add_with(
        &["button".to_string(), "--dry-run".to_string()],
        &mut io,
        &fetch,
    )
    .unwrap();
    server.join().unwrap();
    assert!(
        String::from_utf8(out)
            .unwrap()
            .contains("2 registry item(s), 1 pulled in as dependencies")
    );
}
