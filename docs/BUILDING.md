# Build tailwind.com

## A program for this computer

You need Rust 1.85 or later.

```sh
cargo build --release      # target/release/tailwind
```

This is the same program as `tailwind.com`, for one system. Use it for
development and for the tests.

## tailwind.com

You need, on Linux x86_64:

- Rust (stable), with the target `x86_64-unknown-linux-musl`;
- a nightly Rust with `rust-src` (default `nightly-2026-11-01`), for the
  aarch64 half of the file;
- `curl` and `unzip`, for the download of `cosmocc` 4.0.2 (the script
  checks its SHA-256).

```sh
rustup target add x86_64-unknown-linux-musl
rustup toolchain install nightly-2026-11-01 --profile minimal --component rust-src
make                       # build/tailwind.com
```

`scripts/build.sh` has the steps, and its header names the environment
variables (`COSMOCC`, `COSMOCC_VERSION`, `RUST_NIGHTLY`, `CPUS`).
`CPUS=x86_64 sh scripts/build.sh` makes a file for x86_64 only, with no
nightly Rust. See "The file tailwind.com" in
[`INTERNALS.md`](INTERNALS.md) for how the link works.

## The WASI program

```sh
rustup target add wasm32-wasip1
make wasm                  # build/tailwind.wasm
```

## The library only

The compiler is a library with no command line program, no scanner and
no optimizer:

```sh
cargo build -p tailwind --no-default-features
```

```rust
let mut compiler = tailwind::compile(css, tailwind::CompileOptions {
    base: String::new(),
    polyfills: tailwind::POLYFILLS_ALL,
    load_stylesheet: Some(&|id, base| { /* give the stylesheet of an @import */ }),
    test_mode: false,
})?;
let css = compiler.build(&["flex".to_string(), "md:p-4".to_string()]);
```

## The Erlang and Elixir modules

`mix compile` builds the two (Elixir 1.14 or later). `rebar3 compile`
builds the Erlang module only. They do not build the Rust code: they
download or find `tailwind.com`.

## A release

A tag `v*` starts the release job of CI. It publishes `tailwind.com`,
`tailwind.wasm` and `SHA256SUMS`, with a signed provenance. The tag has
the version of Tailwind CSS that the compiler follows (for example
`v4.3.5`), so the `version` of a Phoenix configuration gives the URL of
the download.
