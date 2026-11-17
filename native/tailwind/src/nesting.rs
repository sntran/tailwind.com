//! The limits of nesting, and the stack of a run.
//!
//! Tailwind CSS is JavaScript. A recursive function of the compiler that
//! goes too deep throws `RangeError: Maximum call stack size exceeded`, and
//! the official program prints this error. With Node.js 22, a stylesheet
//! with 1,737 nested rules gives this error, and a value with 5,143 nested
//! functions (`--spacing()` in `calc()`) gives it too.
//!
//! This compiler is Rust, and a stack overflow stops the process: the
//! program, or the Erlang VM of the NIF. Thus the parsers have limits, and
//! give the error of the official program for a deeper input. The limits
//! are larger than the limits of Node.js, so each stylesheet that the
//! official program compiles compiles here too. Each run has a large stack
//! (`STACK_SIZE`): it holds the recursion of the compiler and of Lightning
//! CSS at these limits.
//!
//! - `MAX_BLOCKS`: the CSS parser gives the error for a deeper block.
//! - `MAX_FUNCTIONS`: the parsers of values and selectors mark the run
//!   (`exceed`), and keep the deeper text flat. `index::compile` and
//!   `Compiler::try_build` then give the error (`take_error`).
//!
//! Some parts of Tailwind CSS are not recursive: `walk` and the parser of
//! values. There, a deeper input works in the official program, and it
//! works here too (for example `var()` in `var()`, 200,000 times: see
//! `utils/variables.rs`).

use crate::Error;
use std::cell::Cell;

/// The largest number of nested blocks (rules and at-rules) of a
/// stylesheet.
pub const MAX_BLOCKS: usize = 2_000;

/// The largest number of nested functions of a value or a selector.
pub const MAX_FUNCTIONS: usize = 10_000;

/// The stack of the thread of a run: the program (`main.rs`) and the NIF.
/// The system reserves the memory, and uses only the pages that the run
/// touches. At the limits, a run uses less than 40 MiB (Lightning CSS for
/// a selector with 9,990 nested `:is()`), and 2 MiB for 2,000 nested
/// rules. A program that calls `compile` on its own thread needs such a
/// stack too.
pub const STACK_SIZE: usize = 256 << 20;

/// The message of the error, as Node.js gives it.
pub const MESSAGE: &str = "Maximum call stack size exceeded";

thread_local! {
    static EXCEEDED: Cell<bool> = const { Cell::new(false) };
}

/// The error of the official program for an input that nests too deep.
pub fn error() -> Error {
    Error { message: MESSAGE.to_string(), name: std::borrow::Cow::Borrowed("RangeError"), causes: Vec::new() }
}

/// Mark the run of this thread: an input is deeper than a limit.
pub fn exceed() {
    EXCEEDED.with(|exceeded| exceeded.set(true));
}

/// The error when a parser marked the run of this thread since the last
/// call. The call removes the mark.
pub fn take_error() -> Option<Error> {
    EXCEEDED.with(|exceeded| exceeded.replace(false)).then(error)
}

/// Run `f` on a thread with a stack of `STACK_SIZE`, and give its result.
/// When the system gives no such thread, `f` runs on this thread.
///
/// A WebAssembly program has no threads: there, `f` runs on this thread.
/// The engine of the host (for example V8) sets the depth of its calls,
/// and a deep input can stop the program before the limits of this file.
pub fn with_stack<T: Send, F: FnOnce() -> T + Send>(f: F) -> T {
    #[cfg(not(target_family = "wasm"))]
    {
        let slot = std::sync::Mutex::new(Some(f));
        let take = || slot.lock().ok().and_then(|mut f| f.take());
        let result = std::thread::scope(|scope| {
            let thread = std::thread::Builder::new().stack_size(STACK_SIZE).spawn_scoped(scope, || take().map(|f| f()));
            match thread.map(|thread| thread.join()) {
                Ok(Ok(result)) => result,
                Ok(Err(panic)) => std::panic::resume_unwind(panic),
                Err(_) => None,
            }
        });
        match result {
            Some(result) => result,
            None => match take() {
                Some(f) => f(),
                None => unreachable!("the closure of the run is in the slot or ran"),
            },
        }
    }
    #[cfg(target_family = "wasm")]
    {
        f()
    }
}
