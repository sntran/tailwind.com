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
  A record with `modules` > 0 uses a JavaScript plugin or config. The module is a JavaScript value of the test file,
  so this file does not have it: `plugin-suite.jsonl.gz` has these records with the modules as source text.
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

### `plugin-suite.jsonl.gz`

The records of `suite.jsonl.gz` that load a JavaScript plugin or config, from `tools/gen-plugin-suite.mjs`.
The file has 296 lines: 306 compile calls of the unit tests load a module, and 10 of them are not in the file (see below).

The script runs the unit tests of `packages/tailwindcss` with the recorder, as for `suite.jsonl.gz`.
The recorder writes each module that a test gives to `loadModule` as JavaScript source text (`tools/recorder.ts`, with `TW_ORACLE_MODULES=1`):

- A function is its own source text (`toString()`). Objects and arrays are written item by item.
- `plugin(handler, config)` gives a plain object, so it needs no special work.
  For `plugin.withOptions(...)`, the script makes `plugin.ts` of the clone keep the two arguments, and the text is a call of `plugin.withOptions` with them.
- Vitest changes the names of imports in the code of a test (`__vite_ssr_import_0__.default`). The recorder puts the names of the test file back.
- The text starts with the names that the modules of the tests use: `plugin`, `css`, `html`, an `expect` and a `vi.fn` that do nothing,
  and the default imports of `plugin`, `colors`, `default-theme` and `flatten-color-palette` with `require('tailwindcss/...')`.

A function that uses a variable of its test is not complete as text.
For this reason, the script runs each record again with the upstream compiler and the modules from their text, and keeps a record only when
the error, the features, the sources and the CSS of each build are the same.
For v4.3.5, 10 records fail this check: 9 use the local function `fn` of a group of tests in `plugin-api.test.ts`, and 1 uses a local variable `didCallPluginFn`.

Fields: as `suite.jsonl.gz`, except:

- `modules`: each `loadModule` call in order: `id`, `base`, `hint`, `source` (the CommonJS source text), `resultBase` (the `base` of the result),
  or `error` when the function of the test throws.
- `builds`: `candidates`, and `css` or `error`.

The Rust test `native/tailwind/tests/plugin_suite.rs` gives each `source` to the compiler as a file in memory.

### `candidates.jsonl.gz`

From `tools/gen-candidates.mjs`: `parseCandidate` of the default design system as JSON, for 10,983 candidates of `classes.jsonl.gz`
(each 25th candidate, and more of the candidates with variants or modifiers). Each line has `candidate` and `parsed` (a list: one item for each reading).
A JavaScript plugin gets a candidate in this shape. The Rust test `tests/js_plugins.rs` compares its JSON with this file.

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
in `stdout.css` and in the files, also in the JSON text of a map that is in a comment as Base64 text.
Lightning CSS gives the paths without the first slash: there the record has `PROJECT`.
`tests/cli.sh` does the same with the output of the program before the comparison.
In `stderr.txt`, the text `<project>` replaces the path of the project, as in `cli-js/`.

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
| `plugin-file`, `plugin-file-minify` | `@plugin` with a file: `addBase`, `addComponents`, `addUtilities` (also `@keyframes`), `matchUtilities`, and `addVariant` with two selectors. The map is a file. Plain and `--minify`. |
| `config-file`, `config-file-minify` | `@config` with a file: theme values, keyframes, and a plugin function in `plugins`. The map is a file. Plain and `--minify`. |
| `plugin-config-inline`, `plugin-config-inline-minify` | `@plugin` and `@config` in one stylesheet, in the two sequences. The map is in the output. Plain and `--minify`. |
| `error-map-dash`, `error-map-dash-value` | `--map=-` and `--map -`: the exit code is 1. |
| `error-syntax-stdout`, `error-syntax-with-output`, `error-syntax-in-import` | A CSS syntax error: the message has the path, the line and the column. |

The cases use no stylesheet of Tailwind CSS: the official CLI gives the path in `node_modules` for it,
and the Rust program has these stylesheets in the program.
The six cases with `@plugin` or `@config` need a program with JavaScript.

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
### `cli-js/`

