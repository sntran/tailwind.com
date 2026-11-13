//! What the program reads from its process: the working directory, the
//! environment, the standard output and error, and the end of the watch.
//!
//! The program (`cli::run`) reads them from the process. A caller in
//! another program gives its own with `cli::run_with`: the NIF of
//! `tailwind_nif` runs the program in the Erlang VM, where the program
//! cannot change the working directory or the environment of the VM.
//!
//! The context is for the thread of the call. Each part of the program
//! that reads it runs on that thread: the build, the watch, the version
//! selection and the JavaScript engine. The threads of the scanner read
//! nothing of it.

use std::cell::RefCell;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

/// The state of the process for one run of the program.
pub struct Context {
    /// The working directory: the relative paths of the arguments, and of
    /// the messages, start here.
    pub cwd: PathBuf,
    /// Changes of the environment of the process: a value, or `None` to
    /// remove the variable. The other variables are the variables of the
    /// process.
    pub env: Vec<(String, Option<String>)>,
    /// The standard output: the CSS when there is no `--output`, and
    /// `--help` and `--version`.
    pub stdout: Box<dyn Write>,
    /// The standard error: the messages.
    pub stderr: Box<dyn Write>,
    /// The messages have colors, as on a terminal. `NO_COLOR` and
    /// `FORCE_COLOR` of `env` still apply.
    pub terminal: bool,
    /// The watch stops when this is true. There is no standard input: the
    /// end of the standard input does not stop the watch, and the input
    /// `-` is empty.
    pub stop: Arc<AtomicBool>,
}

thread_local! {
    static CURRENT: RefCell<Option<Context>> = const { RefCell::new(None) };
    static ARGS: RefCell<Option<Vec<String>>> = const { RefCell::new(None) };
}

/// The context of the thread until the guard drops.
pub(crate) struct Guard;

impl Drop for Guard {
    fn drop(&mut self) {
        CURRENT.with(|current| current.borrow_mut().take());
        ARGS.with(|args| args.borrow_mut().take());
    }
}

pub(crate) fn enter(context: Context, args: Vec<String>) -> Guard {
    CURRENT.with(|current| *current.borrow_mut() = Some(context));
    ARGS.with(|current| *current.borrow_mut() = Some(args));
    Guard
}

/// The arguments of a run of `run_with`, for `process.argv` of a plugin.
pub fn args() -> Option<Vec<String>> {
    ARGS.with(|args| args.borrow().clone())
}

/// True in a run of `run_with`: the program is not a process of its own.
pub fn embedded() -> bool {
    CURRENT.with(|current| current.borrow().is_some())
}

/// The working directory.
pub fn cwd() -> PathBuf {
    CURRENT
        .with(|current| current.borrow().as_ref().map(|context| context.cwd.clone()))
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_default())
}

/// A variable of the environment. A value that is not Unicode text is
/// not there.
pub fn var(name: &str) -> Option<String> {
    let changed = CURRENT.with(|current| {
        current
            .borrow()
            .as_ref()
            .and_then(|context| context.env.iter().rev().find(|(key, _)| key == name).map(|(_, value)| value.clone()))
    });
    match changed {
        Some(value) => value,
        None => std::env::var(name).ok(),
    }
}

/// Each variable of the environment, as `process.env` of Node.js has them.
pub fn vars() -> Vec<(String, String)> {
    let mut vars: Vec<(String, String)> =
        std::env::vars_os().filter_map(|(name, value)| Some((name.into_string().ok()?, value.into_string().ok()?))).collect();
    CURRENT.with(|current| {
        if let Some(context) = current.borrow().as_ref() {
            for (name, value) in &context.env {
                vars.retain(|(key, _)| key != name);
                if let Some(value) = value {
                    vars.push((name.clone(), value.clone()));
                }
            }
        }
    });
    vars
}

/// True when the standard error is a terminal.
pub fn terminal() -> bool {
    use std::io::IsTerminal;
    CURRENT
        .with(|current| current.borrow().as_ref().map(|context| context.terminal))
        .unwrap_or_else(|| std::io::stderr().is_terminal())
}

/// True when the caller asks the watch to stop.
pub fn stopped() -> bool {
    CURRENT.with(|current| current.borrow().as_ref().is_some_and(|context| context.stop.load(Ordering::SeqCst)))
}

/// Write text to the standard output.
pub fn out(text: &str) {
    let written = CURRENT.with(|current| {
        current.borrow_mut().as_mut().map(|context| {
            let _ = context.stdout.write_all(text.as_bytes());
            let _ = context.stdout.flush();
        })
    });
    if written.is_none() {
        let mut stdout = std::io::stdout().lock();
        let _ = stdout.write_all(text.as_bytes());
        let _ = stdout.flush();
    }
}

/// Write text to the standard error.
pub fn err(text: &str) {
    let written = CURRENT.with(|current| {
        current.borrow_mut().as_mut().map(|context| {
            let _ = context.stderr.write_all(text.as_bytes());
            let _ = context.stderr.flush();
        })
    });
    if written.is_none() {
        let _ = std::io::stderr().lock().write_all(text.as_bytes());
    }
}

/// `println!` to the standard output of the context.
macro_rules! outln {
    ($($arg:tt)*) => { $crate::cli::context::out(&format!("{}\n", format_args!($($arg)*))) };
}

/// `eprintln!` to the standard error of the context.
macro_rules! errln {
    ($($arg:tt)*) => { $crate::cli::context::err(&format!("{}\n", format_args!($($arg)*))) };
}

pub(crate) use {errln, outln};
