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
| `tests/source_maps.rs` | The source maps of 317 stylesheets (`source-maps.jsonl.gz`): the locations of the parser, the decoded map, its JSON text, and the code and the map of the optimizer, plain and minified. The messages of 23 syntax errors with their positions. The cases of `translation-map.test.ts`. The cases of `line-table.test.ts` and of the test of `@tailwindcss/node` are unit tests in `src/source_maps/line_table.rs` and `src/cli/source_maps.rs`. The records have the six `compile` calls of `source-map.test.ts`. |
| `tests/value_parser.rs`, `tests/selector_parser.rs`, `tests/attribute_selector_parser.rs`, `tests/utils.rs` | The cases of the test files of Tailwind CSS for these modules. |

Useful variables: `NODES_KIND`, `NODES_FILTER`, `NODES_SHOW`
(`tests/nodes.rs`), `SUITE_FILTER`, `SUITE_SHOW` (`tests/suite.rs`),
`MAPS_FILTER`, `MAPS_SHOW`, `MAPS_DUMP`, `MAPS_FILE`
(`tests/source_maps.rs`).

The upstream tests of the source maps compare pictures of the mappings
(`visualize-source-map.ts`). The port has no such picture: the tests
compare the numbers of the mappings.

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

It also runs the 23 projects of `tests/upstream/cli-map` for `--map`:
the map in the output and in a file (in the directory of the output and
in another one), with no `--output`, with `--cwd`, with `--minify` and
`--optimize` (with and without the replacement of `@media not (`), with
imports of more than one file, with stdin, with text that is not ASCII,
the `--map=-` error, and syntax errors with their positions. For these
cases it compares the output file and the map file too. A map has the
path of the project, so the records have `/PROJECT` at its place, and
the test puts the same text into the output of the program (also into
a map that is in a comment as Base64 text; this needs the command
`base64`). These cases do not run on Windows.

`tests/watch.sh PROGRAM [--poll=ms]` tests the watch: an edit of a
source file, a new file in a new directory, a file replaced by a
rename, an edit of the input (also in the source map: the watch runs
with `--map`), an error and its repair, and the end of stdin. CI runs it with the file events of each system and with the
poll.

`node tests/wasi.mjs build/tailwind.wasm` runs the same cases with the
WASI program in Node.js. The project is the root directory there, so a
source of a map is `/app.css`.

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
