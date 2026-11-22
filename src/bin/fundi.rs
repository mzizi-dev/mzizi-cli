//! `fundi`, the old name of `mz`. Same program; prints a deprecation notice on stderr.
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect(); // nosemgrep: rust.lang.security.args.args

    std::process::exit(mzizi_cli::cli::main_with("fundi", args));
}
