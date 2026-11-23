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
a deprecation note). There is no postinstall script and no download. If your package manager leaves the bundled binary without the
execute bit (0.14.0 did), the launcher sets it, or, when the package directory is read-only, runs a copy from
`~/.cache/mz/bin` (mode 0700) that it verified against the checksums shipped in the package.

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
refuses to install when the sha256 in `SHA256SUMS` does not match. `MZ_VERSION=0.15.0` pins a
version. It supports macOS and Linux (glibc and musl, x86_64 and aarch64); on Windows use npm or cargo.

## Homebrew

`packaging/homebrew/mz.rb` is a draft formula. It needs a tap and real checksums from a release.

## How a release runs

A release is fully automatic after the tag push. No step waits for a person.

1. Push the tag `v<version>`. It must equal `version` in `Cargo.toml` and `npm/package.json`, or the run stops.
2. The workflow builds all 7 targets, writes `SHA256SUMS`, stages the one npm package and creates a **draft** GitHub release.
3. Only after every build succeeded: it publishes the crate to crates.io (the `release` environment, the org
   `CARGO_REGISTRY_TOKEN`), then `npm publish --provenance` over OIDC (no npm token).
4. When both succeeded, it makes the GitHub release public.
5. It then installs the published package from npm on Linux, macOS and Windows and the crate with
   `cargo install` on Linux, and runs `mz --version` and `mz floor` on a tiny fixture. A failure here fails the run.

Every publish step skips a version that is already published, so a failed job is fixed with "Re-run failed jobs",
never with a new tag. The release stays a draft until everything has published. Afterwards, fill in the Homebrew
formula's checksums (`packaging/homebrew/mz.rb`) from the release's `SHA256SUMS`.

## Locking the release down (settings the owner makes)

Because nothing is approved by hand, the controls are these. The workflow already has them in code: it triggers on
tag push only (plus a dry-run `workflow_dispatch`), no `pull_request_target`, every action pinned to a commit SHA,
read-only default permissions, `id-token: write` on the npm job only, `contents: write` on the release jobs only,
and provenance on, so consumers can verify the build.

1. **Environment `release` (Settings, Environments, release):** deployment branches and tags set to "Selected
   branches and tags", with one rule, tag `v*`. No required reviewers. The publish jobs then run only from a
   release tag, never from a branch or a pull request.
2. **A tag ruleset on `v*` (Settings, Rules, Rulesets):** target tags matching `v*`; rules Restrict creations,
   Restrict updates, Restrict deletions, Block force pushes; bypass list: Repository admin. Only an admin can create
   or move a release tag, and nobody can delete or change one. (The enterprise ruleset already blocks update,
   deletion and force-push of `v[0-9]*.[0-9]*.[0-9]*` tags but does not restrict who creates them.)
3. **Code owners:** `.github/CODEOWNERS` covers the workflows, `npm/`, `install.sh` and the release scripts. It only
   takes effect where a ruleset on `main` has "Require review from Code Owners" turned on; turn that on.
4. **npm trusted publisher** for `@nyuchi/mzizi-cli` (npmjs.com, package settings): repository
   `mzizi-dev/mzizi-cli`, workflow `release.yml`, environment `release`, "Allow npm publish" checked,
   "Allow npm dist-tag" unchecked.
5. **crates.io:** own the `mzizi-cli` name. The org secret `CARGO_REGISTRY_TOKEN` is already visible to this
   repository.
6. The Homebrew tap repository.
