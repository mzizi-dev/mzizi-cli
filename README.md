# mz

The Mzizi CLI, in Rust. One binary, `mz`: `mz add` installs registry components and
their dependencies from api.mzizi.dev, `mz floor` enforces the Bundu spacing floor,
`mz explore` snapshots a project, and `mz plan` / `mz chat` are the fundi architecture
agent. Apache-2.0. A Bundu Foundation project.

> **Status: the Rust port is at parity work, not released.** This repository replaces the
> TypeScript `@nyuchi/mzizi-cli` (`mzizi-dev/agent-tools`, `mzizi-cli/`, 0.13.0). The
> table below says what has landed. Nothing here is published to crates.io or npm yet.

| Command            | Source of truth                          | State |
| ------------------ | ---------------------------------------- | ----- |
| `mz add`           | TypeScript CLI 0.13.0                    | see `docs/porting/add.md` |
| `mz floor`         | TypeScript `mzizi check`                 | see `docs/porting/floor.md` |
| `mz explore`, `plan`, `chat`, `login`, `logout`, `whoami` | TypeScript CLI 0.13.0 | see `docs/porting/fundi.md` |
| `mz check`, `fix`, `contract`, `outline`, `hash`, `ir`, `build`, `run`, `harness` | the language compiler in mzizi-dev/mzizi | **not here yet**: the compiler's dispatch lives in its `main.rs`, so it needs a library entry point first. Owner decision pending. See [#1](https://github.com/mzizi-dev/mzizi-cli/issues/1). |

## Names

- The binary is `mz`. `mzizi` and `fundi` are the TypeScript CLI's old names; they run the
  same program, print a deprecation note, and keep `check` meaning `floor`.
- Environment: `MZ_MCP_URL`, `MZ_MODEL`, `MZ_AUTH_TOKEN` (the old `MZIZI_*` names still
  work, deprecated). `ANTHROPIC_API_KEY` and `WORKOS_ACCESS_TOKEN` are external names.
- Sign-in token: OS keychain when available, else `$XDG_CONFIG_HOME/mz/auth.json`.

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