Cases for the official CLI with JavaScript plugins and JavaScript configs (`@plugin` and `@config`).
The layout of a case is the same as in `cli/`: `project/`, `args.txt`, `stdout.css`, `exit.txt` and, for an error, `stderr.txt`.
There are 88 cases. Each project is small. A project does not use files outside its `project/` directory.
The only exception is the module `tailwindcss` (`tailwindcss/plugin`, `tailwindcss/colors`, `tailwindcss/defaultTheme`):
the program that runs the case gives these modules.

The plugin files and the config files that are part of this repository use only these modules:
`tailwindcss/plugin`, `tailwindcss/colors`, `tailwindcss/defaultTheme`, and relative files (JavaScript files and one JSON file).
The third-party files are not changed. They also use `fs` (`readdirSync`, `readFileSync`), `path` (`join`, `basename`), `__dirname`,
`util` (`util.deprecate`), `console.log`, `atob`, `encodeURIComponent` and `decodeURIComponent`.

Most cases start with these lines, thus the output has no preflight and the scanner reads only `index.html`:

```css
@layer theme, base, components, utilities;
@import "tailwindcss/theme.css" layer(theme);
@import "tailwindcss/utilities.css" layer(utilities) source(none);
@source "./index.html";
```

In `stderr.txt`, the text `<project>` replaces the absolute path of the project directory.
The message of `error-plugin-syntax` comes from the parser of the official program (jiti). Another JavaScript engine gives a different text.

A case with a file `standalone.txt` uses the module interop of the standalone program (`tools/bun-interop.mjs`).
The standalone program runs on Bun. Phoenix uses the standalone program.
For a CommonJS module with the exports `__esModule` and `default`, Bun gives the `default` export as the plugin. Node.js gives the full exports object.
Without the interop, the npm CLI stops with the message `The plugin "../vendor/daisyui" does not accept options` for the Phoenix project.
The record of these cases comes from the npm CLI with the interop hook, and not from the standalone program file.

Plugin API cases (`@plugin` with a local file):

| Case | Content |
| --- | --- |
| `plugin-add-base` | `addBase` with an object and with an array: nested rules, an at-rule, `theme()` values. |
| `plugin-add-base-preflight` | `addBase` together with the preflight and a `@layer base` block of the CSS file. `@font-face`. |
| `plugin-add-utilities-object` | `addUtilities` with an object: camelCase properties, a selector list, an array of values, variants, `!`. |
| `plugin-add-utilities-array` | `addUtilities` with an array. The same class occurs two times, and has the name of a core utility. |
| `plugin-add-utilities-nested` | `addUtilities` with nested selectors (`&:hover`, `> svg`, `.dark &`) and with compound selectors. |
| `plugin-add-utilities-at-rules` | `addUtilities` with `@media`, `@supports`, `@container`, `@layer` and `@keyframes` in a class. |
| `plugin-add-utilities-apply` | `@apply` in `addUtilities` and in `addComponents`, and `@apply` of a plugin utility in the CSS file. |
| `plugin-add-utilities-prefix` | `prefix(tw)` on the imports: `addUtilities`, `addComponents`, `matchUtilities`, and the `prefix()` function. |
| `plugin-match-utilities-values` | `matchUtilities` with `values`, a `DEFAULT` value, arbitrary values, `theme('spacing')` values, and no values. |
| `plugin-match-utilities-multiple-rules` | `matchUtilities` functions that return an array of rules, nested rules, or an empty object. |
| `plugin-match-utilities-types` | `matchUtilities` with `type`: `color`, `length`, `any`, a list of types, and two plugins with the same root. |
| `plugin-match-utilities-modifiers` | `matchUtilities` with `modifiers` (an object and `'any'`), and the opacity modifier of a color utility. |
| `plugin-match-utilities-negative` | `matchUtilities` with `supportsNegativeValues`. |
| `plugin-add-components` | `addComponents` with an object and an array. The sort order with a core utility. |
| `plugin-match-components` | `matchComponents` with values, object values, a color type and modifiers. |
| `plugin-add-variant-string` | `addVariant` with a string, an array of strings, at-rules, and compound variants (`group-`, `peer-`, `not-`, `has-`, `in-`). |
| `plugin-add-variant-object` | `addVariant` with a CSS-in-JS object and `@slot`. |
| `plugin-match-variant` | `matchVariant` with `values`, `DEFAULT`, arbitrary values, `sort`, modifiers, and an array result. |
| `plugin-theme-function` | `theme()` in a plugin: dot paths, bracket paths, a default value, opacity modifiers, tuples, and CSS theme variables. |
| `plugin-config-function` | `config()` in a plugin, with a `@config` file. |
| `plugin-esm` | ES modules: `.mjs` files, ESM syntax in a `.js` file, a relative `import`, a plain function as the default export. |
| `plugin-plain-function` | Plain functions with no `plugin()` wrapper (`.js` and `.cjs`), `require` of a local file, and the object form (`handler` and `config`). |
| `plugin-with-options` | `plugin.withOptions` with `@plugin "./x.js" { ... }` options. One plugin file loads two times. A plugin with no options. |
| `plugin-options-types` | The types of the `@plugin` option values: string, quoted string, number, boolean, `null`, lists, functions. |
| `plugin-with-config` | The second argument of `plugin()`: a config with `theme` and `theme.extend`. |
| `plugin-order` | Two plugins and CSS rules in one file: the order of base, components, utilities and variants in the output. |
| `plugin-cjs-esmodule` | A CommonJS file with the exports `__esModule` and `default`. Uses `standalone.txt`. |

