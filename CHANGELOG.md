# Changelog

All notable changes to this project are recorded here, in the Keep a Changelog style.

## [Unreleased]

### Changed — the Homebrew draft carries the v0.15.0 sha256 values

- **`packaging/homebrew/mz.rb`:** version 0.15.0 and its SHA256SUMS, matching mzizi-dev/homebrew-tap. Closes #22.

### 0.15.0 — the one Rust `mz`, with a working npm package

The first release to carry everything since 0.13.0 (the TypeScript `@nyuchi/mzizi-cli` in agent-tools): one Rust binary, `mz`, for the registry, the spacing floor, the fundi agent tools and the Mzizi language. Versions 0.14.0 to 0.14.2 were the path here; read this entry rather than them. Refs #19.

- **One binary.** `mz add`, `mz floor`, `mz explore`, `mz plan`, `mz chat`, `mz login`, `mz whoami`, and the language commands (`mz check`, `contract`, `fix`, `build`, `run`, `outline`, `ir`, `harness`) from `mzizi-lang-compiler` 0.18.0 on crates.io. `mzizi` and `fundi` still run, with a deprecation note; `MZ_*` environment variables replace `MZIZI_*`, which still work as deprecated fallbacks.
- **`mz floor` reads its thresholds from the Mzizi tokens** (`src/floor/generated.rs` is generated from the registry's `tokens.css`, and CI checks it is current). The `--json` report says `"tool": "mz floor"`.
- **One bundled npm package.** `@nyuchi/mzizi-cli` carries the binary of all seven platforms (about 17 MB packed, 37 MB installed) behind a small launcher, so there is no per-platform dependency to resolve. It repairs a lost execute bit at run time (0.14.0 shipped its binaries as mode 0644 and `npm install` gave EACCES).
- **`pnpm dlx @nyuchi/mzizi-cli` and `bunx` work.** The package has a bin named like itself (`mzizi-cli`), next to `mz`, `mzizi` and `fundi`.
- **The Linux keychain is the Secret Service.** It was the kernel keyutils store, which is memory-only. Sign-in tokens are kept in the OS keychain (macOS Keychain, Windows Credential Manager, Linux Secret Service), then in `$XDG_CONFIG_HOME/mz/auth.json` at mode 0600, written through an `O_EXCL` temp file and renamed into place. Old `mzizi` tokens migrate.
- **Registry and file safety.** Only `https://api.mzizi.dev/v1/` URLs are fetched, with same-origin redirects only; every file write goes through one path-safety module (relative paths only, no symlinks followed, directories made a level at a time).
- **A fully automatic release.** One tag push builds seven targets, publishes to crates.io then npm with provenance, makes the GitHub release public, and runs smoke tests that install the published package on ubuntu, macOS and Windows and `cargo install` it on ubuntu.
- **The language repo no longer ships `mz`.** mzizi-dev/mzizi keeps only a test-only `mz-driver` for the compiler's own tests (mzizi-dev/mzizi#190).
- **`--help` and `-h` work for every subcommand.** `mz add --help`, `mz floor -h`, `mz plan --help` and the language commands (`check`, `fix`, `contract`, `outline`, `hash`, `ir`, `build`, `run`, `harness`) print that command's usage on stdout and exit 0, before any argument is checked. Before, `mz floor --help` printed the global help, `mz add --help` failed with "add requires at least one component name" and `mz add -h` looked up a component named `-h`. The old names `mzizi` and `fundi` behave the same (`check` still means `floor` there). After `--`, `-h` is an operand. `tests/help.rs` walks every subcommand and the old names.
- **Install:** `npm i -g @nyuchi/mzizi-cli`, `npx @nyuchi/mzizi-cli`, `cargo install mzizi-cli --locked`, `brew install mzizi-dev/tap/mz`, or `install.sh`.
- Minimum Rust is 1.89.

### 0.14.2 — `pnpm dlx` and `bunx` run the package

- `@nyuchi/mzizi-cli` has a bin named like the package (`mzizi-cli`), so `pnpm dlx @nyuchi/mzizi-cli` and `bunx` run it; `npx` already did (#17). Nothing else changes in the binary. Refs #3.

### Fixed — `pnpm dlx @nyuchi/mzizi-cli` needs a bin named like the package

- **`npm/package.json`:** the package has three bins (`mz`, `mzizi`, `fundi`) and none named `mzizi-cli`, so `pnpm dlx @nyuchi/mzizi-cli --version` stopped with a hint ("pnpm --package=@nyuchi/mzizi-cli dlx mzizi") instead of running; `npx` happened to pick the first bin. A fourth bin, `mzizi-cli`, points at the same launcher, so `npx`, `pnpm dlx` and `bunx` all run it. The launcher treats it as `mz`. Takes effect in the next release (0.14.2). Refs #3.

### Fixed — smoke tests wait for the registry, and a timing test is looser

- **`release.yml` smoke test:** the 0.14.1 npm smoke test gave up after about 2.5 minutes with `ETARGET` while the version was still propagating (it was served minutes later). It now waits up to 10 minutes for `npm view` to see the version, then installs. Re-running the failed jobs passed on Linux, macOS and Windows.
- **`tests/floor_scan.rs`:** the wall-clock bound on hostile inputs is 10 s in debug builds (was 2 s); it failed once on a busy macOS runner on `main`. A backtracking blow-up takes minutes, so it is still caught.
- **`packaging/homebrew/mz.rb`:** the real sha256 values of the v0.14.1 release.

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
