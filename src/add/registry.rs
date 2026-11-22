//! The Mzizi registry client: fetch a component document, then resolve its transitive
//! `registryDependencies` closure. Port of `registry.ts`.

use std::collections::{HashMap, VecDeque};
use std::time::Duration;

use serde::Deserialize;
use serde_json::Value;

use super::target::{REGISTRY_ORIGIN, REGISTRY_PREFIX, Target, declared_types, registry_item_url};
use crate::Env;

/// An HTTP answer.
#[derive(Debug, Clone)]
pub struct Response {
    /// The status code.
    pub status: u16,
    /// The body, as text.
    pub body: String,
}

/// The one door to the network, so tests and the parity harness never touch it.
pub trait Fetch: Sync {
    /// GET `url`. `Err` is a transport failure (DNS, TLS, timeout), not an HTTP status.
    fn get(&self, url: &str, user_agent: &str, timeout: Duration) -> Result<Response, String>;
}

/// The real transport: blocking reqwest with rustls.
///
/// `MZ_REGISTRY_BASE` and `MZ_CRATES_BASE` (for example `http://127.0.0.1:8080`) replace the
/// scheme and host of requests to `https://api.mzizi.dev` and `https://crates.io`. They exist
/// so a fixture server can stand in for the network; URL validation still sees the real URL.
pub struct HttpFetch {
    client: reqwest::blocking::Client,
    registry_base: Option<String>,
    crates_base: Option<String>,
}

impl HttpFetch {
    /// Build the transport, reading the two override variables from `env`.
    pub fn new(env: &Env) -> Self {
        let base = |k: &str| {
            env.get_trimmed(k)
                .map(|v| v.trim_end_matches('/').to_string())
        };
        HttpFetch {
            client: reqwest::blocking::Client::builder()
                .build()
                .expect("http client"),
            registry_base: base("MZ_REGISTRY_BASE"),
            crates_base: base("MZ_CRATES_BASE"),
        }
    }

    fn rewrite(&self, url: &str) -> String {
        for (origin, base) in [
            (REGISTRY_ORIGIN, &self.registry_base),
            ("https://crates.io", &self.crates_base),
        ] {
            if let (Some(rest), Some(b)) = (url.strip_prefix(origin), base) {
                return format!("{b}{rest}");
            }
        }
        url.to_string()
    }
}

impl Fetch for HttpFetch {
    fn get(&self, url: &str, user_agent: &str, timeout: Duration) -> Result<Response, String> {
        let res = self
            .client
            .get(self.rewrite(url))
            .header("accept", "application/json")
            .header("user-agent", user_agent)
            .timeout(timeout)
            .send()
            .map_err(|e| e.to_string())?;
        let status = res.status().as_u16();
        let body = res.text().map_err(|e| e.to_string())?;
        Ok(Response { status, body })
    }
}

/// A file inside a registry document. `target` is where it lands in a project.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct RegistryFile {
    /// The registry-internal path (the fallback destination).
    #[serde(default, deserialize_with = "lenient_string")]
    pub path: String,
    /// shadcn's discriminator plus Mzizi's `registry:rust`.
    #[serde(default, rename = "type", deserialize_with = "lenient_string")]
    pub kind: String,
    /// Where it lands in a project.
    #[serde(default, deserialize_with = "lenient_opt_string")]
    pub target: Option<String>,
    /// The file body; `None` when the document carries none (or a non-string).
    #[serde(default, deserialize_with = "lenient_opt_string")]
    pub content: Option<String>,
    /// `base64` on a binary asset.
    #[serde(default, deserialize_with = "lenient_opt_string")]
    pub encoding: Option<String>,
}

/// A crates.io dependency, declared by `registry:rust` documents.
#[derive(Debug, Clone, Default, Deserialize, PartialEq, Eq)]
pub struct RegistryCrate {
    /// The crate name.
    #[serde(default, deserialize_with = "lenient_string")]
    pub name: String,
    /// The registry it lives on.
    #[serde(default, deserialize_with = "lenient_string")]
    pub registry: String,
    /// A version requirement, when declared.
    #[serde(default, deserialize_with = "lenient_opt_string")]
    pub version: Option<String>,
}

