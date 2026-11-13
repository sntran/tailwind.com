# The NIF

The NIF is the program tailwind.com in the Erlang VM: the same arguments,
the same output, the same exit status, and `--watch`. There is no process
of the operating system, and no file to download. A build of a small
project takes about a third of the time of a port, because no program
starts (see [Speed](#speed)).

The NIF is a choice. tailwind.com gives the NIF: the crate `native/nif`,
the module `tailwind_nif`, and, with each release, the native NIF
objects for BEAM.com. A builder puts the NIF in `priv`. A project with no
NIF runs the file tailwind.com as a port, as before.

## A project

A project does not change. When the NIF is in `priv/native` of the
application `tailwind`, `tailwind:run/2` and `Tailwind.run/2` use it, and
`Tailwind.install_and_run/2` (the watcher of a Phoenix endpoint)
downloads nothing. `mix tailwind.install --if-missing` also downloads
nothing.

The configuration `nif` selects:

| Value | What |
|---|---|
| `true` (the default) | The NIF when it is in `priv/native`, else the file. |
| `false` | The file only. |
| `"PATH"` | The NIF at `PATH` (with no extension), else the file. |

```elixir
config :tailwind, nif: false
```

## BEAM.com

`beam.com` is one static file with no `dlopen()`. It loads a native NIF
object in place of a library: for `erlang:load_nif("PATH", 0)`, it loads
`PATH.x86_64.o` or `PATH.aarch64.o` (the CPU of the computer) into the
running program (`docs/NIFS.md` of BEAM.com). The NIF then runs in the
VM, as a NIF that `beam.com` links, but it is not a part of the file
`beam.com`: only a project with the objects has it. Nothing goes to the
disk, and no program starts. BEAM.com needs no Rust and no compiler for
the NIF:

1. Each release of tailwind.com has the two objects,
   `libtailwind_nif.x86_64.o` and `libtailwind_nif.aarch64.o` (14 MB
   each, 4.1 MB deflated in a zip), with their SHA-256 in `SHA256SUMS`. Use the objects of the release of the
   Erlang and Elixir modules: the functions of the NIF change with the
   module.
2. Put them at `priv/native/libtailwind_nif.x86_64.o` and `.aarch64.o` of
   the application `tailwind`. In `beam.com`, they are NIF files of
   `beam.com`: `lib/beam_com/priv/nifs/tailwind-VSN/native/`, with `VSN`
   the version of the package `tailwind` of tailwind.com. The script
   `scripts/assemble.erl` of BEAM.com adds them to a `beam.com`:

   ```sh
   beam.com scripts/assemble.erl -- beam.com beam-tailwind.com \
     lib/beam_com/priv/nifs/tailwind-0.2.0/native/libtailwind_nif.x86_64.o=libtailwind_nif.x86_64.o \
     lib/beam_com/priv/nifs/tailwind-0.2.0/native/libtailwind_nif.aarch64.o=libtailwind_nif.aarch64.o
   ```

   `beam-tailwind.com DIR -o OUT` then puts them into the priv directory
   of the package `tailwind` of the project, and `tailwind_nif` loads
   them.

An object has one global name, `tailwind_nif_nif_init`. It uses only what
each `beam.com` gives to an object: the `enif_*` functions (Rustler calls
them by name) and the functions of the C library of
`c_src/cosmo/beam_com_nif_libc.h` of BEAM.com. The table must have the
names that BEAM.com added on 2026-11-11 (`asinf`, `atanf`, `expm1`,
`malloc_usable_size` and `__hostos`): a `beam.com` before that does not
load the object, and the error names the first name that is not there.

The object is the program of the file tailwind.com, with the NIF in
place of `main()`. It has each version of Tailwind CSS 4, as the file
has: the store of the npm packages (the newest in full, each older
version as a patch, 1.1 MB) is in the object (`TAILWIND_COM_EMBED_PACKAGES`
of `native/tailwind/build.rs`). `config :tailwind, version: "4.1.0"`
works with the two files only. One difference:

- The watch has the file events of Linux (inotify). On macOS, the BSDs
  and Windows, it polls the files every 250 ms: the table of `beam.com`
  does not have the functions of kqueue and of Windows.

An object has one section for each kind (`native/nif/sections.ld`), not
one for each function: about 36,000 sections took 8 MB for their headers,
names and symbols. LTO already removed the code that the crate does not
use, so the loader has no more to remove.

`make nif` (`scripts/nif.sh`) builds the objects in `build/nif`, with the
cosmocc and the Rust of `make`. The job `nif-beam` of CI
loads them in the `beam.com` of the release `edge` of BEAM.com, on
x86_64 and aarch64, with `tests/nif_object.erl`:

```sh
make nif
sh beam.com tests/nif_object.erl -- build/nif/libtailwind_nif
```

## Another VM

For a VM that loads a library, a builder does these steps:

1. Build the crate `native/nif` (the package `tailwind_nif`) of the same
   commit as the Erlang and Elixir modules. `cargo build --release -p
   tailwind_nif` gives `target/release/libtailwind_nif.so` (`.dylib` on
   macOS, `tailwind_nif.dll` on Windows). The crate has QuickJS, in C:
   the crate `cc` compiles it with the C compiler of `CC` or of
   `CC_<target>`.
2. Put the library at `priv/native/libtailwind_nif.so` of the application
   `tailwind` (`libtailwind_nif.dll` on Windows).
3. Optional: the older versions of Tailwind CSS. The NIF has the
   compiler of Tailwind CSS 4.3.5 only. For each other version, it uses
   the npm package `tailwindcss` of that version in
   `priv/tailwindcss/VERSION`. `sh scripts/packages.sh priv/tailwindcss`
   puts each version of the file `PACKAGES` there (about 350 KB for each
   version, 23 MB for all). The file tailwind.com has these files in its ZIP part.

The NIF needs threads: a run is on a thread of its own. There is no
build of the NIF for WebAssembly yet.

## The module `tailwind_nif`

| Function | What |
|---|---|
| `available()` | True when the NIF is loaded. The first call loads it from `priv/native/libtailwind_nif`. |
| `load(Path)` | Load the NIF from `Path`. |
| `versions()` | The version of the program and of its Tailwind CSS, as binaries. |
| `run(Args, Options)` | Run the program. The output goes to the group leader. The result is the exit status. |
| `run(Args, Options, Fun, Acc)` | Run the program. `Fun(stdout \| stderr, Binary, Acc)` gets each part of the output. The result is `{Status, Acc}`. |

The options are a map:

| Option | What |
|---|---|
| `cd` | The working directory of the run (default: the working directory of the VM). |
| `env` | Changes of the environment of the run: a list of `{Name, Value}` or a map. `false` or `nil` removes a variable. |
| `colors` | The messages have colors (default: `false`, as for a port). |

```erlang
0 = tailwind_nif:run(["-i", "assets/css/app.css", "-o", "priv/static/assets/css/app.css"],
                     #{cd => "/path/of/project", env => [{"TAILWINDCSS_VERSION", "4.1.0"}]}).
```

The module does not load the NIF at its own load (there is no
`-on_load`). Thus the module loads in a VM with no NIF, and
`available()` is then `false`.

## The run

The NIF function starts a thread for the run, and returns at once. The
thread sends each part of the output to the calling process as a
message, and the exit status as the last message. `run/2` and `run/4`
wait for these messages. The schedulers of the VM do not run the
compiler.

The run is not a process, and these are different from a port:

- The working directory and the environment of the run do not change the
  VM. Each run has its own.
- There is no standard input. The input `-` is empty.
- A watch stops when the calling process ends: the process keeps the
  resource of the run, and when the garbage collector frees it, the
  watch stops at its next tick. This is also true for `--watch=always`.
- A panic of the program does not stop the VM: the exit status is 101.
- The standard output and the standard error are two streams (`run/4`).
  `run/2` writes the two to the group leader, as a port with
  `stderr_to_stdout`.

## Speed

A build of a small project (`@import "tailwindcss"` and one HTML file),
from Erlang, 20 times, on Linux x86_64:

| Run | Time of a build |
|---|---|
| The NIF | 2.1 ms |
| A port: the program of `cargo build --release` | 6.4 ms |
| A port: `tailwind.com` | 7.9 ms |

The difference is the start of a program. For a large project, the build
itself takes most of the time, and the difference is smaller.

## Tests

```sh
cargo build --release -p tailwind -p tailwind_nif
TAILWIND_NIF=target/release/libtailwind_nif TAILWIND_COM=target/release/tailwind mix test
```

With `TAILWIND_NIF`, the tests of `tests/tailwind_test.exs` run the
program as a NIF, and `tests/tailwind_nif_test.exs` also runs. The job
`nif` of `.github/workflows/build.yml` does this.
