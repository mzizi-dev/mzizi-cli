# Install the Mzizi CLI

The CLI is one Rust binary, `mz`. Pick whichever route suits you; they all end in the same program.

| Route         | Command                                                                                  | Needs      |
| ------------- | ---------------------------------------------------------------------------------------- | ---------- |
| npm, one shot | `npx @nyuchi/mzizi-cli add button`                                                       | Node 18+   |
| npm, global   | `npm i -g @nyuchi/mzizi-cli`, then `mz --help`                                           | Node 18+   |
| pnpm          | `pnpm dlx @nyuchi/mzizi-cli --version`                                                   | Node 18+   |
| cargo         | `cargo install mzizi-cli`                                                                | Rust 1.85+ |
| script        | `curl -fsSL https://raw.githubusercontent.com/mzizi-dev/mzizi-cli/main/install.sh \| sh` | curl       |
| Homebrew      | `brew install mzizi-dev/tap/mz`                                                          | Homebrew   |

Status: nothing has been released from this repository yet. None of these commands works until
the first release is published. The Homebrew tap does not exist yet.

## npm

`@nyuchi/mzizi-cli` is one package with every platform binary inside it (about 15 MB packed, 30 MB
installed, measured on the staged package). A launcher with no CLI logic picks the binary for your
platform and runs it, passing arguments, stdio, exit codes and signals straight through. The
package keeps the bin names `mz`, `mzizi` and `fundi` (the last two are the old names, which print
a deprecation note). There is no postinstall script and no download.

| Platform                 | Bundled target               |
| ------------------------ | ---------------------------- |
| macOS, Apple silicon     | `aarch64-apple-darwin`       |
| macOS, Intel             | `x86_64-apple-darwin`        |
| Linux x64, glibc         | `x86_64-unknown-linux-gnu`   |
| Linux x64, musl (Alpine) | `x86_64-unknown-linux-musl`  |
| Linux arm64, glibc       | `aarch64-unknown-linux-gnu`  |
| Linux arm64, musl        | `aarch64-unknown-linux-musl` |
| Windows x64              | `x86_64-pc-windows-msvc`     |

On any other platform the launcher says so and exits 1; use cargo or install.sh.

- `MZ_BIN=/path/to/mz` runs that binary instead. If the path does not exist it is an error.
  (`MZIZI_BIN` still works, deprecated.)

## cargo

```sh
cargo install mzizi-cli
```

Installs `mz`, and `mzizi` and `fundi`, the old names, which print a deprecation note.

## install.sh

Installs the release binary to `~/.local/bin` (or `$MZ_INSTALL_DIR`). It never uses sudo, and it
refuses to install when the sha256 in `SHA256SUMS` does not match. `MZ_VERSION=0.0.2` pins a
version. It supports macOS and Linux (glibc and musl, x86_64 and aarch64); on Windows use npm or cargo.

## Homebrew

`packaging/homebrew/mz.rb` is a draft formula. It needs a tap and real checksums from a release.

## Release setup (for the owner)

Nothing is published and no tag exists. To make the first release:

1. npm: the `@nyuchi` scope exists; set this repository and `release.yml` as the trusted publisher of
   the one package `@nyuchi/mzizi-cli` on npmjs.com (it already exists at 0.13.0, so this is a
   settings change, not a bootstrap).
2. crates.io: own the `mzizi-cli` name; add a `CARGO_REGISTRY_TOKEN` secret to a `release`
   environment (or move to crates.io trusted publishing).
3. A `release` environment in this repository, with the approval rules you want.
4. Tag `v<version>` after the version in `Cargo.toml` and `npm/package.json` match. The workflow
   builds every target, drafts the GitHub release (publish the draft yourself), then publishes.
5. The Homebrew tap repository, and the formula's real checksums.
