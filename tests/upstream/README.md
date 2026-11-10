# Upstream fixtures

These files record the output of the official Tailwind CSS compiler (TypeScript).
The Rust tests replay them. The tests do not need Node.js.

## Versions

| Item | Value |
| --- | --- |
| Upstream repository | https://github.com/tailwindlabs/tailwindcss |
| Tag | `v4.3.5` |
| Commit | `9b2c925c19aae46ff4f9cf54a2fa6d2dbac7fa19` |
| npm packages for the CLI cases | `tailwindcss@4.3.5`, `@tailwindcss/cli@4.3.5`, `@tailwindcss/oxide@4.3.5` |
| lightningcss (npm) | `1.33.0`, for the unit tests and for the CLI |
| lightningcss (Rust crate) | `1.0.0-alpha.72` (the `Cargo.toml` at the lightningcss tag `v1.33.0`) |
| parcel_sourcemap (Rust crate) | `2.1.1` (the `Cargo.lock` at the lightningcss tag `v1.33.0`) |
| For the source maps of the clone | `source-map-js` 1.2.1, `magic-string` 1.1.0, `@jridgewell/remapping` 2.3.5 |
| For the source maps of the CLI cases (the npm install) | `source-map-js` 1.2.2, `magic-string` 1.4.5, `@jridgewell/remapping` 2.3.5 |
| Node.js | 22 |
| pnpm | 11.9.0 |
| vitest | 4.1.10 |

The upstream patch `patches/lightningcss@1.33.0.patch` changes only the loader of the native package. It does not change the CSS output.

## Files

### `suite.jsonl.gz`

One JSON line for each call of `compile(input, options)` in the unit tests of `packages/tailwindcss`.
All 44 test files run (`packages/tailwindcss/src/**/*.test.ts`). The file has 2334 lines and 2239 builds.

Fields:

- `file`: the path of the test file, relative to the repository.
- `test`: the full test name.
- `input`: the CSS string.
- `options`: `base`, `from`, `polyfills` (`null` when not set), `hasLoadModule`, `hasLoadStylesheet`.
- `stylesheets`: each `loadStylesheet` call in order: `id`, `base`, and `result` (`path`, `base`, `content`) or `error`.
- `modules`: the number of `loadModule` calls. `moduleRequests` gives the `id`, `base` and `hint` of each call.
  A record with `modules` > 0 uses a JavaScript plugin or config. The Rust tests cannot replay such a record.
- `builds`: each `build(candidates)` call in order: `candidates`, and `css` or `error`.
  `optimized` is `optimize(css).code`. `minified` is `optimize(css, { minify: true }).code`.
  `optimizeError` is there when `optimize` throws.
  The output of a compiler is cumulative, thus the order of the builds is important.
- `error`: the message when `compile` throws or rejects. Then `sources`, `root` and `features` are absent.
- `sources`, `root`, `features`: the values of the compiler object.

The tests run with `NODE_ENV=test`. Thus the output has no license banner comment.
The lines are in the order of the test files. In one test file, the lines are in call order.
Six tests in the `sorting` group of `index.test.ts` shuffle their candidates with `Math.random()`.
Thus the `builds` of these six lines change from run to run. All the other lines are stable.
The tests that call only `__unstable__loadDesignSystem`, the candidate parser or other internal functions give no lines.

### `suite-extra.jsonl.gz`

Records in the format of `suite.jsonl.gz` (82 lines), from `tools/gen-extra.mjs` and not from the upstream unit tests.
The cases are for the parts that have a different structure in the Rust port:
the sequence of the `@import` loads, `@apply` with `@utility`, `theme()` with a key path such as `colors.red.500`
(the compatibility layer with the default theme), the legacy utilities, and the errors of `compile`.
The base is `/root`. A file `./NAME.css` has the path `BASE/NAME.css` and the base `BASE/NAME`.

### `classes.jsonl.gz`

One JSON line for each candidate: `{"candidate", "kind", "css"}`.
A fresh compiler builds each candidate alone. `css` is the raw output of `build([candidate])`.
The input CSS is:

```css
@layer theme, base, components, utilities;
@import "tailwindcss/theme.css" layer(theme);
@import "tailwindcss/utilities.css" layer(utilities);
```

`kind` tells the source of the candidate:

