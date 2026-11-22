//! Turn a resolved registry closure into an install plan, then apply it. Port of `install.ts`.
//!
//! Resolve everything before writing anything, and treat a registry document as remote input:
//! every `files[].target` goes through [`resolve_within`] before it touches the project.

use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};

use base64::Engine;

use super::registry::{RegistryCrate, RegistryFile, RegistryItem, ResolvedItem};
use super::target::{Target, is_native_file, list_types};

/// One file the plan would write.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedWrite {
    /// Project-relative destination.
    pub path: String,
    /// Size on disk once decoded.
    pub bytes: usize,
    /// Which registry item it came from.
    pub from: String,
    /// Whether the destination already exists.
    pub exists: bool,
}

/// What an install would do.
#[derive(Debug, Clone)]
pub struct InstallPlan {
    /// The chosen type.
    pub target: Target,
    /// The closure, dependencies first.
    pub items: Vec<ResolvedItem>,
    /// Files to write.
    pub writes: Vec<PlannedWrite>,
    /// npm packages to add, deduplicated and sorted.
    pub npm_dependencies: Vec<String>,
    /// crates.io dependencies declared by `registry:rust` documents.
    pub crates: Vec<RegistryCrate>,
    /// Non-fatal things the user must be told.
    pub warnings: Vec<String>,
}

