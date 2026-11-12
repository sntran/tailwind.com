# Benchmarks

`tests/bench/run.mjs` measures `tailwind.com` against the two official
programs of Tailwind CSS: the npm CLI (`@tailwindcss/cli` in Node.js) and
the standalone program (`tailwindcss-linux-x64`, made with Bun). It
measures the time of a build, the time of a watch, the peak memory and
the size on disk.

Each output is also compared with the output of the official npm CLI. A
cell with a different output has the mark `≠`. In the results below, no
cell has the mark: all programs give the same CSS.

## The results

Linux x86_64, 4 CPUs (Intel Xeon at 2.30 GHz), Node.js 24.22.2, Tailwind
CSS 4.3.5. The median of 9 runs, and the fastest run in parentheses.

| Workload | tailwind.com | tailwind.com, script engine | native | tailwind.wasm | official npm CLI | official standalone |
|---|---:|---:|---:|---:|---:|---:|
| start: --help | 4 ms (3 ms) | 3 ms (3 ms) | 4 ms (3 ms) |  | 139 ms (122 ms) | 80 ms (77 ms) |
| preflight only | 10 ms (8 ms) | 88 ms (77 ms) | 9 ms (7 ms) | 146 ms (133 ms) | 176 ms (158 ms) | 126 ms (118 ms) |
| Phoenix | 12 ms (10 ms) | 83 ms (80 ms) | 12 ms (9 ms) | 161 ms (150 ms) | 186 ms (174 ms) | 139 ms (129 ms) |
| Phoenix, --minify | 13 ms (12 ms) | 87 ms (84 ms) | 14 ms (12 ms) | 279 ms (267 ms) | 202 ms (177 ms) | 147 ms (137 ms) |
| Phoenix 1.8.9 with daisyUI | 182 ms (150 ms) | 242 ms (226 ms) | 145 ms (135 ms) | 504 ms (487 ms) | 310 ms (290 ms) | 251 ms (213 ms) |
| typography and forms | 133 ms (126 ms) | 193 ms (181 ms) | 137 ms (119 ms) | 431 ms (418 ms) | 265 ms (243 ms) | 204 ms (188 ms) |
| large project | 222 ms (207 ms) | 1.45 s (1.36 s) | 209 ms (189 ms) | 382 ms (345 ms) | 548 ms (507 ms) | 430 ms (382 ms) |
| large project, --minify | 418 ms (393 ms) | 1.65 s (1.53 s) | 404 ms (360 ms) | 876 ms (771 ms) | 721 ms (656 ms) | 655 ms (585 ms) |
| watch: an edit to a new output | 25 ms | 27 ms | 205 ms |  | 58 ms | 57 ms |
| peak memory: Phoenix 1.8.9 with daisyUI | 13 MB | 14 MB | 15 MB | 109 MB | 103 MB | 89 MB |
| peak memory: large project | 74 MB | 63 MB | 75 MB | 135 MB | 186 MB | 148 MB |
| size on disk | 19.4 MB | 19.4 MB | 11.3 MB | 9.0 MB | 37.7 MB + Node.js | 103.9 MB |

The columns:

- **tailwind.com**: the file of the release (`make`), with its native
  engine: the Rust compiler.
- **tailwind.com, script engine**: the same file with
  `TAILWIND_COM_ENGINE=script`: the official compiler of the package
  of 4.3.5, in the QuickJS of the file ([`VERSIONS.md`](VERSIONS.md)).
  A version that is newer than the port has this speed.
- **native**: the program of `cargo build --release` for this system.
- **tailwind.wasm**: the WASI program, in Node.js, with `runWasm` of
  the npm package (`tests/bench/wasm.mjs`).
- **official npm CLI**: `@tailwindcss/cli` in Node.js, with the module
  rules of the standalone program (`tests/upstream/tools/bun-interop.mjs`).
- **official standalone**: the program of the release of Tailwind CSS.

The workloads:

