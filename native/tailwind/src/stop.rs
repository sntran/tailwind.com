//! The stop of a run by its caller: the NIF of `tailwind_nif` stops a
//! build when its process ends, or at the end of the time of `timeout`.
//!
//! The caller gives a flag for the thread of the run (`watch`). The long
//! loops read it (`requested`), and the run ends soon after the caller
//! sets it:
//!
//! - the compiler: each candidate, and each part of a brace expansion;
//! - the program: before and after the scan of the sources, and before
//!   the optimizer and the write of the output;
//! - the JavaScript engine of the plugins: each 10,000 instructions or so
//!   (the interrupt handler of QuickJS).
//!
//! The scanner of the sources and Lightning CSS have no check: one scan
//! or one pass of the optimizer is the longest delay.

use crate::Error;
use std::cell::RefCell;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

thread_local! {
    static FLAG: RefCell<Option<Arc<AtomicBool>>> = const { RefCell::new(None) };
}

/// The message of the error of a run that stopped.
pub const MESSAGE: &str = "The caller stopped the run.";

/// The flag of this thread until the guard drops.
pub struct Guard(Option<Arc<AtomicBool>>);

impl Drop for Guard {
    fn drop(&mut self) {
        let previous = self.0.take();
        FLAG.with(|flag| *flag.borrow_mut() = previous);
    }
}

/// Read `flag` for the runs of this thread, until the guard drops.
pub fn watch(flag: Arc<AtomicBool>) -> Guard {
    Guard(FLAG.with(|current| current.borrow_mut().replace(flag)))
}

/// True when the caller of the run of this thread asks it to stop.
pub fn requested() -> bool {
    FLAG.with(|flag| flag.borrow().as_ref().is_some_and(|flag| flag.load(Ordering::Relaxed)))
}

/// The error of a run that stopped.
pub fn error() -> Error {
    Error::new(MESSAGE)
}

/// `Err(error())` when the caller asks the run to stop.
pub fn check() -> Result<(), Error> {
    if requested() {
        Err(error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flag_is_for_the_thread_until_the_guard_drops() {
        assert!(!requested());
        let flag = Arc::new(AtomicBool::new(false));
        {
            let _guard = watch(flag.clone());
            assert!(!requested());
            flag.store(true, Ordering::SeqCst);
            assert!(requested());
            assert_eq!(check().unwrap_err().message, MESSAGE);
            // Another thread has no flag.
            assert!(!std::thread::spawn(requested).join().unwrap());
        }
        assert!(!requested());
        assert!(check().is_ok());
    }
}
