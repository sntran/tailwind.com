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
//!
//! For each build with QuickJS, this script also makes the bytecode of
//! `assets/compat.js` (`compat_bytecode`).

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

/// Write `embedded_packages.rs`: the files of the store of packages in
/// the program (`EMBEDDED` of `src/cli/engine.rs`). The variable
/// TAILWIND_COM_EMBED_PACKAGES names a store (a directory: the newest
/// package in full, and each older version as a patch file).
/// `scripts/nif.sh` gives it, for the native NIF objects of BEAM.com.
/// With no variable, the list is empty.
fn embedded_packages() {
    use std::fmt::Write;
    println!("cargo:rerun-if-env-changed=TAILWIND_COM_EMBED_PACKAGES");
    let mut text = String::from("static EMBEDDED: &[(&str, &[u8])] = &[\n");
    if let Some(store) = std::env::var_os("TAILWIND_COM_EMBED_PACKAGES").filter(|value| !value.is_empty()) {
        let store = std::path::PathBuf::from(store);
        let mut files = Vec::new();
        let mut directories = vec![store.clone()];
        while let Some(directory) = directories.pop() {
            println!("cargo:rerun-if-changed={}", directory.display());
            for entry in std::fs::read_dir(&directory).unwrap_or_else(|error| panic!("{}: {error}", directory.display())).flatten() {
                let path = entry.path();
                if path.is_dir() {
                    directories.push(path);
                } else {
                    files.push(path);
                }
            }
        }
        files.sort();
        for path in files {
            let name: Vec<String> =
                path.strip_prefix(&store).unwrap().components().map(|part| part.as_os_str().to_string_lossy().into_owned()).collect();
            writeln!(text, "    ({:?}, include_bytes!({:?})),", name.join("/"), path.display().to_string()).unwrap();
        }
    }
    text.push_str("];\n");
    let out = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap()).join("embedded_packages.rs");
    std::fs::write(out, text).unwrap();
}

/// Write `compat.bc`: the bytecode of `assets/compat.js`, the plugin layer
/// of the version of the port (`src/compat/js/bundles.rs`). The program
/// then does not compile its text at each start. `c_src/bytecode.c` and
/// QuickJS compile for the computer of the build, and the tool writes the
/// bytecode. The file is empty for a build with no QuickJS, and when the
/// computer of the build cannot make it (a warning): the program then
/// compiles the text, with the same result.
fn compat_bytecode() {
    use std::path::PathBuf;
    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let out = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    let output = out.join("compat.bc");
    let c_src = manifest.join("c_src");
    let quickjs = c_src.join("quickjs");
    let input = manifest.join("assets/compat.js");
    let sources = [
        c_src.join("bytecode.c"),
        quickjs.join("quickjs.c"),
        quickjs.join("libregexp.c"),
        quickjs.join("libunicode.c"),
        quickjs.join("cutils.c"),
        quickjs.join("dtoa.c"),
    ];
    for file in sources.iter().chain([&input, &quickjs.join("quickjs.h"), &quickjs.join("VERSION")]) {
        println!("cargo:rerun-if-changed={}", file.display());
    }

    let wasi = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("wasi");
    let quickjs_in_program = std::env::var_os("CARGO_FEATURE_JS").is_some()
        && std::env::var_os("CARGO_FEATURE_HOST_JS").is_none()
        && (!wasi || std::env::var_os("WASI_SDK_PATH").is_some());
    if !quickjs_in_program {
        std::fs::write(&output, b"").unwrap();
        return;
    }
    if let Err(message) = make_bytecode(&c_src, &sources, &input, &out, &output) {
        println!("cargo:warning=No bytecode of assets/compat.js: {message}. The program compiles its text at each start.");
        std::fs::write(&output, b"").unwrap();
    }
}

/// Compile `c_src/bytecode.c` and QuickJS for the computer of the build,
/// and write the bytecode of `input` to `output`.
#[cfg(feature = "js")]
fn make_bytecode(
    c_src: &std::path::Path,
    sources: &[std::path::PathBuf],
    input: &std::path::Path,
    out: &std::path::Path,
    output: &std::path::Path,
) -> Result<(), String> {
    let host = std::env::var("HOST").map_err(|error| error.to_string())?;
    let compiler = cc::Build::new()
        .target(&host)
        .host(&host)
        .opt_level(0)
        .cargo_metadata(false)
        .warnings(false)
        .try_get_compiler()
        .map_err(|error| error.to_string())?;
    if compiler.is_like_msvc() {
        return Err("the C compiler of this computer is MSVC".into());
    }
    let version = std::fs::read_to_string(c_src.join("quickjs/VERSION")).map_err(|error| error.to_string())?;
    let tool = out.join("compat-bytecode-tool");
    let status = compiler
        .to_command()
        .args(["-w", "-D_GNU_SOURCE", "-fwrapv", "-funsigned-char"])
        .arg(format!("-DCONFIG_VERSION=\"{}\"", version.trim()))
        .arg("-I")
        .arg(c_src)
        .arg("-o")
        .arg(&tool)
        .args(sources)
        .args(["-lm", "-lpthread"])
        .status()
        .map_err(|error| format!("{}: {error}", compiler.path().display()))?;
    if !status.success() {
        return Err(format!("the compile of c_src/bytecode.c failed ({status})"));
    }
    let status = std::process::Command::new(&tool)
        .arg(input)
        .arg("tailwindcss/compat.js")
        .arg(output)
        .status()
        .map_err(|error| format!("{}: {error}", tool.display()))?;
    if !status.success() {
        return Err(format!("c_src/bytecode.c failed ({status})"));
    }
    Ok(())
}

#[cfg(not(feature = "js"))]
fn make_bytecode(
    _: &std::path::Path,
    _: &[std::path::PathBuf],
    _: &std::path::Path,
    _: &std::path::Path,
    _: &std::path::Path,
) -> Result<(), String> {
    Err("the build has no feature js".into())
}

fn main() {
    compat_patches();
    embedded_packages();
    compat_bytecode();
    println!("cargo:rustc-check-cfg=cfg(tw_js)");
    println!("cargo:rustc-check-cfg=cfg(tw_host_js)");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-env-changed=WASI_SDK_PATH");

    if std::env::var_os("CARGO_FEATURE_HOST_JS").is_some() {
        // The engine of the host: no C code. The program exports the
        // functions that the host calls.
        println!("cargo:rustc-cfg=tw_js");
        println!("cargo:rustc-cfg=tw_host_js");
        for name in ["tw_host_alloc", "tw_host_free", "tw_host_call", "tw_host_release"] {
            println!("cargo:rustc-link-arg-bins=--export={name}");
        }
    } else if std::env::var_os("CARGO_FEATURE_APE").is_some() {
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
