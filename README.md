# tailwind.com

Tailwind CSS 4 in one executable file, with no Node.js. The same file
runs on Linux, macOS, Windows, FreeBSD, NetBSD and OpenBSD, on x86_64 and
aarch64.

`tailwind.com` is a Tailwind CSS compiler in Rust, as one
[Actually Portable Executable](https://justine.lol/ape.html) made with
[Cosmopolitan Libc](https://github.com/jart/cosmopolitan), in the way of
[BEAM.com](https://github.com/sntran/BEAM.com). It takes the place of:

- **The official `tailwindcss` program**: the same options, the same
  output, in one file of about 19 MB for all the systems and both CPUs.
- **The Hex package `tailwind`** in a Phoenix project: this repository
  is also an Erlang and Elixir project with the application name
  `tailwind`, the module `Tailwind`, and the tasks `mix tailwind` and
  `mix tailwind.install`.

The compiler follows Tailwind CSS 4.3.5 file by file. The tests compare
its output with the recorded output of the official compiler: see
[Compatibility](#compatibility).

## Quick start

```sh
curl -fLO https://github.com/sntran/tailwind.com/releases/latest/download/tailwind.com
sh ./tailwind.com --input app.css --output dist/app.css --minify
```

On Windows, run `tailwind.com` in the command prompt. With no `--input`,
the input is `@import "tailwindcss";`. The stylesheets of Tailwind CSS
(`tailwindcss`, `tailwindcss/theme`, `tailwindcss/preflight`,
`tailwindcss/utilities`) are in the file.

```
tailwind.com [--input input.css] [--output output.css] [--watch] [--poll=ms] [options…]

-i, --input      Input file (- for stdin)
-o, --output     Output file (default: stdout)
-w, --watch      Watch for changes and rebuild as needed; --watch=always
                 keeps the watch when stdin closes
    --poll       Use polling instead of file system events (--poll=ms
                 sets the interval)
-m, --minify     Optimize and minify the output
    --optimize   Optimize the output without minifying
    --map        Generate a source map: in the output, or with --map=FILE
                 in a file
    --cwd        The current working directory
    --silent     Suppress non-error output
-h, --help       Display usage information
-v, --version    The versions of this program and of Tailwind CSS
```

## Phoenix

In `mix.exs`, replace the Hex package `tailwind`:

```elixir
{:tailwind, github: "sntran/tailwind.com", runtime: Mix.env() == :dev}
```

The configuration of the project does not change:

```elixir
config :tailwind,
  version: "4.3.5",
  my_app: [
    args: ~w(
      --input=assets/css/app.css
      --output=priv/static/assets/css/app.css
    ),
    cd: Path.expand("..", __DIR__)
  ]
```

`mix tailwind.install` downloads `tailwind.com` to `_build/`, and
`mix tailwind my_app` runs it. The watcher of the endpoint
(`tailwind: {Tailwind, :install_and_run, [:my_app, ~w(--watch)]}`) works
as before: the watch stops when the Phoenix server stops.

A new Phoenix project has `@plugin` lines in `assets/css/app.css` (for
heroicons and daisyUI). These plugins are JavaScript. The program runs
them: see [JavaScript plugins and configs](#javascript-plugins-and-configs).
The output for the project of `mix phx.new` is the same as the output of
the official program, byte for byte, for the two layouts of Phoenix 1.8:
daisyUI in `assets/vendor` (to 1.8.5), and daisyUI in `deps/` with
`NODE_PATH` and the `phoenix-colocated` import (1.8.6 and later).

An Erlang project uses the module `tailwind` (`tailwind:install/0`,
`tailwind:run/2`), with the same configuration in the application
environment.

## npm

This repository is also the npm package `tailwind.com`:

```sh
npx tailwind.com -i app.css -o dist/app.css --minify
```

The package does not hold `tailwind.com`. At the first run,
`npx tailwind.com` downloads the file of its release from GitHub, and
checks its SHA-256 against the value that the package holds. Then the
file stays in the cache (`~/.cache/tailwind.com/npm/`).

- `TAILWIND_COM`: the path of a `tailwind.com` to use. Then nothing is
  downloaded.
- `TAILWIND_COM_DOWNLOAD`: the URL of a directory with the file
  `tailwind.com`, in place of the GitHub release.
- `TAILWIND_COM_CACHE`: the cache directory.

The package has the WebAssembly module too, so a program in Node.js 22
or later can run the compiler in its own process, with no download:

```js
import { run, runWasm } from 'tailwind.com';

// The native program:
await run(['-i', 'app.css', '-o', 'dist/app.css', '--minify']);

// The same program as WebAssembly. The directory `cwd` is all that it sees.
const { status, stdout, stderr } = await runWasm(['-i', 'app.css'], { cwd: 'project' });
```

`import wasm from 'tailwind.com/tailwind.wasm'` gives the module itself,
for another WASI host.

Each release also publishes the package to GitHub Packages, with the
name `@sntran/tailwind.com` (registry `https://npm.pkg.github.com`), and
holds the package file itself.

## Compatibility

The output of the official compiler is the specification. `cargo test`
replays these records of Tailwind CSS 4.3.5 (`tests/upstream`), and
expects the same text, byte for byte:

| Record | Size |
|---|---|
| Each `compile` call of the unit tests of Tailwind CSS | 2,334 calls |
| Each class of the default design system, each variant, 6,000 random candidates and 784 edge cases, one at a time | 75,082 candidates |
| The same candidates in one build (the sort of the output) | 24 MB of CSS |
| `@import "tailwindcss"` with real-world classes, also through the optimizer | 686 candidates |
| The official command line program on small projects, also the warnings of the optimizer and of `@utility` | 20 projects |
| The source maps of the official compiler and of its optimizer | 317 stylesheets |
| The official command line program with `--map`, also with a JavaScript plugin and config | 29 projects |
| The official command line program on projects with JavaScript and TypeScript plugins and configs (daisyUI, heroicons, `@tailwindcss/typography`, `@tailwindcss/forms`, two Phoenix projects) | 100 projects |
| The `compile` calls of the unit tests of Tailwind CSS that load a JavaScript plugin or config | 296 calls |
| Node.js 22 for the parts of Node.js that a plugin gets (`Buffer`, `URL`, timers, `fs`, `util`, ...), and for the files that `require` and `import` find | 124 values, 160 questions |

See [`docs/COMPATIBILITY.md`](docs/COMPATIBILITY.md) for the results and
the known differences, and [`docs/TESTING.md`](docs/TESTING.md) for the
other tests: unit tests from the test files of Tailwind CSS, property
tests, and a TLA+ model of the watch.

The scanner of source files is the Rust crate of Tailwind CSS itself
(`tailwindcss-oxide`), and `--minify` and `--optimize` use the same
version of Lightning CSS with the same options.

## JavaScript plugins and configs

`@plugin` and `@config` work. The file has a JavaScript engine
([QuickJS](https://bellard.org/quickjs/)), and the plugin API is the
JavaScript of Tailwind CSS itself (`addBase`, `addUtilities`,
`matchUtilities`, `addComponents`, `matchComponents`, `addVariant`,
`matchVariant`, `theme()`, `config()`, `plugin.withOptions`, presets,
`darkMode`, `prefix`, `important`, `content`, `blocklist`).

- A module is a CommonJS file or an ES module (`.js`, `.cjs`, `.mjs`),
  a TypeScript file (`.ts`, `.mts`, `.cts`), or a JSON file. `require`
  and `import` find relative files and packages in `node_modules`
  (`exports`, `imports` and `main` of `package.json`), and the
  directories of `NODE_PATH`.
- **TypeScript**: `@config "./tailwind.config.ts"` and
  `@plugin "./plugin.ts"` work. The program removes the types. It does
  not check them, as the official program.
- `tailwindcss/plugin`, `tailwindcss/colors`, `tailwindcss/defaultTheme`
  and `tailwindcss/lib/util/flattenColorPalette` are in the file. A
  project needs no `node_modules` for them.
- `@tailwindcss/forms`, `@tailwindcss/typography` and
  `@tailwindcss/aspect-ratio` are in the file, as in the standalone
  program of Tailwind CSS: `@plugin "@tailwindcss/typography"` works
  with no `node_modules`.
- A module gets a part of Node.js: `Buffer`, `URL`, `TextEncoder`,
  `setTimeout`, `structuredClone`, `process.env`, and the modules `fs`
  (reads only), `path`, `os`, `url`, `util`, `assert` and `module`.
- A module cannot write files, start a process, or open a network
  connection: such a call throws an error that names the rule (see
  [`SECURITY.md`](SECURITY.md)).
- The watch builds again when a plugin or config file changes.

What does not work:

- **The other modules of Node.js** (`crypto`, `stream`, `events`,
  `zlib`, ...), `Intl` and `Temporal`. A plugin that needs them at load
  time gives an error.
- **A TypeScript file with JSX** (`.tsx`), and decorators.
- **`import "tailwindcss"` for a value** in a config (the full compiler
  as a JavaScript module). `import type { Config } from "tailwindcss"`
  works: a type import has no JavaScript.

See [`docs/COMPATIBILITY.md`](docs/COMPATIBILITY.md) for all the
differences.

## Limits

- **Source maps of the stylesheets in the file**: with `--map`, a
  source of a stylesheet of Tailwind CSS that is in the program has the
  path `/$tailwindcss/NAME.css`. With `node_modules/tailwindcss` in the
  project, the path is the path of the file, as with the official
  program.
- **The watch on Windows polls**: there, it reads the times of the files
  at an interval. On the other systems it uses the events of the file
  system (inotify, kqueue), as the file watcher of BEAM.com.
- `canonicalize`, the second command of the official program, is not
  there.

See [`docs/ROADMAP.md`](docs/ROADMAP.md).

## WebAssembly

`tailwind.wasm` of each release is the same program for WASI
(`wasm32-wasip1`): it runs in Node.js, Wasmtime, and other hosts with a
WASI file system. It has the same JavaScript engine for `@plugin` and
`@config`, with TypeScript and the three plugins in the file.

## Documentation

| File | What |
|---|---|
| [`docs/COMPATIBILITY.md`](docs/COMPATIBILITY.md) | The results against Tailwind CSS, and the known differences. |
| [`docs/INTERNALS.md`](docs/INTERNALS.md) | The parts of the compiler and of the file, and how they relate to Tailwind CSS. |
| [`docs/BUILDING.md`](docs/BUILDING.md), [`docs/TESTING.md`](docs/TESTING.md) | Build `tailwind.com`, and the tests. |
| [`docs/SYNC.md`](docs/SYNC.md) | Move to a new version of Tailwind CSS. |
| [`docs/ROADMAP.md`](docs/ROADMAP.md) | What comes next. |

## Contributing

See [`CONTRIBUTING.md`](CONTRIBUTING.md). Report a security problem in
private: see [`SECURITY.md`](SECURITY.md).

## License

tailwind.com is licensed under the [Apache License 2.0](LICENSE). The
compiler is a port of Tailwind CSS (MIT License), and the file
`tailwind.com` contains other software under its own licenses:
[`NOTICE`](NOTICE) names them, and the license texts are in
[`licenses/`](licenses).
