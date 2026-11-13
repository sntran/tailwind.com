# Changelog

All important changes to this project are in this file. The format
follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/). The
version of the package is in `mix.exs`, `package.json`,
`src/tailwind.app.src` and `native/tailwind/Cargo.toml`. Each release
also names its version of Tailwind CSS (see `UPSTREAM`).

## 0.3.0 - 2026-11-13

The program in the Erlang VM, as a NIF (with native NIF objects for
BEAM.com in each release), and builds of about half the time of before.

### Fixed

- A list of paths in the `env` of a profile (the `NODE_PATH` of a new
  Phoenix project) becomes one value with the path separator of the
  system, as the Hex package `tailwind` does: `;` on Windows (also in
  `beam.com`, where `os:type()` is `{unix, windows}`), else `:`. Before,
  `mix assets.deploy` stopped with an error of `open_port/2`.
- `mix tailwind.install`, and the install of `install_and_run/2`, write
  `tailwind.com` to the priv directory of the application `tailwind`
  (`install_path/0`), not to `_build/`. A release then has the file, and
  a program can build CSS at run time (in BEAM.com, from `/zip`).
  `bin_path/0` still finds a file in `_build/`.
- The start of the application does not check the version of
  `tailwind.com` or of the NIF: the first run does. A program that builds
  no CSS loads no NIF and runs no program.

### Added

- `tailwind::scanner`: the scanner of Tailwind CSS, for a program that
  finds the candidates of a project.
- The NIF: the program in the Erlang VM, with no process and no
  download (the crate `native/nif` and the module `tailwind_nif`,
  `docs/NIF.md`). When a builder puts the NIF in `priv/native`,
  `tailwind:run/2` and `Tailwind.run/2` use it, and the project does not
  change. The configuration `nif` selects the NIF or the file.
- The native NIF objects for BEAM.com: `libtailwind_nif.x86_64.o` and
  `libtailwind_nif.aarch64.o` with each release (`make nif`,
  `scripts/nif.sh`). `beam.com` loads them with no compiler, and they
  have each version of Tailwind CSS 4: the store of the packages is in
  the objects (`TAILWIND_COM_EMBED_PACKAGES`). CI loads them in
  `beam.com` on x86_64 and aarch64 (`tests/nif_object.erl`).
- `tailwind.com` in the priv directory of the application `tailwind` is
  the default path when the configuration has no `path`, and a path in
  `/zip` (the zip of a program of BEAM.com) starts with no shell.
- Each release has the file `SOURCE` (`git URL commit=SHA tag=TAG`): the
  package of its NIF objects, for the directory of NIF files of a custom
  build of BEAM.com (`docs/NIF.md`).
- `tailwind::cli::run_with`: the program as a part of another program,
  with its own working directory, environment, output and stop.

### Changed

- A build with no `--watch` does not free the compiler at its end: the
  system frees the memory of the process. A build of a large project
  (1000 files, about 5600 classes) takes 28% less time. A watch, and a run
  of `run_with` (the NIF), free the compiler as before.
- The program `tailwind` of `cargo build` uses mimalloc 2 as its
  allocator (the feature `mimalloc`, a default feature): a build spends
  about a third of its time in `malloc` and `free`. A large build takes
  about 25% less time, and its peak memory is 91 MB, not 75 MB. The
  library does not set an allocator, and `wasm32-wasip1` does not use
  mimalloc.
- The file `tailwind.com` uses mimalloc 2 too (the feature
  `ape-mimalloc`): `scripts/build.sh` compiles it with cosmocc, from the C
  source of the crate `libmimalloc-sys` of `Cargo.lock`. With the change
  above, a large build takes 124 ms, not 232 ms, and the peak memory does
  not change. The file is 370 KB larger. The native NIF objects keep the
  allocator of `beam.com`.
- The maps of the compiler use FxHash (the crate `rustc-hash`, which the
  scanner uses too), not the SipHash of `std` (`src/hash.rs`). A large
  build has 7% fewer instructions.
