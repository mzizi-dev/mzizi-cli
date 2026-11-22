# Changelog

All notable changes to this project are recorded here, in the Keep a Changelog style.

## [Unreleased]

### Fixed

- **`release.yml`:** the aarch64-apple-darwin build ran on the `macos-14` label, which never got a runner (the job sat unassigned and was cancelled in the dry run); it uses `macos-latest`, which CI already uses. Refs #3.

### Changed

- **Version 0.14.0** across the crate, the npm package, install.sh, the Homebrew draft and the docs, continuing `@nyuchi/mzizi-cli` 0.13.0 (owner-approved release-policy exception; the baseline is the `v0.13.0` tag). Refs #3.
- **One npm package.** `@nyuchi/mzizi-cli` bundles the binary of every platform (about 15 MB packed, 30 MB installed) and the six per-platform packages and the download fallback are gone; only one package needs npm publishing setup.
- **`mz floor` reads its thresholds from the Mzizi tokens.** `src/floor/generated.rs` is generated from the registry's `styles/pattern/tokens.css` by `scripts/generate-floor-tokens.mjs`; CI regenerates it and compares.
- The `--json` report says `"tool": "mz floor"` (was `mzizi check`).

### Added

- `.github/PULL_REQUEST_TEMPLATE.md`, the org template tailored to this repo (no signed-commit or DCO boxes; `cargo fmt`, `clippy` and `test` as the local checks). The bot's generic AGENTS.md, CONTRIBUTING.md, SECURITY.md and CODE_OF_CONDUCT.md were not adopted; the repo keeps its own. Refs #3.
- The Rust port of the TypeScript `@nyuchi/mzizi-cli` 0.13.0 (`mzizi-dev/agent-tools`):
  one binary, `mz`, with `mzizi` and `fundi` as deprecated names. Refs #1.
