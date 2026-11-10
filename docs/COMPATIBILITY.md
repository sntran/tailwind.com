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
| Lightning CSS from Rust against the optimizer of `@tailwindcss/node`, on 322 outputs of the unit tests, plain and minified | 644 of 644 the same |

The scanner is the crate of Tailwind CSS itself, so the classes that it
finds in the source files are the same by construction.

## Known differences

- **`@plugin` and `@config`** are errors: the program has no JavaScript
  engine. The message is ``"`@plugin` needs JavaScript. This compiler
  does not support `@plugin` and `@config`."`` The errors of Tailwind CSS
  for a wrong `@plugin` or `@config` rule come first, as upstream.
- **Source maps**: `--map` is an error.
- **The watch** polls. `--poll` only sets the interval.
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

## What is the same on purpose

Some results of Tailwind CSS look wrong, and the port keeps them,
because the output must be the same. `tests/optimize.rs` has cases for
each one. For example: two equal at-rules with direct declarations
merge, and the second one loses the declarations that the first one
does not have.

## A new version of Tailwind CSS

1. Make the record of the new tag: `tests/upstream/tools/make-all.sh`
   (see [`tests/upstream/README.md`](../tests/upstream/README.md)).
2. Run `cargo test`. The failures show the changes of behavior.
3. Port the changes: each file of `native/tailwind/src` names its file
   of Tailwind CSS.
4. Update the stylesheets of `native/tailwind/assets` from the npm
   package, the tag of `tailwindcss-oxide` and the version of
   `lightningcss` in `native/tailwind/Cargo.toml`, and `VERSION` in
   `index.rs`.
