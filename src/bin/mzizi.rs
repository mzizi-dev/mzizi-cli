//! `mzizi`, the old name of `mz`. Same program; prints a deprecation notice on stderr.
fn main() {
    eprintln!("note: `mzizi` is now `mz`; this name stays for one release.");
    std::process::exit(mzizi_cli::cli::main_with(std::env::args().skip(1).collect()));
}
