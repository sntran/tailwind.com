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
| The official command line program on 20 projects (also `--minify` and `--optimize`, and 3 projects with warnings) | 20 of 20 the same output, exit status and warning lines, with the native program, with `tailwind.com` and with `tailwind.wasm` |
| The source maps of 317 stylesheets (331 builds): the decoded map of `buildSourceMap()`, the JSON text of `@tailwindcss/node`, and the code and the map of the optimizer, plain and minified | 317 of 317 the same |
| The source locations of the parser for these stylesheets, and the position in the message of 23 syntax errors | the same |
| The official command line program with `--map` on 29 projects (the map in the output and in a file, `--minify`, `--optimize`, imports, stdin, a JavaScript plugin and a JavaScript config, errors): the output and the map file | 29 of 29 the same, with the native program, with `tailwind.com` and with `tailwind.wasm`, on Linux. The test replaces the path of the project in the maps. |
| The official command line program on 100 projects with JavaScript plugins and configs: the plugin API, config files, TypeScript configs and plugins (5 projects), `@tailwindcss/typography`, `@tailwindcss/forms`, `@tailwindcss/aspect-ratio` (from `node_modules`, and the copies in the program with no `node_modules`), `tailwindcss-animate`, `@tailwindcss/container-queries`, daisyUI, a Phoenix 1.8.5 project and a Phoenix 1.8.9 project, 19 errors | 100 of 100 the same output and exit status, with the native program, with `tailwind.com` and with `tailwind.wasm`. 17 of the 19 error messages are the same, line for line: the two other ones are syntax errors of the JavaScript parser. The warnings of 2 projects are the same. |
| The parts of Node.js that a module gets (`Buffer`, `URL`, `URLSearchParams`, `TextEncoder`, `TextDecoder`, the timers, `structuredClone`, `process`, `os`, `url`, `util`, `assert`, `fs`, `module`), against Node.js 22 | 124 of 124 values the same |
| The file that `require` and `import` find, against `require.resolve` and `import.meta.resolve` of Node.js 22: `exports` with conditions in more than one sequence, patterns, `imports` (`#name`) | 160 of 160 the same |
| The warnings of the optimizer of the official program (the exact text with its color codes), plain and minified | 26 of 26 the same |
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
- **The warnings before an error**: the warning for a data type that
  `--value()` of `@utility` cannot have goes to the standard error
  after the stylesheet compiles. When the same stylesheet has an error,
  the program prints the error only. The official program prints the
  warning first.
- **The warnings of Node.js itself** (a line that starts with
  `(node:PID)`, for example for a deprecated function) have another
  form: `util.deprecate` prints `[CODE] DeprecationWarning: MESSAGE`.
- **`@source inline()` with an endless range** is an error. The official
  compiler runs until it has no more memory.
- **The help text** has the same options, in another layout.
- **Text that is not valid Unicode** cannot be an input. JavaScript
  strings can hold a lone surrogate.

## JavaScript modules

`@plugin` and `@config` load JavaScript modules. The official program
loads them with Node.js (the npm package) or Bun (the standalone
program). This program loads them with its own loader. The differences:

- **TypeScript modules** (`.ts`, `.mts`, `.cts`) lose their types and
  then load as JavaScript. The official program does this with Node.js
  or Bun, and then with jiti. Here the TypeScript transform of Sucrase
  does it, with one more step for `namespace`. What is different:
  - There is no type check (the same as upstream), and each file is
    one unit: a `const enum` of another file is not replaced by its
    value. Export the enum as a value.
  - An exported name of a `namespace` goes to the object of the
    namespace at the end of the namespace body. A later change of an
    exported `let` is not in the object. A namespace with a dotted name
    (`namespace A.B`) is removed, as a type.
  - Decorators are a syntax error (the engine has no decorators).
  - A file with JSX (`.tsx`) is the error `Cannot load '…': a
    TypeScript module with JSX (.tsx) is not supported. Use a .ts
    file.`
  - A `.ts` file is CommonJS, unless it has the syntax of an ES module.
    A file with `import` and with `require(…)` or `module.exports` in
    it works with jiti, and not here.
  - The message of a syntax error is the message of Sucrase with the
    path, the line and the column. The official program gives the
    message of its own parser.
- **`@tailwindcss/forms`, `@tailwindcss/typography` and
  `@tailwindcss/aspect-ratio`** are in the program, at the versions of
  the standalone program of Tailwind CSS. The rule is the rule of that
  program (`__tw_resolve` and `__tw_load` of `@tailwindcss/standalone`):
  for these three names in `@plugin`, the copy of the program loads,
  also when the project has the package in `node_modules`. The npm
  package of Tailwind CSS has no such copies: it loads the package of
  the project. `require("@tailwindcss/typography")` in a config file
  finds the package in `node_modules` only, in all three programs.
- **The module `tailwindcss`** (`import { compile } from "tailwindcss"`
  for a value) is not there. The standalone program has it. A type
  import (`import type { Config } from "tailwindcss"`) has no
  JavaScript, and works.
