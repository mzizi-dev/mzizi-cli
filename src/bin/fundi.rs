//! `fundi`, the old name of `mz`. Same program; prints a deprecation notice on stderr.
fn main() {
    std::process::exit(mzizi_cli::cli::main_with("fundi", std::env::args().skip(1).collect()));
}
