# Contributing

Open a pull request into `main`. Every pull request names an issue (`Closes #n`, `Refs #n`,
or `No issue: <reason>`) and adds an entry under `## [Unreleased]` in `CHANGELOG.md`.

Before you push, run the same gate CI runs:

```sh
cargo fmt -- --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
node --test npm/test/
```

Rules specific to this repo:

- Behaviour parity with `@nyuchi/mzizi-cli` 0.13.0 is the contract until the port is
  declared done. A change to behaviour is a separate pull request from the port.
- No live network in tests. Record the response once under `tests/fixtures/recorded/`.
- No OpenSSL, no JavaScript runtime in the Rust binary; TLS is rustls.
- Every GitHub Action is pinned to a full commit SHA with the version in a comment.
- Nothing is published from a pull request. Releases come from a tag, by the release workflow.
