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
- **Source maps**: `--map` is an error.
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
   of Tailwind CSS. The plugin layer is not a port: make
   `native/tailwind/assets/compat.js` again
   (`tests/upstream/tools/build-compat-bundle.mjs`). When `compat/` of
   Tailwind CSS uses a new method of the theme, the utilities or the
   variants, add it to `native/tailwind/js/design-system.ts` and to the
   host operations of `src/compat/js/runtime.rs`.
4. Update the stylesheets of `native/tailwind/assets` from the npm
   package, the tag of `tailwindcss-oxide` and the version of
   `lightningcss` in `native/tailwind/Cargo.toml`, and `VERSION` in
   `index.rs`.
