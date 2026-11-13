# Build tailwind.com

## The source

QuickJS is a git submodule (`native/tailwind/c_src/quickjs`, see
[`native/tailwind/c_src/QUICKJS.md`](../native/tailwind/c_src/QUICKJS.md)):

```sh
git clone --recursive https://github.com/sntran/tailwind.com
# or, in a clone: git submodule update --init
```

## A program for this computer

You need Rust 1.85 or later, and a C compiler (`cc`: GCC or Clang) for
the JavaScript engine (QuickJS, `native/tailwind/c_src/quickjs`). The crate `cc` finds
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
SHA-256 check). The tool `tools/twpatch` (a small Rust program for the
computer of the build) makes a store from them: the newest package in
full, and a patch file for each older version. The store goes into the
ZIP directory `tailwindcss/` of `tailwind.com`. `PACKAGES=none make` builds a file with no packages,
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
`malloc_usable_size`), and an empty `setjmp.h` (`native/tailwind/c_src/wasi`), because
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

- `native/tailwind/c_src/quickjs` is the git submodule of QuickJS, with no change.
  `sh scripts/sync-quickjs.sh COMMIT_OR_TAG` moves it to another version
  (see `native/tailwind/c_src/QUICKJS.md`).
- `native/tailwind/assets/compat.js` is the plugin layer of Tailwind
  CSS with the parts of Node.js of `native/tailwind/js`, in one file.
  `native/tailwind/assets/compat/` has the plugin layer of each older
  version that the Rust compiler serves, as patch files
  ([`SWITCHES.md`](SWITCHES.md), "The plugin layer"). After a change of
  `native/tailwind/js`, or for a new version of Tailwind CSS, make them
  all again with Node.js, cargo and a clone of Tailwind CSS:

  ```sh
  (cd tests/upstream/tools && npm ci --legacy-peer-deps --ignore-scripts)   # esbuild, sucrase, the three plugins
  sh scripts/compat.sh /path/to/tailwindcss
  ```

  The script gets the tag of each version into the clone, and uses
  `tests/upstream/tools/build-compat-bundle.mjs` for one version.

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
download or find `tailwind.com`, or use the NIF that a builder puts in
`priv/native` (the crate `native/nif`, [`NIF.md`](NIF.md)).

## A release

The packages have one version of their own: `package.json`, `mix.exs`,
`src/tailwind.app.src`, and the Rust crates (`[workspace.package]` and
the three crates of the scanner in `[workspace.dependencies]` of
`Cargo.toml`). `sh scripts/sync-upstream.sh check` compares them. The
version of Tailwind CSS that the compiler follows is in `UPSTREAM`.
`tailwind.com --version` prints the two.

Before the tag, set the new version in these places, the `tag` of
the Phoenix example in `README.md`, and the title of the section
"Unreleased" of `CHANGELOG.md`. A tag `vX.Y.Z` with the version of the
packages starts the release job of CI. The job:

1. Publishes the GitHub release `vX.Y.Z` with `tailwind.com`,
   `tailwind.wasm`, the file of the npm package and `SHA256SUMS`, with a
   signed provenance. The Erlang and Elixir modules and the npm package
   of version X.Y.Z download from this release. One file has each version
   of Tailwind CSS, so the version of tailwind.com is not the version of
   Tailwind CSS.
2. Publishes the npm package `tailwind.com` to npm, when the repository
   has the secret `NPM_TOKEN`.
3. Publishes the same package to GitHub Packages, with the name
   `@OWNER/tailwind.com` (GitHub Packages accepts only a name with the
   scope of the owner).
4. Publishes the four crates to crates.io, when the repository has the
   secret `CARGO_REGISTRY_TOKEN` (see "The crates" below).
5. Starts `.github/workflows/pages.yml`, so the site has the program of
   the new release ([`SITE.md`](SITE.md)).

The release `v4.3.5` has the files of 0.1.0. The modules of 0.1.0
downloaded from the release with the version of Tailwind CSS, and they
can use only 4.3.5. That release stays as it is.

## The crates

The compiler is the crate `tailwind` on crates.io (`native/tailwind`,
with the library and the program `tailwind`). It
needs the three crates of the scanner (`native/scanner/README.md`):

| Crate | Directory |
|---|---|
| `tailwind` | `native/tailwind` |
| `tailwind-scanner` | `native/scanner/oxide` |
| `tailwind-classification-macros` | `native/scanner/classification-macros` |
| `tailwind-ignore` | `native/scanner/ignore` |

A crate of crates.io builds alone, from the files of its package. So
`native/tailwind` has the C code that `build.rs` compiles
(`native/tailwind/c_src`, with QuickJS as a git submodule), and
`include` in its manifest names the files of the package: not the tests,
which read the records of the repository. The README of the crate is
also its documentation (`src/lib.rs`), so `cargo test` runs its
examples.

The job `crates` of `.github/workflows/build.yml` runs

```sh
cargo publish --dry-run -p tailwind-classification-macros -p tailwind-ignore -p tailwind-scanner -p tailwind
```

for each push: Cargo makes each package, and builds it alone. The
release job publishes the four crates in the same order. The first
version of a new crate needs an API token of crates.io in the secret
`CARGO_REGISTRY_TOKEN`. After that, the trusted publishing of crates.io
can replace the token.
