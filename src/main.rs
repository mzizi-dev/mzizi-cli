//! `mz`, the Mzizi CLI.
fn main() {
    std::process::exit(mzizi_cli::cli::main_with(
        "mz",
        std::env::args().skip(1).collect(),
    ));
}
