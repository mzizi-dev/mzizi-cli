//! `mz`, the Mzizi CLI.
fn main() {
    // nosemgrep: rust.lang.security.args.args (not a security decision: these are the CLI's own arguments)
    std::process::exit(mzizi_cli::cli::main_with(
        "mz",
        std::env::args().skip(1).collect(),
    ));
}
