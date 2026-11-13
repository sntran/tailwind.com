//! The command line program.

/// The allocator of the program (the feature `mimalloc`).
#[cfg(all(feature = "mimalloc", not(target_family = "wasm")))]
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    std::process::exit(tailwind::cli::run(args));
}