Config cases (`@config`):

| Case | Content |
| --- | --- |
| `config-extend-colors` | `theme.extend.colors`: nested objects, `DEFAULT`, `<alpha-value>`, an override of one default color. |
| `config-theme-functions` | Theme values as functions: `({ theme, colors }) => ...`. |
| `config-esm` | A `tailwind.config.mjs` file with `import`: colors, spacing, screens, fontFamily, a plugin in the config. |
| `config-esm-js` | ESM syntax in `tailwind.config.js`. |
| `config-extend-fonts` | `fontFamily` (list, string, with font feature settings), `fontSize` (string, tuples with `lineHeight`), `fontWeight`, `lineHeight`, `letterSpacing`. |
| `config-extend-animation` | `keyframes` and `animation`, and the transition scales. |
| `config-extend-radius-shadow` | `borderRadius`, `boxShadow`, `dropShadow`, `borderWidth`, `blur`, `aspectRatio`. |
| `config-theme-override` | `theme` keys that replace the defaults (colors, spacing, screens, fontSize, borderRadius), together with `extend`. |
| `config-dark-selector`, `config-dark-class`, `config-dark-media` | `darkMode` as a string. |
| `config-dark-class-custom`, `config-dark-selector-custom` | `darkMode` as `['class', ...]` and `['selector', ...]`. |
| `config-dark-variant`, `config-dark-variant-list` | `darkMode` as `['variant', ...]` with one selector and with a list. |
| `config-dark-css-wins` | `darkMode` in the config and a `@custom-variant dark` in the CSS file. |
| `config-prefix`, `config-prefix-legacy` | `prefix` in the config. The legacy form `tw-` gives a warning on stderr and the exit code 0. |
| `config-important-true` | `important: true`, with plugins and `@utility`. |
| `config-important-selector` | `important: '#app'` with `source(none)` on the import of the utilities. The selector is not in the output. |
| `config-important-selector-wrap` | `important: '#app'` with no `source(none)`. The selector is in the output. |
| `config-plugins` | `plugins: [...]` with `require` of local files, a directory, a JSON file, `withOptions` with and without a call, and inline plugins. |
| `config-presets` | `presets`, also a preset in a preset. The order of the merge. |
| `config-content` | `content` with files, globs and a negated glob. The config file is in a subdirectory. The paths are relative to the config file. |
| `config-content-object` | `content` as an object with `relative` and `files`, with automatic source detection and a `.gitignore`. |
| `config-blocklist` | `blocklist`. |
| `config-container`, `config-container-simple` | `theme.container`: `center`, `padding` (string and object), `screens`. |
| `config-screens` | `theme.screens`: strings, `min`, `max`, `raw`, a list. |
| `config-supports-aria-data` | `theme.extend.supports`, `aria`, `data` and `containers`. |
| `config-ignored-options` | `corePlugins`, `separator`, `safelist`, `future`, `experimental`, `variants`: version 4 ignores them. |
| `config-theme-function-css` | The legacy `theme('colors.blue.500')` function in CSS with values from the config. |
| `config-and-plugin` | `@theme`, `@config` and `@plugin` give the same theme keys. |
| `config-two-configs` | Two `@config` rules in one file. |

