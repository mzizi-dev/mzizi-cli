# @nyuchi/mzizi-cli

The Mzizi CLI. This package is a thin launcher: it runs the Rust `mz` binary from
[mzizi-dev/mzizi-cli](https://github.com/mzizi-dev/mzizi-cli) and contains no CLI logic.

```sh
npx @nyuchi/mzizi-cli add button
npm i -g @nyuchi/mzizi-cli && mz --help
pnpm dlx @nyuchi/mzizi-cli --version
```

The binary comes from a per-platform optional dependency (`@nyuchi/mzizi-cli-<platform>`).
If that is missing, the launcher downloads the release binary and verifies its sha256.
Set `MZIZI_BIN` to use your own binary; set `MZIZI_SKIP_DOWNLOAD=1` to forbid the download.
Node 18 or later. See [docs/install.md](https://github.com/mzizi-dev/mzizi-cli/blob/main/docs/install.md).