- A build with no `--watch` takes the compiled nodes of each candidate
  out of the cache of the compiler, with no copy
  (`DesignSystem::set_keep_compiled`). A watch keeps the cache for its
  next build, as before. The sort of the rules sorts their indexes, not
  the rules. A large build has 7% fewer instructions. With the changes
  above, a large build of the program of `cargo build` takes 83 ms, not
  189 ms.
- The optimizer keeps the `color-mix(…)` fallback of each value for its
  run: in a large project, many declarations have the same value. A large
  build has 4% fewer instructions.
- The scanner of Tailwind CSS is a copy in the repository
  (`native/scanner`), not a dependency on the git repository of Tailwind
  CSS: three crates with no change in their code, and manifests of ours
  with the names `tailwind-scanner`,
  `tailwind-classification-macros` and `tailwind-ignore`.
  crates.io takes a crate only when each dependency is on crates.io.
  `scripts/scanner.sh` makes the copy, and CI compares it with the tag of
  Tailwind CSS and runs its tests.
- The crate of the compiler, `tailwind`, is ready for crates.io. Its
  README is the documentation of the crate, with examples that
  `cargo test` runs.
- The C code (QuickJS and the code for Cosmopolitan) is in
  `native/tailwind/c_src`, in the crate: a crate of crates.io builds only
  from its own files.
- CI checks that crates.io takes the four crates
  (`cargo publish --dry-run`), and the release job publishes them when the
  repository has the secret `CARGO_REGISTRY_TOKEN`.

## 0.2.0 - 2026-11-12

Each version of Tailwind CSS 4, from 4.0.0 to 4.3.5, in one file.

### Added

- A second engine, the script engine: the official compiler of the npm
  package `tailwindcss` of a version (`dist/lib.js`), with no change, in
  the QuickJS of the file. The Rust compiler (the native engine) stays
  the default engine.
- `tailwind.com` has the package of each stable version of Tailwind
  CSS 4, from 4.0.0 to 4.3.5 (48 versions), in its ZIP directory
  `tailwindcss/`: an APE file is also a ZIP file. The file has the
  newest package in full, and each older version as a patch file with
  the differences from the next newer version (`tools/twpatch` makes
  them). All 48 versions are 0.37 MB of the file.
- `TAILWINDCSS_VERSION` selects the version, `TAILWIND_COM_ENGINE` the
  engine (`native` or `script`), and `TAILWIND_COM_PACKAGES` a
  directory with more packages. `--version` shows the selection.
- The file `PACKAGES` lists the versions with the SHA-256 of each
  package. `scripts/packages.sh` gets the packages, and `--record` adds
  a version. A new version of Tailwind CSS needs one line in
  `PACKAGES`, and no change of the code.
- The Erlang and Elixir modules give `version` of the configuration to
  the program, so each version of Tailwind CSS 4 works in a Phoenix
  project (for example `"4.3.0"` of Phoenix 1.8.9). The default URL of
  the download has the version of this package (`$program`).
- For the script engine, the JavaScript runtime has the sort algorithm
  of V8 and `Intl.ListFormat` for English, so that the compiler gives
  the same output as in Node.js. A plugin gets them too.
- With the script engine, a plugin or a config can load the module
  `tailwindcss` itself.
- The Rust compiler can compile as an older version: a behavior that
  changed in a release is a switch (`src/switches.rs`). It has the 195
  switches of 4.0.1 to 4.3.5, so each version from 4.0.0 to 4.3.5 uses
  the Rust compiler.
  `docs/SWITCHES.md` has the procedure, and the list of the changes of
  Tailwind CSS for each release.
- The site on GitHub Pages (`site/`, `docs/SITE.md`): `tailwind.wasm` in
  the browser, with each version. It shows the markup, the input CSS,
  the output CSS and a preview. It compares two versions, or the Rust
  compiler with the official compiler of one version, and opens the
  recorded case of each switch. A WASI host in JavaScript
  (`site/wasi.mjs`) runs the program with a file system in memory. The
  site deploys from `main`, and a viewer can also select the
  `tailwind.wasm` of a release.
