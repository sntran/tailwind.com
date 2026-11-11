//! The switches of the compiler (`src/switches.rs`) against records.
//!
//! Each line of `tests/upstream/switches.jsonl` is a case for a behavior
//! that changed in a release of Tailwind CSS: a stylesheet, candidates,
//! and the result of the official compiler of two or more versions
//! (`results`: the version before the change, and the version with it).
//! The Rust compiler must give each result when it compiles as that
//! version.
//!
//! `node tests/fuzz/compare.mjs --official DIR --builder FILE --record
//! tests/upstream/switches.jsonl` writes the result of the version in DIR
//! into each line. A case must be complete in itself (`@theme` and
//! `@tailwind utilities`, no `@import "tailwindcss"`): the test has no
//! stylesheets of other versions.

use tailwind::{compile, CompileOptions, Version};

fn result(css: &str, candidates: &[String], version: Version) -> serde_json::Value {
    let options = CompileOptions { test_mode: true, version: Some(version), ..CompileOptions::default() };
    match compile(css, options).and_then(|mut compiler| compiler.try_build(candidates)) {
        Ok(css) => serde_json::json!({ "css": css }),
        Err(error) => serde_json::json!({ "error": error.message }),
    }
}

#[test]
fn the_compiler_gives_the_result_of_each_recorded_version() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/upstream/switches.jsonl");
    let text = std::fs::read_to_string(path).expect("tests/upstream/switches.jsonl");
    let mut cases = 0;
    let mut with_a_change = 0;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let case: serde_json::Value = serde_json::from_str(line).expect("a JSON case");
        let name = case["name"].as_str().unwrap_or("?");
        let css = case["css"].as_str().expect("css");
        let candidates: Vec<String> =
            case["candidates"].as_array().map(|list| list.iter().filter_map(|c| c.as_str().map(String::from)).collect()).unwrap_or_default();
        let results = case["results"].as_object().unwrap_or_else(|| panic!("{name}: no results: record them with compare.mjs"));
        assert!(results.len() >= 2, "{name}: a case needs the result of two versions");
        for (version, expected) in results {
            let parsed = Version::parse(version).unwrap_or_else(|| panic!("{name}: the version {version}"));
            assert_eq!(&result(css, &candidates, parsed), expected, "{name}, as Tailwind CSS {version}");
        }
        let mut values = results.values();
        let first = values.next().unwrap();
        if values.any(|other| other != first) {
            with_a_change += 1;
        }
        cases += 1;
    }
    assert!(cases > 0);
    // Most cases must show a change: a case with the same result in each
    // version tests no switch.
    assert!(with_a_change * 2 >= cases, "{with_a_change} of {cases} cases have a result that changes");
}

#[test]
fn each_switch_is_in_the_list() {
    // The list is for the documents: a switch that is not in it is easy
    // to forget. The newest switches are first.
    let all = tailwind::switches::ALL;
    assert!(all.windows(2).all(|pair| pair[0].since >= pair[1].since), "the switches are not in sequence");
    for switch in all {
        assert!(switch.since > Version::OLDEST || switch.since <= Version::NATIVE);
        assert!(!switch.what.is_empty());
    }
}
