# Language commands

`src/lang.rs` is `mzizi-dev/mzizi` `compiler/src/main.rs` (584 lines) moved here with its
history (`git log --follow src/lang.rs`). The only edit is `fn main()` becoming
`pub fn run_command(args)`. It uses the library crate `mzizi-lang-compiler` from crates.io (0.18.0, default features off)
(the `cli` feature is the language repo's own `mz` binary); the dependency points from this repo to the language, never back.

Parity, measured 2026-11-22 against the language repo's own `mz` at the same tag:
`check`, `check --agent`, `contract`, `outline`, `hash`, `ir` over all 27 examples and
primitives (162 cases), `build` over the 12 examples (output tree and text), `harness
version|definition|entry`, `run` of two programs, and `fix`: identical stdout, stderr and exit
status. Reproduce with `scripts/parity-lang.sh`. Not covered here: the compiler's own
integration tests (`compiler/tests/*.rs`) spawn `CARGO_BIN_EXE_mz` and stay in the language
repo until its `[[bin]]` is removed (see the migration checklist, #3).
