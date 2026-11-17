//! The command line program.

/// The allocator of the program (the feature `mimalloc`).
#[cfg(all(feature = "mimalloc", not(target_family = "wasm")))]
#[global_allocator]
static ALLOCATOR: mimalloc::MiMalloc = mimalloc::MiMalloc;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    // The main thread has a small stack on some systems (1 MiB on
    // Windows): the run has a thread with a large one (`nesting.rs`).
    std::process::exit(tailwind::nesting::with_stack(|| tailwind::cli::run(args)));
}