- **Node.js modules.** These modules are there, also with the `node:`
  prefix: `assert`, `buffer`, `fs` and `fs/promises` (reads only:
  `readFile`, `readdir` with `withFileTypes` and `recursive`, `exists`,
  `stat`, `lstat`, `realpath`, `access`, each one as a `Sync` function,
  with a callback and with a promise), `module` (`createRequire`,
  `isBuiltin`, `builtinModules`), `os` (`EOL`, `platform`, `homedir`,
  `tmpdir`, `type`, `arch`), `path` (POSIX paths), `process`, `timers`,
  `url` (`fileURLToPath`, `pathToFileURL`, `URL`, `URLSearchParams`)
  and `util` (`inspect`, `format`, `promisify`, `callbackify`,
  `deprecate`, `inherits`, `isDeepStrictEqual`, `types`,
  `TextEncoder`, `TextDecoder`). The modules for a process
  (`child_process`, `cluster`, `worker_threads`) and for the network
  (`dgram`, `dns`, `http`, `http2`, `https`, `net`, `tls`) load, and
  each of their functions throws the error of its rule (see
  `SECURITY.md`). `require` of another module of Node.js (`crypto`,
  `events`, `stream`, `zlib`, ...) is the error `Cannot find module`.
- **Globals.** `console`, `process` (`env`, `cwd()`, `platform`,
  `arch`, `argv`, `versions`, `nextTick`, `stdout.write`), `global`,
  `Buffer`, `URL`, `URLSearchParams`, `TextEncoder`, `TextDecoder`,
  `setTimeout`, `setInterval`, `setImmediate`, `queueMicrotask`,
  `structuredClone`, `atob` and `btoa` are there. `fetch` is a function
  that throws the error of the network rule. `Intl`, `Temporal`,
  `crypto`, `AbortController`, `Blob` and streams are not there.
- **What the parts of Node.js do not have.** `tests/node_api.rs`
  compares 124 values with Node.js 22. Outside these cases:
  - `Buffer` has the text encodings (`utf8`, `base64`, `base64url`,
    `hex`, `latin1`, `ascii`, `utf16le`), and no functions for numbers
    (`readUInt32LE`, ...) except `readUInt8`.
  - `URL` has no IDNA and no IPv4 number forms for a host: a host is
    lower case text. `TextDecoder` knows UTF-8, UTF-16LE and Latin-1.
  - `util.inspect` has the default options of Node.js and the option
    `depth` only. The text of an error is its name, its message and the
    stack of QuickJS. `%o` of `util.format` is `inspect` with a depth
    of 4.
  - A generated message of `assert` has no diff of the two values.
  - `fs.readdirSync` with `withFileTypes` and `fs.statSync` give the
    kind of the target of a symbolic link. `fs.lstatSync` knows a link.
    A mode is `0o644` for a file and `0o755` for a directory.
  - `process.versions.node` is `22.0.0`: the version of Node.js whose
    API the modules follow. `process.pid` is 1.
- **Timers** have no clock. `setTimeout(fn, ms)` runs `fn` when no job
  of a promise is pending: in the sequence of the times, and then of
  the calls, with no real delay. A timer that no module waits for runs
  before the end of `compile`. An interval stops after 10,000 runs.
  `Date.now()` is the real time: it does not move with the timers.
- **`process.env`** is the environment of the program. In the WASI
  program it is the environment that the host gives.
- **`process.platform`** is the system that runs the file (`linux`,
  `darwin`, `win32`, `freebsd`, `openbsd`, `netbsd`), and `wasi` in the
  WASI program. `path` is the POSIX form on each system: on Windows,
  `tailwind.com` uses paths with `/`.
- **`console.log`** writes to the standard output, and `console.warn`
  and `console.error` to the standard error, as Node.js. So a line that
  a plugin prints comes before the CSS when the output is the standard
  output (daisyUI prints one line). The values have the format of
  `util.format` of Node.js.
- **The default export of a CommonJS bundle.** For a CommonJS module
  with the exports `__esModule` and `default`, the plugin is the
  `default`, as in the standalone program (Bun). The npm package on
  Node.js gives the full exports object, and such a plugin then fails
  there. The daisyUI files of Phoenix 1.8 need this rule.
- **The format of a `.js` file** with no `type` in its `package.json`
  is CommonJS, unless the file has a syntax error as CommonJS: then it
  is an ES module. Node.js looks for the syntax of an ES module. The
  result is the same for a valid file.
- **The resolver** also tries the extensions `.cjs`, `.mjs`, `.ts`,
  `.mts` and `.cts`, uses `module` of a `package.json` when there is no
  `main`, and gives `file.ts` for `./file.js` when there is no
  `file.js` (the rule of jiti). For `exports` and `imports` it has the
  conditions of Node.js 22: `node`, `module-sync`, `node-addons`,
  `default`, and `require` or `import`. The standalone program (Bun)
  also has the condition `bun`.
- **A module with a top-level `await`** works when the promise needs
  no input or output: the timers run (see above).
- **Error messages.** The message of an exception is the same. A
  syntax error has the message of QuickJS and the path of the file,
  and not the message of the parser of Node.js or jiti. The `cause`
  chain of an error is in `Caused by:` boxes, as in the official
  program.
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
(`tests/upstream/tools/build-compat-bundle.mjs`), and the three plugins
of the standalone program at the versions of its lock file
(`tests/upstream/tools/build-plugin-bundles.mjs`). When `compat/` of
Tailwind CSS uses a new method of the theme, the utilities or the
variants, add it to `native/tailwind/js/design-system.ts` and to the
host operations of `src/compat/js/runtime.rs`.
