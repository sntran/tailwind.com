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
    // Each case and version with another result than the record.
    let mut different: Vec<String> = Vec::new();
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
            if &result(css, &candidates, parsed) != expected {
                different.push(format!("{name}, as Tailwind CSS {version}"));
            }
        }
        let mut values = results.values();
        let first = values.next().unwrap();
        if values.any(|other| other != first) {
            with_a_change += 1;
        }
        cases += 1;
    }
    assert!(cases > 0);
    assert!(different.is_empty(), "{} results are not the record:\n{}", different.len(), different.join("\n"));
    // Most cases must show a change: a case with the same result in each
    // version tests no switch.
    assert!(with_a_change * 2 >= cases, "{with_a_change} of {cases} cases have a result that changes");
}

/// The message of the error of `compile`, with the path of the stylesheet
/// (the `from` option). The records have no such option.
fn error_with_from(css: &str, version: Version) -> String {
    let options =
        CompileOptions { test_mode: true, version: Some(version), from: Some("input.css".to_string()), ..CompileOptions::default() };
    match compile(css, options) {
        Ok(_) => String::new(),
        Err(error) => error.message,
    }
}

#[test]
fn a_css_syntax_error_has_its_location_from_4_1_18() {
    // The results of the official compiler of 4.1.17 and 4.1.18, with
    // `compile(css, { from: 'input.css' })`.
    let cases = [
        (".a { color: red; ", "Missing closing } at .a", "input.css:1:1: Missing closing } at .a"),
        (".a {\n  color red;\n}", "Invalid declaration: `color red`", "input.css:2:3: Invalid declaration: `color red`"),
        ("@tailwind utilities;\n  }", "Missing opening {", "input.css:2:3: Missing opening {"),
        (".c { content: \"a\nb\"; }", "Unterminated string: \"a\"", "input.css:1:15: Unterminated string: \"a\""),
    ];
    for (css, before, after) in cases {
        assert_eq!(error_with_from(css, Version(4, 1, 17)), before, "{css:?}, as Tailwind CSS 4.1.17");
        assert_eq!(error_with_from(css, Version(4, 1, 18)), after, "{css:?}, as Tailwind CSS 4.1.18");
        assert_eq!(error_with_from(css, Version::NATIVE), after, "{css:?}, as the newest version");
    }
}

#[test]
fn a_closing_curly_bracket_with_no_open_bracket_is_not_valid_in_each_version() {
    // Before 4.1.0, the official compiler had no check of an arbitrary
    // value: `w-[}]` gave `width: };`, and that `}` closes the rule too
    // early. This output is not valid CSS, so the Rust compiler gives
    // nothing for such a candidate, as 4.1.0 does.
    let css = "@tailwind utilities;";
    let candidates: Vec<String> =
        ["w-[}]", "[color:red}html{color:blue]", "bg-[red}html{color:blue]", "[&}]:flex", "data-[a}]:flex", "[--x:{}}]"]
            .iter()
            .map(|candidate| candidate.to_string())
            .collect();
    for version in [Version(4, 0, 17), Version(4, 1, 0), Version::NATIVE] {
        assert_eq!(result(css, &candidates, version), result(css, &[], version), "as Tailwind CSS {version}");
    }
    // A `}` that closes a `{` of the value was valid before 4.1.0.
    let block = ["[--x:{a:b}]".to_string()];
    assert_eq!(result(css, &block, Version(4, 0, 17)), serde_json::json!({ "css": ".\\[--x\\:\\{a\\:b\\}\\] {\n  --x: {a:b};\n}\n" }));
    assert_eq!(result(css, &block, Version(4, 1, 0)), result(css, &[], Version(4, 1, 0)));
}

