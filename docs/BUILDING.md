# Build tailwind.com

## The source

QuickJS is a git submodule (`c_src/quickjs`, see
[`c_src/QUICKJS.md`](../c_src/QUICKJS.md)):

```sh
git clone --recursive https://github.com/sntran/tailwind.com
# or, in a clone: git submodule update --init
```

## A program for this computer

You need Rust 1.85 or later, and a C compiler (`cc`: GCC or Clang) for
the JavaScript engine (QuickJS, `c_src/quickjs`). The crate `cc` finds
the compiler, and `CC` selects another one.

```sh
cargo build --release      # target/release/tailwind
```

`cargo build --release --no-default-features --features cli` is not a
build without JavaScript: the feature `cli` has the feature `js`. See
"The library only" below for the features.

This is the same program as `tailwind.com`, for one system. Use it for
development and for the tests.

## tailwind.com

You need, on Linux x86_64:

- Rust (stable), with the target `x86_64-unknown-linux-musl`;
- a nightly Rust with `rust-src` (default `nightly-2026-11-01`), for the
  aarch64 half of the file;
- `curl` and `unzip`, for the download of `cosmocc` 4.0.2 (the script
  checks its SHA-256);
- `zip` and `tar`, for the packages of Tailwind CSS in the file.

```sh
rustup target add x86_64-unknown-linux-musl
rustup toolchain install nightly-2026-11-01 --profile minimal --component rust-src
make                       # build/tailwind.com
```

`cosmocc` also compiles QuickJS for each CPU: the C library of the file
is Cosmopolitan, so the C compiler of the system is not used here.

After the link, the build downloads the npm package `tailwindcss` of
each version of the file `PACKAGES` (`scripts/packages.sh`, with a
SHA-256 check) and adds its files to the ZIP directory `tailwindcss/`
of `tailwind.com`. `PACKAGES=none make` builds a file with no packages,
and `PACKAGES="4.2.4 4.3.0" make` a file with these versions only. See
[`VERSIONS.md`](VERSIONS.md).

`scripts/build.sh` has the steps, and its header names the environment
variables (`COSMOCC`, `COSMOCC_VERSION`, `RUST_NIGHTLY`, `CPUS`,
`PACKAGES`).
`CPUS=x86_64 sh scripts/build.sh` makes a file for x86_64 only, with no
nightly Rust. See "The file tailwind.com" in
[`INTERNALS.md`](INTERNALS.md) for how the link works.

## The WASI program

```sh
rustup target add wasm32-wasip1
WASI_SDK_PATH=/path/to/wasi-sdk make wasm     # build/tailwind.wasm
```

`WASI_SDK_PATH` is the directory of an unpacked
[wasi-sdk](https://github.com/WebAssembly/wasi-sdk/releases) (CI uses
version 27, and checks its SHA-256): its clang compiles QuickJS for
`wasm32-wasip1`. With no `WASI_SDK_PATH`, the build works and prints a
warning, and the program has no JavaScript: `@plugin` and `@config` are
then errors.

`native/tailwind/build.rs` has the two things that QuickJS needs on
WASI, with no change of its files: `-include malloc.h` (for
`malloc_usable_size`), and an empty `setjmp.h` (`c_src/wasi`), because
`dtoa.c` includes this header and uses nothing of it.

## The library only

The compiler is a library with no command line program, no scanner and
no optimizer:

```sh
cargo build -p tailwind --no-default-features                  # no JavaScript
cargo build -p tailwind --no-default-features --features js    # with @plugin and @config
```

```rust
let mut compiler = tailwind::compile(css, tailwind::CompileOptions {
    base: String::new(),
    polyfills: tailwind::POLYFILLS_ALL,
    load_stylesheet: Some(&|id, base| { /* give the stylesheet of an @import */ }),
    test_mode: false,
    // The files of the JavaScript modules of @plugin and @config: an
    // implementation of tailwind::ModuleLoader (read a file, the kind
    // of a path, the names in a directory). None: these rules are errors.
    module_loader: None,
})?;
let css = compiler.build(&["flex".to_string(), "md:p-4".to_string()]);
```

## The JavaScript parts

The repository has generated or copied parts. A build needs no Node.js
for them.

- `c_src/quickjs` is the git submodule of QuickJS, with no change.
  `sh scripts/sync-quickjs.sh COMMIT_OR_TAG` moves it to another version
  (see `c_src/QUICKJS.md`).
- `native/tailwind/assets/compat.js` is the plugin layer of Tailwind
  CSS with the parts of Node.js of `native/tailwind/js`, in one file.
  After a change of `native/tailwind/js`, or for a new version of
  Tailwind CSS, make it again with Node.js and a clone of Tailwind CSS:

  ```sh
  (cd tests/upstream/tools && npm install)      # esbuild, sucrase, the three plugins
  node tests/upstream/tools/build-compat-bundle.mjs /path/to/tailwindcss
  ```

- `native/tailwind/assets/typescript.js` (the TypeScript transform of
  Sucrase, with `native/tailwind/js/typescript-entry.ts`) and
  `native/tailwind/assets/plugins/` (`@tailwindcss/forms`,
  `@tailwindcss/typography`, `@tailwindcss/aspect-ratio`, each one with
  its dependencies) come from the packages of
  `tests/upstream/tools/package.json`:

  ```sh
  node tests/upstream/tools/build-plugin-bundles.mjs /path/to/tailwindcss
  ```

  The script stops when the version of a plugin is not the version of
  the lock file of that clone.

After a change of a part of Node.js in `native/tailwind/js`, add a case
to `native/tailwind/tests/fixtures/node_api_cases.cjs`, and record the
values of Node.js 22 again:

```sh
node tests/upstream/tools/gen-node-api.mjs
```

## The Erlang and Elixir modules

`mix compile` builds the two. They need Erlang/OTP 29 and Elixir 1.20
or later, the versions of BEAM.com: the Erlang module uses `-doc`
attributes and `maybe`. `rebar3 compile`
builds the Erlang module only. They do not build the Rust code: they
download or find `tailwind.com`.

## A release

The packages have one version of their own: `package.json`, `mix.exs`,
`src/tailwind.app.src` and the crate (`sh scripts/sync-upstream.sh check`
compares them). The version of Tailwind CSS that the compiler follows is
in `UPSTREAM`. `tailwind.com --version` prints the two.

A tag `vX.Y.Z` with the version of the packages starts the release job
of CI. The job:

1. Publishes the GitHub release `vX.Y.Z` with `tailwind.com`,
   `tailwind.wasm`, the file of the npm package and `SHA256SUMS`, with a
   signed provenance.
2. Makes the release with the version of Tailwind CSS again (for example
   `v4.3.5`), with the same files. The Erlang and Elixir modules download
   from this release, so the `version` of a Phoenix configuration gives
   the newest file for that version of Tailwind CSS.
3. Publishes the npm package `tailwind.com` to npm, when the repository
   has the secret `NPM_TOKEN`.
4. Publishes the same package to GitHub Packages, with the name
   `@OWNER/tailwind.com` (GitHub Packages accepts only a name with the
   scope of the owner).