- `class` (68109): each class of `getClassList()`, and the class with at most 3 of its modifiers (first, middle, last).
- `variant` (189): each variant of `getVariants()` as `NAME:flex`, at most 5 values as `NAME-VALUE:flex`, and compound and named forms.
- `random` (6000): random candidates. The generator is mulberry32 with the seed `20240405`.
  A random candidate has 0 to 3 variants, a class root or an arbitrary form, and optional `-`, `!` and modifier parts.
- `edge` (784): hand-made invalid candidates and edge cases from `tools/candidate-lists.mjs`.

Each candidate occurs one time. A later duplicate is dropped.
Many candidates are invalid. Then `css` is the output with no utilities:
`@layer theme, base, components, utilities;\n@layer utilities;\n`.
Some entries of `getClassList()` also give no utilities, for example `border/50`.

### `classes-all.json.gz`

One JSON object: `{"input", "candidates", "css", "optimized", "minified"}`.
`candidates` has all the candidates of `classes.jsonl.gz` in the same order.
`css` is the raw output of one compiler with all the candidates in one `build` call.
This file checks the sort order of the rules. The CSS strings are large (24 MB to 30 MB each).

### `full.json.gz`

A JSON array with two entries. Each entry is `{"input", "candidates", "css", "optimized", "minified"}`.
The input is `@import "tailwindcss";`.

1. An entry with 686 common candidates.
2. An entry with no candidates (the theme defaults and preflight only).

### `css/`

Copies of `index.css`, `theme.css`, `preflight.css` and `utilities.css` from `packages/tailwindcss` at the tag.
The generator scripts give these files to the compiler with a virtual path: `/tailwindcss/NAME.css`, base `/tailwindcss`.

### `cli/`

Cases for the official CLI. Each case is a directory `cli/NAME/`:

- `project/`: the input CSS and the source files.
- `args.txt`: the CLI arguments, one for each line. There is no `-o`, thus the output goes to stdout.
- `stdout.css`: the exact stdout of the official CLI. The current directory is `project/`.
- `exit.txt`: the exit code.
- `stderr.txt`: the stderr text, only when the exit code is not 0. It contains ANSI color codes.

The CLI does not run with `NODE_ENV=test`. Thus `stdout.css` starts with the license banner comment.

| Case | Content |
| --- | --- |
| `auto-detect` | `@import "tailwindcss"` with automatic source detection: `.html`, `.js`, `.vue`, `.svelte`, a nested directory, a `.gitignore`, binary files, a `.css` file and a lock file that the scanner must ignore. |
| `source-none` | `source(none)` and `@source "../x"`. |
| `source-not` | `@source not` for a directory and for a glob, and an added `@source` glob. |
| `source-inline` | `@source inline(...)` with brace expansion, and `@source not inline(...)`. |
| `imports` | Relative `@import` with `layer()`, media queries, `supports()`, and a nested import. |
| `theme` | `@theme` overrides, `--color-*: initial`, `@theme inline`, `@theme static`, `@keyframes` in `@theme`. |
| `utility` | Static and functional `@utility` with `--value()` and `--modifier()`, and `@apply` in `@utility`. |
| `variants` | `@custom-variant` (selector form and block form with `@slot`), and `@variant` in CSS rules. |
| `apply` | `@apply` in `@layer components`, in `@layer base`, in nested rules, with variants and with `!`. |
| `reference` | `@reference` to a file with a theme, a utility and a custom variant. |
| `functions` | `theme()`, `--theme()`, `--spacing()`, `--alpha()`, also in media queries. Uses `--optimize`. |
| `prefix-important` | `prefix(tw)` and `important`. Uses `--minify`. |
| `layers` | `theme.css` and `utilities.css` with no preflight, plain CSS in layers with nesting. Uses `--minify`. |
| `phoenix` | A Phoenix-style `app.css` with `.heex` and `.ex` files and `~H` sigils. There is no `@plugin`. |
| `cwd` | The `--cwd` argument. |
| `error-circular-apply` | A circular `@apply`. The exit code is 1 and stdout is empty. |
| `error-unknown-apply` | An `@apply` of an unknown utility. The exit code is 1 and stdout is empty. |

Some projects contain a `.gitignore` file and files that it ignores (for example `cli/auto-detect/project/ignored/`).
Git also obeys these nested `.gitignore` files. Use `git add -f tests/upstream/cli` to add the ignored files.

The `-m`/`--minify` and `--optimize` outputs come from lightningcss.

### `cli-map/`

