# Install the Mzizi CLI

The CLI is one Rust binary, `mz`. Pick whichever route suits you; they all end in the same program.

| Route | Command | Needs |
| ----- | ------- | ----- |
| npm, one shot | `npx @nyuchi/mzizi-cli add button` | Node 18+ |
| npm, global | `npm i -g @nyuchi/mzizi-cli`, then `mz --help` | Node 18+ |
| pnpm | `pnpm dlx @nyuchi/mzizi-cli --version` | Node 18+ |
| cargo | `cargo install mzizi-cli` | Rust 1.85+ |
| script | `curl -fsSL https://raw.githubusercontent.com/mzizi-dev/mzizi-cli/main/install.sh \| sh` | curl |
| Homebrew | `brew install mzizi-dev/tap/mz` | Homebrew |

Status: nothing has been released from this repository yet. None of these commands works until
the first release is published. The Homebrew tap does not exist yet.

## npm

`@nyuchi/mzizi-cli` is a launcher with no CLI logic. It runs the `mz` binary from a per-platform
package that npm installs as an optional dependency, and passes arguments, stdio, exit codes and
signals straight through. The package keeps the bin names `mz`, `mzizi` and `fundi`.

| Platform | Package |
| -------- | ------- |
| macOS, Apple silicon | `@nyuchi/mzizi-cli-darwin-arm64` |
| macOS, Intel | `@nyuchi/mzizi-cli-darwin-x64` |
| Linux x64, glibc | `@nyuchi/mzizi-cli-linux-x64-gnu` |
| Linux x64, musl (Alpine) | `@nyuchi/mzizi-cli-linux-x64-musl` |
| Linux arm64, glibc | `@nyuchi/mzizi-cli-linux-arm64-gnu` |
| Linux arm64, musl | `@nyuchi/mzizi-cli-linux-arm64-musl` |
| Windows x64 | `@nyuchi/mzizi-cli-win32-x64` |

If the platform package is absent (`--omit=optional`, a mirror that lacks it), the launcher
downloads the release binary at install time or on first run and checks its sha256 against the
release's `SHA256SUMS` before keeping it. A mismatch deletes nothing and installs nothing. If no
binary can be found the launcher says so and exits 1.

- `MZIZI_BIN=/path/to/mz` runs that binary instead. If the path does not exist it is an error.
- `MZIZI_SKIP_DOWNLOAD=1` forbids the download fallback.
- `MZIZI_DOWNLOAD_BASE` points the fallback at a mirror. Files are read from `<base>/v<version>/`.

## cargo

```sh
cargo install mzizi-cli
```

Installs `mz`, and `mzizi` and `fundi`, the old names, which print a deprecation note.

## install.sh

Installs the release binary to `~/.local/bin` (or `$MZIZI_INSTALL_DIR`). It never uses sudo, and it
refuses to install when the sha256 in `SHA256SUMS` does not match. `MZIZI_VERSION=0.14.0` pins a
version. It supports macOS and Linux (glibc and musl, x86_64 and aarch64); on Windows use npm or cargo.

## Homebrew

`packaging/homebrew/mz.rb` is a draft formula. It needs a tap and real checksums from a release.

## Release setup (for the owner)

Nothing is published and no tag exists. To make the first release:

1. npm: the `@nyuchi` scope must exist, and each of the eight packages (the launcher and seven
   platform packages) needs this repository and `release.yml` set as its trusted publisher on
   npmjs.com. New package names can only be configured after a first publish or by creating the
   package from a granted token once; decide how to bootstrap them.
2. crates.io: own the `mzizi-cli` name; add a `CARGO_REGISTRY_TOKEN` secret to a `release`
   environment (or move to crates.io trusted publishing).
3. A `release` environment in this repository, with the approval rules you want.
4. Tag `v<version>` after the version in `Cargo.toml` and `npm/package.json` match. The workflow
   builds every target, drafts the GitHub release (publish the draft yourself), then publishes.
5. The Homebrew tap repository, and the formula's real checksums.
