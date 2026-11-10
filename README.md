# tailwind.com

Tailwind CSS 4 in one executable file, with no Node.js. The same file
runs on Linux, macOS, Windows, FreeBSD, NetBSD and OpenBSD, on x86_64 and
aarch64.

`tailwind.com` is a Tailwind CSS compiler in Rust, as one
[Actually Portable Executable](https://justine.lol/ape.html) made with
[Cosmopolitan Libc](https://github.com/jart/cosmopolitan), in the way of
[BEAM.com](https://github.com/sntran/BEAM.com). It takes the place of:

- **The official `tailwindcss` program**: the same options, the same
  output, in one file of about 18 MB for all the systems and both CPUs.
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
The output for the project of `mix phx.new` (Phoenix 1.8) is the same as
the output of the official program, byte for byte.

An Erlang project uses the module `tailwind` (`tailwind:install/0`,
`tailwind:run/2`), with the same configuration in the application
environment.

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
| The official command line program on small projects | 17 projects |
| The official command line program on projects with JavaScript plugins and configs (daisyUI, heroicons, `@tailwindcss/typography`, `@tailwindcss/forms`, a Phoenix project) | 88 projects |
| The `compile` calls of the unit tests of Tailwind CSS that load a JavaScript plugin or config | 296 calls |

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
  or a JSON file. `require` and `import` find relative files and
  packages in `node_modules` (`exports` and `main` of `package.json`),
  and the directories of `NODE_PATH`.
- `tailwindcss/plugin`, `tailwindcss/colors`, `tailwindcss/defaultTheme`
  and `tailwindcss/lib/util/flattenColorPalette` are in the file. A
  project needs no `node_modules` for them.
- A module can read files: `fs` (`readFileSync`, `readdirSync`,
  `existsSync`, `statSync`), `path`, and a small `util`. It cannot write
  files, start a process, or open a network connection.
- The watch builds again when a plugin or config file changes.

What does not work:

- **TypeScript modules** (`tailwind.config.ts`): the error names the
  file. Use a JavaScript file.
- **The other modules of Node.js** (`os`, `crypto`, `child_process`,
  `http`, ...), `Buffer`, timers and `fetch`. A plugin that needs them
  at load time gives an error.
- **`@tailwindcss/forms`, `@tailwindcss/typography` and
  `@tailwindcss/aspect-ratio` are not in the file**, as they are in the
  standalone program of Tailwind CSS. They work from `node_modules`.

## Limits

- **No source maps**: `--map` is an error.
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
`@config`.

## Documentation

| File | What |
|---|---|
| [`docs/COMPATIBILITY.md`](docs/COMPATIBILITY.md) | The results against Tailwind CSS, and the known differences. |
| [`docs/INTERNALS.md`](docs/INTERNALS.md) | The parts of the compiler and of the file, and how they relate to Tailwind CSS. |
| [`docs/BUILDING.md`](docs/BUILDING.md), [`docs/TESTING.md`](docs/TESTING.md) | Build `tailwind.com`, and the tests. |
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
