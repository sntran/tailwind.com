# The NIF

The NIF is the program tailwind.com in the Erlang VM: the same arguments,
the same output, the same exit status, and `--watch`. There is no process
of the operating system, and no file to download. A build of a small
project takes about a third of the time of a port, because no program
starts (see [Speed](#speed)).

The NIF is a choice. tailwind.com gives the NIF (the crate `native/nif`
and the module `tailwind_nif`), and a builder builds it. A project with
no NIF runs the file tailwind.com as a port, as before.

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

## A builder

A builder (for example the builder of BEAM.com) does these steps:

1. Build the crate `native/nif` (the package `tailwind_nif`) of the same
   commit as the Erlang and Elixir modules. `cargo build --release -p
   tailwind_nif` gives `target/release/libtailwind_nif.so` (`.dylib` on
   macOS, `tailwind_nif.dll` on Windows). The crate has QuickJS, in C:
   the crate `cc` compiles it with the C compiler of `CC` or of
   `CC_<target>`.
2. Put the library at `priv/native/libtailwind_nif.so` of the application
   `tailwind` (`libtailwind_nif.dll` on Windows). For a VM that links its
   NIFs into its program, build the crate as a `staticlib` (`cargo rustc
   -p tailwind_nif --crate-type staticlib`): the entry point of the
   module `tailwind_nif` is `tailwind_nif_nif_init` (Rustler).
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
