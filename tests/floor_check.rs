//! Port of `test/check.test.ts` (vitest) case by case, plus a few tests of the JSON/number
//! formatting that JavaScript gave the TypeScript CLI for free. Mapping: docs/porting/floor.md.

use std::fs;
use std::path::{Path, PathBuf};

use mzizi_cli::floor::tokens::{HIT_DESKTOP_PX, HIT_TOUCH_PX, SPACING_LAYERS, js_num};
use mzizi_cli::floor::{CheckError, CheckOptions, check_project, json_pretty, render_report};
use mzizi_cli::{CliError, Env, Io, cli};
use serde_json::Value;

struct Root(tempfile::TempDir);
impl Root {
    fn new() -> Self {
        Root(
            tempfile::Builder::new()
                .prefix("mzizi-check-")
                .tempdir()
                .expect("tempdir"),
        )
    }
    fn path(&self) -> PathBuf {
        self.0.path().to_path_buf()
    }
    fn put(&self, rel: &str, text: &str) {
        let abs = self.0.path().join(rel);
        fs::create_dir_all(abs.parent().expect("parent")).expect("mkdir");
        fs::write(abs, text).expect("write");
    }
}

fn opts(cwd: &Path) -> CheckOptions {
    CheckOptions {
        cwd: cwd.to_path_buf(),
        ..Default::default()
    }
}

fn vkeys(r: &mzizi_cli::floor::CheckReport) -> Vec<String> {
    r.violations
        .iter()
        .map(|v| {
            format!(
                "{}:{}:{}:{}",
                v.file, v.finding.line, v.finding.column, v.finding.rule
            )
        })
        .collect()
}

/// Run the CLI in `cwd`. Returns (stdout, result).
fn run(cwd: &Path, args: &[&str]) -> (String, Result<(), CliError>) {
    let argv: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let r = {
        let mut io = Io {
            out: &mut out,
            err: &mut err,
            cwd: cwd.to_path_buf(),
            env: Env::default(),
        };
        cli::run(&argv, &mut io)
    };
    (String::from_utf8(out).expect("utf8"), r)
}

// ---- checkProject ----

#[test]
fn passes_a_project_at_the_floor() {
    let r = Root::new();
    r.put(
        "src/a.tsx",
        r#"<div className="px-4 gap-2 leading-normal">x</div>"#,
    );
    r.put(
        "src/a.css",
        ".a { padding: 14px; gap: 7px; line-height: 1.5 }",
    );
    let report = check_project(&opts(&r.path())).expect("report");
    assert!(report.ok());
    assert_eq!(report.files_scanned, 2);
    assert!(report.violations.is_empty());
}

#[test]
fn reports_file_line_and_column_sorted_with_posix_paths() {
    let r = Root::new();
    r.put("src/b.tsx", "const x = 1\n<div className=\"gap-0\">\n");
    r.put("src/a.css", ".a {\n  padding-left: 4px;\n}\n");
    let report = check_project(&opts(&r.path())).expect("report");
    assert!(!report.ok());
    assert_eq!(
        vkeys(&report),
        vec![
            "src/a.css:2:3:padding-below-inset",
            "src/b.tsx:2:17:gap-below-floor"
        ]
    );
    let json = report.to_json();
    let floor = &json["floor"];
    assert_eq!(
        floor["layers"]["controls"]["gap"]["default"].as_f64(),
        Some(9.0)
    );
    assert!(floor["layers"]["chips"].get("gap").is_none());
    assert!(render_report(&report).contains("src/a.css:2:3  padding-below-inset"));
}

#[test]
fn keeps_the_0_11_json_floor_keys_as_aliases_of_the_layer_floors() {
    let r = Root::new();
    r.put("a.css", ".a { padding: 14px }");
    let json = check_project(&opts(&r.path())).expect("report").to_json();
    let f = &json["floor"];
    assert_eq!(f["insetPx"], f["unplaced"]["paddingPx"]);
    assert_eq!(f["controlGapPx"], f["layers"]["controls"]["gap"]["default"]);
    // Sibling items take --space-card-gap (14); the cards layer's smallest gap is the 7px stack.
    assert_eq!(f["itemGapPx"].as_f64(), Some(14.0));
    assert_eq!(f["labelGapPx"].as_f64(), Some(7.0));
    for key in [
        "insetPx",
        "controlGapPx",
        "labelGapPx",
        "itemGapPx",
        "leadingBody",
    ] {
        assert!(f.get(key).is_some(), "{key}");
    }
}

