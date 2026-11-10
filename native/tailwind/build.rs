//! The build of the C code of the feature `js`: QuickJS (`c_src/quickjs`)
//! and its API for Rust (`c_src/js.c`).
//!
//! The cfg `tw_js` is set when the program has the JavaScript engine:
//!
//! - A native build compiles the C files with the C compiler of the system.
//! - With the feature `ape`, `scripts/build.sh` compiles the C files with
//!   cosmocc and links them. This script compiles nothing.
//! - For `wasm32-wasip1`, this script uses the clang of a wasi-sdk
//!   (`WASI_SDK_PATH`). With no wasi-sdk, the program has no JavaScript.

fn main() {
    println!("cargo:rustc-check-cfg=cfg(tw_js)");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=WASI_SDK_PATH");

    if std::env::var_os("CARGO_FEATURE_APE").is_some() {
        if std::env::var_os("CARGO_FEATURE_JS").is_some() {
            println!("cargo:rustc-cfg=tw_js");
        }
    } else {
        #[cfg(feature = "js")]
        compile();
    }
}

#[cfg(feature = "js")]
fn compile() {
    use std::path::PathBuf;

    let c_src = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("../../c_src");
    let quickjs = c_src.join("quickjs");
    let version = std::fs::read_to_string(quickjs.join("VERSION")).unwrap();
    let files = [
        quickjs.join("quickjs.c"),
        quickjs.join("libregexp.c"),
        quickjs.join("libunicode.c"),
        quickjs.join("cutils.c"),
        quickjs.join("dtoa.c"),
        c_src.join("js.c"),
    ];
    for file in &files {
        println!("cargo:rerun-if-changed={}", file.display());
    }
    println!("cargo:rerun-if-changed={}", c_src.join("js.h").display());

    let mut build = cc::Build::new();
    build
        .files(&files)
        .define("_GNU_SOURCE", None)
        .define("CONFIG_VERSION", Some(format!("\"{}\"", version.trim()).as_str()))
        .flag_if_supported("-fwrapv")
        .flag_if_supported("-funsigned-char")
        .warnings(false);
    // The engine is slow without optimization, also in a debug build.
    if std::env::var("OPT_LEVEL").as_deref() == Ok("0") {
        build.opt_level(2);
    }

    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("wasi") {
        let Some(sdk) = std::env::var_os("WASI_SDK_PATH").map(PathBuf::from) else {
            println!("cargo:warning=WASI_SDK_PATH is not set: this build has no JavaScript (@plugin, @config)");
            return;
        };
        build
            .compiler(sdk.join("bin/clang"))
            .archiver(sdk.join("bin/llvm-ar"))
            .flag(format!("--sysroot={}", sdk.join("share/wasi-sysroot").display()))
            .target("wasm32-wasip1");
        wasi_defines(&mut build);
    }

    build.compile("tw_quickjs");
    println!("cargo:rustc-cfg=tw_js");
}

/// What QuickJS needs on WASI.
#[cfg(feature = "js")]
fn wasi_defines(build: &mut cc::Build) {
    // `__EMSCRIPTEN__` selects the code of QuickJS for WebAssembly: no
    // threads (Atomics), no stack check with the address of the stack, and
    // no `malloc_usable_size`.
    build.define("__EMSCRIPTEN__", None);
    build.define("_WASI_EMULATED_SIGNAL", None);
}
