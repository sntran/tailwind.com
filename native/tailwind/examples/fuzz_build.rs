//! The builder of the differential fuzzer (`tests/fuzz/fuzz.mjs`).
//!
//! Usage: fuzz_build ASSETS_DIR < cases.jsonl > results.jsonl
//!
//! Each line of the input is a case `{"css": "...", "candidates": [...],
//! "base": "DIR"}`. `base` is the directory of the files of `@plugin` and
//! `@config` (optional). The JavaScript of a plugin reads the file system,
//! and its `console` messages are not printed.
//! For each one, a new compiler compiles the stylesheet and builds the
//! candidates, and the output line is `{"css": "..."}` or
//! `{"error": "..."}`, in the sequence of the input. An `@import` of
//! `tailwindcss` or of `tailwindcss/NAME` gets the file of ASSETS_DIR, as
//! the npm package gives it. A panic is an output line too
//! (`{"panic": "..."}`), so one case does not stop the run.
//!
//! With `FUZZ_ENGINE=script` in the environment, the script engine builds
//! each case: the official compiler of the package in ASSETS_DIR
//! (`dist/lib.js`), in the JavaScript engine of the program. Then the
//! fuzzer compares the two JavaScript engines on one compiler, for the
//! version of Tailwind CSS that is in ASSETS_DIR.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use tailwind::cli::modules::FileSystem;
use tailwind::{compile, CompileOptions, Error, FileKind, FileStat, ModuleLoader, Stylesheet};

/// The file system of the computer, with no output for `console`: the
/// output of this program is the results.
#[derive(Default)]
struct Quiet(FileSystem);

impl ModuleLoader for Quiet {
    fn read(&self, path: &str) -> Option<String> {
        self.0.read(path)
    }
    fn read_bytes(&self, path: &str) -> Option<Vec<u8>> {
        self.0.read_bytes(path)
    }
    fn stat(&self, path: &str) -> Option<FileStat> {
        self.0.stat(path)
    }
    fn is_symlink(&self, path: &str) -> bool {
        self.0.is_symlink(path)
    }
    fn real_path(&self, path: &str) -> Option<String> {
        self.0.real_path(path)
    }
    fn read_dir(&self, path: &str) -> Option<Vec<(String, FileKind)>> {
        self.0.read_dir(path)
    }
    fn node_path(&self) -> Vec<String> {
        self.0.node_path()
    }
    fn cwd(&self) -> String {
        self.0.cwd()
    }
}

/// The npm package `tailwindcss` in ASSETS_DIR.
struct NpmPackage {
    directory: PathBuf,
    version: String,
}

impl tailwind::script::Package for NpmPackage {
    fn version(&self) -> &str {
        &self.version
    }
    fn read(&self, name: &str) -> Option<String> {
        std::fs::read_to_string(self.directory.join(name)).ok()
    }
}

fn one_script(package: &Rc<NpmPackage>, css: &str, candidates: &[String], base: &str) -> serde_json::Value {
    let assets = package.directory.clone();
    let loader = |id: &str, base: &str| load(&assets, id, base);
    let options = tailwind::script::ScriptOptions {
        base: base.to_string(),
        from: None,
        load_stylesheet: Some(&loader),
        module_loader: Rc::new(Quiet::default()),
    };
    match tailwind::script::compile(css, options, package.clone()).and_then(|mut compiler| compiler.try_build(candidates)) {
        // The compiler of a package always prints the license comment.
        // The fuzzer compares the CSS with no such line.
        Ok(css) => {
            let css = match css.split_once('\n') {
                Some((first, rest)) if first.starts_with("/*! tailwindcss v") => rest.to_string(),
                _ => css,
            };
            serde_json::json!({ "css": css })
        }
        Err(e) => serde_json::json!({ "error": e.message }),
    }
}

fn load(assets: &Path, id: &str, _base: &str) -> tailwind::Result<Stylesheet> {
    let name = if id == "tailwindcss" {
        "index"
    } else if let Some(rest) = id.strip_prefix("tailwindcss/") {
        rest
    } else if let Some(rest) = id.strip_prefix("./") {
        rest
    } else {
        return Err(Error::new(format!("Could not resolve '{id}'")));
    };
    let name = name.strip_suffix(".css").unwrap_or(name);
    let path = assets.join(format!("{name}.css"));
    let content = std::fs::read_to_string(&path).map_err(|_| Error::new(format!("Could not resolve '{id}'")))?;
    Ok(Stylesheet {
        path: path.to_string_lossy().into_owned(),
        base: assets.to_string_lossy().into_owned(),
        content,
    })
}

fn one(assets: &Path, css: &str, candidates: &[String], base: &str) -> serde_json::Value {
    let loader = |id: &str, base: &str| load(assets, id, base);
    let options = CompileOptions {
        base: base.to_string(),
        load_stylesheet: Some(&loader),
        module_loader: Some(Rc::new(Quiet::default())),
        test_mode: true,
        ..CompileOptions::default()
    };
    match compile(css, options).and_then(|mut compiler| compiler.try_build(candidates)) {
        Ok(css) => serde_json::json!({ "css": css }),
        Err(e) => serde_json::json!({ "error": e.message }),
    }
}

fn main() {
    let assets = PathBuf::from(std::env::args().nth(1).expect("usage: fuzz_build ASSETS_DIR < cases.jsonl"));
    let script = std::env::var("FUZZ_ENGINE").is_ok_and(|engine| engine == "script").then(|| {
        let text = std::fs::read_to_string(assets.join("package.json")).expect("package.json in ASSETS_DIR");
        let json: serde_json::Value = serde_json::from_str(&text).expect("package.json");
        Rc::new(NpmPackage { directory: assets.clone(), version: json["version"].as_str().unwrap_or_default().to_string() })
    });
    // A panic of a case is a result, not a message on stderr.
    std::panic::set_hook(Box::new(|_| {}));
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    for line in std::io::stdin().lock().lines() {
        let line = line.expect("stdin");
        if line.trim().is_empty() {
            continue;
        }
        let case: serde_json::Value = serde_json::from_str(&line).expect("a JSON case");
        let css = case["css"].as_str().unwrap_or("").to_string();
        let candidates: Vec<String> = case["candidates"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        let base = case["base"].as_str().unwrap_or("").to_string();
        let build = std::panic::AssertUnwindSafe(|| match &script {
            Some(package) => one_script(package, &css, &candidates, &base),
            None => one(&assets, &css, &candidates, &base),
        });
        let result = std::panic::catch_unwind(build).unwrap_or_else(|p| {
            let text = p
                .downcast_ref::<String>()
                .cloned()
                .or_else(|| p.downcast_ref::<&str>().map(|s| s.to_string()))
                .unwrap_or_else(|| "panic".into());
            serde_json::json!({ "panic": text })
        });
        writeln!(out, "{result}").expect("stdout");
        out.flush().expect("stdout");
    }
}
