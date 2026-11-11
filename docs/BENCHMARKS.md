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

| Workload | tailwind.com | native | tailwind.wasm | official npm CLI | official standalone |
|---|---:|---:|---:|---:|---:|
| start: --help | 3 ms (3 ms) | 3 ms (2 ms) |  | 102 ms (99 ms) | 72 ms (69 ms) |
| preflight only | 7 ms (6 ms) | 6 ms (6 ms) | 115 ms (107 ms) | 139 ms (132 ms) | 98 ms (95 ms) |
| Phoenix | 10 ms (9 ms) | 8 ms (7 ms) | 141 ms (116 ms) | 141 ms (137 ms) | 123 ms (115 ms) |
| Phoenix, --minify | 13 ms (10 ms) | 10 ms (9 ms) | 219 ms (205 ms) | 154 ms (148 ms) | 119 ms (117 ms) |
| Phoenix 1.8.9 with daisyUI | 140 ms (129 ms) | 126 ms (123 ms) | 374 ms (352 ms) | 241 ms (221 ms) | 186 ms (182 ms) |
| typography and forms | 109 ms (108 ms) | 102 ms (99 ms) | 323 ms (304 ms) | 211 ms (206 ms) | 170 ms (161 ms) |
| large project | 202 ms (188 ms) | 167 ms (153 ms) | 298 ms (293 ms) | 395 ms (377 ms) | 309 ms (299 ms) |
| large project, --minify | 354 ms (344 ms) | 296 ms (286 ms) | 587 ms (558 ms) | 523 ms (494 ms) | 463 ms (439 ms) |
| watch: an edit to a new output | 24 ms | 204 ms |  | 55 ms | 55 ms |
| peak memory: Phoenix 1.8.9 with daisyUI | 13 MB | 15 MB | 111 MB | 101 MB | 91 MB |
| peak memory: large project | 74 MB | 75 MB | 134 MB | 189 MB | 146 MB |
| size on disk | 18.2 MB | 10.9 MB | 8.5 MB | 37.7 MB + Node.js | 103.9 MB |

The columns:

- **tailwind.com**: the file of the release (`make`).
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
  classes of the design system (about 18000 different classes, 1.9 MB of
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

- **The start.** `tailwind.com` starts in 3 ms. The official programs
  start a JavaScript engine and load the compiler first: 70 ms to
  100 ms. For a small project, the start is most of the time, so
  `tailwind.com` is 12 to 20 times faster.
- **JavaScript plugins.** QuickJS is an interpreter, and V8 and
  JavaScriptCore compile JavaScript to machine code. daisyUI is a large
  plugin: about 120 ms of the 140 ms is the time of QuickJS. Then
  `tailwind.com` is 1.3 to 1.9 times faster than the official programs,
  and not 10 times. The peak memory is 7 to 8 times smaller.
- **Large projects.** The scanner is the same Rust crate
  (`tailwindcss-oxide`) in all programs, and `--minify` uses Lightning
  CSS in all programs. The difference is the compiler: `tailwind.com` is
  1.3 to 2 times faster, and uses less than half the memory of the npm
  CLI.
- **The watch.** `tailwind.com` gets the file events of the system
  (inotify here) and builds again in 24 ms. The native program of
  `cargo build` has no file events: they are in the C part of the APE
  build (the feature `ape`), so it polls, and the poll interval is most
  of its time.
- **tailwind.com and native.** The APE file is about 20% slower than
  the native program on the large project. The two programs have the
  same Rust code. Thus the difference is in the C library and the
  memory allocator of Cosmopolitan.
- **tailwind.wasm.** Node.js compiles the module of 8.5 MB again for
  each run: this is about 100 ms of each cell (`preflight only`). With
  JavaScript plugins, QuickJS also runs as WebAssembly, and this is
  slower again.

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
