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
| `tests/js_plugins.rs` | `@plugin` and `@config` with a file system in memory: CommonJS and ES modules, `require` and `import` between them, cycles, JSON, packages, the modules of the program, `fs` and `path`, the errors, an exception in a build. The JSON of 10,983 candidates against `parseCandidate` of Tailwind CSS. The Rust code and the JavaScript code for `theme(…)` and the legacy utilities give the same output. |
| `tests/nodes.rs` | For each of 75,082 candidates: the rules before the last pass, the order of their properties and the count of declarations. |
| `tests/css_utility.rs` | `@utility`: 222 stylesheets with 35,044 candidates. |
| `tests/variants.rs` | The variants: the cases of `variants.test.ts`, `@variant`, `@slot`, and the sort order of 345 variants. |
| `tests/optimize.rs` | The last pass: 937 cases of nesting and 802 cases of the full pass, as trees. |
| `tests/css_parser.rs`, `tests/to_css.rs`, `tests/walk.rs` | The cases of `css-parser.test.ts`, `ast.test.ts` and `walk.test.ts`. |
| `tests/value_parser.rs`, `tests/selector_parser.rs`, `tests/attribute_selector_parser.rs`, `tests/utils.rs` | The cases of the test files of Tailwind CSS for these modules. |

Useful variables: `NODES_KIND`, `NODES_FILTER`, `NODES_SHOW`
(`tests/nodes.rs`), `SUITE_FILTER`, `SUITE_SHOW` (`tests/suite.rs`),
`PLUGIN_SUITE_ONLY` (a part of a test name, `tests/plugin_suite.rs`).

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
- `tests/js_plugins.rs`: a tree is the same after the change to JSON
  and back, also through `JSON.parse` and `JSON.stringify` of the
  engine; text with any Unicode character is the same after a pass
  through the host function.
- In the test files of the parsers and of `utils`: no panic on any
  text; parse and print round trips; `segment` and join;
  `unescape(escape(x)) == x`; `compare` is a total order; the number
  functions of `utils/js.rs` round trip.

## The program

`tests/cli.sh PROGRAM` runs the program on 17 small projects, and
compares the output and the exit status with the record of the official
program (`tests/upstream/cli`): automatic source detection with
`.gitignore`, `source(none)`, `@source`, `@source not`,
`@source inline()`, imports, `@theme`, `@utility`, `@variant`, `@apply`,
`@reference`, the functions, a prefix, `--minify`, `--optimize`, `--cwd`,
a Phoenix project, and two errors. CI runs it with `tailwind.com` on
Linux, macOS and Windows, on x86_64 and aarch64.

`CASES=tests/upstream/cli-js sh tests/cli.sh PROGRAM` runs the 88
projects with JavaScript plugins and configs: each function of the
plugin API, config files (theme, `darkMode`, `prefix`, `important`,
`content`, presets), ES modules and CommonJS, `@tailwindcss/typography`,
`@tailwindcss/forms`, `tailwindcss-animate`, daisyUI, a Phoenix 1.8
project with heroicons and daisyUI, and 18 errors. For an error, each
line of the message of the official program must be in the error
output. One case (`error-plugin-syntax`) has a message of the JavaScript
parser of the official program, which another engine cannot repeat: its
file `stderr-contains.txt` has the text that must be there (the path of
the file).

`tests/watch.sh PROGRAM [--poll=ms]` tests the watch: an edit of a
source file, a new file in a new directory, a file replaced by a
rename, an edit of the input, an error and its repair, an edit of a
JavaScript plugin, and the end of stdin. CI runs it with the file events of each system and with the
poll.

`node tests/wasi.mjs build/tailwind.wasm` runs the same cases with the
WASI program in Node.js, and `CASES=tests/upstream/cli-js node
tests/wasi.mjs build/tailwind.wasm` runs the cases with JavaScript (the
program must come from a build with `WASI_SDK_PATH`).

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