#[cfg(unix)]
#[test]
fn skips_build_output_dependencies_symlinks_and_unrelated_files_but_reads_an_explicit_file() {
    let r = Root::new();
    r.put("node_modules/x/a.css", ".a{padding:0}");
    r.put("dist/a.css", ".a{padding:0}");
    r.put("src/legacy/old.css", ".a{padding:0}");
    r.put("notes.txt", "p-0");
    r.put("a.min.css", ".a{padding:0}");
    std::os::unix::fs::symlink(r.path().join("dist"), r.path().join("linked")).expect("symlink");
    let mut o = opts(&r.path());
    o.exclude = vec!["src/legacy".into()];
    assert!(check_project(&o).expect("report").violations.is_empty());
    let mut o = opts(&r.path());
    o.paths = vec!["notes.txt".into()];
    assert_eq!(check_project(&o).expect("report").violations.len(), 1);
    let mut o = opts(&r.path());
    o.paths = vec!["src".into()];
    assert_eq!(check_project(&o).expect("report").violations.len(), 1);
}

#[test]
fn lists_exempted_findings_and_the_legacy_notice_and_still_passes() {
    let r = Root::new();
    r.put(
        "a.tsx",
        "<html data-mzizi-sizes=\"legacy\">\n{/* mzizi-spacing-exempt: image */}\n<div className=\"p-0\" />",
    );
    let report = check_project(&opts(&r.path())).expect("report");
    assert!(report.ok());
    assert_eq!(report.exempted.len(), 1);
    assert_eq!(report.exempted[0].file, "a.tsx");
    assert_eq!(report.exempted[0].exempted.finding.line, 3);
    assert_eq!(report.exempted[0].exempted.reason, "image");
    assert_eq!(report.notices.len(), 1);
    assert_eq!(
        (report.notices[0].file.as_str(), report.notices[0].line),
        ("a.tsx", Some(1))
    );
}

#[cfg(unix)]
#[test]
fn follows_a_symlink_the_user_names_and_fails_rather_than_pass_having_read_nothing() {
    let r = Root::new();
    r.put("real/a.css", ".a{padding:0}");
    std::os::unix::fs::symlink(r.path().join("real"), r.path().join("alias")).expect("symlink");
    let mut o = opts(&r.path());
    o.paths = vec!["alias".into()];
    assert_eq!(check_project(&o).expect("report").violations.len(), 1);
    r.put("docs/readme.txt", "p-0");
    let mut o = opts(&r.path());
    o.paths = vec!["docs".into()];
    match check_project(&o) {
        Err(CheckError::Usage(m)) => assert!(m.contains("no files to check"), "{m}"),
        other => panic!("expected a usage error, got {other:?}"),
    }
    let bare = check_project(&opts(&r.path().join("docs"))).expect("report");
    assert!(bare.ok());
    assert!(bare.notices[0].message.contains("no files to check"));
}

#[test]
fn exclude_takes_relative_dot_slash_trailing_slash_and_absolute_paths() {
    let r = Root::new();
    r.put("a/x.css", ".a{padding:0}");
    r.put("b/y.css", ".a{padding:0}");
    let abs_a = r.path().join("a").to_string_lossy().into_owned();
    for ex in ["a", "./a", "a/", abs_a.as_str()] {
        let mut o = opts(&r.path());
        o.exclude = vec![ex.to_string(), "b".into()];
        assert!(
            check_project(&o).expect("report").violations.is_empty(),
            "{ex}"
        );
    }
    let mut o = opts(&r.path().join("b"));
    o.exclude = vec!["../a".into()];
    assert_eq!(check_project(&o).expect("report").violations.len(), 1);
}

