# @nyuchi/mzizi-cli

The Mzizi CLI. This package is a thin launcher: it runs the Rust `mz` binary from
[mzizi-dev/mzizi-cli](https://github.com/mzizi-dev/mzizi-cli) and contains no CLI logic.

```sh
npx @nyuchi/mzizi-cli add button
npm i -g @nyuchi/mzizi-cli && mz --help
pnpm dlx @nyuchi/mzizi-cli --version
```

Every platform binary is bundled in this one package (macOS arm64 and x64, Linux x64 and arm64
on glibc and musl, Windows x64); the launcher picks yours. Set `MZ_BIN` to use your own binary.
`mzizi` and `fundi` are the old names of `mz`, kept as bins for one release.
Node 18 or later. See [docs/install.md](https://github.com/mzizi-dev/mzizi-cli/blob/main/docs/install.md).
