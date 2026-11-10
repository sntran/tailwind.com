# Internals

## The parts

```
native/tailwind/        the compiler and the command line program (Rust)
  src/*.rs              the compiler: one file for each file of Tailwind CSS
  src/cli/              the command line program
  assets/*.css          the stylesheets of Tailwind CSS, in the program
c_src/                  the entry in C, the calls of Rust for Cosmopolitan, the file events
scripts/build.sh        the link of the Rust library into tailwind.com
src/tailwind.erl        Erlang: install and run tailwind.com
lib/                    Elixir: the module Tailwind and the Mix tasks
specs/                  the TLA+ models
tests/upstream/         the recorded output of Tailwind CSS, and its tools
```

## The compiler

Each file of `native/tailwind/src` follows one file of
`packages/tailwindcss/src` of Tailwind CSS 4.3.5:

| Here | Tailwind CSS | What |
|---|---|---|
| `css_parser.rs` | `css-parser.ts` | CSS text to the tree. |
| `ast.rs`, `to_css.rs` | `ast.ts` | The tree, and the tree to CSS text. |
| `optimize.rs` | `ast.ts` (`optimizeAst`, `handleNesting`) | The last pass: unused theme variables, `@property` and `color-mix()` fallbacks, nesting. |
| `walk.rs` | `walk.ts` | The walk of a tree with replacement. |
| `value_parser.rs`, `selector_parser.rs`, `attribute_selector_parser.rs` | the files of the same names | The parsers of values and selectors. |
| `theme.rs` | `theme.ts` | The values of `@theme`. |
| `candidate.rs` | `candidate.ts` | A class name to a candidate: variants, root, value, modifier. |
| `utilities/` | `utilities.ts` | The utilities, in four parts, and `@utility` (`css_utility.rs`). |
| `variants.rs` | `variants.ts` | The variants, `@variant`, `@slot`. |
| `compile.rs`, `property_order.rs` | `compile.ts`, `property-order.ts` | Candidates to rules, and the sort of the rules. |
| `design_system.rs` | `design-system.ts` | The theme, the utilities, the variants, and the caches. |
| `css_functions.rs` | `css-functions.ts` | `theme()`, `--theme()`, `--spacing()`, `--alpha()`. |
| `apply.rs` | `apply.ts` | `@apply`. |
| `at_import.rs` | `at-import.ts` | `@import`. |
| `index.rs` | `index.ts` | `compile`: all the at-rules of Tailwind CSS, and `build`. |
| `compat.rs`, `compat/` | `compat/` | The parts of the compatibility layer that need no JavaScript. |
| `utils/` | `utils/` | Small functions. `utils/js.rs` has the number and string behavior of JavaScript. |

The differences in structure come from the two languages:

- **No node identity.** Tailwind CSS keeps references to nodes and
  changes them later. The Rust tree has one owner, so the code uses
  paths of indices, ids in an arena (`optimize.rs`), or shared cells for
  the `@utility` rules.
- **No captured design system.** A utility or a variant of Tailwind CSS
  is a closure over the theme. Here each function gets `&DesignSystem`
  when it runs. The theme marks its used variables through `&self`, with
  a `RefCell`.
- **JavaScript numbers and strings.** The output has numbers that
  JavaScript prints (`calc(1 / 3 * 100%)`, `0.5` to `50%`), and sorts
  that compare UTF-16 code units. `utils/js.rs` gives the same results.
- **No regular expressions.** Each one is a small matcher by hand.

The compiler is single-threaded and has no dependency except `indexmap`.

## The command line program

`src/cli/` follows `@tailwindcss/cli` and `@tailwindcss/node`:

- `mod.rs`: the options, the build, the write of the output, the watch.
- `events.rs`: the file events of `c_src/watch.c` for the watch.
- `resolve.rs`: the paths of `@import` (relative files, packages in
  `node_modules` with the `style` condition, and the stylesheets of
  Tailwind CSS in the program).
- `optimize_css.rs`: Lightning CSS for `--minify` and `--optimize`: the
  same version and options as Tailwind CSS, in two passes.
- The scanner is the crate `tailwindcss-oxide` of Tailwind CSS: the
  detection of source files, `.gitignore`, and the extraction of
  candidates are the code of Tailwind CSS itself.

### The watch

A tick of the watch starts at a file event of the system, or at the end
of an interval. At each tick:

1. When the stamp (the time and the size) of the input or of an imported
   stylesheet changed: make a new compiler and a new scanner, scan,
   build and write. With an error, print it and keep the old compiler.
2. If not: scan the source files (the scanner reads only the changed
   files) and build. The compiler gives the same output when there is
   no new candidate, and the program then writes nothing.

The program compares the files itself at each tick, so an event only
wakes it. A lost event or a false event changes the time of a build, not
its result.

The events are in `c_src/watch.c`. It uses the method of the file
watcher of BEAM.com (`c_src/cosmo/beam_com_watch.c`):

| System | Events | Interval |
|---|---|---|
| Linux | inotify on the directories of the files. Cosmopolitan has no wrappers for inotify, so the calls are direct system calls. | 500 ms |
| macOS, FreeBSD, NetBSD, OpenBSD | kqueue on the directories and, while there are descriptors (half of the open file limit, at most 4096), on the files. `struct kevent` has four layouts, and the code selects one at run time. | 500 ms |
| Windows | None. | 250 ms |
| With `--poll[=ms]`, and in a build that is not `tailwind.com` | None. | `--poll` (default 250 ms) |

After an event, the program waits until no more events come for 20 ms
(at most 0.2 s), because an editor often writes a file in more than one
step. The interval finds the changes that have no event: a file in a
directory that has no scanned file yet, and files above the limit of
kqueue.

The program records the stamp of a file before it reads the file. The
model `specs/Watch.tla` shows why: with the other order, an edit between
the read and the stamp is lost. A tick of the model is a tick from an
event or from the interval: the model does not need the difference.

The end of stdin stops the watch (not with `--watch=always`), so the
program stops with the process that started it.

## The file tailwind.com

`scripts/build.sh` builds the crate as a static library for the targets
`x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`, and links
the two with `cosmocc` into one Actually Portable Executable. This is
the method of BEAM.com for Rust:

- `c_src/cosmo_rust.c` translates the calls of the Rust standard library
  that have the numbers of Linux in them (errno, the clocks, the flags
  of `open` and `mmap`, the futex, `stat`) to Cosmopolitan, which sets
  these values for each system at run time. `objcopy --redefine-sym`
  points the Rust library to them.
- On aarch64, Cosmopolitan reserves the registers x18 and x28. The Rust
  standard library builds again with these registers reserved
  (`-Zbuild-std`, a nightly Rust).
- `scripts/rust_link.sh` makes one object with one global name,
  `tailwind_main`, and `c_src/main.c` calls it.

## The Erlang and Elixir modules

`src/tailwind.erl` has the work: the path of the file, the download
(`httpc` with TLS, the proxy of the environment, an optional SHA-256),
and the run through a port. On the systems that are not Windows, the
shell starts the file (`sh tailwind.com`), because the kernel does not
start an APE file directly in all cases. When the owner of the port
ends, the standard input of the program closes, and a watch stops.

`lib/tailwind.ex` gives the API of the Hex package `tailwind` on top of
it, and `lib/mix/tasks/` has the two tasks.
