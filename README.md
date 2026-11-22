# mz

The Mzizi CLI, in Rust. One binary, `mz`: `mz add` installs registry components and
their dependencies from api.mzizi.dev, `mz floor` enforces the Bundu spacing floor,
`mz explore` snapshots a project, and `mz plan` / `mz chat` are the fundi architecture
agent. Apache-2.0. A Bundu Foundation project.

> **Status: ported and parity-checked, not released.** This repository replaces the
> TypeScript `@nyuchi/mzizi-cli` (`mzizi-dev/agent-tools`, `mzizi-cli/`, 0.13.0). Nothing here is published to crates.io or npm yet.

- `mz add`, `mz floor` (the TypeScript `mzizi check`), `mz explore`, `mz plan`, `mz chat`,
  `mz login`, `mz logout`, `mz whoami`: ported from the TypeScript CLI 0.13.0. Parity notes
  and the test maps are in `docs/porting/add.md`, `floor.md` and `fundi.md`.
- `mz check`, `fix`, `contract`, `outline`, `hash`, `ir`, `build`, `run`, `harness`: the
  language commands, moved here from the compiler's `main.rs` with history
  (`docs/porting/lang.md`); the compiler stays in mzizi-dev/mzizi as a library.

## Names

- The binary is `mz`. `mzizi` and `fundi` are the TypeScript CLI's old names; they run the
  same program, print a deprecation note, and keep `check` meaning `floor`.
- Environment: `MZ_MCP_URL`, `MZ_MODEL`, `MZ_AUTH_TOKEN` (the old `MZIZI_*` names still
  work, deprecated). `ANTHROPIC_API_KEY` and `WORKOS_ACCESS_TOKEN` are external names.
- Sign-in token: OS keychain when available (macOS Keychain, Windows Credential Manager, or the Linux Secret Service over D-Bus), else `$XDG_CONFIG_HOME/mz/auth.json` (mode 0600). A headless Linux box has no keychain, so it uses the file.

## Install

See [`docs/install.md`](docs/install.md): `cargo install mzizi-cli`, `install.sh`,
Homebrew, and the npm wrapper `@nyuchi/mzizi-cli`. None of these is published yet.

## Develop

```sh
cargo fmt -- --check
cargo clippy --all-targets -- -D warnings
cargo test
```

Tests never touch the network: registry and MCP responses are recorded fixtures served
from a local socket. `docs/porting/` maps every TypeScript test to its Rust replacement.

## Contributing and security

[CONTRIBUTING.md](CONTRIBUTING.md), [SECURITY.md](SECURITY.md),
[CODE_OF_CONDUCT.md](CODE_OF_CONDUCT.md). Security reports: `security@nyuchi.com`.
Contact: `support@bundu.org`.
