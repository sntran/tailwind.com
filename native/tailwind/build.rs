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

/// Write `compat_patches.rs`: the table of the patch files of
/// `assets/compat` (the plugin layer of each older version that the Rust
/// compiler serves: `src/compat/js/bundles.rs`).
fn compat_patches() {
    use std::fmt::Write;
    let directory = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("assets/compat");
    println!("cargo:rerun-if-changed={}", directory.display());
    let mut names: Vec<String> = std::fs::read_dir(&directory)
        .map(|entries| entries.flatten().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect())
        .unwrap_or_default();
    names.retain(|name| name.ends_with(".patch"));
    names.sort();
    let mut text = String::from("pub static COMPAT_PATCHES: &[(&str, &[u8])] = &[\n");
    for name in names {
        let version = name.trim_end_matches(".patch");
        let path = directory.join(&name);
        writeln!(text, "    ({version:?}, include_bytes!({:?})),", path.display().to_string()).unwrap();
    }
    text.push_str("];\n");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("compat_patches.rs");
    std::fs::write(out, text).unwrap();
}

fn main() {
    compat_patches();
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

    let c_src = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("c_src");
    let quickjs = c_src.join("quickjs");
    if !quickjs.join("quickjs.c").is_file() {
        panic!("c_src/quickjs is empty: run `git submodule update --init`");
    }
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
            // quickjs.c uses `malloc_usable_size`, and includes <malloc.h>
            // only for the systems that it knows.
            .flag("-include")
            .flag("malloc.h")
            // An empty <setjmp.h>: see c_src/wasi/setjmp.h.
            .include(c_src.join("wasi"));
    }

    build.compile("tw_quickjs");
    println!("cargo:rustc-cfg=tw_js");
}