Cases for the official CLI with `--map`, in the layout of `cli/`. A case can also have:

- `stdin.txt`: the text for the standard input of the CLI.
- `files/`: each file that the CLI made in `project/`: the output file of `-o` and the source map of `--map FILE`.

A source map has the absolute path of the project. `tools/run-cli.mjs` writes `/PROJECT` at its place
in `stdout.css`, in `stderr.txt` and in the files, also in the JSON text of a map that is in a comment as Base64 text.
Lightning CSS gives the paths without the first slash: there the record has `PROJECT`.
`tests/cli.sh` does the same with the output of the program before the comparison.

| Case | Content |
| --- | --- |
| `inline-stdout` | `--map` with no `-o`: the map is in a comment of the output. The source is the input file. |
| `inline-stdin`, `inline-output` | `-i -` with `-o -` and with `-o out.css`: the source is `stdin.css`. |
| `inline-stdin-minify`, `inline-stdin-optimize`, `inline-stdin-minify-no-replacement` | The same with the optimizer. |
| `inline-file-minify` | `-o out.css --map --minify` with an input file. |
| `file` | `-o dist/out.css --map dist/out.css.map`. |
| `file-other-directory` | `--map=maps/x.map` with the output in `dist/css/`: the comment has `../../maps/x.map`. |
| `file-stdout` | `--map out.map` with no `-o`. |
| `file-cwd` | `--cwd sub` with relative paths. |
| `file-minify`, `file-optimize` | `--minify` and `--optimize`. The output has `@media not (`, so the replacement makes a new map. |
| `file-minify-no-replacement`, `file-optimize-no-replacement` | The same with no `@media not (`: the map is the map of Lightning CSS. |
| `file-imports`, `file-imports-minify` | Imports of four files, one with `layer()`, one with a media query, and a nested import. |
| `file-text-not-ascii` | Text that is not ASCII in the input and in a class. |
| `error-map-dash`, `error-map-dash-value` | `--map=-` and `--map -`: the exit code is 1. |
| `error-syntax-stdout`, `error-syntax-with-output`, `error-syntax-in-import` | A CSS syntax error: the message has the path, the line and the column. |

The cases use no stylesheet of Tailwind CSS: the official CLI gives the path in `node_modules` for it,
and the Rust program has these stylesheets in the program.

### `source-maps.jsonl.gz`

One JSON line for each stylesheet (317 lines, 331 builds), from `tools/gen-source-maps.mjs`.
Upstream compiles the input with the `from` option. Fields:

- `name`, `input`, `from`, `base`, `polyfills`, and `stylesheets` (the loader calls, as in `suite.jsonl.gz`).
- `locations`: the `[start, end]` of the `src` of each node of `parse(input, { from })`, in document order
  (`null` for a node with no `src`). The offsets are UTF-16 code units.
- `builds`: for each `build(candidates)`: `candidates`, `css`, and
  - `map`: the decoded map of `buildSourceMap()`: `sources` (`url`, `content`) and `mappings`
    (`[generated line, generated column, source index, original line, original column]`; lines start at 1),
  - `raw`: the JSON text of `toSourceMap(map)` of `@tailwindcss/node`,
  - `optimized`, `minified`: the `code` and the `map` of `optimize(css, { file, minify, map: raw })`.
- `error`: for an input that upstream rejects, the message with its position (23 lines).

A content that is equal to a stylesheet of `css/` is `{"css": NAME}`, and `"@@css:NAME@@"` in the JSON text of a map.
The content of the generated source of a decoded map is `{"generated": true}`: it is the `css` of the build.

The inputs: each 13th record of `suite.jsonl.gz` that uses no JavaScript module and has no error (150 lines),
the six records of `source-maps/source-map.test.ts`, the 33 records of `suite-extra.jsonl.gz` with no error,
the two entries of `full.json.gz`, 73 cases of the script (`@theme`, `@apply`, `@utility`, `@custom-variant`, `@variant`,
`@reference`, imports of more than one file, fallbacks, nesting, text that is not ASCII, CRLF, the optimizer),
and 30 lists of random candidates of `classes.jsonl.gz` (the seed is `20260405`).
`SOURCE_MAPS_STEP=1` uses each record of the suite (about 2,100 lines): the Rust test reads such a file with `MAPS_FILE`.

## How to make the files again

