//! The CLI-only token store (port of cli-token-store.ts, plus the OS keychain).
//!
//! Read order: OS keychain (service "mz", account "cli"), then the file
//! `$XDG_CONFIG_HOME/mz/auth.json` (default `~/.config/mz/auth.json`, mode 0600, the same
//! JSON as the TypeScript CLI wrote), then the deprecated TypeScript location
//! `.../mzizi/auth.json`, which is migrated on first read: written to the new store, a
//! note printed on stderr, and the old file removed.
//! Write order: the keychain when it works, else the file. `logout` removes all three.
//! `MZ_NO_KEYCHAIN=1` forces the file store (headless boxes, CI, tests).
//!
//! "The OS keychain" is the macOS Keychain, the Windows Credential Manager, or on Linux
//! the Secret Service over D-Bus (GNOME Keyring, KeePassXC, KWallet). Linux has no
//! kernel-keyring store on purpose: it is memory-only and would lose a "saved" sign-in at
//! reboot. A headless Linux box (no session bus, no secret service) cannot use the
//! keychain, so the token goes to the 0600 file, and `mz login` says which.

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

/// The real OS keychain: the macOS Keychain or Windows Credential Manager via `keyring`,
/// the Secret Service on Linux. Anywhere it cannot be used (no session bus, no secret
/// service, a locked collection that needs a prompt) it reports failure and the token
/// store uses the file instead.
pub struct OsKeychain;

#[cfg(any(target_os = "macos", windows))]
impl OsKeychain {
    fn entry() -> Option<keyring::Entry> {
        keyring::Entry::new("mz", "cli").ok()
    }
}

#[cfg(any(target_os = "macos", windows))]
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

#[cfg(target_os = "linux")]
mod secret_service_store {
    //! One item in the default collection, attributes `service=mz`, `username=cli` (the
    //! same ones `keyring` writes, so an existing entry is found), secret sent over an
    //! encrypted (DH) session.
    use std::collections::HashMap;

    use secret_service::{EncryptionType, blocking::SecretService};
    use zeroize::Zeroizing;

    fn attrs() -> HashMap<&'static str, &'static str> {
        HashMap::from([("service", "mz"), ("username", "cli")])
    }

    pub fn get() -> Option<String> {
        let ss = SecretService::connect(EncryptionType::Dh).ok()?;
        let found = ss.search_items(attrs()).ok()?;
        let item = found.unlocked.first().or(found.locked.first())?;
        item.ensure_unlocked().ok()?;
        let bytes = Zeroizing::new(item.get_secret().ok()?);
        String::from_utf8(bytes.to_vec()).ok()
    }

    pub fn set(secret: &str) -> bool {
        let Ok(ss) = SecretService::connect(EncryptionType::Dh) else {
            return false;
        };
        let Ok(collection) = ss.get_default_collection() else {
            return false;
        };
        collection.ensure_unlocked().is_ok()
            && collection
                .create_item("cli@mz", attrs(), secret.as_bytes(), true, "text/plain")
                .is_ok()
    }

    pub fn delete() -> bool {
        let Ok(ss) = SecretService::connect(EncryptionType::Dh) else {
            return false;
        };
        let Ok(found) = ss.search_items(attrs()) else {
            return false;
        };
        let mut removed = false;
        for item in found.unlocked.iter().chain(found.locked.iter()) {
            removed |= item.ensure_unlocked().is_ok() && item.delete().is_ok();
        }
        removed
    }
}

#[cfg(target_os = "linux")]
impl Keychain for OsKeychain {
    fn get(&self) -> Option<String> {
        secret_service_store::get()
    }
    fn set(&self, secret: &str) -> bool {
        secret_service_store::set(secret)
    }
    fn delete(&self) -> bool {
        secret_service_store::delete()
    }
}

/// Other platforms have no keychain here: the file store only.
#[cfg(not(any(target_os = "macos", windows, target_os = "linux")))]
impl Keychain for OsKeychain {
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

    /// Read an auth file, but only a regular file this user owns: a symlink or another
    /// user's file is not trusted with a sign-in.
    fn read_file(path: &PathBuf) -> Option<StoredAuth> {
        if !trusted_file(path) {
            return None;
        }
        Self::parse(&zeroize::Zeroizing::new(
            std::fs::read_to_string(path).ok()?,
        ))
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
        let json = zeroize::Zeroizing::new(serde_json::to_string_pretty(auth)?);
        if self.keychain.set(json.as_str()) {
            // A stale file must not shadow or outlive the keychain copy.
            let _ = std::fs::remove_file(self.path());
            return Ok(Saved::Keychain);
        }
        let path = self.path();
        if let Some(dir) = path.parent() {
            create_private_dir(dir)?;
        }
        write_private(
            &path,
            &zeroize::Zeroizing::new(format!("{}\n", json.as_str())),
        )?;
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

/// Is `path` absent, or a regular file (not a symlink) owned by this user?
#[cfg(unix)]
fn trusted_file(path: &std::path::Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match std::fs::symlink_metadata(path) {
        Ok(m) => m.file_type().is_file() && m.uid() == rustix::process::geteuid().as_raw(),
        Err(_) => false,
    }
}

#[cfg(not(unix))]
fn trusted_file(path: &std::path::Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|m| m.file_type().is_file())
}

#[cfg(unix)]
fn untrusted(path: &std::path::Path, why: &str) -> std::io::Error {
    std::io::Error::new(
        std::io::ErrorKind::PermissionDenied,
        format!("refusing to use {}: {why}", path.display()),
    )
}

/// Create the config directory (and parents) with mode 0700 where we make it. The
/// directory that holds the token must be a real directory this user owns, and not
/// readable by others: a symlink or another user's directory is refused, a looser one of
/// ours is tightened to 0700.
#[cfg(unix)]
fn create_private_dir(dir: &std::path::Path) -> std::io::Result<()> {
    use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(dir)?;
    let m = std::fs::symlink_metadata(dir)?;
    if !m.file_type().is_dir() {
        return Err(untrusted(dir, "not a real directory (a symlink?)"));
    }
    if m.uid() != rustix::process::geteuid().as_raw() {
        return Err(untrusted(dir, "owned by another user"));
    }
    if m.permissions().mode() & 0o077 != 0 {
        std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn create_private_dir(dir: &std::path::Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dir)
}

/// Write `body` to `path` atomically and privately: a fresh temp file in the same
/// directory, created `O_EXCL` at mode 0600 (it never exists with looser bits), synced,
/// then renamed over the target. An existing target that is a symlink or another user's
/// file is refused.
#[cfg(unix)]
fn write_private(path: &PathBuf, body: &str) -> std::io::Result<()> {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    if std::fs::symlink_metadata(path).is_ok() && !trusted_file(path) {
        return Err(untrusted(path, "not a regular file this user owns"));
    }
    let dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let tmp = dir.join(format!(".auth.{}.{nanos}.tmp", std::process::id()));
    let result = (|| {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(body.as_bytes())?;
        f.sync_all()?;
        drop(f);
        std::fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&tmp);
    }
    result
}

#[cfg(not(unix))]
fn write_private(path: &PathBuf, body: &str) -> std::io::Result<()> {
    std::fs::write(path, body)
}
