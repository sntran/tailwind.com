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
| `tests/nodes.rs` | For each of 75,082 candidates: the rules before the last pass, the order of their properties and the count of declarations. |
| `tests/css_utility.rs` | `@utility`: 222 stylesheets with 35,044 candidates. |
| `tests/variants.rs` | The variants: the cases of `variants.test.ts`, `@variant`, `@slot`, and the sort order of 345 variants. |
| `tests/optimize.rs` | The last pass: 937 cases of nesting and 802 cases of the full pass, as trees. |
| `tests/css_parser.rs`, `tests/to_css.rs`, `tests/walk.rs` | The cases of `css-parser.test.ts`, `ast.test.ts` and `walk.test.ts`. |
| `tests/value_parser.rs`, `tests/selector_parser.rs`, `tests/attribute_selector_parser.rs`, `tests/utils.rs` | The cases of the test files of Tailwind CSS for these modules. |

Useful variables: `NODES_KIND`, `NODES_FILTER`, `NODES_SHOW`
(`tests/nodes.rs`), `SUITE_FILTER`, `SUITE_SHOW` (`tests/suite.rs`).

## Property tests

The crate uses `proptest`:

- `tests/properties.rs`, on the full compiler: the sequence of the
  candidates does not change the output; a repeated candidate changes
  nothing; two builds give the output of one build with all the
  candidates; the output is a fixed point of parse and print; no text
  makes the compiler panic, as a candidate or as a stylesheet.
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

`tests/watch.sh PROGRAM [--poll=ms]` tests the watch: an edit of a
source file, a new file in a new directory, a file replaced by a
rename, an edit of the input, an error and its repair, and the end of
stdin. CI runs it with the file events of each system and with the
poll.

`node tests/wasi.mjs build/tailwind.wasm` runs the same cases with the
WASI program in Node.js.

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