/// One registry document, in shadcn's `registry-item.json` shape.
#[derive(Debug, Clone, Default, Deserialize, PartialEq)]
pub struct RegistryItem {
    /// The component name.
    #[serde(default, deserialize_with = "lenient_string")]
    pub name: String,
    /// The document type.
    #[serde(default, rename = "type", deserialize_with = "lenient_string")]
    pub kind: String,
    /// The build target the document names (read only for `astro` and a retired `dioxus`).
    #[serde(default, deserialize_with = "lenient_opt_string")]
    pub target: Option<String>,
    /// npm package dependencies.
    #[serde(default, deserialize_with = "lenient_strings")]
    pub dependencies: Vec<String>,
    /// Absolute URLs of other registry items.
    #[serde(
        default,
        rename = "registryDependencies",
        deserialize_with = "lenient_strings"
    )]
    pub registry_dependencies: Vec<String>,
    /// The crate, on `registry:rust` documents.
    #[serde(default, rename = "crate", deserialize_with = "lenient_crate")]
    pub krate: Option<RegistryCrate>,
    /// The files.
    #[serde(default, deserialize_with = "lenient_files")]
    pub files: Vec<RegistryFile>,
}

// The TypeScript client never validated a document beyond `name`; a field of the wrong type
// was simply absent. These keep that: a wrong type reads as missing instead of failing.
fn lenient_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<String, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::String(s) => s,
        _ => String::new(),
    })
}
fn lenient_opt_string<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<String>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::String(s) => Some(s),
        _ => None,
    })
}
fn lenient_strings<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Array(a) => a
            .into_iter()
            .filter_map(|v| v.as_str().map(String::from))
            .collect(),
        _ => Vec::new(),
    })
}
fn lenient_crate<'de, D: serde::Deserializer<'de>>(
    d: D,
) -> Result<Option<RegistryCrate>, D::Error> {
    let v = Value::deserialize(d)?;
    Ok(if v.is_object() {
        serde_json::from_value(v).ok()
    } else {
        None
    })
}
fn lenient_files<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<RegistryFile>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Array(a) => a
            .into_iter()
            .filter(Value::is_object)
            .filter_map(|v| serde_json::from_value(v).ok())
            .collect(),
        _ => Vec::new(),
    })
}

/// A resolved item plus the URL it came from, so errors can name the source.
#[derive(Debug, Clone)]
pub struct ResolvedItem {
    /// Where it came from.
    pub url: String,
    /// The document.
    pub item: RegistryItem,
    /// URLs of the items that pulled this one in. Empty for the root request.
    pub required_by: Vec<String>,
}

const UA: &str = "mzizi-cli";

/// Reject anything that is not an absolute https URL. A bare name resolves against shadcn's
/// own registry, installs a different component and reports success.
fn assert_fetchable_url(raw: &str, required_by: &[String]) -> Result<String, String> {
    let parsed = reqwest::Url::parse(raw).map_err(|_| {
        let by = required_by.last().map(|l| format!(" (required by {l})")).unwrap_or_default();
        format!(
            "registryDependencies entry \"{raw}\" is not an absolute URL{by}.\nA bare name resolves against shadcn's own registry, not Mzizi's — it would install a different component and report success. Write the full URL: {REGISTRY_ORIGIN}{REGISTRY_PREFIX}/ui/{raw}"
        )
    })?;
    if parsed.scheme() != "https" {
        return Err(format!("refusing to fetch {raw}: not https"));
    }
    Ok(parsed.to_string())
}

/// Fetch one registry document.
pub fn fetch_item(fetch: &dyn Fetch, url: &str) -> Result<RegistryItem, String> {
    let res = fetch.get(url, UA, Duration::from_secs(30))?;
    if !(200..300).contains(&res.status) {
        return Err(format!("HTTP {} from {url}", res.status));
    }
    let body: Value =
        serde_json::from_str(&res.body).map_err(|e| format!("{url} returned invalid JSON: {e}"))?;
    if !body.is_object() {
        return Err(format!("{url} did not return a registry document"));
    }
    match body.get("name") {
        Some(Value::String(s)) if !s.is_empty() => {}
        _ => return Err(format!("{url} returned a document with no name")),
    }
    serde_json::from_value(body).map_err(|e| format!("{url} is not a registry document: {e}"))
}

