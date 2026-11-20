# Internals

## The parts

```
native/tailwind/        the compiler and the command line program (Rust)
  src/*.rs              the compiler: one file for each file of Tailwind CSS
  src/cli/              the command line program
  src/compat/js/        the JavaScript engine for @plugin and @config (Rust)
  js/                   the entry and the shims of the plugin layer, and the parts of Node.js (TypeScript)
  assets/*.css          the stylesheets of Tailwind CSS, in the program
  assets/compat.js      the plugin layer of Tailwind CSS, in the program (generated)
  assets/typescript.js  the TypeScript transform, in the program (generated)
  assets/plugins/       the three plugins of the standalone program of Tailwind CSS (generated)
  c_src/                the entry in C, the calls of Rust for Cosmopolitan, the file events
  c_src/quickjs/, js.c  QuickJS, and its API for Rust
native/scanner/         the scanner of Tailwind CSS: a copy of its three crates (native/scanner/README.md)
scripts/build.sh        the link of the Rust library into tailwind.com
src/tailwind.erl        Erlang: install and run tailwind.com
lib/                    Elixir: the module Tailwind and the Mix tasks
specs/                  the TLA+ models
tests/upstream/         the recorded output of Tailwind CSS, and its tools
```

## The compiler

Each file of `native/tailwind/src` follows one file of
`packages/tailwindcss/src` of Tailwind CSS 4.3.5:

| Here | Tailwind CSS | What |
|---|---|---|
| `css_parser.rs` | `css-parser.ts` | CSS text to the tree. |
| `ast.rs`, `to_css.rs` | `ast.ts` | The tree, and the tree to CSS text. |
| `optimize.rs` | `ast.ts` (`optimizeAst`, `handleNesting`) | The last pass: unused theme variables, `@property` and `color-mix()` fallbacks, nesting. |
| `walk.rs` | `walk.ts` | The walk of a tree with replacement. |
| `value_parser.rs`, `selector_parser.rs`, `attribute_selector_parser.rs` | the files of the same names | The parsers of values and selectors. |
| `theme.rs` | `theme.ts` | The values of `@theme`. |
| `candidate.rs` | `candidate.ts` | A class name to a candidate: variants, root, value, modifier. |
| `utilities/` | `utilities.ts` | The utilities, in four parts, and `@utility` (`css_utility.rs`). |
| `variants.rs` | `variants.ts` | The variants, `@variant`, `@slot`. |
| `compile.rs`, `property_order.rs` | `compile.ts`, `property-order.ts` | Candidates to rules, and the sort of the rules. |
| `design_system.rs` | `design-system.ts` | The theme, the utilities, the variants, and the caches. |
| `css_functions.rs` | `css-functions.ts` | `theme()`, `--theme()`, `--spacing()`, `--alpha()`. |
| `apply.rs` | `apply.ts` | `@apply`. |
| `at_import.rs` | `at-import.ts` | `@import`. |
| `index.rs` | `index.ts` | `compile`: all the at-rules of Tailwind CSS, and `build`. |
| `source_maps/` | `source-maps/` | The source of a node, offsets to lines and columns, and the decoded source map of a tree. |
| `compat.rs`, `compat/` | `compat/` | The parts of the compatibility layer that need no JavaScript, and the engine for the parts that are JavaScript: see "JavaScript plugins". |
| `sort.rs`, `intellisense.rs` | `sort.ts`, `intellisense.ts` | The order of classes (`getClassOrder`), and the class list and the variants for the editor tools (`getClassList`). |
| `canonicalize/` | `canonicalize-candidates.ts`, `constant-fold-declaration.ts`, `expand-declaration.ts`, `canonicalize-calc-expressions.ts` | The canonical form of candidates, for the command `canonicalize`. |
| `utils/` | `utils/` | Small functions. `utils/js.rs` has the number and string behavior of JavaScript. |

The differences in structure come from the two languages:

- **No node identity.** Tailwind CSS keeps references to nodes and
  changes them later. The Rust tree has one owner, so the code uses
  paths of indices, ids in an arena (`optimize.rs`), or shared cells for
  the `@utility` rules.
- **No captured design system.** A utility or a variant of Tailwind CSS
  is a closure over the theme. Here each function gets `&DesignSystem`
  when it runs. The theme marks its used variables through `&self`, with
  a `RefCell`.
- **JavaScript numbers and strings.** The output has numbers that
  JavaScript prints (`calc(1 / 3 * 100%)`, `0.5` to `50%`), and sorts
  that compare UTF-16 code units. `utils/js.rs` gives the same results.
- **No regular expressions.** Each one is a small matcher by hand.

### The canonical form of candidates

The algorithm of `canonicalize/` is the algorithm of the newest version
of Tailwind CSS, for a design system of each version: it has no
switches for its own history. The design system keeps its version
(`DesignSystem::version`), and `canonicalize_candidates`,
`get_class_order` and `get_class_list` follow it (`version::enter`), so
each signature is the CSS of that version. `docs/VERSIONS.md` has the
reasons and the differences from the official command of the older
versions. Its parts:

- `signatures.rs`: the signature of a utility (its CSS through `@apply`
  in a normal form: no `@theme inline`, the inlined theme values, the
  fold of `calc()`, sorted and expanded declarations) and of a variant.
  An invalid candidate has a signature of its own (a new `Symbol()`
  upstream). The index of the class list by signature (the
  "pre-computed utilities") is the slow part: about 23,000 signatures
  for the default theme, one time for each design system.
- `utilities.rs` and `variants.rs`: the steps of `UTILITY_CANONICALIZATIONS`
  and `VARIANT_CANONICALIZATIONS`, in the same sequence. A generator of
  upstream (`tryReplacements`) is a function with a visitor: the visitor
  stops it at the first replacement that it takes, so the caches see the
  same calls as upstream.
