# Compatibility with Tailwind CSS

The compiler follows Tailwind CSS **4.3.5** (commit `9b2c925`). The
output of the official compiler is the specification: a difference is a
bug, except the items of "Known differences".

## Results

`cargo test` compares with the record of the official compiler
(`tests/upstream`). The comparison is on the exact text.

| Record | Result |
|---|---|
| The `compile` calls of the unit tests of Tailwind CSS that need no JavaScript (output or error message, features, sources, root) | 2,028 of 2,028 the same |
| The `compile` calls that load a JavaScript plugin or config | 306: each one gives the "needs JavaScript" error |
| More recorded cases for imports, `@apply` with `@utility`, `theme()` key paths, legacy utilities and errors | 82 of 82 the same |
| 75,082 candidates in one build: each class of the default design system, its variants, 6,000 random candidates, 784 edge cases (24 MB of CSS) | the same |
| Each 50th of these candidates alone, with a new compiler | 1,502 of 1,502 the same |
| `@import "tailwindcss"` with 686 real-world classes, and with no class | 2 of 2 the same |
| The rules of each of the 75,082 candidates before the last pass | 75,031 the same, 0 different. 51 need the compatibility layer, which this test does not load; the builds above cover them. |
| `@utility`: 222 stylesheets, 35,044 candidates | the same |
| The last pass of the compiler, as trees | 1,739 of 1,739 the same |
| The official command line program on 17 projects (also `--minify` and `--optimize`) | 17 of 17 the same, with the native program, with `tailwind.com` and with `tailwind.wasm` |
| The source maps of 317 stylesheets (331 builds): the decoded map of `buildSourceMap()`, the JSON text of `@tailwindcss/node`, and the code and the map of the optimizer, plain and minified | 317 of 317 the same |
| The source locations of the parser for these stylesheets, and the position in the message of 23 syntax errors | the same |
| The official command line program with `--map` on 23 projects (the map in the output and in a file, `--minify`, `--optimize`, imports, stdin, errors): the output and the map file | 23 of 23 the same with the native program on Linux. The test replaces the path of the project in the maps. |
| Lightning CSS from Rust against the optimizer of `@tailwindcss/node`, on 322 outputs of the unit tests, plain and minified | 644 of 644 the same |

The scanner is the crate of Tailwind CSS itself, so the classes that it
finds in the source files are the same by construction.

## Known differences

- **`@plugin` and `@config`** are errors: the program has no JavaScript
  engine. The message is ``"`@plugin` needs JavaScript. This compiler
  does not support `@plugin` and `@config`."`` The errors of Tailwind CSS
  for a wrong `@plugin` or `@config` rule come first, as upstream.
- **Source maps of the stylesheets in the program**: a stylesheet of
  Tailwind CSS that is in the program has the source path
  `/$tailwindcss/NAME.css` in a source map. The official program from
  npm gives the path in `node_modules`. With `node_modules/tailwindcss`
  in the project, the paths are the same.
- **Source locations without `--map`**: the official program always
  gives the path of the input to the compiler. This program does it only
  with `--map`, because the locations have a cost. The output is the
  same. For the position in the message of a syntax error, the program
  compiles the stylesheet a second time when the first time fails.
- **The source map before the first build**: `build_source_map` of the
  library gives the map of the tree of the input then. No record
  compares this map with `buildSourceMap()` of Tailwind CSS before
  `build`. The program always builds first.
- **The source map in the watch with the optimizer**: in the official
  program, a build that gives the same CSS as the build before it
  writes the map from before the optimizer, with `--minify` and with
  `--optimize`. This program keeps the map of the optimizer.
- **A symbolic link in the path of an imported stylesheet**: the
  official program gives the real path as the source. This program gives
  the path with the link.
- **Source maps on Windows**: no test compares them with the official
  program there.
- **The watch** uses inotify or kqueue, and polls on Windows. The
  official program uses `@parcel/watcher`. `--poll` is the same.
- **`canonicalize`**, the class list, and the sort of classes for
  editors are not there.
- **An invalid `@variant` in a custom variant**: Tailwind CSS throws
  from `build` when a candidate uses that variant. Here the candidate
  gives no rule. In `@apply`, the error is the same as upstream.
- **A warning** of Tailwind CSS for an unsupported data type in
  `--value()` of `@utility` is not printed. The output is the same.
- **Warnings of the optimizer** (Lightning CSS, with `--minify` or
  `--optimize`) are not printed.
- **`@source inline()` with an endless range** is an error. The official
  compiler runs until it has no more memory.
- **The help text** has the same options, in another layout.
- **Text that is not valid Unicode** cannot be an input. JavaScript
  strings can hold a lone surrogate.

## What is the same on purpose in the source maps

- Tailwind CSS can put one node object at two places of the tree (a
  custom variant with two selectors, a compound variant on such a
  variant, a `@slot` used two times). The printer then gives the node
  the position of its last place, and the map has two equal mappings
  there and none at the first place. The port gives its copies of such
  a node one identity, and the same result.
- The optimizer gives source paths with no first slash
  (`tmp/project/app.css`), because Lightning CSS makes them relative to
  the root directory.
- With no match of `@media not (` in the output of the optimizer, the
  map is the JSON text of Lightning CSS (`sourceRoot`, then `mappings`).
  With a match, it is the JSON text of `@jridgewell/remapping`
  (`mappings`, `names`, `ignoreList`, then `sources`).

## What is the same on purpose

Some results of Tailwind CSS look wrong, and the port keeps them,
because the output must be the same. `tests/optimize.rs` has cases for
each one. For example: two equal at-rules with direct declarations
merge, and the second one loses the declarations that the first one
does not have.

## A new version of Tailwind CSS

`sh scripts/sync-upstream.sh apply VERSION` does the mechanical part and
gives the list of the changes to port. See [`SYNC.md`](SYNC.md).
