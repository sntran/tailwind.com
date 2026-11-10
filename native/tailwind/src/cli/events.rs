//! The file events of the watch: `c_src/watch.c`.
//!
//! In `tailwind.com`, the watch waits for an event of the system (inotify
//! on Linux, kqueue on macOS and the BSDs), as the file watcher of
//! BEAM.com. In the other builds, and on Windows, there are no events: the
//! wait is a sleep, and the watch polls.

use std::path::Path;
use std::time::Duration;

#[cfg(feature = "ape")]
extern "C" {
    fn tw_watch_start() -> std::os::raw::c_int;
    fn tw_watch_add(path: *const std::os::raw::c_char, is_dir: std::os::raw::c_int);
    fn tw_watch_wait(ms: std::os::raw::c_int) -> std::os::raw::c_int;
}

/// The source of the events.
pub struct Events {
    active: bool,
}

impl Events {
    /// Start the events. With `enabled` false (`--poll`), or when the
    /// system has no events, the watch polls.
    pub fn start(enabled: bool) -> Self {
        #[cfg(feature = "ape")]
        let active = enabled && unsafe { tw_watch_start() } == 1;
        #[cfg(not(feature = "ape"))]
        let active = {
            let _ = enabled;
            false
        };
        Self { active }
    }

    /// True when the system gives events.
    pub fn active(&self) -> bool {
        self.active
    }

    /// Watch a directory or a file. A watched path stays watched.
    pub fn add(&self, path: &Path, is_dir: bool) {
        #[cfg(feature = "ape")]
        if self.active {
            if let Ok(path) = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()) {
                unsafe { tw_watch_add(path.as_ptr(), is_dir as std::os::raw::c_int) };
            }
        }
        #[cfg(not(feature = "ape"))]
        let _ = (path, is_dir);
    }

    /// Wait for a change, at most `timeout`. The result is true when an
    /// event came before the end of the time.
    pub fn wait(&mut self, timeout: Duration) -> bool {
        #[cfg(feature = "ape")]
        if self.active {
            match unsafe { tw_watch_wait(timeout.as_millis().min(i32::MAX as u128) as std::os::raw::c_int) } {
                1 => return true,
                0 => return false,
                // The events do not work: poll from now on.
                _ => self.active = false,
            }
        }
        std::thread::sleep(timeout);
        false
    }
}