```sh
# 1. Get the upstream sources.
git clone --depth 1 --branch v4.3.5 https://github.com/tailwindlabs/tailwindcss.git /path/to/tw
cd /path/to/tw
npm install -g pnpm
pnpm install --frozen-lockfile --ignore-scripts

# 2. Make all the files. The second argument is an empty scratch directory.
cd /path/to/this/repository
tests/upstream/tools/make-all.sh /path/to/tw /path/to/scratch
```

The Rust crates of the upstream repository are not necessary. No build step is necessary.

The script `make-all.sh` does these steps:

```sh
# suite.jsonl.gz
node tests/upstream/tools/instrument.mjs /path/to/tw
(cd /path/to/tw && TW_ORACLE_DIR=/path/to/scratch/events pnpm vitest run --project tailwindcss)
node tests/upstream/tools/merge-suite.mjs /path/to/scratch/events tests/upstream/suite.jsonl.gz

# classes.jsonl.gz and classes-all.json.gz (approximately 4 minutes)
node tests/upstream/tools/gen-classes.mjs /path/to/tw tests/upstream

# full.json.gz and css/
node tests/upstream/tools/gen-full.mjs /path/to/tw tests/upstream

# cli/*/stdout.css and cli/*/exit.txt, and the same for cli-map with files/
node tests/upstream/tools/run-cli.mjs /path/to/scratch/cli tests/upstream/cli 4.3.5
node tests/upstream/tools/run-cli.mjs /path/to/scratch/cli tests/upstream/cli-map 4.3.5

# source-maps.jsonl.gz (it reads suite.jsonl.gz, suite-extra.jsonl.gz, full.json.gz and classes.jsonl.gz)
node tests/upstream/tools/gen-source-maps.mjs /path/to/tw tests/upstream

# Read each file and print the number of records.
python3 tests/upstream/tools/check.py tests/upstream
```

## Tools

| File | Function |
| --- | --- |
| `tools/instrument.mjs` | Adds the recorder to `packages/tailwindcss/src/index.ts` of the clone. It wraps the exported `compile` function. |
| `tools/recorder.ts` | The recorder. Each vitest worker appends events to `events-<pid>.jsonl` in the directory `TW_ORACLE_DIR`. |
| `tools/merge-suite.mjs` | Joins the event files into `suite.jsonl.gz`. |
| `tools/lib.mjs` | Shared helpers: loads the upstream TypeScript sources with jiti, the stylesheet loader, the random number generator. |
| `tools/candidate-lists.mjs` | The hand-made lists: arbitrary forms, variants for the random stacks, modifiers, edge candidates. |
| `tools/gen-classes.mjs` | Makes `classes.jsonl.gz` and `classes-all.json.gz`. |
| `tools/gen-full.mjs` | Makes `full.json.gz` and `css/`. |
| `tools/run-cli.mjs` | Installs the npm packages into the scratch directory, copies each project there, runs the CLI. For `cli-map`, it also records the files that the CLI made. |
| `tools/gen-source-maps.mjs` | Makes `source-maps.jsonl.gz`: `node tests/upstream/tools/gen-source-maps.mjs /path/to/tw tests/upstream`. |
| `tools/gen-extra.mjs` | Makes `suite-extra.jsonl.gz`: `node tests/upstream/tools/gen-extra.mjs /path/to/tw tests/upstream`. |
| `tools/gen-default-theme.mjs` | Makes `native/tailwind/src/compat/default_theme.rs` from `compat/default-theme.ts`: `node tests/upstream/tools/gen-default-theme.mjs /path/to/tw native/tailwind/src/compat/default_theme.rs`. |
| `tools/check.py` | Reads each file and prints the number of records. |
| `tools/make-all.sh` | Runs all the steps. |

The generator scripts set `NODE_ENV=test`, the same as the upstream unit tests.

### `nodes.jsonl.gz`

One JSON line for each candidate of `classes.jsonl.gz`: `{"candidate", "kind", "nodes"}`.
`nodes` has the rules of the candidate before the last pass of the compiler (`compileAstNodes`, then `toCss` of each rule):
`css`, and `order` and `count` of the property sort. `tools/gen-nodes.mjs` makes the file.

### Fixtures in `native/tailwind/tests/fixtures`

`tools/gen-css-utility.mjs`, `tools/gen-variants.mjs` and `tools/gen-optimize.mjs` make `css_utility.json`, `variants.json` and `optimize.json`.
Each script takes the path of the upstream clone as its first argument.
