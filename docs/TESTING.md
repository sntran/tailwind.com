# Tests

```sh
make test          # all the tests
```

## The compiler against Tailwind CSS

`cargo test` runs the tests of the crate. The largest part replays the
record of the official compiler in `tests/upstream` (see
[`tests/upstream/README.md`](../tests/upstream/README.md) for the files
and for the tools that make them again). The tests need no Node.js.

| Test file | What it compares |
|---|---|
| `tests/suite.rs` | Each `compile` call of the unit tests of Tailwind CSS: the output, or the error message; the features, the sources and the root. Then all the recorded candidates in one build, and `@import "tailwindcss"` with real-world classes. |
| `tests/plugin_suite.rs` | The 296 `compile` calls of the unit tests of Tailwind CSS that load a JavaScript plugin or config, with each module as source text (`plugin-suite.jsonl.gz`): the CSS of each build, the error message, the features and the sources. |
| `tests/js_plugins.rs` | `@plugin` and `@config` with a file system in memory: CommonJS and ES modules, TypeScript modules, `require` and `import` between them, cycles, JSON, packages, `imports` of a `package.json`, `import()` in a CommonJS file, the modules of the program, the three plugins in the program, `fs` and `path`, the errors and their `cause` chain, an exception in a build. The JSON of 10,983 candidates against `parseCandidate` of Tailwind CSS. The Rust code and the JavaScript code for `theme(…)` and the legacy utilities give the same output. |
| `tests/node_api.rs` | The parts of Node.js that a module gets, against the record of Node.js 22 (`tests/fixtures/node_api.json`): 124 values for `Buffer`, `URL`, `URLSearchParams`, `TextEncoder`, `TextDecoder`, the timers, `structuredClone`, `process`, `os`, `url`, `util`, `assert`, `fs`, `module`. The resolver against `require.resolve` and `import.meta.resolve` of Node.js: 80 names, each one for `require` and for `import`. The three rules: each call that writes a file, opens a connection or starts a process throws the error of its rule. `import.meta`, the timers at the end of a module, and the format of `console.log`. |
| `tests/warnings.rs`, `tests/optimize_warnings.rs` | The warnings for `--value(…)` of `@utility`. The warnings of the optimizer against the record of the official program (`tests/fixtures/optimize_warnings.json`): the code and the exact text with its color codes, for 13 inputs, plain and minified. |
| `tests/nodes.rs` | For each of 75,082 candidates: the rules before the last pass, the order of their properties and the count of declarations. |
| `tests/css_utility.rs` | `@utility`: 222 stylesheets with 35,044 candidates. |
| `tests/variants.rs` | The variants: the cases of `variants.test.ts`, `@variant`, `@slot`, and the sort order of 345 variants. |
| `tests/optimize.rs` | The last pass: 937 cases of nesting and 802 cases of the full pass, as trees. |
| `tests/css_parser.rs`, `tests/to_css.rs`, `tests/walk.rs` | The cases of `css-parser.test.ts`, `ast.test.ts` and `walk.test.ts`. |
| `tests/source_maps.rs` | The source maps of 317 stylesheets (`source-maps.jsonl.gz`): the locations of the parser, the decoded map, its JSON text, and the code and the map of the optimizer, plain and minified. The messages of 23 syntax errors with their positions. The cases of `translation-map.test.ts`. The cases of `line-table.test.ts` and of the test of `@tailwindcss/node` are unit tests in `src/source_maps/line_table.rs` and `src/cli/source_maps.rs`. The records have the six `compile` calls of `source-map.test.ts`. |
| `tests/value_parser.rs`, `tests/selector_parser.rs`, `tests/attribute_selector_parser.rs`, `tests/utils.rs` | The cases of the test files of Tailwind CSS for these modules. |

Useful variables: `NODES_KIND`, `NODES_FILTER`, `NODES_SHOW`
(`tests/nodes.rs`), `SUITE_FILTER`, `SUITE_SHOW` (`tests/suite.rs`),
`MAPS_FILTER`, `MAPS_SHOW`, `MAPS_DUMP`, `MAPS_FILE`
(`tests/source_maps.rs`),
`PLUGIN_SUITE_ONLY` (a part of a test name, `tests/plugin_suite.rs`).

The upstream tests of the source maps compare pictures of the mappings
(`visualize-source-map.ts`). The port has no such picture: the tests
compare the numbers of the mappings.

The unit tests of the crate (`cargo test --lib`) have the tests of the
JavaScript engine (`compat/js/engine.rs`: evaluate, call, exceptions
with their messages, the jobs of promises, the host function, ES
modules, the memory limit and the stack limit) and of the resolver of
modules (`compat/js/resolve.rs`).

The tests with JavaScript need the engine. In a build with no engine
(`--no-default-features`, or WASI with no wasi-sdk), they are not
compiled.

## Property tests

The crate uses `proptest`:

