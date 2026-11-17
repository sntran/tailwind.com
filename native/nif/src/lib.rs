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
//! The run stops when the last reference to the resource goes (the
//! process that keeps it ends), or at `nif_stop/1`: a watch ends at its
//! next tick, and a build at its next check (`stop.rs` of the crate
//! `tailwind`). After the stop, the thread sends no more messages.

use rustler::{Encoder, LocalPid, OwnedBinary, OwnedEnv, ResourceArc};
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use tailwind::cli::context::Context;

mod atoms {
    rustler::atoms! {
        tailwind_nif,
        stdout,
        stderr,
        exit,
    }
}

/// The state of a run for the thread and for the resource.
struct Shared {
    /// The program reads it: the run stops when it is true.
    stop: Arc<AtomicBool>,
    /// A message goes out only with this lock, and only when `stop` is
    /// false. So after `stop`, no message comes.
    gate: Mutex<()>,
}

impl Shared {
    fn stop(&self) {
        let _gate = self.gate.lock();
        self.stop.store(true, Ordering::SeqCst);
    }

    /// Send a message, when the run did not stop.
    fn send(&self, env: &mut OwnedEnv, pid: &LocalPid, message: impl FnOnce(rustler::Env) -> rustler::Term) {
        let _gate = self.gate.lock();
        if !self.stop.load(Ordering::SeqCst) {
            let _ = env.send_and_clear(pid, message);
        }
    }
}

/// A run of the program. Its drop stops the run.
struct Run {
    shared: Arc<Shared>,
}

#[rustler::resource_impl]
impl rustler::Resource for Run {}

impl Drop for Run {
    fn drop(&mut self) {
        self.shared.stop();
    }
}

/// A stream of the program (the standard output or error): each write is
/// a message to the process.
struct Messages {
    pid: LocalPid,
    id: u64,
    stream: rustler::Atom,
    env: OwnedEnv,
    shared: Arc<Shared>,
}

impl Write for Messages {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if bytes.is_empty() {
            return Ok(0);
        }
        let (id, stream) = (self.id, self.stream);
        self.shared.send(&mut self.env, &self.pid, |env| {
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
    let shared = Arc::new(Shared { stop: Arc::new(AtomicBool::new(false)), gate: Mutex::new(()) });
    let run = ResourceArc::new(Run { shared: shared.clone() });
    std::thread::Builder::new()
        .name("tailwind".into())
        // The stack of the program: the compiler and Lightning CSS go deep
        // for some stylesheets (`nesting.rs` of the crate `tailwind`).
        .stack_size(tailwind::nesting::STACK_SIZE)
        .spawn(move || {
            let stream = |stream| Messages {
                pid,
                id,
                stream,
                env: OwnedEnv::new(),
                shared: shared.clone(),
            };
            let context = Context {
                cwd: PathBuf::from(cwd),
                env: vars,
                stdout: Box::new(stream(atoms::stdout())),
                stderr: Box::new(stream(atoms::stderr())),
                terminal,
                stop: shared.stop.clone(),
            };
            // A panic must not stop the VM: it is the status 101, as for a
            // panic of the program.
            let status = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                tailwind::cli::run_with(argv, context)
            }))
            .unwrap_or(101);
            shared.send(&mut OwnedEnv::new(), &pid, |env| (atoms::tailwind_nif(), id, atoms::exit(), status).encode(env));
        })
        .map_err(|error| rustler::Error::Term(Box::new(error.to_string())))?;
    Ok(run)
}

/// Stop a run: a build stops at its next check, and a watch at its next
/// tick. After this call, the process gets no more message of the run.
#[rustler::nif]
fn nif_stop(run: ResourceArc<Run>) -> rustler::Atom {
    run.shared.stop();
    rustler::types::atom::ok()
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
