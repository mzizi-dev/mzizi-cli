//! WorkOS AuthKit helpers (port of auth.ts). The token is opaque here: the claims are
//! decoded for UX only and the signature is NOT verified; resource servers verify it.

use base64::Engine;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use serde_json::{Map, Value};

use crate::Env;

/// Auth failure (TS `AuthError`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthError(pub String);

impl std::fmt::Display for AuthError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}
impl std::error::Error for AuthError {}

/// Identity decoded from an access token.
#[derive(Debug, Clone, PartialEq)]
pub struct Identity {
    /// The `sub` claim.
    pub sub: String,
    /// Email, if present.
    pub email: Option<String>,
    /// Organization id, if present.
    pub organization_id: Option<String>,
    /// String permissions.
    pub permissions: Vec<String>,
    /// Expiry, seconds since the epoch.
    pub exp: f64,
}

impl Identity {
    /// The JSON the TS CLI printed for `whoami` (`undefined` fields omitted, key order kept).
    pub fn to_json(&self) -> Value {
        let mut m = Map::new();
        m.insert("sub".into(), Value::String(self.sub.clone()));
        if let Some(e) = &self.email {
            m.insert("email".into(), Value::String(e.clone()));
        }
        if let Some(o) = &self.organization_id {
            m.insert("organizationId".into(), Value::String(o.clone()));
        }
        m.insert(
            "permissions".into(),
            Value::Array(
                self.permissions
                    .iter()
                    .cloned()
                    .map(Value::String)
                    .collect(),
            ),
        );
        m.insert("exp".into(), num(self.exp));
        Value::Object(m)
    }
}

/// A JS-style number: integral values print without a fraction.
pub fn num(n: f64) -> Value {
    if n.fract() == 0.0 && n.abs() < 9.0e15 {
        Value::from(n as i64)
    } else {
        Value::from(n)
    }
}

/// Resolve the token: explicit, then `WORKOS_ACCESS_TOKEN`, then `MZ_AUTH_TOKEN`, then the
/// deprecated `MZIZI_AUTH_TOKEN` (which warns once on stderr when it is the one used).
pub fn resolve_token(
    explicit: Option<&str>,
    env: &Env,
    warn: &mut dyn std::io::Write,
) -> Option<String> {
    if let Some(t) = explicit.map(str::trim).filter(|t| !t.is_empty()) {
        return Some(t.to_string());
    }
    if let Some(t) = env.get_trimmed("WORKOS_ACCESS_TOKEN") {
        return Some(t.to_string());
    }
    env_with_fallback(env, "MZ_AUTH_TOKEN", "MZIZI_AUTH_TOKEN", warn)
}

/// Read `new`, else the deprecated `old` with a one-line warning. Blank values count as unset.
pub fn env_with_fallback(
    env: &Env,
    new: &str,
    old: &str,
    warn: &mut dyn std::io::Write,
) -> Option<String> {
    if let Some(v) = env.get_trimmed(new) {
        return Some(v.to_string());
    }
    let v = env.get_trimmed(old)?;
    let _ = writeln!(warn, "warning: {old} is deprecated, use {new}");
    Some(v.to_string())
}

/// Like [`resolve_token`] but an [`AuthError`] when nothing is set.
pub fn assert_token(
    explicit: Option<&str>,
    env: &Env,
    label: &str,
    warn: &mut dyn std::io::Write,
) -> Result<String, AuthError> {
    resolve_token(explicit, env, warn).ok_or_else(|| {
        AuthError(format!(
            "{label}: no WorkOS token; set WORKOS_ACCESS_TOKEN or pass options.auth.accessToken"
        ))
    })
}

/// Decode the claims and return an [`Identity`], judged against `now_sec`.
pub fn get_identity_at(token: &str, now_sec: i64) -> Result<Identity, AuthError> {
    let claims = decode_claims(token)?;
    let sub = match claims.get("sub") {
        Some(Value::String(s)) if !s.is_empty() => s.clone(),
        _ => return Err(AuthError("WorkOS access token missing 'sub' claim".into())),
    };
    let exp = match claims.get("exp") {
        Some(Value::Number(n)) => n.as_f64().unwrap_or(f64::NAN),
        _ => return Err(AuthError("WorkOS access token missing 'exp' claim".into())),
    };
    if exp < now_sec as f64 {
        let ago = now_sec as f64 - exp;
        return Err(AuthError(format!(
            "WorkOS access token expired {}s ago",
            fmt_num(ago)
        )));
    }
    let s = |k: &str| match claims.get(k) {
        Some(Value::String(v)) => Some(v.clone()),
        _ => None,
    };
    let organization_id = s("organizationId")
        .or_else(|| s("organization_id"))
        .or_else(|| s("org_id"));
    let permissions = match claims.get("permissions") {
        Some(Value::Array(a)) => a
            .iter()
            .filter_map(|p| p.as_str().map(str::to_string))
            .collect(),
        _ => Vec::new(),
    };
    Ok(Identity {
        sub,
        email: s("email"),
        organization_id,
        permissions,
        exp,
    })
}

/// [`get_identity_at`] against the system clock.
pub fn get_identity(token: &str) -> Result<Identity, AuthError> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    get_identity_at(token, now)
}

fn fmt_num(n: f64) -> String {
    if n.fract() == 0.0 {
        format!("{}", n as i64)
    } else {
        format!("{n}")
    }
}

fn decode_claims(token: &str) -> Result<Map<String, Value>, AuthError> {
    let bad = |m: &str| AuthError(format!("invalid WorkOS access token: {m}"));
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() == 5 {
        return Err(bad(
            "Only JWTs using Compact JWS serialization can be decoded",
        ));
    }
    if parts.len() != 3 {
        return Err(bad("Invalid JWT"));
    }
    let raw = URL_SAFE_NO_PAD
        .decode(parts[1].trim_end_matches('='))
        .map_err(|_| bad("Invalid JWT"))?;
    match serde_json::from_slice::<Value>(&raw) {
        Ok(Value::Object(m)) => Ok(m),
        _ => Err(bad("Invalid JWT")),
    }
}

/// The `Authorization: Bearer` header pair, or nothing.
pub fn bearer_header(token: Option<&str>) -> Vec<(String, String)> {
    match token {
        Some(t) if !t.is_empty() => vec![("Authorization".into(), format!("Bearer {t}"))],
        _ => Vec::new(),
    }
}
