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
refuses to install when the sha256 in `SHA256SUMS` does not match. `MZ_VERSION=0.14.0` pins a
version. It supports macOS and Linux (glibc and musl, x86_64 and aarch64); on Windows use npm or cargo.

## Homebrew

`packaging/homebrew/mz.rb` is a draft formula. It needs a tap and real checksums from a release.

## How a release runs

1. Push the tag `v<version>` (it must equal `version` in `Cargo.toml` and `npm/package.json`).
2. The workflow builds all 7 targets, writes `SHA256SUMS`, stages the one npm package and drafts the GitHub release.
3. It publishes the crate to crates.io (the `release` environment, the org `CARGO_REGISTRY_TOKEN`).
4. Only then does it run `npm stage publish --provenance` over OIDC. The npm trusted publisher allows
   staging only: no `npm publish` and no `npm dist-tag`, so the workflow does neither. The version waits in
   npm's stage queue.
5. You approve the staged version with 2FA, on npmjs.com (Staged Packages tab, Approve) or with
   `npm stage list @nyuchi/mzizi-cli` then `npm stage approve <stage-id>`. Approval publishes it and publishes it
   under the `latest` tag. The tag is fixed when the version is staged (`npm stage publish` defaults it to
   `latest`), so the workflow needs no `npm dist-tag` step.
6. You publish the draft GitHub release. Then the Homebrew formula's checksums.

To discard a staged version (a bad build, a failed run): reject it with `npm stage reject <stage-id>` or on npmjs.com (Staged Packages tab, Reject); both ask for 2FA. To
inspect it first: `npm stage view <stage-id>` or `npm stage download <stage-id>`. Staging needs npm 11.15.0 or newer
and Node 22.14.0 or newer, which the workflow installs; staged publishing is documented at
<https://docs.npmjs.com/staged-publishing>.

## Release setup (for the owner)

1. npm: set this repository, the workflow `release.yml` and the environment `release` as the trusted publisher of
   `@nyuchi/mzizi-cli`, with "Allow npm publish" and "Allow npm dist-tag" unchecked (stage only).
2. crates.io: own the `mzizi-cli` name. The org secret `CARGO_REGISTRY_TOKEN` is already visible to this repository.
3. The `release` environment is created on first use; add approval rules if you want them.
4. The Homebrew tap repository, and the formula's real checksums.