/// Resolve an item and everything it depends on, breadth-first, dependencies BEFORE the items
/// that need them. Keyed on the resolved URL; a visited set makes a diamond fetch once and a
/// cycle terminate.
pub fn resolve_closure(
    fetch: &dyn Fetch,
    root_url: &str,
    max_items: usize,
) -> Result<Vec<ResolvedItem>, String> {
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut order: Vec<ResolvedItem> = Vec::new();
    let mut queue: VecDeque<(String, Vec<String>)> = VecDeque::new();
    queue.push_back((root_url.to_string(), Vec::new()));

    while let Some((raw, required_by)) = queue.pop_front() {
        let url = assert_fetchable_url(&raw, &required_by)?;
        if let Some(&i) = seen.get(&url) {
            for r in required_by {
                if !order[i].required_by.contains(&r) {
                    order[i].required_by.push(r);
                }
            }
            continue;
        }
        if seen.len() >= max_items {
            return Err(format!(
                "dependency closure exceeded {max_items} items starting from {root_url}. That is almost certainly a cycle the visited set could not collapse, or a registry document pointing at an unrelated registry."
            ));
        }
        let item = fetch_item(fetch, &url).map_err(|reason| {
            let chain = if required_by.is_empty() {
                String::new()
            } else {
                format!("\n  required by: {}", required_by.join("\n  required by: "))
            };
            format!("could not resolve {url}: {reason}{chain}")
        })?;
        seen.insert(url.clone(), order.len());
        for dep in &item.registry_dependencies {
            let mut by = required_by.clone();
            by.push(url.clone());
            queue.push_back((dep.clone(), by));
        }
        order.push(ResolvedItem {
            url,
            item,
            required_by,
        });
    }
    order.reverse();
    Ok(order)
}

/// Which types does the registry have for `name`? Probes `/ui/` (its files say whether it is
/// TypeScript, Swift or Kotlin), `/rs/` and `/astro/`. A 404 means "not that type"; any other
/// failure is an error, never read as "absent".
pub fn available_types(fetch: &dyn Fetch, name: &str) -> Result<Vec<Target>, String> {
    let probe = |target: Target| -> Result<Option<Value>, String> {
        let url = registry_item_url(name, target);
        let res = fetch.get(&url, UA, Duration::from_secs(30))?;
        if res.status == 404 {
            return Ok(None);
        }
        if !(200..300).contains(&res.status) {
            return Err(format!("HTTP {} from {url}", res.status));
        }
        serde_json::from_str(&res.body)
            .map(Some)
            .map_err(|e| format!("{url} returned invalid JSON: {e}"))
    };
    let [ui, rs, astro] = std::thread::scope(|s| {
        let hs = [Target::Tsx, Target::Mzizi, Target::Astro].map(|t| s.spawn(move || probe(t)));
        hs.map(|h| h.join().expect("probe thread"))
    });
    let (ui, rs, astro) = (ui?, rs?, astro?);
    let mut found: Vec<Target> = Vec::new();
    if let Some(ui) = ui {
        let paths: Vec<&str> = ui
            .get("files")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .map(|f| f.get("path").and_then(Value::as_str).unwrap_or(""))
                    .collect()
            })
            .unwrap_or_default();
        found.extend(declared_types(&paths));
    }
    if rs.is_some() {
        found.push(Target::Mzizi);
    }
    if astro.is_some() {
        found.push(Target::Astro);
    }
    Ok(super::target::TARGETS
        .into_iter()
        .filter(|t| found.contains(t))
        .collect())
}

/// The URL for a named component on a type, for the root request.
pub fn item_url_for(name: &str, target: Target) -> String {
    registry_item_url(name, target)
}

