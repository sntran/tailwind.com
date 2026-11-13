//! The NIF of tailwind.com: the program `tailwind` in the Erlang VM, for
//! the module `tailwind_nif` (`src/tailwind_nif.erl`, `docs/NIF.md`).
//!
//! `nif_start/6` runs the program (`tailwind::cli::run_with`) on a thread of
//! its own, and gives a resource at once. The thread sends the output and
//! the exit status to a process:
//!
//! ```text
//! {tailwind_nif, Id, stdout, Binary}
//! {tailwind_nif, Id, stderr, Binary}
//! {tailwind_nif, Id, exit, Status}
//! ```
//!
//! When the last reference to the resource goes (the process that keeps it
//! ends), the run stops: a watch ends at its next tick, as the program
//! does at the end of its standard input.

use rustler::{Encoder, LocalPid, OwnedBinary, OwnedEnv, ResourceArc};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tailwind::cli::context::Context;

mod atoms {
    rustler::atoms! {
        tailwind_nif,
        stdout,
        stderr,
        exit,
    }
}

/// The stack of the thread of a run. The compiler goes deep for some
/// stylesheets, and the main thread of a program has 8 MiB.
const STACK: usize = 16 << 20;

/// A run of the program. Its drop stops the run.
struct Run {
    stop: Arc<AtomicBool>,
}

#[rustler::resource_impl]
impl rustler::Resource for Run {}

impl Drop for Run {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
    }
}

/// A stream of the program (the standard output or error): each write is
/// a message to the process.
struct Messages {
    pid: LocalPid,
    id: u64,
    stream: rustler::Atom,
    env: OwnedEnv,
}

impl Write for Messages {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let (id, stream) = (self.id, self.stream);
        let _ = self.env.send_and_clear(&self.pid, |env| {
            let mut binary = OwnedBinary::new(bytes.len());
            binary.as_mut_slice().copy_from_slice(bytes);
            (atoms::tailwind_nif(), id, stream, binary.release(env)).encode(env)
        });
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// Run the program with the arguments `argv`, in the directory `cwd`,
/// with the changes `vars` of the environment (`nil` removes a variable).
/// The messages go to `pid`, with the number `id`. `terminal`: the
/// messages have colors.
#[rustler::nif]
fn nif_start(
    argv: Vec<String>,
    cwd: String,
    vars: Vec<(String, Option<String>)>,
    pid: LocalPid,
    id: u64,
    terminal: bool,
) -> rustler::NifResult<ResourceArc<Run>> {
    let stop = Arc::new(AtomicBool::new(false));
    let run = ResourceArc::new(Run { stop: stop.clone() });
    std::thread::Builder::new()
        .name("tailwind".into())
        .stack_size(STACK)
        .spawn(move || {
            let stream = |stream| Messages {
                pid,
                id,
                stream,
                env: OwnedEnv::new(),
            };
            let context = Context {
                cwd: PathBuf::from(cwd),
                env: vars,
                stdout: Box::new(stream(atoms::stdout())),
                stderr: Box::new(stream(atoms::stderr())),
                terminal,
                stop,
            };
            // A panic must not stop the VM: it is the status 101, as for a
            // panic of the program.
            let status = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                tailwind::cli::run_with(argv, context)
            }))
            .unwrap_or(101);
            let _ = OwnedEnv::new().send_and_clear(&pid, |env| {
                (atoms::tailwind_nif(), id, atoms::exit(), status).encode(env)
            });
        })
        .map_err(|error| rustler::Error::Term(Box::new(error.to_string())))?;
    Ok(run)
}

/// The version of the program and of the Tailwind CSS of its compiler.
#[rustler::nif]
fn nif_versions() -> (String, String) {
    (
        tailwind::cli::PROGRAM_VERSION.to_string(),
        tailwind::VERSION.to_string(),
    )
}

rustler::init!("tailwind_nif");
