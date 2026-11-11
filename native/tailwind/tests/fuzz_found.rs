//! The cases that the differential fuzzer found (`tests/fuzz/fuzz.mjs`),
//! with the output of the official compiler: `tests/upstream/fuzz-found.jsonl`.
//!
//! Each line is `{"css", "candidates", "official": {"css"} or {"error"}}`,
//! the reduced case of a failure, and for a case with JavaScript `files`
//! (name to text) and `copy` (name to a file of this repository). A new
//! compiler compiles each one in a directory with these files, and the
//! output must be the same as the output of the official compiler.

use std::path::{Path, PathBuf};
use std::rc::Rc;
use tailwind::cli::modules::FileSystem;
use tailwind::{compile, CompileOptions, Error, Stylesheet};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn load(id: &str, _base: &str) -> tailwind::Result<Stylesheet> {
    let name = id.strip_prefix("tailwindcss/").or(id.strip_prefix("./")).unwrap_or(if id == "tailwindcss" { "index" } else { id });
    let name = name.strip_suffix(".css").unwrap_or(name);
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("assets").join(format!("{name}.css"));
    let content = std::fs::read_to_string(&path).map_err(|_| Error::new(format!("Could not resolve '{id}'")))?;
    Ok(Stylesheet { path: path.to_string_lossy().into_owned(), base: String::new(), content })
}

#[test]
fn the_cases_of_the_fuzzer() {
    let text = std::fs::read_to_string(root().join("tests/upstream/fuzz-found.jsonl")).unwrap();
    let mut count = 0;
    for (i, line) in text.lines().filter(|l| !l.trim().is_empty()).enumerate() {
        let case: serde_json::Value = serde_json::from_str(line).unwrap();
        let css = case["css"].as_str().unwrap();
        let candidates: Vec<String> =
            case["candidates"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect();
        // The files of the case, in a directory of their own.
        let dir = std::env::temp_dir().join(format!("tailwind-fuzz-found-{}-{i}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        for (name, text) in case["files"].as_object().into_iter().flatten() {
            std::fs::write(dir.join(name), text.as_str().unwrap()).unwrap();
        }
        for (name, from) in case["copy"].as_object().into_iter().flatten() {
            std::fs::copy(root().join(from.as_str().unwrap()), dir.join(name)).unwrap();
        }
        let loader = load;
        let options = CompileOptions {
            base: dir.to_string_lossy().into_owned(),
            load_stylesheet: Some(&loader),
            module_loader: Some(Rc::new(FileSystem::default())),
            test_mode: true,
            ..CompileOptions::default()
        };
        let ours = compile(css, options).and_then(|mut c| c.try_build(&candidates));
        let _ = std::fs::remove_dir_all(&dir);
        let official = &case["official"];
        match (ours, official["css"].as_str(), official["error"].as_str()) {
            (Ok(out), Some(expected), _) => assert_eq!(out, expected, "line {}: {css}", i + 1),
            (Err(e), _, Some(expected)) => assert_eq!(e.message, expected, "line {}: {css}", i + 1),
            (ours, _, _) => panic!("line {}: {css}\nhere: {ours:?}\nofficial: {official}", i + 1),
        }
        count += 1;
    }
    eprintln!("fuzz-found.jsonl: {count} cases, all the same");
}