- `tests/properties.rs`, on the full compiler: the sequence of the
  candidates does not change the output; a repeated candidate changes
  nothing; two builds give the output of one build with all the
  candidates; the output is a fixed point of parse and print; no text
  makes the compiler panic, as a candidate or as a stylesheet.
- `tests/source_maps.rs`, with generated stylesheets and candidates:
  each mapping of a map is in the output and in its source; the
  generated positions are sorted; the JSON text has the same mappings
  as the decoded map; the CSS is the same with and without the source
  locations; a tree with locations is equal to the tree without them;
  the optimizer gives the same code with and without a map, and each
  mapping of its map is in the output and in a source.
- `tests/js_plugins.rs`: a tree is the same after the change to JSON
  and back, also through `JSON.parse` and `JSON.stringify` of the
  engine; text with any Unicode character is the same after a pass
  through the host function.
- In the test files of the parsers and of `utils`: no panic on any
  text; parse and print round trips; `segment` and join;
  `unescape(escape(x)) == x`; `compare` is a total order; the number
  functions of `utils/js.rs` round trip.

## The program

`tests/cli.sh PROGRAM` runs the program on 20 small projects, and
compares the output and the exit status with the record of the official
program (`tests/upstream/cli`): automatic source detection with
`.gitignore`, `source(none)`, `@source`, `@source not`,
`@source inline()`, imports, `@theme`, `@utility`, `@variant`, `@apply`,
`@reference`, the functions, a prefix, `--minify`, `--optimize`, `--cwd`,
a Phoenix project, two errors, and three projects with warnings (the
optimizer with one warning and with three, and `--value(…)` of
`@utility`).

A case can have more files than the output and the exit status:

| File | What |
|---|---|
| `stderr.txt` | For an error: each line of the message of the official program must be in the error output. |
| `stderr-contains.txt` | With the exit status 0: the warning lines of the official program, with no color codes. Each line must be in the error output. With an error: the lines that must be there, for a message that another JavaScript engine cannot repeat. |
| `env.txt` | The environment variables for the program, one `NAME=VALUE` for each line. `<project>` is the path of the project. These cases do not run on Windows. |
| `project.txt` | In place of `project/`: the name of another case. The two cases use one project. |
| `standalone.txt` | The record comes from the npm program with the module rules of the standalone program: see `tests/standalone.sh` below.

CI runs the cases with `tailwind.com` on Linux and macOS (x86_64 and
aarch64), on Windows, and in a VM on FreeBSD, NetBSD and OpenBSD 7.3
(x86_64).

`TAILWIND_RUN` in the environment is a program that starts `PROGRAM`,
with an absolute path, for `tests/cli.sh` and `tests/watch.sh`: the APE
loader of cosmocc (`ape-x86_64.elf`) on NetBSD, where `sh` cannot start
an APE file. With no `TAILWIND_RUN`, `sh` starts a file with the name
`*.com` (on Windows, the file is a program).

`CASES=tests/upstream/cli-map sh tests/cli.sh PROGRAM` runs the 29
projects of `tests/upstream/cli-map` for `--map`:
the map in the output and in a file (in the directory of the output and
in another one), with no `--output`, with `--cwd`, with `--minify` and
`--optimize` (with and without the replacement of `@media not (`), with
imports of more than one file, with stdin, with text that is not ASCII,
with a JavaScript plugin and a JavaScript config (6 projects), the
`--map=-` error, and syntax errors with their positions. For these
cases it compares the output file and the map file too. A map has the
path of the project, so the records have `/PROJECT` at its place, and
the test puts the same text into the output of the program (also into
a map that is in a comment as Base64 text; this needs the command
`base64`, or `openssl` where there is no such command). These cases do
not run on Windows.

`CASES=tests/upstream/cli-js sh tests/cli.sh PROGRAM` runs the 100
projects with JavaScript plugins and configs: each function of the
plugin API, config files (theme, `darkMode`, `prefix`, `important`,
`content`, presets), ES modules and CommonJS, TypeScript configs and
plugins (`.ts`, `.mts`, `.cts`, and a JavaScript config that imports
TypeScript files), `@tailwindcss/typography`, `@tailwindcss/forms`,
`tailwindcss-animate`, daisyUI, the three plugins in the program with
no `node_modules` directory, two Phoenix projects with heroicons and
daisyUI (the layout to Phoenix 1.8.5 with vendor files, and the layout
of Phoenix 1.8.9 with `deps/`, `NODE_PATH` and `phoenix-colocated`),
two configs with a warning, and 19 errors. For an error, each line of
the message of the official program must be in the error output. Two
cases (`error-plugin-syntax`, `error-ts-syntax`) have a message of the
JavaScript parser of the official program, which another engine cannot
repeat: their file `stderr-contains.txt` has the text that must be
there (the path of the file).

