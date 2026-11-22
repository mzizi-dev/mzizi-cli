# AGENTS.md: mzizi-cli

The Mzizi CLI, in Rust: one binary, `mz` (owner decision, #1). Crate `mzizi-cli`, binary
`mz`, with `mzizi` and `fundi` as deprecated compatibility names. The npm package
`@nyuchi/mzizi-cli` stays, as a thin launcher in `npm/` that runs the Rust binary.

- Behaviour parity with the TypeScript CLI 0.13.0 until the port is declared done; the
  vitest-to-Rust test maps are in `docs/porting/`.
- The language commands (`mz check`, `fix`, ...) belong to the compiler in `mzizi-dev/mzizi`
  and are not in this repo yet; the spacing-floor check is `mz floor`.
- Env prefix `MZ_` (the language repo's convention); the old `MZIZI_*` names are deprecated
  fallbacks. Config under `$XDG_CONFIG_HOME/mz/`.
- Rust only for the tool. No OpenSSL, no JS runtime, tests never use the network.
- Never publish (crates.io, npm) or tag from an agent; releases come from a tag through
  `.github/workflows/release.yml`.
- Commits in this repo: `git -c commit.gpgsign=false commit`.