- The program has the plugin layer of Tailwind CSS (the JavaScript for
  `@plugin` and `@config`) of each version that the Rust compiler
  serves, made from the source of that version with no change
  (`scripts/compat.sh`). An older version is a patch file in the program
  (170 KB in all). So a stylesheet with plugins as an older version uses
  the Rust compiler too: a new Phoenix project has Tailwind CSS 4.3.3.
  `tests/fuzz/compare.mjs` compares given cases with the official
  compiler of a version.
- `docs/VERSIONS.md`.
- Tests of the script engine: the 150 recorded projects with the
  package of 4.3.5, each version of `PACKAGES` on each system
  (`tests/versions.sh`), the sort and `Intl.ListFormat` against V8
  (`tests/v8.mjs`), and the fuzzer against the official compiler of
  five versions in Node.js.
- `tests/bench/versions.mjs`: the speed of each engine for older
  versions of Tailwind CSS (`docs/BENCHMARKS.md`).

### Changed

- The version of tailwind.com is not paired with a version of Tailwind
  CSS: one file has each version. The Erlang and Elixir modules and the
  npm package download from the release of their own version. A release
  does not make a second release with the version of Tailwind CSS (for
  example `v4.3.5`) any more. The release `v4.3.5` keeps the files of
  0.1.0 for the modules of 0.1.0.
- `version:` in the configuration of the Hex package can be each
  version of Tailwind CSS 4 from 4.0.0 to 4.3.5. Before, it was only
  `4.3.5`.
- `Intl.ListFormat` for English works in plugins.

### Fixed

- The order of rules with variants could be different from the official
  program in a project with a JavaScript config that has `screens` with
  `min` and `max`: the sort of the variants now uses the algorithm of
  V8. The fuzzer in CI found it.

### Known limits

- For another version than 4.3.5, the scanner and the optimizer are the
  code of this program (the versions of 4.3.5). With `--minify` or
  `--optimize`, the text can be different from the official program of
  that version in small details. `--map` needs Tailwind CSS 4.1.6 or
  later.
- The script engine is slower than the native engine: about 80 ms in
  place of 12 ms for a Phoenix project, and about 1.45 s in place of
  0.22 s for a project with 5600 different classes. With no setting, it
  is only for a version that is newer than the port.
- Tailwind CSS 3 does not work.

## 0.1.0 - 2026-11-11

The first version, with Tailwind CSS 4.3.5.

### Added

#### The compiler

- A Tailwind CSS 4 compiler in Rust that follows Tailwind CSS 4.3.5 file
  by file:
  - the CSS parser and printer;
  - the theme;
  - the candidate, value and selector parsers;
  - all utilities and variants;
  - `@utility`, `@apply`, `@import`, `@source`, `@theme`, `@variant`
    and `@custom-variant`;
  - the compatibility layer;
  - the last pass on the tree.
- The scanner of source files is the Rust crate of Tailwind CSS
  (`tailwindcss-oxide`). `--minify` and `--optimize` use Lightning CSS
  with the options of the official program.
- Source maps: the locations of the parser, and the map of the compiler
  and of the optimizer.
- JavaScript plugins and configs (`@plugin`, `@config`) with QuickJS
  (a git submodule of the official QuickJS) and the plugin layer of
  Tailwind CSS:
  - CommonJS, ES modules, JSON, and TypeScript (the types are removed
    with Sucrase);
  - `require` and `import` with `node_modules`, `exports`, `imports`
    and `NODE_PATH`;
  - a part of Node.js;
  - `@tailwindcss/forms`, `@tailwindcss/typography` and
    `@tailwindcss/aspect-ratio` in the file, as in the standalone
    program.
- An engine error has the V8 text of the official program for the
  common errors.

#### The programs