- **start: --help**: the start of the program, with no build.
- **preflight only**: `@import "tailwindcss" source(none)`: the theme
  and preflight, with no classes.
- **Phoenix**: the Phoenix project of `tests/upstream/cli/phoenix`, with
  no JavaScript.
- **Phoenix 1.8.9 with daisyUI**: a project of `mix phx.new` of Phoenix
  1.8.9 (`tests/upstream/cli-js/phoenix-deps`): daisyUI and heroicons
  as JavaScript plugins.
- **typography and forms**: `@tailwindcss/typography` and
  `@tailwindcss/forms`, from `tests/upstream/cli-js`.
- **large project**: 1000 HTML files with 30 classes each, from the
  classes of the design system (about 5600 different classes, 1.9 MB of
  CSS).
- **--minify**: the same project, with `--minify` (Lightning CSS).
- **watch**: `--watch`, then the time from the write of a source file to
  the new class in the output file (the median of 5 edits).
- **peak memory**: the largest resident set of the processes of one
  build. For `tailwind.wasm` and the npm CLI, this includes Node.js.
- **size on disk**: the program. For the npm CLI, the size of its
  `node_modules` (with the plugins of the benchmarks), and Node.js is
  more.

## What the results show

- **The start.** `tailwind.com` starts in 4 ms. The official programs
  start a JavaScript engine and load the compiler first: 80 ms to
  139 ms. For a small project, the start is most of the time, so
  `tailwind.com` is 12 to 18 times faster.
- **JavaScript plugins.** QuickJS is an interpreter, and V8 and
  JavaScriptCore compile JavaScript to machine code. daisyUI is a large
  plugin: most of the 182 ms is the time of QuickJS. Then
  `tailwind.com` is 1.4 to 2.0 times faster than the official programs,
  and not 10 times. The peak memory is 7 to 8 times smaller.
- **Large projects.** The scanner is the same Rust crate
  (`tailwindcss-oxide`) in all programs, and `--minify` uses Lightning
  CSS in all programs. The difference is the compiler: `tailwind.com` is
  1.6 to 2.5 times faster, and uses less than half the memory of the
  npm CLI.
- **The script engine.** The official compiler in QuickJS needs about
  70 ms more than the native engine for a small project: the engine
  reads the compiler (about 25 ms) and compiles the stylesheet. This is
  still less than the start of the official programs, so a Phoenix
  project builds in 83 ms, against 139 ms and 186 ms. QuickJS is an
  interpreter, so the large project needs 1.45 s: about 3 times the time
  of the official programs, and 6.5 times the time of the native engine.
  A build of the watch after an edit is fast in each engine, because
  the compiler keeps its state.
- **The watch.** `tailwind.com` gets the file events of the system
  (inotify here) and builds again in 25 ms. The native program of
  `cargo build` has no file events: they are in the C part of the APE
  build (the feature `ape`), so it polls, and the poll interval is most
  of its time.
- **tailwind.com and native.** The APE file is about 5% slower than the
  native program on the large project, and 25% slower with daisyUI. The
  two programs have the same Rust code. Thus the difference is in the C
  library and the memory allocator of Cosmopolitan.
- **tailwind.wasm.** Node.js compiles the module of 9.0 MB again for
  each run: this is about 130 ms of each cell (`preflight only`). With
  JavaScript plugins, QuickJS also runs as WebAssembly, and this is
  slower again.

## Older versions of Tailwind CSS

`tests/bench/versions.mjs` measures `tailwind.com` with
`TAILWINDCSS_VERSION` for older versions, with each engine: the native
engine (the Rust compiler, with the switches of that version,
[`SWITCHES.md`](SWITCHES.md)) and the script engine (the official
compiler of that version in QuickJS). The projects are the projects of
the table above. The same machine, the median of 11 runs. The two
engines give the same output in each cell.