/// Does the crate a `registry:rust` document declares actually exist on crates.io? Asked live,
/// never hard-coded. `None` means crates.io could not be reached: unknown is not missing.
pub fn crate_exists(fetch: &dyn Fetch, name: &str) -> Option<bool> {
    let url = format!(
        "https://crates.io/api/v1/crates/{}",
        super::target::encode_uri_component(name)
    );
    let ua = "mzizi-cli (+https://mzizi.dev; support@bundu.org)";
    match fetch.get(&url, ua, Duration::from_secs(15)) {
        Ok(r) if r.status == 404 => Some(false),
        Ok(r) if (200..300).contains(&r.status) => Some(true),
        _ => None,
    }
}

#[cfg(test)]
pub(crate) mod testutil {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;

    /// An in-memory registry. Any URL not in `docs` answers 404.
    #[derive(Default)]
    pub struct FakeFetch {
        pub docs: HashMap<String, (u16, String)>,
        pub calls: Mutex<Vec<String>>,
    }
    impl FakeFetch {
        pub fn with(mut self, url: &str, body: Value) -> Self {
            self.docs.insert(url.to_string(), (200, body.to_string()));
            self
        }
        pub fn calls(&self) -> Vec<String> {
            self.calls.lock().unwrap().clone()
        }
    }
    impl Fetch for FakeFetch {
        fn get(&self, url: &str, _ua: &str, _t: Duration) -> Result<Response, String> {
            self.calls.lock().unwrap().push(url.to_string());
            Ok(match self.docs.get(url) {
                Some((s, b)) => Response {
                    status: *s,
                    body: b.clone(),
                },
                None => Response {
                    status: 404,
                    body: "not found".into(),
                },
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::testutil::FakeFetch;
    use super::*;
    use serde_json::json;

    const ORIGIN: &str = "https://api.mzizi.dev";
    fn ui(n: &str) -> String {
        format!("{ORIGIN}/v1/ui/{n}")
    }
    fn leaf(name: &str) -> Value {
        json!({"name": name, "type": "registry:ui", "files": []})
    }
    fn with_deps(name: &str, deps: &[String]) -> Value {
        json!({"name": name, "type": "registry:ui", "files": [], "registryDependencies": deps})
    }
    fn names(v: &[ResolvedItem]) -> Vec<String> {
        v.iter().map(|r| r.item.name.clone()).collect()
    }

    // registry.test.ts > fetchItem
    #[test]
    fn fetch_item_returns_the_document_at_the_canonical_v1_address() {
        let f = FakeFetch::default().with(&ui("input"), leaf("input"));
        assert_eq!(fetch_item(&f, &ui("input")).unwrap().name, "input");
        assert_eq!(f.calls(), vec![ui("input")]);
    }
    #[test]
    fn fetch_item_propagates_a_404() {
        let f = FakeFetch::default();
        assert!(fetch_item(&f, &ui("nope")).unwrap_err().contains("404"));
    }
    #[test]
    fn fetch_item_rejects_a_200_that_is_not_a_registry_document() {
        let mut f = FakeFetch::default();
        f.docs.insert(ui("x"), (200, "[]".into()));
        assert!(
            fetch_item(&f, &ui("x"))
                .unwrap_err()
                .contains("registry document")
        );
    }

    // registry.test.ts > resolveClosure
    #[test]
    fn returns_dependencies_before_the_items_that_need_them() {
        let f = FakeFetch::default()
            .with(
                &ui("address-input"),
                with_deps("address-input", &[ui("input")]),
            )
            .with(&ui("input"), leaf("input"));
        let c = resolve_closure(&f, &ui("address-input"), 200).unwrap();
        assert_eq!(names(&c), ["input", "address-input"]);
    }
    #[test]
    fn fetches_a_diamond_dependency_exactly_once() {
        let f = FakeFetch::default()
            .with(&ui("page"), with_deps("page", &[ui("card"), ui("form")]))
            .with(&ui("card"), with_deps("card", &[ui("button")]))
            .with(&ui("form"), with_deps("form", &[ui("button")]))
            .with(&ui("button"), leaf("button"));
        let c = resolve_closure(&f, &ui("page"), 200).unwrap();
        assert_eq!(f.calls().iter().filter(|u| **u == ui("button")).count(), 1);
        let mut n = names(&c);
        n.sort();
        assert_eq!(n, ["button", "card", "form", "page"]);
    }
    #[test]
    fn terminates_on_a_dependency_cycle() {
        let f = FakeFetch::default()
            .with(&ui("a"), with_deps("a", &[ui("b")]))
            .with(&ui("b"), with_deps("b", &[ui("a")]));
        let mut n = names(&resolve_closure(&f, &ui("a"), 200).unwrap());
        n.sort();
        assert_eq!(n, ["a", "b"]);
    }
    #[test]
    fn refuses_a_bare_dependency_name_and_says_what_it_would_have_installed() {
        let f = FakeFetch::default().with(&ui("thing"), with_deps("thing", &["input".into()]));
        let e = resolve_closure(&f, &ui("thing"), 200).unwrap_err();
        assert!(
            e.contains("not an absolute URL") && e.contains("shadcn's own registry"),
            "{e}"
        );
    }
    #[test]
    fn refuses_a_non_https_dependency_url() {
        let f = FakeFetch::default().with(
            &ui("thing"),
            with_deps("thing", &["http://api.mzizi.dev/v1/ui/x".into()]),
        );
        assert!(
            resolve_closure(&f, &ui("thing"), 200)
                .unwrap_err()
                .contains("not https")
        );
    }
    #[test]
    fn names_the_component_that_pulled_in_a_dependency_that_failed() {
        let f = FakeFetch::default().with(&ui("thing"), with_deps("thing", &[ui("gone")]));
        let e = resolve_closure(&f, &ui("thing"), 200).unwrap_err();
        assert!(e.contains("required by") && e.contains("thing"), "{e}");
    }
    #[test]
    fn stops_when_the_closure_explodes() {
        struct Chain;
        impl Fetch for Chain {
            fn get(&self, url: &str, _: &str, _: Duration) -> Result<Response, String> {
                let n: u64 = url.rsplit('/').next().unwrap().parse().unwrap();
                let body = json!({"name": format!("c{n}"), "type": "registry:ui", "registryDependencies": [ui(&(n + 1).to_string())]});
                Ok(Response {
                    status: 200,
                    body: body.to_string(),
                })
            }
        }
        let e = resolve_closure(&Chain, &ui("0"), 12).unwrap_err();
        assert!(e.contains("exceeded 12 items"), "{e}");
    }

    // Beyond the vitest suite: availableTypes and crateExists were untested in TS.
    #[test]
    fn available_types_reads_the_three_routes_in_order() {
        let f = FakeFetch::default()
            .with(
                &ui("button"),
                json!({"name":"button","files":[{"path":"b.tsx"}]}),
            )
            .with(&format!("{ORIGIN}/v1/rs/button"), leaf("button"))
            .with(&format!("{ORIGIN}/v1/astro/button"), leaf("button"));
        assert_eq!(
            available_types(&f, "button").unwrap(),
            vec![Target::Mzizi, Target::Tsx, Target::Astro]
        );
    }
    #[test]
    fn available_types_treats_a_non_404_failure_as_an_error_not_absence() {
        let mut f = FakeFetch::default();
        f.docs.insert(ui("button"), (500, "boom".into()));
        assert!(
            available_types(&f, "button")
                .unwrap_err()
                .contains("HTTP 500")
        );
    }
    #[test]
    fn crate_exists_distinguishes_missing_from_unknown() {
        let mut f = FakeFetch::default();
        f.docs.insert(
            "https://crates.io/api/v1/crates/there".into(),
            (200, "{}".into()),
        );
        f.docs.insert(
            "https://crates.io/api/v1/crates/flaky".into(),
            (503, "".into()),
        );
        assert_eq!(crate_exists(&f, "there"), Some(true));
        assert_eq!(crate_exists(&f, "gone"), Some(false));
        assert_eq!(crate_exists(&f, "flaky"), None);
    }
}
