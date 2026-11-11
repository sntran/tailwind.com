//! The builder of the differential fuzzer (`tests/fuzz/fuzz.mjs`).
//!
//! Usage: fuzz_build ASSETS_DIR < cases.jsonl > results.jsonl
//!
//! Each line of the input is a case `{"css": "...", "candidates": [...]}`.
//! For each one, a new compiler compiles the stylesheet and builds the
//! candidates, and the output line is `{"css": "..."}` or
//! `{"error": "..."}`, in the sequence of the input. An `@import` of
//! `tailwindcss` or of `tailwindcss/NAME` gets the file of ASSETS_DIR, as
//! the npm package gives it. A panic is an output line too
//! (`{"panic": "..."}`), so one case does not stop the run.

use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};
use tailwind::{compile, CompileOptions, Error, Stylesheet};

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

fn one(assets: &Path, css: &str, candidates: &[String]) -> serde_json::Value {
    let loader = |id: &str, base: &str| load(assets, id, base);
    let options = CompileOptions { load_stylesheet: Some(&loader), test_mode: true, ..CompileOptions::default() };
    match compile(css, options).and_then(|mut compiler| compiler.try_build(candidates)) {
        Ok(css) => serde_json::json!({ "css": css }),
        Err(e) => serde_json::json!({ "error": e.message }),
    }
}

fn main() {
    let assets = PathBuf::from(std::env::args().nth(1).expect("usage: fuzz_build ASSETS_DIR < cases.jsonl"));
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
        let result = std::panic::catch_unwind(|| one(&assets, &css, &candidates)).unwrap_or_else(|p| {
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