Error cases (the exit code is 1, stdout is empty):

| Case | Content |
| --- | --- |
| `error-plugin-throws`, `error-plugin-throws-on-load`, `error-plugin-syntax` | A plugin function throws. A module throws when it loads. A file has a syntax error. |
| `error-plugin-missing`, `error-plugin-missing-package`, `error-plugin-no-path` | A missing file, a missing package, `@plugin` with no path. |
| `error-plugin-nested` | `@plugin` in a rule. |
| `error-plugin-options-object`, `error-plugin-options-object-value`, `error-plugin-options-no-function` | Invalid `@plugin` options, and options for a plugin that does not accept options. |
| `error-plugin-utility-selector`, `error-plugin-match-utilities-name` | Invalid names in `addUtilities` and `matchUtilities`. |
| `error-config-body`, `error-config-nested`, `error-config-missing` | `@config` with a body, `@config` in an at-rule, a missing config file. |
| `error-config-throws` | A config file with a `require` of a missing file. |
| `error-config-content-raw` | `content` with a `raw` entry. |
| `error-config-apply-unknown` | `@apply` of an unknown utility in a plugin of the config. |

Third-party plugins. Each project has its own `node_modules` directory, as `npm install` makes it:

| Case | Content |
| --- | --- |
| `typography` | `@plugin "@tailwindcss/typography"`: `prose prose-lg prose-slate dark:prose-invert`, element modifiers. |
| `typography-config` | The typography plugin in `plugins` of a config, with the `className` option and `theme.extend.typography`. |
| `forms`, `forms-class` | `@plugin "@tailwindcss/forms"`, and with the option `strategy: "class"`. |
| `animate` | `@plugin "tailwindcss-animate"`. |
| `container-queries` | `@plugin "@tailwindcss/container-queries"`. It loads on version 4. |
| `daisyui-bundle` | The one-file bundle of daisyUI with the `themes` option, `btn` and `card` classes. Uses `standalone.txt`. |
| `phoenix` | A Phoenix 1.8 project: `assets/css/app.css` and the vendor files of `mix phx.new`, heroicons, `.heex` and `.ex` files. Uses `standalone.txt`. |
| `phoenix-minify` | The same project with `--minify`. |

The Phoenix project comes from the installer templates of Phoenix `v1.8.5`.
The values of the template are: the application `my_app`, with LiveView, gettext and daisyUI.
The first line of `stdout.css` of the Phoenix cases is `/*! 🌼 daisyUI 5.0.35 */`. The daisyUI plugin prints it with `console.log`.
Phoenix `v1.8.6` and later versions do not have the daisyUI files in `assets/vendor`. They load daisyUI from the `deps` directory with `NODE_PATH`.

The npm package `daisyui` (version 5) has a size of 4.2 MB. For this reason, there is no case with `@plugin "daisyui"` from `node_modules`.

Versions of the third-party packages (the file `cli-js/NOTICE.md` gives the sources and the licenses):

| Package | Version |
| --- | --- |
| `@tailwindcss/typography` | 0.5.20 (with `postcss-selector-parser` 6.0.10, `cssesc` 3.0.0, `util-deprecate` 1.0.2) |
| `@tailwindcss/forms` | 0.5.11 (with `mini-svg-data-uri` 1.4.4) |
| `tailwindcss-animate` | 1.0.7 |
| `@tailwindcss/container-queries` | 0.1.1 |
| daisyUI bundle (`daisyui.js`, `daisyui-theme.js`) | 5.0.35 |
| heroicons | 2.2.0 |
| Phoenix installer templates | 1.8.5 |

To record the cases again:

```sh
node tests/upstream/tools/run-cli.mjs /path/to/scratch/cli tests/upstream/cli-js 4.3.5
```

To record only some cases, add the case names after the version.
The script does not install the third-party packages. They are in the repository.
To install the packages of one project again, run this command in its `project/` directory.
The `package.json` file of the project has the exact versions.

```sh
npm install --no-package-lock --ignore-scripts --legacy-peer-deps
```

The option `--legacy-peer-deps` keeps the peer package `tailwindcss` out of the project.

After the install, delete the directory `node_modules/.bin` of the project. Its entries are symbolic links,
and a copy of a symbolic link fails on Windows. The fixtures keep no symbolic link:
`find tests/upstream -type l` must print nothing.

