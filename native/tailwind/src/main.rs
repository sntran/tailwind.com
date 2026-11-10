//! The command line program.

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(tailwind::cli::run(args));
}