- `collapse.rs`: one utility for more utilities, with the same
  combinations in the same sequence (Gosper's hack) and the order of
  `Set.prototype.intersection` of V8.

The caches are on the design system (`Storage`), as the `storage` of
upstream: a later call of the same design system sees the caches of the
earlier calls, so the tests replay the calls in their sequence. The
order of the sets and of the maps of JavaScript is the order of
insertion here too (`IndexSet`, `IndexMap`). The class list needs the
suggestions of each utility (`utilities/suggestions.rs`, and the
functions that make utilities), of `@utility` (`css_utility.rs`), and of
the plugins: `designSystem.utilities.suggest` and
`designSystem.variants.suggest` of the plugin layer keep their function
in JavaScript under a number, and Rust calls it only for the class list.

An exception of a plugin in a compile of upstream gives no result in the
cache of the compiled candidates, so the next compile throws again. The
cache of `compile_ast_nodes` does the same: it keeps no result of a
compile that had an exception (`js_error_count`).

The memory. The index and the collapse compile and parse about 200,000
candidates, and upstream keeps each one in its caches (about 600 MB).
Here `DesignSystem::transient_compiles` removes the entries of the
compiled cache and of the parse cache that cannot change a result, at
the end of each signature: an entry whose compile read no `inline`
option of the theme (`Theme::inline_reads`), in a design system with no
plugins. Such a compile gives the same nodes the next time. The other
entries stay, because a compile with `without_inline` and one without
it give different nodes, and upstream uses the entry that is in the
cache. Each entry of the compiled cache holds its candidate, so no other
candidate gets its address while the entry lives. A run of 400 lists
takes about 135 MB (`tests/canonicalize_memory.rs`).

The suggestions are data (`utilities/suggestions.rs`): a table of
groups, and one function that reads them for the class list. The
feature `canonicalize` has the canonicalizer, the class list and the
suggestions; `tailwind-browser.wasm` does not have it.

The compiler is single-threaded. Without the feature `js`, it has no
dependency except `indexmap`. The feature `js` adds `serde_json` and the
C code of QuickJS.

## JavaScript plugins

`@plugin` and `@config` load JavaScript modules. Tailwind CSS has a
plugin layer for them (`compat/` of its source: the plugin API, the
merge of configs, the default theme of v3, `darkMode`, `screens`,
`container`). This layer is 4,000 lines of JavaScript that change with
each release, and plugins depend on its exact behavior. So the program
does not port it: it runs the JavaScript of Tailwind CSS itself, on top
of the Rust compiler.

There are two paths (`compat.rs`):

- A stylesheet with no `@plugin` and no `@config` uses the Rust code
  only (`legacy_utilities.rs`, `theme_fn.rs`). The program makes no
  engine: the start is fast.
- A stylesheet with one of these rules makes one engine for its design
  system, and `applyCompatibilityHooks` of Tailwind CSS does all the
  work of the layer, also `theme(…)` with a key path. A test
  (`tests/js_plugins.rs`) checks that the two paths give the same
  output where they do the same work.

### The engine

`native/tailwind/c_src/quickjs` is QuickJS (Fabrice Bellard) as a git submodule, with no
change: see `native/tailwind/c_src/QUICKJS.md` for the version and the files. The build has no
`quickjs-libc`, so JavaScript has no `std`, no `os`, no files and no
network.

`native/tailwind/c_src/js.c` is a small API over QuickJS (`js.h`). Only UTF-8 text goes
across it, so the Rust code needs no QuickJS types and no generated
bindings:

| Function | What |
|---|---|
| `tw_js_new`, `tw_js_free` | An engine: one runtime and one context, with a memory limit and a stack limit. |
| `tw_js_eval` | Evaluate a script or an ES module from text. |
| `tw_js_eval_bytecode` | Evaluate a script from its bytecode: the plugin layer of the version of the port (see "The bytecode of the plugin layer"). |
| `tw_js_call` | Call a global function with one string, and get a string or nothing. |
| `tw_js_run_jobs` | Run the pending jobs of promises until none is left. |
| `tw_js_platform` | `process.platform` for the system that runs the file. `tailwind.com` is one file for all the systems, so Cosmopolitan gives the answer at run time. |

JavaScript gets three global functions from `js.c`: `__host(op, text)`
calls a function of Rust and returns its text, `__await(promise)` runs
the pending jobs until the promise settles (the modules load
synchronously), and `__compile(source, filename)` evaluates a script
that has the name of its file. A CommonJS module is such a script: the
stack of an error has the file, and an `import()` in the module
resolves from the file. When no job is pending, the job loop calls the
function `__tw_run_timer` of the bundle: this is the clock of
`setTimeout` (`js/timers.ts`). The loader of ES modules of QuickJS asks
the host for the name of a module (`module.resolve`) and for its source
(`module.load`), and gives the module `import.meta.url`,
`import.meta.dirname` and `import.meta.filename`.

The process keeps the bytecode of each script and each ES module that it
compiles (`compile_text` of `js.c`): `tw_js_eval`, `__compile` and the
loader of ES modules use it. A new engine compiles the same texts again:
the plugin layer of an older version, and the plugins, in each full build
of the watch and in each run of the NIF. An entry is used only for the
same name and the same text, byte for byte (the cache has a copy of the
text), so a change of a file compiles it again. The cache has 32
entries, and a new text of a name takes the place of the old one. A
module from the cache resolves its imports, as the compile of its text
does. The bytecode has the source and the debug information: a function
from the cache has the same `toString` and the same stack as a function
from the text.

An exception comes back as its message, its name and
its stack. Each side frees its own text: text from C is from `malloc`
and Rust gives it back with `tw_js_free_text`, and C gives the text of a
host result back to Rust.

### The engine of the host

With the feature `host-js` (for `tailwind-browser.wasm`), the program
has no QuickJS: the functions `tw_js_new`, `tw_js_free`, `tw_js_eval` and
`tw_js_call` are imports of the WebAssembly module `tailwind_js`, and
`js/engine.mjs` of the npm package gives them with the JavaScript engine
of the host: a browser (`js/browser.mjs`) or Node.js (`js/node.mjs`). The program exports `tw_host_alloc` and `tw_host_free`
for the text that the host gives, and `tw_host_call` and
`tw_host_release` for `__host`.

- Each engine is a realm: a hidden `<iframe>` of the page, or a context
  of `node:vm` in Node.js. The plugin layer changes the globals of its
  realm, not of the page or the program. The program ends with
  `proc_exit` and does not free its engines, so `runWasm` removes the
  realms of a run at its end.
- The JavaScript files of the program are not in the program: the npm
  package has them in `runtime/` (`compat.js`, `typescript.js`,
  `plugins/`). `js/browser.mjs` downloads the ones that a build needs,
  while the program compiles, and `js/node.mjs` reads each one when the
  program asks.
  The program evaluates an empty `tailwindcss/compat.js`, and the host
  evaluates its copy. `__host('asset', name)` gets the other files from
  the host.
- A browser has no synchronous wait for a promise. The plugin layer has
  a second function, `__tw_compat_async`: the same as `__tw_compat`, with
  `await` in place of `__await`, and `__tw_yield` (one task of the
  browser) in place of the job loop. The program calls it through
  `tw_js_call_async`, the only import that suspends the program (JS
  Promise Integration: `WebAssembly.Suspending`), and the WASI host
  starts the program with `WebAssembly.promising`. In Node.js,
  `node:wasi` starts an object with the memory of the instance and a
  `_start` that calls the program in this way, and `proc_exit` ends the
  run with an exception that gives the status. The other calls stay
  synchronous. A call of `__host` while the program waits enters the
  program again, as with QuickJS.
- A browser cannot import a file that is only in memory: an ES module,
  and a TypeScript file with the syntax of one, becomes CommonJS with the
  `imports` transform of Sucrase (`typescript.js`), and `import.meta`
  becomes an object of the file. A module with a top-level `await`
  cannot become CommonJS.

`src/compat/js/engine.rs` is the safe wrapper (`Engine`). The crate has
`unsafe` code in four places only: this wrapper, the address of the
design system in `runtime.rs` (see below), the file events
(`cli/events.rs`), and the C entry of the program.

### The bundle and the shims

`assets/compat.js` is one JavaScript file (130 KB, minified: 89 KB for
the plugin layer, and 41 KB for the parts of Node.js), made by
`tests/upstream/tools/build-compat-bundle.mjs` with esbuild from the
source of Tailwind CSS and from `native/tailwind/js`. It is in the
repository, so a build needs no Node.js. The same file from the source
of each older version that the Rust compiler serves is a patch file in
`assets/compat/` (`scripts/compat.sh`): `compat/js/bundles.rs` gives the
file of the version that the compiler follows (`SWITCHES.md`, "The
plugin layer").

### The bytecode of the plugin layer

A compile with `@plugin` or `@config` starts the plugin layer, and the
compile of the text of `assets/compat.js` took about 15 ms of each such
build. The build makes its bytecode: `build.rs` compiles
`c_src/bytecode.c` and QuickJS for the computer of the build (with the
C compiler of the computer, also when the program is for another
system), and the tool writes `compat.bc` in the directory of the build.
`compat/js/bundles.rs` includes it, and a start of the plugin layer of
the version of the port reads it (about 1 ms). The bytecode is not in
the repository: a port of a new version (`scripts/compat.sh`) changes
`assets/compat.js`, and the next build makes its bytecode.

- The format of QuickJS has little-endian numbers and the version of
  the format, so the same bytes are good on each CPU and in the WASI
  program. When the engine cannot read the bytes (no bytes, or another
  version of QuickJS), nothing runs, and the program compiles the text,
  with the same result.
- When the computer of the build cannot make the bytecode (for example,
  its C compiler is MSVC), the build gives a warning, and the program
  compiles the text.
- The other versions start from their text (a patch of
  `assets/compat.js`). The cache of `js.c` keeps their bytecode in the
  process.
- The engine of the host (`tailwind-browser.wasm`) has no bytecode: the
  host evaluates the text.
- The bytecode has the source text, as a compile of the text keeps it
  (`Function.prototype.toString`): 486 KB in each program. With no
  source, it has 209 KB.

The bundle has these modules of Tailwind CSS as they are: all of
`compat/` except `legacy-utilities.ts`, `plugin.ts`, the constructors
and `toCss` of `ast.ts`, `walk.ts`, `css-parser.ts`,
`selector-parser.ts`, `value-parser.ts`, the functions of `utilities.ts`
and `variants.ts` that the layer imports (`withAlpha`,
`compoundsForSelectors`, `substituteAtSlot`; esbuild removes the rest),
and `utils/`.

The modules with state are replaced (`js/shims`, and
`js/design-system.ts` for the object that upstream calls
`designSystem`):

| Upstream | Here |
|---|---|
| `designSystem.theme` (`add`, `get`, `getOptions`, `namespace`, `clearNamespace`, `resolve`, `resolveValue`, `hasDefault`, `keysInNamespaces`, `prefix`, `addKeyframes`, ...) | One host operation for each method. The theme is the `Theme` of Rust. |
| `designSystem.utilities.static` and `functional` | The compile function stays in JavaScript under a number. Rust registers a closure that calls the bundle with this number and the candidate as JSON, and gets the tree as JSON, `null` or `undefined`. |
| `designSystem.utilities.suggest`, `designSystem.variants.suggest` | The same for the function of the suggestions: Rust calls it (`__tw_suggest`) only for the class list. |
| `designSystem.variants.static`, `functional`, `compound`, `fromAst`, `group` | The same for the function of a variant (the rule goes to JavaScript and comes back) and for the compare function of a group. |
| `designSystem.variants.get(name).applyFn` (`theme-variants.ts` wraps the core variants `aria`, `supports` and `data`) | Rust keeps the function of the variant under a number (`variants.get`), and JavaScript calls it with `variants.apply`. |
| `designSystem.variants.variants` and `compareFns` (`screens-config.ts` moves the orders) | `variants.orders`, `variants.setOrder`, `variants.compareFnOrders`, `variants.moveCompareFns`. |
| `designSystem.resolveThemeValue = …` | The assignment sets a hook in Rust that calls the JavaScript function for a path that is not a theme variable. The JavaScript function calls the Rust function for a theme variable. |
| `designSystem.important`, `invalidCandidates.add` | Host operations. |
| `apply.ts` (`substituteAtApply`), `css-functions.ts` (`substituteFunctions`) | The tree goes to Rust as JSON and comes back. |
| `compat/legacy-utilities.ts` | One host operation: the Rust code registers the same utilities (`legacy_utilities.rs`), so they need no call into JavaScript in a build. |
| `index.ts`, `theme.ts` | The constants `Features`, `Polyfills` and `ThemeOptions`. |

`js/entry.ts` has the global functions that Rust calls: `__tw_compat`
(run `applyCompatibilityHooks` on the tree of the stylesheet: the tree,
the sources and the features go as JSON and come back),
`__tw_utility`, `__tw_variant`, `__tw_compare`,
`__tw_resolve_theme_value` and `__tw_module_source`. When an exception
leaves one of these functions, the bundle gives the host the text of
each error of its `cause` chain (`error.causes`), and the error of the
compiler has them (`Error::causes`): the program prints each one in a
`Caused by:` box, as `formatError` of the official program.

`src/compat/js/json.rs` has the JSON of the tree, the candidates and the
variants, in the shapes of `ast.ts` and `candidate.ts`. The node of
`@tailwind utilities` is a context node in Rust and an at-rule in
JavaScript, as upstream has it at that time.

`src/compat/js/runtime.rs` has the host operations. The design system
is not owned by the engine, and a closure must not keep a reference to
it (see "No captured design system" above). So each call into
JavaScript puts the address of the design system on a stack for the
time of the call, and a host operation reads it there. In `compile`,
the operations that register utilities and variants get the exclusive
reference, one at a time, and never while another operation holds a
shared reference. In a build, an operation gets a shared reference
only, so a plugin cannot register in a build.

The design system owns the engine (`DesignSystem::js`), and each closure
of a plugin utility or variant has a counted reference to it.

### Modules

`src/compat/js/resolve.rs` resolves a name to a file, as Node.js and
`enhanced-resolve` do: relative paths, the extensions `.js`, `.json`,
`.cjs`, `.mjs`, `.ts`, `.mts` and `.cts`, index files, `node_modules`
in each parent directory, `exports` (conditions and patterns),
`imports` (`#name`) and `main` of a `package.json`, and `NODE_PATH`.
The path of a `@plugin` or `@config` rule uses the conditions `node`
and `import`, and then `node` and `require`, as `@tailwindcss/node`.
`require` and `import` in a module use the conditions of Node.js 22.
The first key of the package that is a condition gives the target:
the sequence of the keys of the package has the meaning, as in Node.js
(`serde_json` keeps it). A test compares 160 answers with
`require.resolve` and `import.meta.resolve` of Node.js.

`js/modules.ts` runs the modules. A CommonJS module runs in a function
with `module`, `exports`, `require`, `__filename` and `__dirname`. An
ES module is a module of QuickJS. `require` of an ES module gives its
namespace. An `import` of a CommonJS module, of a JSON file or of a
module of the program gets a small ES module that exports its values.

The modules of the program are in the bundle:

| Module | File |
|---|---|
| `tailwindcss/plugin`, `tailwindcss/colors`, `tailwindcss/defaultTheme`, `tailwindcss/lib/util/flattenColorPalette` | The source of Tailwind CSS. |
| `fs`, `fs/promises` | `js/node-fs.ts`: reads through the host. |
| `path` | `js/node-path.ts` |
| `util`, `util/types` | `js/node-util.ts`: `inspect` and `format` follow the code of Node.js. |
| `buffer`, `Buffer`, `TextEncoder`, `TextDecoder`, `atob`, `btoa` | `js/node-buffer.ts`, `js/encoding.ts` |
| `url`, `URL`, `URLSearchParams` | `js/node-url.ts` |
| `os`, `assert`, `module`, `process` | `js/node-os.ts`, `js/node-assert.ts`, `js/modules.ts`, `js/globals.ts` |
| `timers`, `setTimeout`, `setInterval`, `setImmediate` | `js/timers.ts` |
| `child_process`, `http`, `net`, ... | `js/denied.ts`: each function throws the error of its rule. |

`tests/node_api.rs` runs one file of cases
(`tests/fixtures/node_api_cases.cjs`) in the engine, and compares each
value with the record of the same file in Node.js 22
(`tests/upstream/tools/gen-node-api.mjs`).

The library has no file access. The caller of `compile` gives a
`ModuleLoader` (`compat/module_loader.rs`: read a file as text or as
bytes, the kind of a path, the names in a directory, and optional
functions for symbolic links, the dependencies, `console`,
`NODE_PATH`, `process.env`, `process.argv` and `process.platform`).
The command line program gives the file system and its environment
(`cli/modules.rs`), and the tests give files in memory
(`MemoryLoader`). Each module file that is not in `node_modules` is a
dependency: the watch makes a new compiler when it changes.

### TypeScript

A TypeScript module (`.ts`, `.mts`, `.cts`) must lose its types before
the engine can run it. Upstream uses Node.js or Bun, and then jiti.
Two ways were built and measured for this program (Linux x86_64, the
release profile of the workspace: `opt-level = "s"`, fat LTO, strip):

| Way | Native program | `wasm32-wasip1` | Builds for `x86_64-unknown-linux-musl` and for aarch64 with `-Zbuild-std` | In `tailwind.com` (two CPUs) |
|---|---|---|---|---|
| Rust, `oxc` 0.158 (parser, semantic, transformer, codegen) | +1.94 MB | +1.95 MB | yes, yes | about +3.9 MB |
| Rust, `swc_ts_fast_strip` | +3.52 MB | +2.88 MB | yes, yes | about +7.1 MB |
| JavaScript in QuickJS: the TypeScript transform of Sucrase 3.35.1, as a second file | +0.21 MB | +0.21 MB | no Rust code | +0.41 MB |

The numbers of the two Rust ways are the size of a program that only
removes the types of one file, minus the size of the same program with
no transformer. The number for `tailwind.com` is two times the number
for musl. The numbers of the JavaScript way are the size of the file
`assets/typescript.js` (206 KB): the program holds it as text.

The program uses the JavaScript way: it adds one tenth of the size of
the smallest Rust way. Its costs:

- **Time.** The engine evaluates the file at the first TypeScript
  module of a build, and not for a project with no TypeScript. This
  adds about 25 ms to a build on Linux x86_64 (the case `ts-config`,
  a config of 60 lines with a plugin of 30 lines: 78 ms. The case
  `config-esm`, a JavaScript config: 53 ms).
- **Namespaces.** Sucrase removes a `namespace` with its content. The
  recorded case `ts-plugin` has a namespace with values, so
  `js/typescript-entry.ts` has a step before Sucrase: it finds each
  namespace with the tokenizer of Sucrase, and writes the function
  that the TypeScript compiler makes for it (`var N; (function (N) { …
  })(N || (N = {}))`), with the exported names at the end of the body.
- **Decorators and JSX** are not there. `oxc` has them.

`js/typescript.ts` loads the file (`asset` of the host) and keeps the
function. `read` of `js/modules.ts` calls it for each TypeScript file
before the file runs as CommonJS or as an ES module.
`tests/upstream/tools/build-plugin-bundles.mjs` makes the file.

### The plugins in the program

The standalone program of Tailwind CSS has three plugins in its file:
`@tailwindcss/forms`, `@tailwindcss/typography` and
`@tailwindcss/aspect-ratio`
(`packages/@tailwindcss-standalone/src/index.ts`). Its function
`__tw_resolve` gives the name itself for these three names, before the
resolver looks at the disk, and `__tw_load` gives the copy of the
program. So `@plugin "@tailwindcss/typography"` always loads that copy,
also when the project has the package in `node_modules`. `loadModule`
of `js/modules.ts` has the same rule: the path of the module is the
name, and its base is `@tailwindcss`, as there. A `require` of the
package in a config file is not a part of the rule: it finds
`node_modules`, or fails.

`assets/plugins/NAME.js` has each plugin with its dependencies as one
CommonJS file (esbuild, minified): 83 KB for typography with
`postcss-selector-parser`, 11 KB for forms, 1 KB for aspect-ratio. The
versions are the versions of the lock file of Tailwind CSS (`UPSTREAM`
has them, and `sync-upstream.sh check` compares). The engine evaluates
a file only when a stylesheet names the plugin.

### What a plugin can do

A plugin can read each file that the `ModuleLoader` gives (in the
program: each file that the user can read) and the environment, and
can use the CPU and the memory of the engine. It cannot write files,
start a process, or open a network connection: `js/denied.ts` has the
functions for these calls, and each one throws an error that names
its rule. See [`SECURITY.md`](../SECURITY.md).

### Warnings

Tailwind CSS gives its warnings to `console.warn`. The program prints
the same text to the standard error:

- The plugin layer is the JavaScript of Tailwind CSS, so its warnings
  (`darkMode`, a prefix with `-`) come from that code, through
  `console` of the `ModuleLoader`.
- `utilities/css_utility.rs` has the warning for a data type that
  `--value(…)` cannot have. The design system collects the texts, and
  `Compiler::take_warnings` gives them to the program.
- `cli/optimize_css.rs` has the warnings of Lightning CSS: the parser
  recovers from an error and gives a warning, and the program prints
  the lines around it as `optimize.ts` does, without the warnings for
  the pseudo-classes of Vue and Angular.

### Source maps

With the option `from` of `compile` (the path of the stylesheet), the
compiler keeps the source locations, and `Compiler::build_source_map`
gives the source map of the last output:

1. **The parser** (`css_parser::parse_with_source`) gives each node its
   `src`: the stylesheet (`Source`, shared with `Rc`) and the start and
   the end offset. `@import` does the same for each imported file. All
   offsets and columns are numbers of UTF-16 code units, as in
   JavaScript, so the maps have the numbers of Tailwind CSS. The parser
   reads bytes: for a text that is not ASCII, it has a table from byte
   offsets to UTF-16 offsets.
2. **Each step** that makes a node from another node copies or sets the
   `src`, at the same places as Tailwind CSS: the theme rule, the
   wrappers of an import, `@apply` (the location of the candidate in the
   rule), the rules of the candidates (the location of
   `@tailwind utilities`, unless the rule comes from `@utility` or
   `@custom-variant`), the fallbacks of the last pass, and the rules
   that the nesting step makes.
3. **The printer** (`to_css::to_css_tracked`) gives each printed node
   its `dst`: its place in the output.
4. **`source_maps::create_source_map`** makes one mapping for each line
   of the `src` of a node, and one for its end, and sorts them.

A node holds its locations in one field, `loc` (`ast::Loc`): a pointer
that is null when the compiler keeps no locations. So a build without
`from` has almost no cost, and the constructors of `ast.rs` need no
change. A comparison of two nodes ignores `loc`: a tree from the parser
with a path is equal to the tree without one.

Tailwind CSS sometimes puts one node object at two places of the tree,
and the printer then gives it one `dst`. The Rust tree has copies. The
steps that make such copies mark them with one identity
(`ast::share_identity`, only when the compiler keeps locations), and
the printer gives the copies of one identity the `dst` of the last one.

The locations also go through JavaScript (`compat/js/json.rs`). A node
in JSON has the keys `src` and `dst`, as upstream, but each value is a
number: the index of the location in a table of the runtime
(`LocTable`). The plugin layer only copies these values (`node.src =
src` in `compat/plugin-api.ts`, `cloneAstNode`). The identity of a
declaration or of a comment is the key `__id`. `host.ts` keeps the
number of each object in a `WeakMap`, gives a new number to an object
that has none, and writes the number only into the JSON text: a copy of
an object in JavaScript is a new object. The compiler calls
`__tw_track` before the first call when it keeps locations. With no
source map, the JSON text has none of these keys.

## The command line program

`src/cli/` follows `@tailwindcss/cli` and `@tailwindcss/node`:

- `mod.rs`: the options, the build, the write of the output, the watch.
- `events.rs`: the file events of `native/tailwind/c_src/watch.c` for the watch, and
  the lines of `TAILWIND_WATCH_DEBUG`.
- `modules.rs`: the files of the JavaScript modules of `@plugin` and
  `@config`, and their record for the watch.
- `resolve.rs`: the paths of `@import` (relative files, packages in
  `node_modules` with the `style` condition, and the stylesheets of
  Tailwind CSS in the program).
- `optimize_css.rs`: Lightning CSS for `--minify` and `--optimize`: the
  same version and options as Tailwind CSS, in two passes, and the
  warnings of the first pass. With a
  source map, each pass gets the map of its input (the crate
  `parcel_sourcemap`, as the Node.js package of Lightning CSS).
- `source_maps.rs`: the JSON text of a source map, as the package
  `source-map-js` writes it, and the comment with the map or with its
  URL.
- `remapping.rs`: the replacement of `@media not (` after the
  optimizer, with a new source map: the parts of `magic-string`
  (`generateMap` with `hires: 'boundary'`) and of
  `@jridgewell/remapping` that Tailwind CSS uses for it.
- The scanner is the code of Tailwind CSS itself: the detection of
  source files, `.gitignore`, and the extraction of candidates. Its three
  crates (`crates/oxide`, `crates/classification-macros` and
  `crates/ignore` of Tailwind CSS) are a copy with no change in
  `native/scanner`, with our manifests, so that crates.io can take them
  (`native/scanner/README.md`).

### The watch

A tick of the watch starts at a file event of the system, or at the end
of an interval. At each tick:

1. When the stamp (the time and the size) of the input, of an imported
   stylesheet, or of a JavaScript module of `@plugin` or `@config`
   changed: make a new compiler and a new scanner, scan,
   build and write. With an error, print it and keep the old compiler.
   Before this, the program waits until these stamps stay the same for
   10 ms (at most 10 times). A shell redirect (`printf ... > app.css`)
   empties the file first and then writes it. A build from the empty
   file gives a wrong output for a short time, and with no
   `source(none)` it scans all of the directory of the input. In that
   scan, a read of a FIFO with no data does not end.
2. If not: scan the source files (the scanner reads only the changed
   files) and build. The compiler gives the same output when there is
   no new candidate, and the program then writes nothing.

The program compares the files itself at each tick, so an event only
wakes it. A lost event or a false event changes the time of a build, not
its result.

The events are in `native/tailwind/c_src/watch.c`. It uses the method of the file
watcher of BEAM.com (`c_src/cosmo/beam_com_watch.c`):

| System | Events | Interval |
|---|---|---|
| Linux | inotify on the directories of the files. Cosmopolitan has no wrappers for inotify, so the calls are direct system calls. | 500 ms |
| macOS, FreeBSD, NetBSD, OpenBSD | kqueue on the directories and, while there are descriptors (half of the open file limit, at most 4096), on the files. `struct kevent` has four layouts, and the code selects one at run time. | 500 ms |
| Windows | `ReadDirectoryChangesW` with overlapped I/O, and one I/O completion port for all the directories: a watch of the full tree of each source directory, and a watch of the entries of each other directory of the files. | 500 ms |
| With `--poll[=ms]`, and in a build that is not `tailwind.com` | None. | `--poll` (default 250 ms) |

After an event, the program waits until no more events come for 20 ms
(at most 0.2 s), because an editor often writes a file in more than one
step. The interval finds the changes that have no event: a file in a
directory that has no scanned file yet, and files above the limit of
kqueue.

On Windows:

- The program gives the base directories of the sources of the scanner
  first (`get_normalized_sources`, as `watchDirectories` of the official
  program). Each has one handle with a watch of its full tree
  (`bWatchSubtree`). A directory of a file that is not in such a tree
  (the input, an imported stylesheet, a JavaScript module) has one
  handle with a watch of its entries, as with inotify. A path in a
  watched tree gets no handle, and a new tree closes the handles below
  it. One handle for each scanned directory is not correct there:
  Windows does not let a user rename a directory while a handle is open
  below it. A tree also has events for a new directory at once.
- All the handles are on one I/O completion port.
  `GetQueuedCompletionStatus` waits for all of them with a time limit,
  and has no limit of 64 handles. After each completion, the code
  starts the read of that directory again. Windows keeps the changes
  between the two reads on the handle.
- The code does not read the names in the buffer: a completion only
  wakes the watch. So the buffer is small (4 KB), and an overflow of
  the buffer (a completion with no data, or `ERROR_NOTIFY_ENUM_DIR`) is
  a change as each other.
- A completion with another error (the directory is deleted), or a read
  that does not start, closes the handle and wakes the watch. The
  program gives the directory again after each tick from an event, when
  the directory is there.
- A path of Cosmopolitan (`/c/dir`, UTF-8) becomes a path of Windows
  (`c:\dir`, UTF-16) with `__mkntpathat` of Cosmopolitan, as its own
  `open` does.
- Cosmopolitan has no `ReadDirectoryChangesW`: the address comes from
  `kernel32.dll` (`GetProcAddress`), and the call has the calling
  convention of Microsoft.
- When the start fails (no function, no port), the watch polls. When no
  directory is watched or the port fails, the wait gives an error, and
  the watch polls from that time (250 ms).

With `TAILWIND_WATCH_DEBUG=1` in the environment, the watch prints to
stderr what it uses (`watch: events (inotify)`, `(kqueue)`, `(windows)`,
or `watch: poll`), each wake from an event (`watch: event`), and what
started each build (`watch: build (event)` or `watch: build (interval)`).
`tests/watch.sh` reads these lines: in CI, a system where the events do
not work fails the test, and the poll does not hide it.

The program records the stamp of a file before it reads the file. The
model `specs/Watch.tla` shows why: with the other order, an edit between
the read and the stamp is lost. A tick of the model is a tick from an
event or from the interval: the model does not need the difference.

The end of stdin stops the watch (not with `--watch=always`), so the
program stops with the process that started it.

## The file tailwind.com

`scripts/build.sh` builds the crate as a static library for the targets
`x86_64-unknown-linux-musl` and `aarch64-unknown-linux-musl`, and links
the two with `cosmocc` into one Actually Portable Executable. This is
the method of BEAM.com for Rust:

- `native/tailwind/c_src/cosmo_rust.c` translates the calls of the Rust standard library
  that have the numbers of Linux in them (errno, the clocks, the flags
  of `open` and `mmap`, the futex, `stat`) to Cosmopolitan, which sets
  these values for each system at run time. `objcopy --redefine-sym`
  points the Rust library to them.
- On aarch64, Cosmopolitan reserves the registers x18 and x28. The Rust
  standard library builds again with these registers reserved
  (`-Zbuild-std`, a nightly Rust).
- `scripts/rust_link.sh` makes one object with one global name,
  `tailwind_main`, and `native/tailwind/c_src/main.c` calls it.
- `scripts/ape_loader.sh` changes the path of the APE loader in the
  shell header from `.ape-1.10` to `.ape-1.10-HASH`, with HASH the
  SHA-256 of the loaders in the file (12 digits). The header of apelink
  runs `$TMPDIR/.ape-1.10` (or `~/.ape-1.10`) when it exists, and does
  not examine it. Other APE programs write a different loader to the
  same path (the loader of BEAM.com has a patch for macOS), and
  `tailwind.com` then failed with their loader (issue #28). The header
  keeps its size: the script takes the bytes from the padding of the
  numbers of the `dd` commands. `tests/ape_loader.sh` checks the file.

### Windows on ARM64

The PE header of an APE file is for AMD64, so Windows on ARM64 starts
the x86_64 half of the file in its x64 emulation. The `cosmocc` of
`scripts/cosmo.sh` gives the aarch64 libc a Windows layer, and a native
loader for Windows on ARM64, `ape-arm64.exe` (7 KB, C58 in
`docs/UPSTREAM.md` of BEAM.com). The loader maps the aarch64 ELF half of
the file at its fixed addresses, and starts it.

- `scripts/build.sh` puts the loader into the ZIP of the file.
- At the start, the x86_64 half asks Windows for the CPU of the system
  (`IsWow64Process2()`). On ARM64, `native/tailwind/c_src/main.c` writes
  the loader to the directory of the temporary files as
  `.ape-arm64-HASH.exe` (HASH: the FNV-1a hash of the loader), and the
  process becomes the loader (`execv()`), with the path of the file and
  the arguments. `execv()` of Cosmopolitan gives the status of the
  loader to the parent process.
- Each start compares the file of the loader with the loader of the
  ZIP, and writes a file that is different again: a changed file does
  not run. Two starts at the same time write two temporary files. Each
  one gives its file the name of the loader. When Windows refuses the
  name (the other loader runs), the start uses the file of the other
  start, if its contents are the loader.
- When the write or the start of the loader fails, the x86_64 half
  continues in the x64 emulation.
- The aarch64 half uses `ReadDirectoryChangesW()` for the watch, as the
  x86_64 half does.

`tests/arch.sh` checks the half that runs on each system: a plugin puts
`process.arch` into the CSS. The job `starts` starts the file 2000 times
on `windows-11-arm`, and a copy with no loader in its ZIP (the x64
emulation) 500 times.

### The allocator

A build spends about a third of its instructions in `malloc` and `free`
(callgrind, on a project of 1000 files). The program of `cargo build`
uses mimalloc 2 (the crate `mimalloc`, `src/main.rs`). The file uses the
same mimalloc: `scripts/cosmo.sh` compiles its C source (from the crate
`libmimalloc-sys` of `Cargo.lock`) with cosmocc, and
`src/cosmo_allocator.rs` is the allocator of Rust (the feature
`ape-mimalloc`). Three changes for Cosmopolitan:

- `MI_USE_BUILTIN_THREAD_POINTER=0`: the id of a thread is the address of
  a thread-local variable. The thread pointer of the CPU (`%fs:0`,
  `tpidr_el0`) is not the thread data of Cosmopolitan.
- `_GNU_SOURCE`, for the declaration of `madvise`.
- `MI_PRIM_HAS_PROCESS_ATTACH=1`: no constructor and no destructor of
  the process. mimalloc starts at its first allocation. Its destructor
  deleted the pthread key of its threads at the exit, while the threads
  of the scanner still freed memory, and Cosmopolitan stopped on an
  assertion in `pthread_getspecific()` (in about one run in ten on a busy
  computer).

mimalloc 2, not 3: the two are as fast, and mimalloc 3 keeps more memory
(a peak of 117 MB, not 91 MB, for the program of `cargo build` on that
project). The native NIF objects keep the allocator of `beam.com`: its
table for objects does not have `madvise` and `mprotect`.

A build with no `--watch` does not free the compiler at its end
(`cli::run`): the system frees the memory of the process, and the drop
took a sixth of the instructions of a large build.

### The size of the file

The file is about 20 MB: two copies of the program, one for x86_64 and
one for aarch64, of about 9.7 MB each, and 0.37 MB for the packages of
the versions of Tailwind CSS in its ZIP directory (see "The versions of
Tailwind CSS" below). In one copy:

| Part | Size |
|---|---|
| Lightning CSS with its CSS parser, for `--minify` and `--optimize`, and its source maps | about 3.5 MB |
| The compiler (the utilities, the variants, the default theme of v3) | about 1 MB |
| The scanner of Tailwind CSS with its regular expressions and its `.gitignore` code | about 1 MB |
| QuickJS with its Unicode tables, and the JSON code | about 1.4 MB |
| The JavaScript files of the program, as text: the plugin layer of Tailwind CSS with the parts of Node.js (136 KB), the patches of the plugin layer of the 47 older versions (170 KB), the runtime of the script engine (58 KB), the TypeScript transform (206 KB), the three plugins of the standalone program (96 KB) | 0.67 MB |
| The Rust standard library, the unwind tables, the stylesheets, Cosmopolitan | the rest |

The source maps (the code of the compiler and of the program, and the
source maps of Lightning CSS with the crate `parcel_sourcemap`) are
about 0.15 MB of one copy.

The JavaScript files are in the two copies, so they are 1.33 MB of
the file. Before TypeScript, the three plugins and the parts of Node.js,
the file had 18.3 MB, with 89 KB of JavaScript in a copy. Then it had
19.1 MB: 0.41 MB for the TypeScript transform, 0.19 MB for the three
plugins, 0.08 MB for the parts of Node.js, and the rest for the Rust
and C code of the loader and of the warnings. `tailwind.wasm` has one
copy: 8.6 MB before, and 8.95 MB then.

The versions of Tailwind CSS came next. Each copy is 0.49 MB larger:
0.15 MB for the script engine, and 0.34 MB for the 196 switches and the
plugin layer of the 47 older versions (half of it is the 170 KB of
patches). With the packages in the ZIP directory, the file has 20.3 MB
now, and `tailwind.wasm` 9.4 MB. The sizes in this section are in
units of 1,000,000 bytes. `docs/BENCHMARKS.md` gives the same sizes in
units of 1,048,576 bytes (19.4 MB and 9.0 MB).

The dependencies build for size (`opt-level = "s"`), and the compiler
and QuickJS (`-O2`) build for speed. With all the code built for speed,
the file is about 3 MB larger, and a build with `--minify` is about 20%
faster.

The JavaScript engine starts only for a stylesheet with `@plugin` or
`@config`. The project of `mix phx.new` (Phoenix 1.8, with heroicons and
daisyUI) builds in about 0.11 s. The official program on Node.js needs
about 0.29 s on the same computer.

### The versions of Tailwind CSS

The command line program has two engines behind one type
(`AnyCompiler` in `src/cli/engine.rs`): the Rust compiler
(`index.rs`), and the script engine (`src/script.rs`). See
[`VERSIONS.md`](VERSIONS.md) for what a user sees.

The script engine starts a QuickJS engine with the bundle
`assets/script.js` (entry: `js/script-entry.ts`). This bundle has the
loader of modules, the parts of Node.js and the TypeScript transform of
the plugin runtime, and no code of Tailwind CSS. It asks the host for
the files of the package (`asset` with `tailwindcss:FILE`), runs
`dist/lib.js` as a CommonJS module, and calls its `compile`:

| The official compiler needs | Where it comes from |
|---|---|
| `loadStylesheet(id, base)` | The host operation `stylesheet.load`: the resolver of `src/cli/resolve.rs`, as for the native engine. `theme.css`, `preflight.css` and `utilities.css` are the files of the package. |
| `loadModule(id, base)` | `loadModule` of `js/modules.ts`: the same loader as for the plugins of the native engine. |
| `tailwindcss/plugin` and the other modules of the package | The CommonJS files that `exports` of the `package.json` of the package names. |
| `compiler.build(candidates)` | The candidates of the Rust scanner, as JSON. The result is the CSS text. |
| `compiler.buildSourceMap()` | The decoded map as JSON. The Rust code of `--map` makes the map file from it, as for the native engine. |

The two runtimes share their host operations for modules, files, the
console and the process (`src/compat/js/platform.rs`).

The packages are files: `DirectoryPackage` reads them from a directory.
In `tailwind.com` that directory is `/zip/tailwindcss/VERSION`:
Cosmopolitan gives the ZIP directory of the file as the path `/zip`.
`scripts/build.sh` adds the files with `zip` after the link. The store
has the newest package as a directory and each older version as a patch
file (`src/patch.rs`): `DirectoryPackage` reads a file of a version
from its directory, or from the reference version of its patch with the
delta of the patch. The tool `tools/twpatch` makes the patch files: it
includes `patch.rs`, and it builds for the computer of the build.

The bundle has two parts that make QuickJS do the same as V8 for the
compiler: `js/sort.ts` (the sort algorithm of V8) and `js/intl.ts`
(`Intl.ListFormat` for English). They are in the plugin runtime of the
native engine too, so a plugin gets the same sort.

## The Erlang and Elixir modules

`src/tailwind.erl` has the work: the path of the file, the download
(`httpc` with TLS, the proxy of the environment, an optional SHA-256),
and the run through a port. On the systems that are not Windows, the
shell starts the file (`sh tailwind.com`), because the kernel does not
start an APE file directly in all cases. When the owner of the port
ends, the standard input of the program closes, and a watch stops.

`lib/tailwind.ex` gives the API of the Hex package `tailwind` on top of
it, and `lib/mix/tasks/` has the two tasks.
