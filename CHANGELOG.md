# Changelog

All notable changes to this project are recorded here, in the Keep a Changelog style.

## [Unreleased]

### Changed

- **Version 0.14.0** across the crate, the npm package, install.sh, the Homebrew draft and the docs, continuing `@nyuchi/mzizi-cli` 0.13.0 (owner-approved release-policy exception; the baseline is the `v0.13.0` tag). Refs #3.
- **One npm package.** `@nyuchi/mzizi-cli` bundles the binary of every platform (about 15 MB packed, 30 MB installed) and the six per-platform packages and the download fallback are gone; only one package needs npm publishing setup.
- **`mz floor` reads its thresholds from the Mzizi tokens.** `src/floor/generated.rs` is generated from the registry's `styles/pattern/tokens.css` by `scripts/generate-floor-tokens.mjs`; CI regenerates it and compares.
- The `--json` report says `"tool": "mz floor"` (was `mzizi check`).

### Added

- The Rust port of the TypeScript `@nyuchi/mzizi-cli` 0.13.0 (`mzizi-dev/agent-tools`):
  one binary, `mz`, with `mzizi` and `fundi` as deprecated names. Refs #1.
