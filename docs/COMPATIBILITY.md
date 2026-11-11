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
| The `compile` calls that load a JavaScript plugin or config, with each module as source text (output or error message, features, sources) | 296 of 296 the same. 10 more calls have a module that uses a variable of its test, and cannot be made again from its text. |
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
| The official command line program with `--map` on 29 projects (the map in the output and in a file, `--minify`, `--optimize`, imports, stdin, a JavaScript plugin and a JavaScript config, errors): the output and the map file | 29 of 29 the same, with the native program, with `tailwind.com` and with `tailwind.wasm`, on Linux. The test replaces the path of the project in the maps. |
| The official command line program on 88 projects with JavaScript plugins and configs: the plugin API, config files, `@tailwindcss/typography`, `@tailwindcss/forms`, `tailwindcss-animate`, `@tailwindcss/container-queries`, daisyUI, a Phoenix 1.8 project, 18 errors | 88 of 88 the same output and exit status, with the native program, with `tailwind.com` and with `tailwind.wasm`. 17 of the 18 error messages are the same, line for line. |
| The JSON of a candidate, as a plugin function gets it, against `parseCandidate` | 10,983 of 10,983 the same |
| Lightning CSS from Rust against the optimizer of `@tailwindcss/node`, on 322 outputs of the unit tests, plain and minified | 644 of 644 the same |

The scanner is the crate of Tailwind CSS itself, so the classes that it
finds in the source files are the same by construction.

## Known differences

- **JavaScript plugins and configs** run in QuickJS, and not in Node.js
  or Bun. The plugin layer is the JavaScript of Tailwind CSS with no
  change, so the plugin API has no known difference. The differences
  are in the environment of a module: see "JavaScript modules" below.
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
- **The watch** uses inotify, kqueue, or `ReadDirectoryChangesW` on
  Windows. The official program uses `@parcel/watcher`. `--poll` is the
  same.
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

## JavaScript modules

`@plugin` and `@config` load JavaScript modules. The official program
loads them with Node.js (the npm package) or Bun (the standalone
program). This program loads them with its own loader. The differences:

- **TypeScript modules** (`.ts`, `.mts`, `.cts`) are an error: `Cannot
  load '…': TypeScript modules are not supported. Use a JavaScript file
  (.js, .mjs or .cjs).` The official program loads them with jiti.
- **Node.js modules.** `fs` (`readFileSync`, `readdirSync`,
  `existsSync`, `statSync`, `lstatSync`, `realpathSync`), `path` (POSIX
  paths), and `util` (`deprecate`, `inspect`, `format`, `inherits`) are
  there, also with the `node:` prefix. `require` of another module of
  Node.js is the error `Cannot find module`. `fs.readFileSync` with no
  encoding gives a String object with the text of the file, and not a
  `Buffer`: `toString()` and the methods of a string work.
- **Globals.** `console`, `process` (`env`, `cwd()`, `platform`,
  `argv`), `global`, `atob`, `btoa` and `structuredClone` (for plain
  data) are there. `process.env` has `NODE_ENV` only. `Buffer`,
  `setTimeout`, `fetch`, `URL`, `TextEncoder`, `Intl` and `Temporal`
  are not there.
- **`console.log`** writes to the standard output, and `console.warn`
  and `console.error` to the standard error, as Node.js. So a line that
  a plugin prints comes before the CSS when the output is the standard
  output (daisyUI prints one line).
- **The default export of a CommonJS bundle.** For a CommonJS module
  with the exports `__esModule` and `default`, the plugin is the
  `default`, as in the standalone program (Bun). The npm package on
  Node.js gives the full exports object, and such a plugin then fails
  there. The vendor file `daisyui.js` of Phoenix 1.8 needs this rule.
- **The format of a `.js` file** with no `type` in its `package.json`
  is CommonJS, unless the file has a syntax error as CommonJS: then it
  is an ES module. Node.js looks for the syntax of an ES module. The
  result is the same for a valid file.
- **The resolver** also tries the extensions `.cjs` and `.mjs`, and
  uses `module` of a `package.json` when there is no `main`. It does
  not read `imports` (`#name`) of a `package.json`.
- **`import()` in a CommonJS file** resolves a relative path from the
  current directory, and not from the file.
- **A module with a top-level `await`** works when the promise needs
  no timer and no input or output.
- **Error messages.** The message of an exception is the same. A
  syntax error has the message of QuickJS and the path of the file,
  and not the message of the parser of Node.js or jiti. An error has no
  `cause` chain in the output.
- **An exception of a plugin function in a build**: the program prints
  the error and stops, as the official program. In the library,
  `Compiler::try_build` gives the error, and `Compiler::build` gives
  the CSS without the candidate of the failure.
- **A plugin function cannot register** a utility or a variant after
  `compile` (in a function that runs in a build). This is the error `A
  plugin cannot change the design system during a build.` Upstream
  accepts it.
- **The watch** builds again when a module file changes that is not in
  a `node_modules` directory. The official program watches the plugin
  or config file and the relative files that it imports.
- **Limits.** JavaScript has 1 GB of memory and 1 MB of stack (512 KB
  in the WASI program). There is no limit of time: a plugin that does
  not end stops the program, as in the official program.
- **`@tailwindcss/forms`, `@tailwindcss/typography` and
  `@tailwindcss/aspect-ratio`** are in the file of the standalone
  program of Tailwind CSS. They are not in this file: they load from
  `node_modules`.

A build with no JavaScript engine (the library with no feature `js`, or
the WASI program from a build with no wasi-sdk) gives the error
``"`@plugin` needs JavaScript. This build of the compiler has no
JavaScript engine."`` The errors of Tailwind CSS for a wrong `@plugin`
or `@config` rule come first, as upstream. With the engine and no
module loader, the error is the error of upstream: ``No `loadModule`
function provided to `compile` ``.

## What is the same on purpose in the source maps

- Tailwind CSS can put one node object at two places of the tree (a
  custom variant with two selectors, a compound variant on such a
  variant, a `@slot` used two times). The printer then gives the node
  the position of its last place, and the map has two equal mappings
  there and none at the first place. The port gives its copies of such
  a node one identity, and the same result.
- A node that a JavaScript plugin or config makes (`addBase`,
  `addUtilities` with `@keyframes`) has the location of its `@plugin` or
  `@config` rule, as upstream. The tree goes between the compiler and
  JavaScript as JSON: a node has the numbers of its locations there,
  and each declaration that comes from JavaScript has the number of its
  object. So a node that a plugin puts at two places (`addVariant` with
  a list of selectors) is one node for the printer, as upstream.
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

The plugin layer is not a port: the sync makes
`native/tailwind/assets/compat.js` again from the source of Tailwind CSS
(`tests/upstream/tools/build-compat-bundle.mjs`). When `compat/` of
Tailwind CSS uses a new method of the theme, the utilities or the
variants, add it to `native/tailwind/js/design-system.ts` and to the
host operations of `src/compat/js/runtime.rs`.
