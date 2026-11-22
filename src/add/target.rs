//! Which implementation (type) of a component does this project want? Port of `target.ts`.
//!
//! Order (owner, 2026-11-04, mzizi-registry#635): Mzizi first, TypeScript second, native
//! (Swift, Kotlin) after. Picking the wrong type installs code the project cannot compile, so
//! the choice is never a guess: `--type`, then the project's own markers (if the component has
//! that type), then the only type the component has, otherwise a refusal listing the types.

use std::path::Path;

/// The registry origin. Every fetch is scoped to it.
pub const REGISTRY_ORIGIN: &str = "https://api.mzizi.dev";
/// The canonical published prefix.
pub const REGISTRY_PREFIX: &str = "/v1";
/// Where the Astro target installs every file.
pub const ASTRO_INSTALL_DIR: &str = "src/components/mzizi";

/// A type `add` can install.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Target {
    /// Mzizi Roots (Rust), `/v1/rs/`. A crate dependency, not a file copy.
    Mzizi,
    /// TypeScript (React), `/v1/ui/`.
    Tsx,
    /// Pure `.astro`, `/v1/astro/`.
    Astro,
    /// Swift files a `/v1/ui/` document declares.
    Swift,
    /// Kotlin files a `/v1/ui/` document declares.
    Kotlin,
}

/// Every type, in the order types are listed everywhere.
pub const TARGETS: [Target; 5] = [
    Target::Mzizi,
    Target::Tsx,
    Target::Astro,
    Target::Swift,
    Target::Kotlin,
];

impl Target {
    /// The type's name as typed on the command line.
    pub fn as_str(self) -> &'static str {
        match self {
            Target::Mzizi => "mzizi",
            Target::Tsx => "tsx",
            Target::Astro => "astro",
            Target::Swift => "swift",
            Target::Kotlin => "kotlin",
        }
    }

    /// The registry route segment. Native types are declared by `/v1/ui/` files.
    pub fn route(self) -> &'static str {
        match self {
            Target::Mzizi => "rs",
            Target::Tsx | Target::Swift | Target::Kotlin => "ui",
            Target::Astro => "astro",
        }
    }
}