- `tailwind.com`: one Actually Portable Executable (Cosmopolitan Libc)
  for Linux, macOS, Windows, FreeBSD, NetBSD and OpenBSD, on x86_64 and
  aarch64. It has the options of the official `tailwindcss` program:
  `--input`, `--output`, `--watch`, `--poll`, `--minify`, `--optimize`,
  `--map`, `--cwd` and `--silent`.
- In `tailwind.com`, `--watch` uses the file events of each system:
  inotify, kqueue and `ReadDirectoryChangesW`. It also has an interval for the changes that
  have no event. The end of stdin stops the watch, and
  `--watch=always` keeps it.
- `tailwind.wasm`: the same program for WASI (`wasm32-wasip1`), with
  JavaScript plugins.

#### The packages

- The Erlang and Elixir modules (`tailwind`, `Tailwind`, and the tasks
  `mix tailwind` and `mix tailwind.install`). They replace the Hex
  package `tailwind` in a Phoenix project, with the same configuration.
- The npm package `tailwind.com`:
  - `npx tailwind.com` downloads the file of its release and checks
    its SHA-256;
  - `run` and `runWasm` let a program in Node.js run the compiler;
  - `runWasm` runs it in the same process.
- The release:
  - a tag for the package version, and a tag that moves for the
    Tailwind CSS version;
  - the npm package, also on GitHub Packages as
    `@sntran/tailwind.com`.

#### The tests and the tools

- The tests compare the output with the recorded output of the official
  compiler and programs:
  - the unit tests of Tailwind CSS;
  - 75,082 candidates;
  - source maps;
  - 20 projects of the command line program, 29 projects with `--map`,
    and 100 projects with JavaScript plugins (two Phoenix 1.8 layouts
    with daisyUI and heroicons).
  `docs/COMPATIBILITY.md` has the results.
- Property tests of the full compiler, and a TLA+ model of the watch
  (`specs/Watch.tla`) that TLC checks in CI.
- CI on Linux (x86_64 and aarch64), macOS (arm64 and x86_64), Windows,
  FreeBSD, NetBSD and OpenBSD 7.3. CI also checks the official
  standalone program against the records.
- A live differential fuzzer (`tests/fuzz/`). It compares the compiler
  with the official Tailwind CSS on random stylesheets, classes,
  projects, plugins and configs, and also on real plugins. It runs for
  each push and each night. The cases that it found stay in
  `tests/upstream/fuzz-found.jsonl`.
- Benchmarks against the official npm CLI and standalone program
  (`tests/bench/`, `docs/BENCHMARKS.md`), in CI on Linux and macOS.
- `scripts/sync-upstream.sh` and a weekly workflow for a new version of
  Tailwind CSS.
- Remote SSH to a CI runner (also a BSD VM), with a bridge in Rust
  (`tools/ws-proxy`), to test on Windows and the BSDs.

### Fixed

These faults were found and fixed before this first version:

- The watch could build from an input file that a program was still
  writing (a shell redirect empties the file first). The output was
  then wrong for a short time, and the scan of a FIFO in the directory
  did not end. The watch now waits until the input stays the same for
  10 ms.
- An interrupted read of stdin stopped the watch.
- On an error, daisyUI printed its name two times: the second compile
  for the position of a syntax error ran the plugins again.

### Known limits

- `version:` in the configuration of the Hex package must be `4.3.5`.
  There is no release for other versions of Tailwind CSS.
- `canonicalize`, the second command of the official program, is not
  there.
- The other modules of Node.js (`crypto`, `stream`, ...), `Intl`,
  `Temporal`, and `.tsx` files do not work in plugins.
- NetBSD and OpenBSD need the APE loader of cosmocc. Cosmopolitan
  supports OpenBSD 7.3 or earlier.
- Node.js 22.23 can stop with SIGSEGV when `tailwind.wasm` minifies a
  large output. Use Node.js 24 or later for `runWasm`.

See `docs/COMPATIBILITY.md` and `docs/ROADMAP.md`.