Some projects contain a `.gitignore` file, a `node_modules` directory or a `deps` directory.
Use `git add -f tests/upstream/cli-js` to add all the files.

## The plugin layer of the program

`native/tailwind/assets/compat.js` is not a test file, but it comes from the same clone: it is the plugin layer of Tailwind CSS
(`packages/tailwindcss/src/compat`) as one JavaScript file, for the JavaScript engine of the program.

```sh
node tests/upstream/tools/build-compat-bundle.mjs /path/to/tw
```

The script uses esbuild. It takes esbuild from `tests/upstream/tools/node_modules` (run `npm install` in `tests/upstream/tools`),
or from the pnpm store of the clone. The entry and the shims are in `native/tailwind/js`: see `docs/INTERNALS.md`.
The script stops when a module of the compiler that a shim must replace (`design-system.ts`, `compile.ts`, `at-import.ts`,
`property-order.ts`) is in the bundle.

## How to make the files again

```sh
# 1. Get the upstream sources.
git clone --depth 1 --branch v4.3.5 https://github.com/tailwindlabs/tailwindcss.git /path/to/tw
cd /path/to/tw
npm install -g pnpm
pnpm install --frozen-lockfile --ignore-scripts

# 2. Make all the files. The second argument is an empty scratch directory.
#    The script changes files of the clone (index.ts, plugin.ts, __oracle.ts).
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

# cli-js/*/stdout.css and cli-js/*/exit.txt
node tests/upstream/tools/run-cli.mjs /path/to/scratch/cli tests/upstream/cli-js 4.3.5

# plugin-suite.jsonl.gz (the unit tests again; the script changes plugin.ts of the clone)
node tests/upstream/tools/gen-plugin-suite.mjs /path/to/tw /path/to/scratch tests/upstream/plugin-suite.jsonl.gz

# candidates.jsonl.gz
node tests/upstream/tools/gen-candidates.mjs /path/to/tw tests/upstream

# native/tailwind/assets/compat.js
node tests/upstream/tools/build-compat-bundle.mjs /path/to/tw

# Read each file and print the number of records.
python3 tests/upstream/tools/check.py tests/upstream
```

## Tools

| File | Function |
| --- | --- |
| `tools/instrument.mjs` | Adds the recorder to `packages/tailwindcss/src/index.ts` of the clone. It wraps the exported `compile` function. |
| `tools/recorder.ts` | The recorder. Each vitest worker appends events to `events-<pid>.jsonl` in the directory `TW_ORACLE_DIR`. |
| `tools/merge-suite.mjs` | Joins the event files into `suite.jsonl.gz`. |
| `tools/gen-plugin-suite.mjs` | Makes `plugin-suite.jsonl.gz`: runs the unit tests with the recorder and `TW_ORACLE_MODULES=1`, and checks each record with the upstream compiler. |
| `tools/gen-candidates.mjs` | Makes `candidates.jsonl.gz`. |
| `tools/build-compat-bundle.mjs` | Makes `native/tailwind/assets/compat.js` with esbuild (`tools/package.json` names the version). |
| `tools/bun-interop.mjs` | The module interop of the standalone program, for the cases of `cli-js/` with `standalone.txt`. |
| `tools/lib.mjs` | Shared helpers: loads the upstream TypeScript sources with jiti, the stylesheet loader, the random number generator. |
| `tools/candidate-lists.mjs` | The hand-made lists: arbitrary forms, variants for the random stacks, modifiers, edge candidates. |
| `tools/gen-classes.mjs` | Makes `classes.jsonl.gz` and `classes-all.json.gz`. |
| `tools/gen-full.mjs` | Makes `full.json.gz` and `css/`. |
| `tools/run-cli.mjs` | Installs the npm packages into the scratch directory, copies each project there, runs the CLI. It records `cli/`, `cli-js/` and `cli-map/`, also the files that the CLI made. |
| `tools/gen-source-maps.mjs` | Makes `source-maps.jsonl.gz`: `node tests/upstream/tools/gen-source-maps.mjs /path/to/tw tests/upstream`. |
| `tools/bun-interop.mjs` | The module interop of the standalone program, for the `cli-js/` cases with a `standalone.txt` file. |
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