impl std::fmt::Display for Target {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// `TARGETS.join(", ")`.
pub fn targets_joined(sep: &str) -> String {
    TARGETS
        .iter()
        .map(|t| t.as_str())
        .collect::<Vec<_>>()
        .join(sep)
}

/// JavaScript's `encodeURIComponent`.
pub fn encode_uri_component(s: &str) -> String {
    let mut out = String::new();
    for b in s.bytes() {
        if b.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

/// The URL of a component's document for a type.
pub fn registry_item_url(name: &str, target: Target) -> String {
    format!(
        "{REGISTRY_ORIGIN}{REGISTRY_PREFIX}/{}/{}",
        target.route(),
        encode_uri_component(name)
    )
}

/// True when the file's path says it is native code for `target` (swift, kotlin).
pub fn is_native_file(target: Target, file_path: &str) -> bool {
    let lower = file_path.to_lowercase();
    match target {
        Target::Swift => lower.ends_with(".swift"),
        Target::Kotlin => lower.ends_with(".kt") || lower.ends_with(".kts"),
        _ => false,
    }
}

/// What a `/v1/ui/` document's files declare: swift and/or kotlin for native files, tsx for
/// anything else.
pub fn declared_types(paths: &[&str]) -> Vec<Target> {
    let (mut swift, mut kotlin, mut tsx) = (false, false, false);
    for p in paths {
        if is_native_file(Target::Swift, p) {
            swift = true;
        } else if is_native_file(Target::Kotlin, p) {
            kotlin = true;
        } else {
            tsx = true;
        }
    }
    TARGETS
        .into_iter()
        .filter(|t| match t {
            Target::Swift => swift,
            Target::Kotlin => kotlin,
            Target::Tsx => tsx,
            _ => false,
        })
        .collect()
}

/// Parse a user-supplied type: a listed type or one of its aliases (`rust`, `roots`,
/// `typescript`, `ts`, `react`). Case-insensitive.
pub fn parse_target(value: Option<&str>) -> Option<Target> {
    let v = value?.to_lowercase();
    if let Some(t) = TARGETS.iter().find(|t| t.as_str() == v) {
        return Some(*t);
    }
    match v.as_str() {
        "rust" | "roots" => Some(Target::Mzizi),
        "typescript" | "ts" | "react" => Some(Target::Tsx),
        _ => None,
    }
}

/// True for a listed type (exact, no aliases).
pub fn is_target(value: &str) -> bool {
    TARGETS.iter().any(|t| t.as_str() == value)
}

/// "mzizi, tsx, swift" in the order types are served.
pub fn list_types(types: &[Target]) -> String {
    TARGETS
        .iter()
        .filter(|t| types.contains(t))
        .map(|t| t.as_str())
        .collect::<Vec<_>>()
        .join(", ")
}

/// What the project's files say.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TargetDetection {
    /// The inferred type, if the project settles it.
    pub target: Option<Target>,
    /// Why. Always printed, so an inferred choice is never invisible.
    pub reason: String,
}

/// The decision for one component.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Choice {
    /// A type, and why.
    Chosen {
        /// The type.
        target: Target,
        /// Why it was chosen.
        reason: String,
    },
    /// A refusal, as the message to print.
    Error(String),
}

/// Decide the type for one component. Pure.
pub fn choose_target(
    name: &str,
    available: &[Target],
    requested: Option<Target>,
    detected: Option<&TargetDetection>,
) -> Choice {
    if available.is_empty() {
        return Choice::Error(format!(
            "{name} has no installable type in the registry (none of {}).",
            targets_joined(", ")
        ));
    }
    if let Some(req) = requested {
        if !available.contains(&req) {
            return Choice::Error(format!(
                "{name} has no {req} build. Available types: {}. Pass --type with one of them.",
                list_types(available)
            ));
        }
        return Choice::Chosen {
            target: req,
            reason: "--target".into(),
        };
    }
    if let Some(d) = detected {
        if let Some(t) = d.target {
            if available.contains(&t) {
                return Choice::Chosen {
                    target: t,
                    reason: d.reason.clone(),
                };
            }
        }
    }
    if available.len() == 1 {
        let only = available[0];
        let looks = match detected.and_then(|d| d.target) {
            Some(t) => format!(" (the project looks like {t}, which {name} does not have)"),
            None => String::new(),
        };
        return Choice::Chosen {
            target: only,
            reason: format!("the only type {name} has in the registry{looks}"),
        };
    }
    let why = match detected {
        Some(d) if !d.reason.is_empty() => format!(": {}", d.reason),
        _ => String::new(),
    };
    Choice::Error(format!(
        "{name} comes in {} types ({}) and nothing in this project says which one it wants{why}. Pass --type {}.",
        available.len(),
        list_types(available),
        available
            .iter()
            .map(|t| t.as_str())
            .collect::<Vec<_>>()
            .join("|")
    ))
}

const ASTRO_CONFIGS: [&str; 4] = [
    "astro.config.mjs",
    "astro.config.ts",
    "astro.config.js",
    "astro.config.mts",
];

fn read_json(p: &Path) -> Option<serde_json::Value> {
    serde_json::from_str(&std::fs::read_to_string(p).ok()?).ok()
}

/// Read the project's markers and decide. Disk reads only: no network, no shelling out.
pub fn detect_target(root: &Path) -> TargetDetection {
    let abs = root;
    let pkg = read_json(&abs.join("package.json"));
    let has_cargo = abs.join("Cargo.toml").exists();
    let has_components_json = abs.join("components.json").exists();
    let astro_config = ASTRO_CONFIGS.iter().find(|c| abs.join(c).exists());

    let has_dep = |name: &str| {
        ["dependencies", "devDependencies"].iter().any(|k| {
            pkg.as_ref()
                .and_then(|p| p.get(k))
                .and_then(|d| d.as_object())
                .is_some_and(|d| d.contains_key(name))
        })
    };
    let has_react = has_dep("react") || has_dep("next");
    let has_astro = has_dep("astro") || astro_config.is_some();

    // Astro first, before the react check: an Astro project with a React island still
    // wants the pure `.astro` implementations.
    if has_astro {
        return TargetDetection {
            target: Some(Target::Astro),
            reason: format!(
                "Astro project ({}); installs pure .astro into {ASTRO_INSTALL_DIR}/",
                astro_config.copied().unwrap_or("astro in package.json")
            ),
        };
    }
    if has_cargo && (pkg.is_some() || has_components_json) {
        return TargetDetection {
            target: None,
            reason: format!(
                "both Cargo.toml and {} are present, so the target is genuinely ambiguous. Pass --type mzizi or --type tsx; guessing here is how a .tsx lands in a Cargo crate.",
                if pkg.is_some() {
                    "package.json"
                } else {
                    "components.json"
                }
            ),
        };
    }
    if has_cargo {
        return TargetDetection {
            target: Some(Target::Mzizi),
            reason: "Cargo.toml present and no package.json".into(),
        };
    }
    if has_components_json {
        return TargetDetection {
            target: Some(Target::Tsx),
            reason: "components.json present (shadcn-configured project)".into(),
        };
    }
    if has_react {
        return TargetDetection {
            target: Some(Target::Tsx),
            reason: "react or next in package.json dependencies".into(),
        };
    }
    if abs.join("Package.swift").exists() {
        return TargetDetection {
            target: Some(Target::Swift),
            reason: "Package.swift present".into(),
        };
    }
    for gradle in [
        "build.gradle.kts",
        "build.gradle",
        "settings.gradle.kts",
        "settings.gradle",
    ] {
        if abs.join(gradle).exists() {
            return TargetDetection {
                target: Some(Target::Kotlin),
                reason: format!("{gradle} present"),
            };
        }
    }
    TargetDetection {
        target: None,
        reason: format!(
            "no Cargo.toml, components.json, react/next dependency, Package.swift or Gradle file — nothing on disk says what this project is. Pass --type {}.",
            targets_joined("|")
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn project(files: &[(&str, &str)]) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for (n, b) in files {
            fs::write(dir.path().join(n), b).unwrap();
        }
        dir
    }
    fn det(files: &[(&str, &str)]) -> TargetDetection {
        let d = project(files);
        detect_target(d.path())
    }

    // target.test.ts > detectTarget
    #[test]
    fn infers_mzizi_from_cargo_toml_with_no_package_json() {
        let d = det(&[("Cargo.toml", "[package]\nname = \"x\"\n")]);
        assert_eq!(d.target, Some(Target::Mzizi));
        assert!(d.reason.contains("Cargo.toml"));
    }
    #[test]
    fn infers_tsx_from_components_json() {
        assert_eq!(det(&[("components.json", "{}")]).target, Some(Target::Tsx));
    }
    #[test]
    fn infers_tsx_from_a_react_dependency() {
        let d = det(&[("package.json", r#"{"dependencies":{"react":"^19"}}"#)]);
        assert_eq!(d.target, Some(Target::Tsx));
    }
    #[test]
    fn refuses_when_cargo_toml_sits_beside_package_json() {
        let d = det(&[
            ("Cargo.toml", "[package]\nname=\"x\"\n"),
            ("package.json", "{}"),
        ]);
        assert_eq!(d.target, None);
        assert!(d.reason.to_lowercase().contains("ambiguous"));
    }
    #[test]
    fn targets_astro_for_an_astro_project() {
        let d = det(&[
            ("astro.config.mjs", "export default {}"),
            ("package.json", "{}"),
        ]);
        assert_eq!(d.target, Some(Target::Astro));
        assert!(d.reason.contains("src/components/mzizi"));
    }
    #[test]
    fn targets_astro_even_when_react_is_also_present() {
        let d = det(&[
            ("astro.config.mjs", "export default {}"),
            (
                "package.json",
                r#"{"dependencies":{"react":"^19","astro":"^5"}}"#,
            ),
        ]);
        assert_eq!(d.target, Some(Target::Astro));
    }
    #[test]
    fn gives_up_with_an_actionable_reason_on_a_bare_directory() {
        let d = det(&[]);
        assert_eq!(d.target, None);
        assert!(d.reason.contains("--type"));
    }

    // target.test.ts > registryItemUrl
    #[test]
    fn routes_tsx_to_ui_mzizi_to_rs_and_astro_to_astro() {
        assert_eq!(
            registry_item_url("app-data-table", Target::Astro),
            "https://api.mzizi.dev/v1/astro/app-data-table"
        );
        assert_eq!(
            registry_item_url("button", Target::Tsx),
            "https://api.mzizi.dev/v1/ui/button"
        );
        assert_eq!(
            registry_item_url("button", Target::Mzizi),
            "https://api.mzizi.dev/v1/rs/button"
        );
    }
    #[test]
    fn encodes_the_name_so_it_cannot_escape_the_route() {
        let u = reqwest::Url::parse(&registry_item_url("../admin", Target::Tsx)).unwrap();
        assert_eq!(u.path(), "/v1/ui/..%2Fadmin");
    }
    #[test]
    fn rejects_anything_that_is_not_a_known_target() {
        assert!(is_target("tsx") && is_target("astro"));
        assert!(!is_target("svelte") && !is_target("wasm") && !is_target(""));
    }

    // target.test.ts > types: Mzizi first, TypeScript second, native after
    #[test]
    fn lists_every_served_type_in_that_order() {
        assert_eq!(targets_joined(","), "mzizi,tsx,astro,swift,kotlin");
    }
    #[test]
    fn accepts_old_and_long_spellings_and_not_dioxus() {
        assert_eq!(parse_target(Some("rust")), Some(Target::Mzizi));
        assert_eq!(parse_target(Some("typescript")), Some(Target::Tsx));
        assert_eq!(parse_target(Some("Swift")), Some(Target::Swift));
        assert_eq!(parse_target(Some("kotlin")), Some(Target::Kotlin));
        assert_eq!(parse_target(Some("dioxus")), None);
        assert!(!is_target("dioxus"));
    }
    #[test]
    fn serves_native_types_from_the_ui_document() {
        assert_eq!(
            registry_item_url("mzizi-tokens-swift", Target::Swift),
            "https://api.mzizi.dev/v1/ui/mzizi-tokens-swift"
        );
        assert_eq!(
            registry_item_url("mzizi-tokens-kotlin", Target::Kotlin),
            "https://api.mzizi.dev/v1/ui/mzizi-tokens-kotlin"
        );
    }

    // target.test.ts > declaredTypes
    #[test]
    fn declared_is_tsx_for_tsx_and_ts() {
        assert_eq!(
            declared_types(&["a/button.tsx", "b/u.ts"]),
            vec![Target::Tsx]
        );
    }
    #[test]
    fn declared_is_swift_for_swift_and_kotlin_for_kt() {
        assert_eq!(
            declared_types(&["lib/tokens/mzizi-tokens-swift.swift"]),
            vec![Target::Swift]
        );
        assert_eq!(
            declared_types(&["lib/tokens/mzizi-tokens-kotlin.kt"]),
            vec![Target::Kotlin]
        );
    }
    #[test]
    fn declared_is_empty_for_no_files() {
        assert!(declared_types(&[]).is_empty());
    }

    // target.test.ts > detectTarget: native projects
    #[test]
    fn infers_swift_from_package_swift() {
        assert_eq!(
            det(&[("Package.swift", "// swift")]).target,
            Some(Target::Swift)
        );
    }
    #[test]
    fn infers_kotlin_from_a_gradle_file() {
        assert_eq!(
            det(&[("build.gradle.kts", "")]).target,
            Some(Target::Kotlin)
        );
    }

    fn err_of(c: Choice) -> String {
        match c {
            Choice::Error(e) => e,
            other => panic!("expected an error, got {other:?}"),
        }
    }
    fn target_of(c: Choice) -> Target {
        match c {
            Choice::Chosen { target, .. } => target,
            other => panic!("expected a target, got {other:?}"),
        }
    }

    // target.test.ts > chooseTarget
    #[test]
    fn uses_type_when_the_component_has_it() {
        let c = choose_target(
            "button",
            &[Target::Mzizi, Target::Tsx],
            Some(Target::Tsx),
            None,
        );
        assert_eq!(target_of(c), Target::Tsx);
    }
    #[test]
    fn refuses_a_requested_type_the_component_lacks() {
        let e = err_of(choose_target(
            "button",
            &[Target::Mzizi, Target::Tsx],
            Some(Target::Swift),
            None,
        ));
        assert!(
            e.contains("no swift build. Available types: mzizi, tsx"),
            "{e}"
        );
    }
    #[test]
    fn takes_the_projects_type_when_the_component_has_it() {
        let d = TargetDetection {
            target: Some(Target::Tsx),
            reason: "components.json".into(),
        };
        let c = choose_target("button", &[Target::Mzizi, Target::Tsx], None, Some(&d));
        assert_eq!(
            c,
            Choice::Chosen {
                target: Target::Tsx,
                reason: "components.json".into()
            }
        );
    }
    #[test]
    fn takes_the_only_type_a_component_has() {
        let d = TargetDetection {
            target: Some(Target::Tsx),
            reason: "react".into(),
        };
        let c = choose_target("mzizi-tokens-swift", &[Target::Swift], None, Some(&d));
        assert_eq!(target_of(c), Target::Swift);
    }
    #[test]
    fn refuses_several_types_with_no_signal() {
        let e = err_of(choose_target(
            "button",
            &[Target::Mzizi, Target::Tsx, Target::Astro],
            None,
            None,
        ));
        assert!(
            e.contains("3 types (mzizi, tsx, astro)") && e.contains("--type mzizi|tsx|astro"),
            "{e}"
        );
    }
    #[test]
    fn refuses_a_component_with_no_type_at_all() {
        assert!(matches!(
            choose_target("x", &[], None, None),
            Choice::Error(_)
        ));
    }
    #[test]
    fn treats_astro_as_a_first_class_type() {
        let all = [Target::Mzizi, Target::Tsx, Target::Astro];
        assert_eq!(
            target_of(choose_target("button", &all, Some(Target::Astro), None)),
            Target::Astro
        );
        let d = TargetDetection {
            target: Some(Target::Astro),
            reason: "Astro project".into(),
        };
        assert_eq!(
            target_of(choose_target("button", &all, None, Some(&d))),
            Target::Astro
        );
        let e = err_of(choose_target(
            "button",
            &[Target::Tsx],
            Some(Target::Astro),
            None,
        ));
        assert!(e.contains("Available types: tsx"));
    }
}