| Tailwind CSS | Phoenix, native | Phoenix, script | daisyUI, native | daisyUI, script | large project, native | large project, script |
|---|---:|---:|---:|---:|---:|---:|
| 4.3.5 | 10 ms | 93 ms | 158 ms | 251 ms | 231 ms | 1.43 s |
| 4.3.4 | 12 ms | 93 ms | 157 ms | 233 ms | 213 ms | 1.44 s |
| 4.3.0 | 11 ms | 79 ms | 162 ms | 227 ms | 220 ms | 1.69 s |
| 4.2.4 | 12 ms | 83 ms | 159 ms | 245 ms | 219 ms | 1.72 s |
| 4.2.0 | 11 ms | 83 ms | 165 ms | 241 ms | 238 ms | 1.69 s |
| 4.1.18 | 11 ms | 78 ms | 147 ms | 220 ms | 196 ms | 1.44 s |
| 4.1.0 | 13 ms | 78 ms | 150 ms | 246 ms | 193 ms | 1.32 s |
| 4.0.17 | 13 ms | 85 ms | 150 ms | 209 ms | 103 ms | 471 ms |
| 4.0.8 | 13 ms | 88 ms | 169 ms | 203 ms | 113 ms | 460 ms |
| 4.0.0 | 15 ms | 67 ms | 157 ms | 200 ms | 137 ms | 413 ms |

- **Each version has the speed of the native engine.** With no plugin,
  the Rust compiler is 4 to 9 times faster than the script engine. With
  daisyUI, it is 1.2 to 1.6 times faster: most of the time is the plugin
  in QuickJS, and the two engines have the same QuickJS. On the large
  project, it is 3 to 8 times faster.
- **An older version costs 1 ms to 5 ms more** than 4.3.5 in the native
  engine. The program reads `theme.css` and `preflight.css` of that
  version through the chain of patches ([`VERSIONS.md`](VERSIONS.md)).
- **The large project has less output before 4.1.0.** Its classes come
  from the design system of 4.3.5, and many of them are not in 4.0. The
  output of 4.0.17 is 0.35 MB, and the output of 4.3.5 is 1.9 MB. Thus
  the times of the 4.0 line are smaller.

## Run the benchmarks

```sh
make                                 # build/tailwind.com
make wasm                            # build/tailwind.wasm
cargo build --release --bin tailwind # target/release/tailwind
sh tests/fuzz/install.sh build/official
sh tests/standalone.sh --download build/standalone   # Linux x86_64

node tests/bench/run.mjs --official build/official \
  --program build/tailwind.com --native target/release/tailwind \
  --wasm build/tailwind.wasm \
  --standalone build/standalone/tailwindcss-linux-x64
```

Each column is optional, except the npm CLI. `--runs N` sets the number
of timed runs (default 7). `--only NAME,...` runs only the workloads with
these names. Each workload runs one time with no measure first, so the
files are in the cache of the system.

The table of the older versions needs only `build/tailwind.com`: the
file has the packages of each version. It has `--runs N` (default 9),
`--versions A,B,...` and `--only NAME,...` too.

```sh
node tests/bench/versions.mjs --runs 11
```

The times are for programs on the same machine at the same time. CI
machines are shared, so a difference of less than about 10% does not
tell much.

In CI, the job `bench` of `.github/workflows/build.yml` runs the
benchmarks on Linux and macOS for each push, and writes the table to the
summary of the run.

## Node.js 22 and tailwind.wasm

Node.js 22.23 can stop with the signal SIGSEGV when `tailwind.wasm`
minifies a large output (`large project, --minify`). The crash is in
V8, not in the program:

- wasmtime runs the same module with the same arguments with no error,
  and gives the same output.
- Node.js 24.22 runs it with no error.
- A larger stack (`--stack-size`, `ulimit -s`) and other options of the
  V8 compilers (`--liftoff-only`, `--no-liftoff`) do not stop the crash.

Use Node.js 24 or later for `runWasm` with large projects. The job
`bench` uses Node.js 24 for this reason.