#[test]
fn a_new_theme_variable_as_a_candidate_makes_a_new_output_from_4_0_15() {
    // The results of the official compiler of 4.0.14 and 4.0.15 for four
    // builds with one compiler.
    let css = "@theme { --color-red-500: red; --color-blue-500: blue; }\n@tailwind utilities;";
    let builds: [&[&str]; 4] = [&["flex"], &["--color-red-500"], &["--color-blue-500", "block"], &["--color-nope"]];
    let flex = ".flex {\n  display: flex;\n}\n";
    let red_and_flex = ":root, :host {\n  --color-red-500: red;\n}\n.flex {\n  display: flex;\n}\n";
    let all = ":root, :host {\n  --color-red-500: red;\n  --color-blue-500: blue;\n}\n.block {\n  display: block;\n}\n.flex {\n  display: flex;\n}\n";
    let cases = [(Version(4, 0, 14), [flex, flex, all, all]), (Version(4, 0, 15), [flex, red_and_flex, all, all])];
    for (version, expected) in cases {
        let options = CompileOptions { test_mode: true, version: Some(version), ..CompileOptions::default() };
        let mut compiler = compile(css, options).expect("the stylesheet compiles");
        for (candidates, expected) in builds.iter().zip(expected) {
            let candidates: Vec<String> = candidates.iter().map(|candidate| candidate.to_string()).collect();
            assert_eq!(compiler.try_build(&candidates).expect("the build"), expected, "{candidates:?}, as Tailwind CSS {version}");
        }
    }

    // A first build with only a variable has the variable in each version.
    let red = ":root, :host {\n  --color-red-500: red;\n}\n";
    let red_and_blue = ":root, :host {\n  --color-red-500: red;\n  --color-blue-500: blue;\n}\n";
    for (version, second) in [(Version(4, 0, 14), red), (Version(4, 0, 15), red_and_blue)] {
        let options = CompileOptions { test_mode: true, version: Some(version), ..CompileOptions::default() };
        let mut compiler = compile(css, options).expect("the stylesheet compiles");
        assert_eq!(compiler.try_build(&["--color-red-500".to_string()]).expect("the build"), red, "as Tailwind CSS {version}");
        assert_eq!(compiler.try_build(&["--color-blue-500".to_string()]).expect("the build"), second, "as Tailwind CSS {version}");
    }
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

/// The result of a list of builds on one compiler: the text of the last
/// build, or the first error.
fn sequence(css: &str, builds: &[Vec<String>], version: Version) -> serde_json::Value {
    let options = CompileOptions { test_mode: true, version: Some(version), ..CompileOptions::default() };
    let mut compiler = match compile(css, options) {
        Ok(compiler) => compiler,
        Err(error) => return serde_json::json!({ "error": error.message }),
    };
    let mut output = String::new();
    for candidates in builds {
        match compiler.try_build(candidates) {
            Ok(css) => output = css,
            Err(error) => return serde_json::json!({ "error": error.message }),
        }
    }
    serde_json::json!({ "css": output })
}

/// The cases where the official compiler of an old version gives another
/// text for another sequence of the candidates, with the first version
/// that does not. The Rust compiler does the same as that version.
const SEQUENCE_SHOWS: &[(&str, Version)] = &[
    // Before 4.3.4, `group-hover/foo//bar:flex` and `group-hover/foo/:flex`
    // are the same variant for the sort, so their rules stay in the
    // sequence of the candidates.
    ("4.3.4 an empty extra modifier", Version(4, 3, 4)),
    // Before 4.0.7, the order of the names is not consistent: `p-[1]` is
    // before `p-[02]`, `p-[02]` is before `p-[0a]`, and `p-[0a]` is before
    // `p-[1]`. So the result of the sort follows the sequence of the
    // candidates.
    ("4.0.7 the sort by name with numbers", Version(4, 0, 7)),
];

/// The watch builds again when the source files have new candidates
/// (`specs/Watch.tla`). The model takes the output as a function of the
/// input and of the set of candidates. So, as each version, builds in a
/// sequence must give the result of one build with all the candidates:
/// in two parts, one candidate at a time, and in the other direction.
/// (The fuzzer compares builds in a sequence with the official compiler:
/// `first` in `tests/fuzz/fuzz.mjs`.)
#[test]
fn builds_in_a_sequence_give_the_result_of_one_build_as_each_version() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/upstream/switches.jsonl");
    let text = std::fs::read_to_string(path).expect("tests/upstream/switches.jsonl");
    let mut different: Vec<String> = Vec::new();
    let mut compared = 0;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let case: serde_json::Value = serde_json::from_str(line).expect("a JSON case");
        let name = case["name"].as_str().unwrap_or("?");
        let css = case["css"].as_str().expect("css");
        let candidates: Vec<String> =
            case["candidates"].as_array().map(|list| list.iter().filter_map(|c| c.as_str().map(String::from)).collect()).unwrap_or_default();
        if candidates.len() < 2 {
            continue;
        }
        let (first, second) = candidates.split_at(candidates.len() / 2);
        let one_at_a_time: Vec<Vec<String>> = candidates.iter().map(|c| vec![c.clone()]).collect();
        let other_direction: Vec<Vec<String>> = candidates.iter().rev().map(|c| vec![c.clone()]).collect();
        let forms: [(&str, Vec<Vec<String>>); 4] = [
            ("two parts", vec![first.to_vec(), second.to_vec()]),
            ("two parts, the second first", vec![second.to_vec(), first.to_vec()]),
            ("one candidate at a time", one_at_a_time),
            ("one candidate at a time, the last first", other_direction),
        ];
        for version in case["results"].as_object().expect("results").keys() {
            let parsed = Version::parse(version).expect("a version");
            if SEQUENCE_SHOWS.iter().any(|(case, fixed)| *case == name && parsed < *fixed) {
                continue;
            }
            let expected = sequence(css, std::slice::from_ref(&candidates), parsed);
            if expected.get("error").is_some() {
                continue;
            }
            for (form, builds) in &forms {
                compared += 1;
                if sequence(css, builds, parsed) != expected {
                    different.push(format!("{name}, as Tailwind CSS {version}, {form}"));
                }
            }
        }
    }
    assert!(compared > 0);
    assert!(different.is_empty(), "{} of {compared} sequences give another result:\n{}", different.len(), different.join("\n"));
}