`sh tests/standalone.sh PROGRAM` is for the official standalone program
of Tailwind CSS, and not for this program. The records of the cases
with `standalone.txt` come from the npm program with a copy of the two
module rules of the standalone program
(`tests/upstream/tools/bun-interop.mjs`). The script runs the real
standalone program on these cases (10 cases: the Phoenix projects, the
daisyUI bundle, the plugins in the program), and a difference from the
record is an error with a diff. `sh tests/standalone.sh --download DIR`
gets `tailwindcss-linux-x64` of the release and checks its SHA-256
(`UPSTREAM`). The job `standalone` of CI does the two steps.

`tests/watch.sh PROGRAM [--poll=ms]` tests the watch: an edit of a
source file, a new file in a new directory, a file replaced by a
rename, an edit of the input (also in the source map: the watch runs
with `--map`), an error and its repair, an edit of a JavaScript plugin,
and the end of stdin (not on Windows: the shell there has no FIFO that
works). CI runs it with the file events of each system and with the
poll.

The test runs the watch with `TAILWIND_WATCH_DEBUG=1` (see "The watch"
in [`INTERNALS.md`](INTERNALS.md)), and prints the count of each line
of the watch. With `WATCH_EXPECT=events`, the watch must use the file
events of the system, must not go to the poll, and at least 3 builds
must start at an event. With `WATCH_EXPECT=poll`, the watch must poll.
CI sets `WATCH_EXPECT=events` on each system (Linux, macOS, Windows,
FreeBSD, NetBSD, OpenBSD), so events that do not work there fail the
job.

`node tests/wasi.mjs build/tailwind.wasm` runs the same cases with the
WASI program in Node.js. `CASES` selects the directory of the cases, as
for `tests/cli.sh`. The project is the root directory there, so a
source of a map is `/app.css`. The cases with JavaScript (all of
`cli-js`, and 6 cases of `cli-map`) need a program from a build with
`WASI_SDK_PATH`. `<project>` in `env.txt` is the directory `/` there.

## The TLA+ models

`make tlc` checks the models of `specs/` with TLC (Java 11 or later; the
script downloads `tla2tools.jar` v1.7.4 and checks its SHA-256).

| Model | What it checks |
|---|---|
| `Watch.tla` | The watch: the output never comes from an input with an error; one error is printed two times at most; when the edits stop, the output becomes the build of the last input with all the candidates. |
| `Watch.wrong_order.cfg` | The same model with the stamp of a file recorded after the read. TLC must find the lost edit: this run must fail. |

TLC found a real error with this model. The first version of the watch
recorded the stamps of the files of a new compiler after it read them.
An edit between the read and the stamp was then lost.

## The Erlang and Elixir modules

`mix test` (ExUnit, `tests/*_test.exs`) runs the program of
`TAILWIND_COM`, or `target/release/tailwind`: the configuration, the
version of the file, a build, the extra arguments, the exit status of an
error, the environment, the end of a watch with its process, the install
from a URL with the SHA-256 check, and the two Mix tasks.

## The npm package

`node --test tests/npm.mjs` tests `js/`: the cache directory, the URL of
the download, a download from a local server, the SHA-256 check, a
status that is not 200, a download that stops, the start of the file by
the shell, `run()`, the exit status of `npx tailwind.com`, and
`runWasm()` with the WebAssembly module.

## The live differential fuzzer

`tests/fuzz/fuzz.mjs` compares this compiler with the official Tailwind
CSS on new random input, live. The official side is the npm packages of
the version of `UPSTREAM` (`sh tests/fuzz/install.sh DIR` installs
them). Two kinds of cases:

- **compiler**: a random stylesheet (imports, `@theme`, `@utility`,
  `@custom-variant`, `@apply`, the functions, `@source inline()`, a
  prefix, `important`) and random candidates (the class list, the
  variants, arbitrary values, edge cases, and small random changes of
  them). `compile()` and `build()` of the npm package against
  `examples/fuzz_build.rs` of this crate.
- **project**: a directory with `app.css` and HTML, HEEx and JavaScript
  files with random classes. The official command line program against
  this program, with the same arguments (also `--minify` and
  `--optimize`). Both use the scanner of Tailwind CSS.

```sh
cargo build --release --bin tailwind --example fuzz_build
sh tests/fuzz/install.sh build/official
node tests/fuzz/fuzz.mjs --official build/official \
  --builder target/release/examples/fuzz_build --program target/release/tailwind \
  --seed 7 --cases 2000 --projects 60
```

A difference goes to `build/fuzz/failures.jsonl` with its reduced form:
the smallest part of the stylesheet and of the candidates that still
differs. The same seed gives the same cases. The workflow
`.github/workflows/fuzz.yml` runs the fuzzer for each push (10 minutes)
and each night (45 minutes), with a new seed each time, and keeps the
failures as the artifact `fuzz`.

To keep a fixed failure fixed, add its reduced case (`css`,
`candidates`, and `official`, the official output) as one line to
`tests/upstream/fuzz-found.jsonl`. `tests/fuzz_found.rs` replays each
line.

A fault made on purpose (a builder that changes the value of
`underline`) shows that the fuzzer finds a difference, and reduces a
case of 16 candidates to two lines of stylesheet.
