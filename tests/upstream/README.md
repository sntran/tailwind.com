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

# cli/*/stdout.css and cli/*/exit.txt
node tests/upstream/tools/run-cli.mjs /path/to/scratch/cli tests/upstream/cli 4.3.5

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
| `tools/run-cli.mjs` | Installs the npm packages into the scratch directory, copies each project there, runs the CLI. |
| `tools/gen-extra.mjs` | Makes `suite-extra.jsonl.gz`: `node tests/upstream/tools/gen-extra.mjs /path/to/tw tests/upstream`. |
| `tools/gen-default-theme.mjs` | Makes `native/tailwind/src/compat/default_theme.rs` from `compat/default-theme.ts`: `node tests/upstream/tools/gen-default-theme.mjs /path/to/tw native/tailwind/src/compat/default_theme.rs`. |
| `tools/check.py` | Reads each file and prints the number of records. |
| `tools/make-all.sh` | Runs all the steps. |

The generator scripts set `NODE_ENV=test`, the same as the upstream unit tests.