#[test]
fn reads_ts_and_js_files_but_not_d_ts() {
    let r = Root::new();
    r.put("v.ts", r#"export const c = cva("gap-0")"#);
    r.put("v.d.ts", r#"export const c = cva("gap-0")"#);
    let report = check_project(&opts(&r.path())).expect("report");
    assert_eq!(
        report
            .violations
            .iter()
            .map(|v| v.file.as_str())
            .collect::<Vec<_>>(),
        vec!["v.ts"]
    );
}

#[test]
fn rejects_a_path_that_does_not_exist() {
    let r = Root::new();
    let mut o = opts(&r.path());
    o.paths = vec!["nope".into()];
    match check_project(&o) {
        Err(CheckError::Usage(m)) => assert!(m.contains("no such file"), "{m}"),
        other => panic!("expected a usage error, got {other:?}"),
    }
}

// ---- mzizi check (CLI) ----

#[test]
fn cli_exits_clean_when_nothing_is_below_the_floor() {
    let r = Root::new();
    r.put("a.css", ".a { padding: 14px }");
    let (out, res) = run(&r.path(), &["floor"]);
    assert!(res.is_ok());
    assert!(out.contains("0 violation(s)"));
}

#[test]
fn cli_fails_after_printing_when_something_is_below_the_floor() {
    let r = Root::new();
    r.put("a.css", ".a { padding: 0 }");
    let (out, res) = run(&r.path(), &["floor", "."]);
    assert!(matches!(res, Err(CliError::CheckFailed)));
    assert!(out.contains("a.css:1:6  padding-zero"));
    assert!(out.contains("mzizi-spacing-exempt"));
}

#[test]
fn cli_json_prints_one_parseable_report_and_still_fails() {
    let r = Root::new();
    r.put("a.css", ".a { gap: 2px }");
    let (out, res) = run(&r.path(), &["floor", "--json"]);
    assert!(matches!(res, Err(CliError::CheckFailed)));
    let report: Value = serde_json::from_str(&out).expect("json");
    assert_eq!(report["tool"], "mzizi check");
    assert_eq!(report["ok"], false);
    let v = &report["violations"][0];
    assert_eq!(v["file"], "a.css");
    assert_eq!(v["line"], 1);
    assert_eq!(v["rule"], "gap-below-floor");
    assert_eq!(v["minimum"], 7);
    assert_eq!(v["value"], 2);
}

#[test]
fn cli_takes_paths_exclude_spacing_unit_and_rem() {
    let r = Root::new();
    r.put("a/x.css", ".a { padding: 0 }");
    r.put("a/z.css", ".a { padding: 14px }");
    r.put("b/y.tsx", r#"<div className="px-2" />"#);
    assert!(matches!(
        run(&r.path(), &["floor", "a"]).1,
        Err(CliError::CheckFailed)
    ));
    assert!(
        run(&r.path(), &["floor", "a", "--exclude", "a/x.css"])
            .1
            .is_ok()
    );
    assert!(
        run(&r.path(), &["floor", "b", "--spacing-unit", "7"])
            .1
            .is_ok()
    );
    assert!(matches!(
        run(&r.path(), &["floor", "b", "--spacing-unit", "4"]).1,
        Err(CliError::CheckFailed)
    ));
    // `--rem` is accepted and applied: 1rem at 14px/rem is 14px.
    r.put("c/r.css", ".a { padding: 1rem }");
    assert!(
        run(&r.path(), &["floor", "c/r.css", "--rem", "14"])
            .1
            .is_ok()
    );
    assert!(matches!(
        run(&r.path(), &["floor", "c/r.css", "--rem", "8"]).1,
        Err(CliError::CheckFailed)
    ));
}

#[test]
fn cli_accepts_exclude_equals_dir_and_prints_help_for_h() {
    let r = Root::new();
    r.put("a/x.css", ".a { padding: 0 }");
    r.put("ok.css", ".a { padding: 14px }");
    assert!(run(&r.path(), &["floor", "--exclude=a"]).1.is_ok());
    let (out, res) = run(&r.path(), &["floor", "-h"]);
    assert!(res.is_ok());
    assert!(out.contains("USAGE"));
}

#[test]
fn cli_rejects_bad_usage_with_a_message() {
    let r = Root::new();
    for args in [
        vec!["floor", "--bogus"],
        vec!["floor", "--exclude"],
        vec!["floor", "--rem", "zero"],
        vec!["floor", "missing-dir"],
    ] {
        assert!(
            matches!(run(&r.path(), &args).1, Err(CliError::Msg(_))),
            "{args:?}"
        );
    }
}

#[test]
fn cli_is_documented_in_help() {
    let help = cli::HELP;
    assert!(help.contains("floor [path...]"));
    assert!(help.contains("mzizi-spacing-exempt"));
    assert!(help.contains("--json"));
}

// ---- the floor the CLI enforces ----

#[test]
fn the_floor_is_the_set_of_layer_tokens_the_mzizi_design_skill_teaches() {
    // Vendored copy of mzizi-skills/skills/mzizi-design/SKILL.md (tests/fixtures).
    let skill = include_str!("fixtures/mzizi-design-SKILL.md");
    assert!(skill.contains("--space-control-gap"));
    let rows: Vec<&str> = skill.lines().collect();
    for t in SPACING_LAYERS {
        let needle = format!("`{}`", t.token);
        let row = rows
            .iter()
            .find(|r| r.contains(&needle))
            .unwrap_or_else(|| panic!("{}", t.token));
        // Cells: [0] "", [1] layer, [2] token, [3] default, [4..5] reference, [6] dense
        let cells: Vec<&str> = row.split('|').map(str::trim).collect();
        assert_eq!(cells[3], js_num(t.px), "{}", t.token);
        if cells[6] != "none" {
            assert_eq!(cells[6], js_num(t.dense), "{}", t.token);
        }
    }
    assert!(skill.contains(&format!("{}px hit region", js_num(HIT_TOUCH_PX))));
    assert!(skill.contains(&format!("{}px hit region", js_num(HIT_DESKTOP_PX))));
    assert!(skill.contains("leading floor is 1.5"));
}

// ---- JSON and number formatting (JavaScript gave these for free) ----

#[test]
fn json_matches_json_stringify_layout_and_key_order() {
    let r = Root::new();
    r.put(
        "a.css",
        ".a { margin-left: -8px; padding: 0 }\n/* mzizi-spacing-exempt */",
    );
    let (out, _) = run(&r.path(), &["floor", "--json"]);
    let top: Vec<&str> = out
        .lines()
        .filter(|l| l.starts_with("  \""))
        .map(|l| l.split('"').nth(1).expect("key"))
        .collect();
    assert_eq!(
        top,
        vec![
            "tool",
            "rule",
            "version",
            "floor",
            "exemptMarker",
            "root",
            "filesScanned",
            "violations",
            "warnings",
            "exempted",
            "notices",
            "ok"
        ]
    );
    // A finding: rule, severity, layer?, found, minimum, value, message, line, column.
    assert!(out.contains("\"rule\": \"negative-margin\",\n      \"severity\": \"error\",\n      \"found\": \"margin-left: -8px\",\n      \"minimum\": 0,\n      \"value\": -8,"));
    // An empty array prints as `[]`, an integral number without a fraction.
    assert!(out.contains("\"warnings\": [],"));
    assert!(out.ends_with("}\n"));
}

#[test]
fn json_pretty_and_js_num_follow_javascript() {
    assert_eq!(js_num(4.0), "4");
    assert_eq!(js_num(1.5), "1.5");
    assert_eq!(js_num(-0.0), "0");
    assert_eq!(js_num(1e-7), "1e-7");
    assert_eq!(js_num(1e21), "1e+21");
    assert_eq!(
        json_pretty(&serde_json::json!({"a": [], "b": {}, "c": [1, 2.5]})),
        "{\n  \"a\": [],\n  \"b\": {},\n  \"c\": [\n    1,\n    2.5\n  ]\n}"
    );
}
