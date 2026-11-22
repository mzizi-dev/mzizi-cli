//! The CLI-only token store (port of cli-token-store.ts, plus the OS keychain).
//!
//! Read order: OS keychain (service "mz", account "cli"), then the file
//! `$XDG_CONFIG_HOME/mz/auth.json` (default `~/.config/mz/auth.json`, mode 0600, the same
//! JSON as the TypeScript CLI wrote), then the deprecated TypeScript location
//! `.../mzizi/auth.json`, which is migrated on first read: written to the new store, a
//! note printed on stderr, and the old file removed.
//! Write order: the keychain when it works, else the file. `logout` removes all three.
//! `MZ_NO_KEYCHAIN=1` forces the file store (headless boxes, CI, tests).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::Env;

/// What is saved.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct StoredAuth {
    /// The access token.
    #[serde(rename = "accessToken")]
    pub access_token: String,
    /// A refresh token, if one was saved.
    #[serde(
        rename = "refreshToken",
        skip_serializing_if = "Option::is_none",
        default
    )]
    pub refresh_token: Option<String>,
    /// ISO timestamp.
    #[serde(rename = "savedAt", default)]
    pub saved_at: String,
}

/// Where a token was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Saved {
    /// The OS keychain.
    Keychain,
    /// The auth file.
    File(PathBuf),
}

/// The keychain, as far as the store needs it (injectable for tests).
pub trait Keychain {
    /// The stored secret, or `None` when absent or the keychain is unusable.
    fn get(&self) -> Option<String>;
    /// Store a secret; `false` when the keychain cannot be used.
    fn set(&self, secret: &str) -> bool;
    /// Delete; `true` when something was removed.
    fn delete(&self) -> bool;
}

/// The real OS keychain via the `keyring` crate.
pub struct OsKeychain;

impl OsKeychain {
    fn entry() -> Option<keyring::Entry> {
        keyring::Entry::new("mz", "cli").ok()
    }
}

impl Keychain for OsKeychain {
    fn get(&self) -> Option<String> {
        Self::entry()?.get_password().ok()
    }
    fn set(&self, secret: &str) -> bool {
        Self::entry().is_some_and(|e| e.set_password(secret).is_ok())
    }
    fn delete(&self) -> bool {
        Self::entry().is_some_and(|e| e.delete_credential().is_ok())
    }
}

/// No keychain at all (the file store only).
pub struct NoKeychain;

impl Keychain for NoKeychain {
    fn get(&self) -> Option<String> {
        None
    }
    fn set(&self, _: &str) -> bool {
        false
    }
    fn delete(&self) -> bool {
        false
    }
}

/// The token store.
pub struct TokenStore {
    config_base: PathBuf,
    keychain: Box<dyn Keychain>,
}

impl TokenStore {
    /// A store over an explicit config base and keychain.
    pub fn new(config_base: PathBuf, keychain: Box<dyn Keychain>) -> Self {
        Self {
            config_base,
            keychain,
        }
    }

    /// The store the CLI uses, from the environment.
    pub fn from_env(env: &Env) -> Self {
        let base = env
            .get_trimmed("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(env.get("HOME").or(env.get("USERPROFILE")).unwrap_or("."))
                    .join(".config")
            });
        let keychain: Box<dyn Keychain> = if env.get_trimmed("MZ_NO_KEYCHAIN").is_some() {
            Box::new(NoKeychain)
        } else {
            Box::new(OsKeychain)
        };
        Self::new(base, keychain)
    }

    /// `<config>/mz/auth.json`.
    pub fn path(&self) -> PathBuf {
        self.config_base.join("mz").join("auth.json")
    }

    /// The deprecated TypeScript location, `<config>/mzizi/auth.json`.
    pub fn legacy_path(&self) -> PathBuf {
        self.config_base.join("mzizi").join("auth.json")
    }

    fn parse(raw: &str) -> Option<StoredAuth> {
        serde_json::from_str::<StoredAuth>(raw).ok()
    }

    fn read_file(path: &PathBuf) -> Option<StoredAuth> {
        Self::parse(&std::fs::read_to_string(path).ok()?)
    }

    /// Read the saved token, migrating the deprecated file when it is the only one.
    pub fn read(&self, note: &mut dyn std::io::Write) -> Option<StoredAuth> {
        if let Some(a) = self.keychain.get().and_then(|s| Self::parse(&s)) {
            return Some(a);
        }
        if let Some(a) = Self::read_file(&self.path()) {
            return Some(a);
        }
        let legacy = self.legacy_path();
        let a = Self::read_file(&legacy)?;
        match self.write(&a) {
            Ok(to) => {
                let _ = std::fs::remove_file(&legacy);
                let where_ = match to {
                    Saved::Keychain => "the OS keychain".to_string(),
                    Saved::File(p) => p.display().to_string(),
                };
                let _ = writeln!(
                    note,
                    "note: moved your saved token from {} to {where_} (the CLI is now `mz`).",
                    legacy.display()
                );
            }
            Err(_) => {
                let _ = writeln!(
                    note,
                    "warning: {} is deprecated, move it to {}",
                    legacy.display(),
                    self.path().display()
                );
            }
        }
        Some(a)
    }

    /// Save: keychain when it works, else the file (mode 0600).
    pub fn write(&self, auth: &StoredAuth) -> std::io::Result<Saved> {
        let json = serde_json::to_string_pretty(auth)?;
        if self.keychain.set(&json) {
            // A stale file must not shadow or outlive the keychain copy.
            let _ = std::fs::remove_file(self.path());
            return Ok(Saved::Keychain);
        }
        let path = self.path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        write_private(&path, &format!("{json}\n"))?;
        Ok(Saved::File(path))
    }

    /// Remove every saved copy. Returns `(keychain, file, legacy_file)`: which held one.
    pub fn clear(&self) -> (bool, bool, bool) {
        (
            self.keychain.delete(),
            std::fs::remove_file(self.path()).is_ok(),
            std::fs::remove_file(self.legacy_path()).is_ok(),
        )
    }
}

#[cfg(unix)]
fn write_private(path: &PathBuf, body: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
    let mut f = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .mode(0o600)
        .open(path)?;
    f.write_all(body.as_bytes())?;
    // `mode` only applies on create; an existing file keeps its old bits.
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
}

#[cfg(not(unix))]
fn write_private(path: &PathBuf, body: &str) -> std::io::Result<()> {
    std::fs::write(path, body)
}
