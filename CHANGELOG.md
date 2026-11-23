# Changelog

All notable changes to this project are recorded here, in the Keep a Changelog style.

## [Unreleased]

### Fixed — the npm package did not run (0.14.1)

- **0.14.1: `npm install @nyuchi/mzizi-cli` ran `mz` as EACCES.** The 0.14.0 tarball held every bundled binary as mode 0644. Cause: the staged package travels between release jobs as a workflow artifact, and `actions/download-artifact` does not keep file modes, so the `chmod 0755` that `stage-npm.mjs` does was lost before `npm publish` (`npm pack` itself keeps the mode; checked). Three fixes: the launcher chmods the binary 0755 when it is not executable, and when it cannot (a read-only package directory) runs a copy from a per-user 0700 cache directory, verified against `vendor/checksums.json`; the publish job restores the bit and fails if the tarball it is about to publish has a non-executable binary; and a post-publish smoke test (below) installs the published package. `cargo install mzizi-cli` was not affected. Refs #3.

### Added — smoke tests after a release

- **`release.yml` smoke tests after publishing:** `npm install @nyuchi/mzizi-cli@<version>` from the registry on ubuntu, macOS and Windows, `mz --version` must print the version, and `mz floor` must pass a clean fixture and fail a bad one; and `cargo install mzizi-cli --version <version> --locked` on ubuntu. A failing smoke test fails the release run.

### Fixed

- **`release.yml`:** the aarch64-apple-darwin build ran on the `macos-14` label, which never got a runner (the job sat unassigned and was cancelled in the dry run); it uses `macos-latest`, which CI already uses. Refs #3.
- **Linux sign-in storage.** The "OS keychain" on Linux was the kernel keyutils store, which is memory-only (gone at reboot) and never the Secret Service, so `mz login` reported a save that did not last, and GNOME Keyring, KeePassXC and KWallet were ignored. Linux now uses the Secret Service over D-Bus through `secret-service` 6 (zbus 6, pure Rust, no libdbus or OpenSSL; `keyring` 3 stays for macOS and Windows; no `cargo deny` ignores, since zbus 4 and 5 carry RUSTSEC unmaintained notices); with no session bus or secret service it falls back to `auth.json`. Closes #9.
- **The `auth.json` fallback is hardened.** Written to a fresh `O_EXCL` temp file at mode 0600 and renamed into place (never present with looser bits); the config directory is 0700 (a looser one of ours is tightened); a symlink, or a directory or file owned by another user, is refused on write and ignored on read; the token text is zeroized after use. Closes #9.
- **Minimum Rust is now 1.89** (`secret-service` 6 needs it), with the clippy `collapsible_if` rewrites that edition 2024 at 1.88 and later enables. Refs #9.

### Changed

- **The release is fully automatic after the tag push.** `release.yml` drafts the GitHub release, publishes to crates.io, then to npm (`npm publish --provenance`, OIDC), and makes the release public when both succeeded. Every publish step skips a version already published, so a failed job re-runs without a new tag. `docs/install.md` lists the repository settings that lock it down. Refs #3.
- **The language compiler comes from crates.io.** `mzizi-lang-compiler = 0.18.0` (default features off) replaces the git tag `v0.17.0`; the Dependabot ignore and the `allow-git` entry in `deny.toml` are gone. Refs #3.
- **Version 0.14.0** across the crate, the npm package, install.sh, the Homebrew draft and the docs, continuing `@nyuchi/mzizi-cli` 0.13.0 (owner-approved release-policy exception; the baseline is the `v0.13.0` tag). Refs #3.
- **One npm package.** `@nyuchi/mzizi-cli` bundles the binary of every platform (about 15 MB packed, 30 MB installed) and the six per-platform packages and the download fallback are gone; only one package needs npm publishing setup.
- **`mz floor` reads its thresholds from the Mzizi tokens.** `src/floor/generated.rs` is generated from the registry's `styles/pattern/tokens.css` by `scripts/generate-floor-tokens.mjs`; CI regenerates it and compares.
- The `--json` report says `"tool": "mz floor"` (was `mzizi check`).

### Added

- `.github/PULL_REQUEST_TEMPLATE.md`, the org template tailored to this repo (no signed-commit or DCO boxes; `cargo fmt`, `clippy` and `test` as the local checks). The bot's generic AGENTS.md, CONTRIBUTING.md, SECURITY.md and CODE_OF_CONDUCT.md were not adopted; the repo keeps its own. Refs #3.
- The Rust port of the TypeScript `@nyuchi/mzizi-cli` 0.13.0 (`mzizi-dev/agent-tools`):
  one binary, `mz`, with `mzizi` and `fundi` as deprecated names. Refs #1.