/// Lexically normalise `root.join(rel)` (like Node's `path.resolve`).
fn lexical_resolve(root: &Path, rel: &str) -> PathBuf {
    let joined = if Path::new(rel).is_absolute() {
        PathBuf::from(rel)
    } else {
        root.join(rel)
    };
    let mut out = PathBuf::new();
    for c in joined.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Resolve a relative project path to an absolute one, refusing to escape the project root.
pub fn resolve_within(root: &Path, rel: &str) -> Option<PathBuf> {
    let abs_root = lexical_resolve(
        &std::env::current_dir().unwrap_or_default(),
        &root.to_string_lossy(),
    );
    let resolved = lexical_resolve(&abs_root, rel);
    resolved.starts_with(&abs_root).then_some(resolved)
}

/// Where a file lands: the document's `target`, else its `path`. Strips ONLY a leading `./`
/// and shadcn's `~/`; a hostile `../../x` must reach the sandbox, not be normalised into it.
fn destination_for(file: &RegistryFile) -> String {
    let dest = file.target.as_deref().unwrap_or(&file.path);
    let dest = dest.strip_prefix("~/").unwrap_or(dest);
    dest.strip_prefix("./").unwrap_or(dest).to_string()
}

/// The `mzizi` type is a crate dependency, not a file copy. Token artifacts are written.
fn is_crate_owned_source(file: &RegistryFile) -> bool {
    file.kind == "registry:rust" && !file.path.contains("n1-tokens/")
}

/// Decode like Node's `Buffer.from(s, "base64")`: lenient about padding, whitespace and the
/// URL-safe alphabet.
fn decode_base64(s: &str) -> Vec<u8> {
    let mut clean: Vec<u8> = s
        .bytes()
        .take_while(|&b| b != b'=')
        .filter_map(|b| match b {
            b'-' => Some(b'+'),
            b'_' => Some(b'/'),
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'+' | b'/' => Some(b),
            _ => None,
        })
        .collect();
    if clean.len() % 4 == 1 {
        clean.pop();
    }
    base64::engine::general_purpose::STANDARD_NO_PAD
        .decode(&clean)
        .unwrap_or_default()
}

fn bytes_of(file: &RegistryFile) -> Vec<u8> {
    let content = file.content.as_deref().unwrap_or("");
    if file.encoding.as_deref() == Some("base64") {
        decode_base64(content)
    } else {
        content.as_bytes().to_vec()
    }
}

fn assert_astro(r: &ResolvedItem) -> Result<(), String> {
    if r.item.target.as_deref() != Some("astro") {
        return Err(format!(
            "{} ({}) is not an Astro document, so it cannot be installed with --type astro. Every component in an Astro closure needs its own .astro in the registry; this one does not have one yet.",
            r.item.name, r.url
        ));
    }
    Ok(())
}

fn assert_native(r: &ResolvedItem, target: Target) -> Result<(), String> {
    if !r.item.files.iter().any(|f| is_native_file(target, &f.path)) {
        return Err(format!(
            "{} ({}) has no {target} files, so it cannot be installed with --type {target}. Types that component's closure can take: {}.",
            r.item.name,
            r.url,
            list_types(&[Target::Mzizi, Target::Tsx, Target::Astro])
        ));
    }
    Ok(())
}

fn retired_target_warning(item: &RegistryItem) -> Option<String> {
    (item.target.as_deref().map(str::to_lowercase).as_deref() == Some("dioxus")).then(|| {
        format!(
            "{}: the registry document names a retired build target (\"dioxus\"). It is ignored; the type is chosen by the files the document declares (mzizi-registry#635).",
            item.name
        )
    })
}

/// Build the plan. Pure apart from checking which destinations already exist.
pub fn build_install_plan(
    items: &[ResolvedItem],
    target: Target,
    project_root: &Path,
) -> Result<InstallPlan, String> {
    let mut writes = Vec::new();
    let mut npm = BTreeSet::new();
    let mut crates: Vec<RegistryCrate> = Vec::new();
    let mut warnings = Vec::new();
    let mut claimed: Vec<(String, String)> = Vec::new();

    for resolved in items {
        let (item, url) = (&resolved.item, &resolved.url);
        if target == Target::Astro {
            assert_astro(resolved)?;
        }
        if matches!(target, Target::Swift | Target::Kotlin) {
            assert_native(resolved, target)?;
        }
        warnings.extend(retired_target_warning(item));
        npm.extend(item.dependencies.iter().cloned());
        if let Some(c) = &item.krate {
            match crates.iter_mut().find(|e| e.name == c.name) {
                Some(slot) => *slot = c.clone(),
                None => crates.push(c.clone()),
            }
        }
        for file in &item.files {
            if file.content.is_none() {
                warnings.push(format!(
                    "{}: {} carries no content and was skipped",
                    item.name, file.path
                ));
                continue;
            }
            if target == Target::Mzizi && is_crate_owned_source(file) {
                continue;
            }
            let rel = destination_for(file);
            let abs = resolve_within(project_root, &rel).ok_or_else(|| {
                format!(
                    "refusing to install {}: its file targets {rel}, which resolves outside the project root. The document came from {url}.",
                    item.name
                )
            })?;
            if let Some((_, prev)) = claimed.iter().find(|(r, _)| *r == rel) {
                if *prev != item.name {
                    return Err(format!(
                        "both {prev} and {} want to write {rel}. Two registry documents claiming one destination is a registry defect; install them separately and report it.",
                        item.name
                    ));
                }
            }
            match claimed.iter_mut().find(|(r, _)| *r == rel) {
                Some(slot) => slot.1 = item.name.clone(),
                None => claimed.push((rel.clone(), item.name.clone())),
            }
            writes.push(PlannedWrite {
                path: rel,
                bytes: bytes_of(file).len(),
                from: item.name.clone(),
                exists: abs.exists(),
            });
        }
    }
    if target == Target::Mzizi && crates.is_empty() {
        warnings.push(
            "no crate dependency was declared by any resolved document — nothing will be added to Cargo.toml".into(),
        );
    }
    Ok(InstallPlan {
        target,
        items: items.to_vec(),
        writes,
        npm_dependencies: npm.into_iter().collect(),
        crates,
        warnings,
    })
}

/// Render the plan for a human, before anything is written.
pub fn render_plan(plan: &InstallPlan, root_name: &str, overwrite: bool) -> String {
    let label = |exists: bool| match (exists, overwrite) {
        (false, _) => "create       ",
        (true, true) => "overwrite    ",
        (true, false) => "skip (exists)",
    };
    let mut lines: Vec<String> = Vec::new();
    lines.push(format!("mz add {root_name} — target {}", plan.target));
    lines.push(String::new());
    let deps = plan
        .items
        .iter()
        .filter(|i| !i.required_by.is_empty())
        .count();
    let mut head = format!("{} registry item(s)", plan.items.len());
    if deps > 0 {
        head.push_str(&format!(", {deps} pulled in as dependencies"));
    }
    lines.push(head);
    for r in &plan.items {
        let parents = r
            .required_by
            .iter()
            .map(|u| {
                plan.items
                    .iter()
                    .find(|i| i.url == *u)
                    .map_or(u.as_str(), |i| i.item.name.as_str())
            })
            .collect::<Vec<_>>()
            .join(", ");
        let by = if parents.is_empty() {
            String::new()
        } else {
            format!("  (required by {parents})")
        };
        lines.push(format!("  - {}{by}", r.item.name));
    }
    lines.push(String::new());
    if plan.writes.is_empty() {
        lines.push("No files to write.".into());
        lines.push(String::new());
    } else {
        lines.push(format!("{} file(s):", plan.writes.len()));
        for w in &plan.writes {
            lines.push(format!(
                "  {} {}  ({} bytes, from {})",
                label(w.exists),
                w.path,
                w.bytes,
                w.from
            ));
        }
        lines.push(String::new());
    }
    if !plan.npm_dependencies.is_empty() {
        lines.push(format!(
            "npm dependencies: {}",
            plan.npm_dependencies.join(", ")
        ));
    }
    for c in &plan.crates {
        let v = c
            .version
            .as_ref()
            .map(|v| format!("@{v}"))
            .unwrap_or_default();
        lines.push(format!("crate dependency: {}{v} ({})", c.name, c.registry));
    }
    for w in &plan.warnings {
        lines.push(format!("warning: {w}"));
    }
    lines.join("\n")
}

/// The outcome of [`apply_plan`].
#[derive(Debug, Default, PartialEq, Eq)]
pub struct ApplyResult {
    /// Files written.
    pub written: Vec<String>,
    /// Existing files left alone.
    pub skipped: Vec<String>,
}

/// Write the plan. `overwrite` is opt-in: silently replacing an edited file cannot be undone.
pub fn apply_plan(
    plan: &InstallPlan,
    project_root: &Path,
    overwrite: bool,
) -> Result<ApplyResult, String> {
    let mut contents: Vec<(String, Vec<u8>)> = Vec::new();
    for r in &plan.items {
        for file in &r.item.files {
            if file.content.is_none()
                || (plan.target == Target::Mzizi && is_crate_owned_source(file))
            {
                continue;
            }
            let dest = destination_for(file);
            let bytes = bytes_of(file);
            match contents.iter_mut().find(|(d, _)| *d == dest) {
                Some(slot) => slot.1 = bytes,
                None => contents.push((dest, bytes)),
            }
        }
    }
    let mut result = ApplyResult::default();
    for w in &plan.writes {
        if w.exists && !overwrite {
            result.skipped.push(w.path.clone());
            continue;
        }
        let abs = resolve_within(project_root, &w.path)
            .ok_or_else(|| format!("refusing to write outside project root: {}", w.path))?;
        let body = contents
            .iter()
            .find(|(d, _)| *d == w.path)
            .map(|(_, b)| b)
            .ok_or_else(|| format!("internal: no content for planned write {}", w.path))?;
        if let Some(dir) = abs.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        std::fs::write(&abs, body).map_err(|e| e.to_string())?;
        result.written.push(w.path.clone());
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }
    fn resolved(item: RegistryItem, required_by: &[&str]) -> ResolvedItem {
        ResolvedItem {
            url: format!("https://api.mzizi.dev/v1/ui/{}", item.name),
            item,
            required_by: required_by.iter().map(|s| s.to_string()).collect(),
        }
    }
    fn file(path: &str, kind: &str, target: Option<&str>, content: Option<&str>) -> RegistryFile {
        RegistryFile {
            path: path.into(),
            kind: kind.into(),
            target: target.map(Into::into),
            content: content.map(Into::into),
            encoding: None,
        }
    }
    fn tsx_item(name: &str, target: &str, content: &str) -> RegistryItem {
        RegistryItem {
            name: name.into(),
            kind: "registry:ui".into(),
            files: vec![file(
                &format!("components/registry/n2-primitives/{name}.tsx"),
                "registry:ui",
                Some(target),
                Some(content),
            )],
            ..Default::default()
        }
    }
    fn plan_of(items: Vec<RegistryItem>, t: Target, dir: &Path) -> Result<InstallPlan, String> {
        let r: Vec<_> = items.into_iter().map(|i| resolved(i, &[])).collect();
        build_install_plan(&r, t, dir)
    }
    fn rust_button() -> RegistryItem {
        RegistryItem {
            name: "button".into(),
            kind: "registry:ui".into(),
            krate: Some(RegistryCrate {
                name: "mzizi-ui".into(),
                registry: "crates.io".into(),
                version: None,
            }),
            files: vec![file(
                "components/registry/n2-primitives/button.rs",
                "registry:rust",
                None,
                Some("// rs"),
            )],
            ..Default::default()
        }
    }

    // install.test.ts > buildInstallPlan — path safety
    #[test]
    fn refuses_a_file_whose_target_escapes_the_project_root() {
        let d = root();
        let e = plan_of(
            vec![tsx_item("evil", "../../../etc/cron.d/payload", "x")],
            Target::Tsx,
            d.path(),
        )
        .unwrap_err();
        assert!(e.contains("outside the project root"), "{e}");
    }
    #[test]
    fn refuses_an_absolute_target_path_rather_than_relocating_it() {
        let d = root();
        let e = plan_of(
            vec![tsx_item("evil", "/etc/passwd", "x")],
            Target::Tsx,
            d.path(),
        )
        .unwrap_err();
        assert!(e.contains("outside the project root"), "{e}");
    }
    #[test]
    fn accepts_the_two_prefixes_that_mean_the_project_root() {
        let d = root();
        let p = plan_of(
            vec![
                tsx_item("a", "./components/ui/a.tsx", "x"),
                tsx_item("b", "~/components/ui/b.tsx", "x"),
            ],
            Target::Tsx,
            d.path(),
        )
        .unwrap();
        let paths: Vec<_> = p.writes.iter().map(|w| w.path.as_str()).collect();
        assert_eq!(paths, ["components/ui/a.tsx", "components/ui/b.tsx"]);
    }
    #[test]
    fn refuses_when_two_components_claim_the_same_destination() {
        let d = root();
        let e = plan_of(
            vec![
                tsx_item("alpha", "components/ui/shared.tsx", "x"),
                tsx_item("beta", "components/ui/shared.tsx", "x"),
            ],
            Target::Tsx,
            d.path(),
        )
        .unwrap_err();
        assert!(e.contains("both alpha and beta want to write"), "{e}");
    }

    // install.test.ts > buildInstallPlan — targets
    #[test]
    fn collects_npm_dependencies_deduplicated_and_sorted() {
        let d = root();
        let mut a = tsx_item("a", "components/ui/a.tsx", "x");
        a.dependencies = vec!["radix-ui".into(), "cva".into()];
        let mut b = tsx_item("b", "components/ui/b.tsx", "x");
        b.dependencies = vec!["cva".into()];
        let p = plan_of(vec![a, b], Target::Tsx, d.path()).unwrap();
        assert_eq!(p.npm_dependencies, ["cva", "radix-ui"]);
    }
    #[test]
    fn does_not_copy_rs_component_sources_rust_is_a_crate_dependency() {
        let d = root();
        let p = plan_of(vec![rust_button()], Target::Mzizi, d.path()).unwrap();
        assert!(p.writes.is_empty());
        assert_eq!(p.crates.len(), 1);
        assert_eq!(p.crates[0].name, "mzizi-ui");
    }
    #[test]
    fn does_copy_the_rust_token_artifacts() {
        let d = root();
        let item = RegistryItem {
            name: "nyuchi-tokens-rust".into(),
            kind: "registry:ui".into(),
            files: vec![file(
                "components/registry/n1-tokens/nyuchi-tokens-rust.rs",
                "registry:rust",
                None,
                Some("// tokens"),
            )],
            ..Default::default()
        };
        let p = plan_of(vec![item], Target::Mzizi, d.path()).unwrap();
        let paths: Vec<_> = p.writes.iter().map(|w| w.path.as_str()).collect();
        assert_eq!(
            paths,
            ["components/registry/n1-tokens/nyuchi-tokens-rust.rs"]
        );
    }
    #[test]
    fn tolerates_a_document_that_still_names_the_retired_dioxus_target() {
        let d = root();
        let mut item = rust_button();
        item.target = Some("dioxus".into());
        let p = plan_of(vec![item], Target::Mzizi, d.path()).unwrap();
        assert_eq!(p.crates[0].name, "mzizi-ui");
        assert!(
            p.warnings
                .join("\n")
                .contains("retired build target (\"dioxus\")")
        );
    }
    #[test]
    fn installs_the_swift_and_kotlin_files_an_item_declares() {
        let d = root();
        let item = RegistryItem {
            name: "mzizi-tokens-swift".into(),
            kind: "registry:lib".into(),
            files: vec![file(
                "components/registry/n1-tokens/mzizi-tokens-swift.swift",
                "registry:lib",
                Some("lib/tokens/mzizi-tokens-swift.swift"),
                Some("// swift"),
            )],
            ..Default::default()
        };
        let p = plan_of(vec![item], Target::Swift, d.path()).unwrap();
        assert_eq!(p.writes[0].path, "lib/tokens/mzizi-tokens-swift.swift");
    }
    #[test]
    fn refuses_a_tsx_only_item_under_type_swift() {
        let d = root();
        let e = plan_of(
            vec![tsx_item("a", "components/ui/a.tsx", "x")],
            Target::Swift,
            d.path(),
        )
        .unwrap_err();
        assert!(e.contains("has no swift files"), "{e}");
    }
    #[test]
    fn still_copies_rs_sources_when_the_target_is_tsx() {
        let d = root();
        let mut item = rust_button();
        item.krate = None;
        let p = plan_of(vec![item], Target::Tsx, d.path()).unwrap();
        assert_eq!(p.writes.len(), 1);
    }
    #[test]
    fn warns_rather_than_silently_dropping_a_file_with_no_content() {
        let d = root();
        let item = RegistryItem {
            name: "hollow".into(),
            kind: "registry:ui".into(),
            files: vec![file(
                "x.tsx",
                "registry:ui",
                Some("components/ui/x.tsx"),
                None,
            )],
            ..Default::default()
        };
        let p = plan_of(vec![item], Target::Tsx, d.path()).unwrap();
        assert!(p.writes.is_empty());
        assert!(p.warnings.join(" ").contains("no content"));
    }

    // install.test.ts > applyPlan
    #[test]
    fn writes_planned_files_and_creates_missing_directories() {
        let d = root();
        let r = vec![resolved(
            tsx_item("button", "components/ui/button.tsx", "// button"),
            &[],
        )];
        let p = build_install_plan(&r, Target::Tsx, d.path()).unwrap();
        let res = apply_plan(&p, d.path(), false).unwrap();
        assert_eq!(res.written, ["components/ui/button.tsx"]);
        assert_eq!(
            std::fs::read_to_string(d.path().join("components/ui/button.tsx")).unwrap(),
            "// button"
        );
    }
    #[test]
    fn does_not_overwrite_an_existing_file_unless_asked() {
        let d = root();
        std::fs::write(d.path().join("button.tsx"), "MINE").unwrap();
        let r = vec![resolved(
            tsx_item("button", "button.tsx", "// registry"),
            &[],
        )];
        let p = build_install_plan(&r, Target::Tsx, d.path()).unwrap();
        let res = apply_plan(&p, d.path(), false).unwrap();
        assert!(res.written.is_empty());
        assert_eq!(res.skipped, ["button.tsx"]);
        assert_eq!(
            std::fs::read_to_string(d.path().join("button.tsx")).unwrap(),
            "MINE"
        );
    }
    #[test]
    fn overwrites_when_overwrite_was_passed() {
        let d = root();
        std::fs::write(d.path().join("button.tsx"), "MINE").unwrap();
        let r = vec![resolved(
            tsx_item("button", "button.tsx", "// registry"),
            &[],
        )];
        let p = build_install_plan(&r, Target::Tsx, d.path()).unwrap();
        apply_plan(&p, d.path(), true).unwrap();
        assert_eq!(
            std::fs::read_to_string(d.path().join("button.tsx")).unwrap(),
            "// registry"
        );
    }

    // install.test.ts > renderPlan
    #[test]
    fn render_names_the_component_that_pulled_in_each_dependency() {
        let d = root();
        let parent = resolved(
            tsx_item("address-input", "components/ui/address-input.tsx", "x"),
            &[],
        );
        let child = resolved(
            tsx_item("input", "components/ui/input.tsx", "x"),
            &["https://api.mzizi.dev/v1/ui/address-input"],
        );
        let p = build_install_plan(&[child, parent], Target::Tsx, d.path()).unwrap();
        assert!(
            render_plan(&p, "address-input", false)
                .contains("- input  (required by address-input)")
        );
    }

    // install.test.ts > the Astro target
    fn astro_item(name: &str, extra: Vec<RegistryFile>) -> ResolvedItem {
        let mut files = vec![file(
            &format!("components/registry/n3-brand/{name}.astro"),
            "registry:astro",
            Some(&format!("src/components/mzizi/{name}.astro")),
            Some("---\n---\n<span />\n"),
        )];
        files.extend(extra);
        ResolvedItem {
            url: format!("https://api.mzizi.dev/v1/astro/{name}"),
            item: RegistryItem {
                name: name.into(),
                kind: "registry:astro".into(),
                target: Some("astro".into()),
                files,
                ..Default::default()
            },
            required_by: vec![],
        }
    }
    #[test]
    fn writes_astro_files_and_decodes_base64_assets_byte_for_byte() {
        let d = root();
        let png: Vec<u8> = vec![0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a, 0xff, 0x00];
        let mut asset = file(
            "components/registry/assets/mark.png",
            "registry:asset",
            Some("src/components/mzizi/assets/mark.png"),
            Some(&base64::engine::general_purpose::STANDARD.encode(&png)),
        );
        asset.encoding = Some("base64".into());
        let items = vec![astro_item("app-brand-mark", vec![asset])];
        let p = build_install_plan(&items, Target::Astro, d.path()).unwrap();
        assert_eq!(
            p.writes
                .iter()
                .find(|w| w.path.ends_with("mark.png"))
                .unwrap()
                .bytes,
            png.len()
        );
        apply_plan(&p, d.path(), false).unwrap();
        assert_eq!(
            std::fs::read(d.path().join("src/components/mzizi/assets/mark.png")).unwrap(),
            png
        );
        assert!(
            std::fs::read_to_string(d.path().join("src/components/mzizi/app-brand-mark.astro"))
                .unwrap()
                .contains("<span />")
        );
    }
    #[test]
    fn refuses_a_closure_that_steps_off_the_astro_target_naming_the_item() {
        let d = root();
        let react = resolved(tsx_item("dialog", "components/ui/dialog.tsx", "x"), &[]);
        let e = build_install_plan(
            &[react, astro_item("app-shell", vec![])],
            Target::Astro,
            d.path(),
        )
        .unwrap_err();
        assert!(
            e.contains("dialog ") && e.contains("is not an Astro document"),
            "{e}"
        );
    }

    // install.test.ts > renderPlan — labels match what applyPlan will do
    fn existing() -> (tempfile::TempDir, InstallPlan) {
        let d = root();
        std::fs::write(d.path().join("button.tsx"), "// mine").unwrap();
        let r = vec![resolved(tsx_item("button", "button.tsx", "x"), &[])];
        let p = build_install_plan(&r, Target::Tsx, d.path()).unwrap();
        (d, p)
    }
    #[test]
    fn says_skip_exists_not_overwrite_when_overwrite_is_not_passed() {
        let (_d, p) = existing();
        let t = render_plan(&p, "button", false);
        assert!(
            t.contains("skip (exists) button.tsx") && !t.contains("overwrite"),
            "{t}"
        );
    }
    #[test]
    fn says_overwrite_only_when_overwrite_will_replace_the_file() {
        let (_d, p) = existing();
        assert!(render_plan(&p, "button", true).contains("overwrite     button.tsx"));
    }

    // Beyond the vitest suite.
    #[test]
    fn base64_decoding_is_as_lenient_as_nodes() {
        assert_eq!(decode_base64("aGk"), b"hi");
        assert_eq!(decode_base64("aGk="), b"hi");
        assert_eq!(decode_base64("a G\nk"), b"hi");
    }
}
