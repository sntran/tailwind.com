//! The warnings of the optimizer against the record of the official
//! program (`tests/fixtures/optimize_warnings.json`, from
//! `tests/upstream/tools/gen-optimize-warnings.mjs`): the code, and the
//! exact text for `console.warn`, with the color codes.

#![cfg(feature = "cli")]

use serde::Deserialize;
use std::path::PathBuf;
use tailwind::cli::optimize_css::optimize;

#[derive(Deserialize)]
struct Record {
    input: String,
    minify: bool,
    code: Option<String>,
    warnings: Vec<String>,
}

#[test]
fn the_optimizer_gives_the_warnings_of_the_official_program() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/optimize_warnings.json");
    let records: Vec<Record> = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    assert!(records.len() >= 26);
    let mut with_warning = 0;
    for record in &records {
        let optimized = optimize(&record.input, "input.css", record.minify, None).unwrap();
        assert_eq!(Some(&optimized.code), record.code.as_ref(), "the code for {:?}", record.input);
        let warnings: Vec<String> = optimized.warnings.into_iter().collect();
        assert_eq!(warnings, record.warnings, "the warnings for {:?} (minify: {})", record.input, record.minify);
        with_warning += warnings.len();
    }
    assert!(with_warning >= 20);
}
