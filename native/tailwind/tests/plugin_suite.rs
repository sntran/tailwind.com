//! The compile calls of the unit tests of Tailwind CSS that load a
//! JavaScript plugin or config (`tests/upstream/plugin-suite.jsonl.gz`).
//!
//! In the tests of Tailwind CSS, a module is a JavaScript value of the
//! test file. `tests/upstream/tools/gen-plugin-suite.mjs` writes each
//! module as JavaScript source text, and keeps a record only when the
//! compiler of Tailwind CSS gives the same results with the module from
//! its text. This test gives the texts to the compiler through a file
//! system in memory, and compares the CSS of each build and each error
//! message with the record.

#![cfg(tw_js)]

use flate2::read::GzDecoder;
use serde::Deserialize;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::rc::Rc;
use tailwind::{compile, CompileOptions, Error, MemoryLoader, Stylesheet, POLYFILLS_ALL};

#[derive(Deserialize)]
struct Options {
    base: Option<String>,
    polyfills: Option<u8>,
}

#[derive(Deserialize)]
struct StylesheetResult {
    path: String,
    base: String,
    content: String,
}

#[derive(Deserialize)]
struct StylesheetCall {
    id: String,
    base: String,
    result: Option<StylesheetResult>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct Module {
    id: String,
    base: String,
    source: Option<String>,
    #[serde(rename = "resultBase")]
    result_base: Option<String>,
}

#[derive(Deserialize)]
struct Build {
    candidates: Vec<String>,
    css: Option<String>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct RecordedSource {
    base: Option<String>,
    pattern: String,
    negated: bool,
}

#[derive(Deserialize)]
struct Record {
    file: String,
    test: String,
    input: String,
    options: Options,
    stylesheets: Vec<StylesheetCall>,
    modules: Vec<Module>,
    builds: Vec<Build>,
    error: Option<String>,
    sources: Option<Vec<RecordedSource>>,
    features: Option<u32>,
}

fn records() -> Vec<Record> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/upstream/plugin-suite.jsonl.gz");
    let file = std::fs::File::open(&path).unwrap_or_else(|e| panic!("cannot open {}: {e}", path.display()));
    BufReader::new(GzDecoder::new(file))
        .lines()
        .map(|line| serde_json::from_str(&line.expect("a line")).unwrap_or_else(|e| panic!("plugin-suite.jsonl.gz: {e}")))
        .collect()
}

/// Replay one record. The result is the first difference.
fn replay(record: &Record) -> Result<(), String> {
    // Each module is one file. The loader gives the file and the base of
    // the record for each request, in the sequence of the record.
    let mut loader = MemoryLoader::default();
    for (index, module) in record.modules.iter().enumerate() {
        let Some(source) = &module.source else { continue };
        let path = format!("/__plugin_suite__/module-{index}.cjs");
        loader.files.insert(path.clone(), source.clone());
        loader
            .resolutions
            .borrow_mut()
            .push(((module.id.clone(), module.base.clone()), (path, module.result_base.clone().unwrap_or_default())));
    }

    let load_stylesheet = |id: &str, base: &str| -> Result<Stylesheet, Error> {
        match record.stylesheets.iter().find(|call| call.id == id && call.base == base) {
            Some(StylesheetCall { result: Some(result), .. }) => {
                Ok(Stylesheet { path: result.path.clone(), base: result.base.clone(), content: result.content.clone() })
            }
            Some(StylesheetCall { error: Some(error), .. }) => Err(Error::new(error.clone())),
            _ => Err(Error::new(format!("no stylesheet for `{id}`"))),
        }
    };

    let compiled = compile(
        &record.input,
        CompileOptions {
            base: record.options.base.clone().unwrap_or_default(),
            polyfills: record.options.polyfills.unwrap_or(POLYFILLS_ALL),
            load_stylesheet: Some(&load_stylesheet),
            test_mode: true,
            module_loader: Some(Rc::new(loader)),
        },
    );

    let mut compiler = match (compiled, &record.error) {
        (Err(error), Some(expected)) if error.message == *expected => return Ok(()),
        (Err(error), Some(expected)) => return Err(format!("the error is different\n    - {expected}\n    + {}", error.message)),
        (Err(error), None) => return Err(format!("compile gave an error: {}", error.message)),
        (Ok(_), Some(expected)) => return Err(format!("compile gave no error, expected: {expected}")),
        (Ok(compiler), None) => compiler,
    };

    if let Some(features) = record.features {
        if compiler.features != features {
            return Err(format!("the features are {}, expected {features}", compiler.features));
        }
    }
    if let Some(sources) = &record.sources {
        let expected: Vec<(String, String, bool)> =
            sources.iter().map(|source| (source.base.clone().unwrap_or_default(), source.pattern.clone(), source.negated)).collect();
        let actual: Vec<(String, String, bool)> =
            compiler.sources.iter().map(|source| (source.base.clone(), source.pattern.clone(), source.negated)).collect();
        if expected != actual {
            return Err(format!("the sources are different\n    - {expected:?}\n    + {actual:?}"));
        }
    }

    for (index, build) in record.builds.iter().enumerate() {
        match (compiler.try_build(&build.candidates), &build.css, &build.error) {
            (Ok(css), Some(expected), _) if css == *expected => {}
            (Ok(css), Some(expected), _) => return Err(format!("build {index}: the CSS is different\n{}", first_difference(expected, &css))),
            (Err(error), _, Some(expected)) if error.message == *expected => {}
            (Err(error), _, expected) => {
                return Err(format!("build {index}: the error is `{}`, expected {expected:?}", error.message));
            }
            (Ok(_), None, expected) => return Err(format!("build {index}: no error, expected {expected:?}")),
        }
    }
    Ok(())
}

fn first_difference(expected: &str, actual: &str) -> String {
    let (expected, actual): (Vec<&str>, Vec<&str>) = (expected.lines().collect(), actual.lines().collect());
    let index = expected.iter().zip(&actual).position(|(a, b)| a != b).unwrap_or(expected.len().min(actual.len()));
    let start = index.saturating_sub(2);
    let show = |lines: &[&str], mark: char| -> String {
        lines.iter().skip(start).take(6).map(|line| format!("    {mark} {line}\n")).collect()
    };
    format!("  at line {}:\n{}{}", index + 1, show(&expected, '-'), show(&actual, '+'))
}

#[test]
fn plugin_suite() {
    let records = records();
    let only = std::env::var("PLUGIN_SUITE_ONLY").ok();
    let mut failures = Vec::new();
    let mut total = 0;
    for record in &records {
        if only.as_ref().is_some_and(|only| !record.test.contains(only.as_str())) {
            continue;
        }
        total += 1;
        let outcome = std::panic::catch_unwind(|| replay(record)).unwrap_or_else(|_| Err("panic".to_string()));
        if let Err(problem) = outcome {
            failures.push(format!("{} > {}\n  {problem}", record.file, record.test));
        }
    }
    println!("plugin-suite.jsonl.gz: {total} records, {} pass, {} fail.", total - failures.len(), failures.len());
    for failure in &failures {
        println!("{failure}\n");
    }
    assert!(failures.is_empty(), "{} of {total} records failed", failures.len());
    assert!(total >= 200 || only.is_some(), "the file has only {total} records");
}
