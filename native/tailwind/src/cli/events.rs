//! The file events of the watch: `c_src/watch.c`.
//!
//! In `tailwind.com`, the watch waits for an event of the system (inotify
//! on Linux, kqueue on macOS and the BSDs, `ReadDirectoryChangesW` on
//! Windows), as the file watcher of BEAM.com. In the other builds there
//! are no events: the wait is a sleep, and the watch polls.
//!
//! With `TAILWIND_WATCH_DEBUG=1` in the environment, the watch prints
//! what it uses and what wakes it to stderr. The tests use these lines
//! (`tests/watch.sh` with `WATCH_EXPECT`).

use std::path::Path;
use std::time::Duration;

#[cfg(feature = "ape")]
extern "C" {
    fn tw_watch_start() -> std::os::raw::c_int;
    fn tw_watch_name() -> *const std::os::raw::c_char;
    fn tw_watch_add(path: *const std::os::raw::c_char, kind: std::os::raw::c_int);
    fn tw_watch_wait(ms: std::os::raw::c_int) -> std::os::raw::c_int;
}

/// The source of the events.
pub struct Events {
    active: bool,
    debug: bool,
}

/// The kinds of a path of `tw_watch_add`.
const FILE: i32 = 0;
const DIR: i32 = 1;
const TREE: i32 = 2;

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
        let debug = super::context::var("TAILWIND_WATCH_DEBUG").is_some_and(|value| value == "1");
        let events = Self { active, debug };
        if active {
            events.debug(&format!("events ({})", events.name()));
        } else {
            events.debug("poll");
        }
        events
    }

    /// The name of the events of this system.
    fn name(&self) -> String {
        #[cfg(feature = "ape")]
        let name = unsafe { std::ffi::CStr::from_ptr(tw_watch_name()) }.to_string_lossy().into_owned();
        #[cfg(not(feature = "ape"))]
        let name = String::from("none");
        name
    }

    /// Print a line of `TAILWIND_WATCH_DEBUG`.
    pub fn debug(&self, text: &str) {
        if self.debug {
            super::context::err(&format!("watch: {text}\n"));
        }
    }

    /// True when the system gives events.
    pub fn active(&self) -> bool {
        self.active
    }

    /// Watch a directory of files, or a file. A watched path stays watched.
    pub fn add(&self, path: &Path, is_dir: bool) {
        self.add_kind(path, if is_dir { DIR } else { FILE });
    }

    /// Watch a source directory and all that is below it. Only Windows
    /// has such a watch: the other systems ignore the path.
    pub fn add_tree(&self, path: &Path) {
        self.add_kind(path, TREE);
    }

    fn add_kind(&self, path: &Path, kind: i32) {
        #[cfg(feature = "ape")]
        if self.active {
            if let Ok(path) = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()) {
                unsafe { tw_watch_add(path.as_ptr(), kind as std::os::raw::c_int) };
            }
        }
        #[cfg(not(feature = "ape"))]
        let _ = (path, kind);
    }

    /// Wait for a change, at most `timeout`. The result is true when an
    /// event came before the end of the time.
    pub fn wait(&mut self, timeout: Duration) -> bool {
        #[cfg(feature = "ape")]
        if self.active {
            match unsafe { tw_watch_wait(timeout.as_millis().min(i32::MAX as u128) as std::os::raw::c_int) } {
                1 => {
                    self.debug("event");
                    return true;
                }
                0 => return false,
                // The events do not work: poll from now on.
                _ => {
                    self.active = false;
                    self.debug("poll (the events stopped)");
                    return false;
                }
            }
        }
        std::thread::sleep(timeout);
        false
    }
}
