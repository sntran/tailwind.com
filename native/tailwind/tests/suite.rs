//! The main compatibility test: the output of this compiler against the
//! records of Tailwind CSS in `tests/upstream`.
//!
//! - `suite`: each `compile` call of the upstream unit tests
//!   (`suite.jsonl.gz`). A record with a JavaScript module has no module
//!   here, so the compiler must refuse it: `plugin_suite.rs` replays
//!   these records with their modules.
//! - `suite_extra`: more records in the same format (`suite-extra.jsonl.gz`).
//! - `classes`: each candidate of `classes.jsonl.gz`. A sample of the
//!   candidates, each with a new compiler, and all the candidates in one
//!   build (`classes-all.json.gz`).
//! - `full`: `@import "tailwindcss"` with a list of usual candidates
//!   (`full.json.gz`).
//!
//! Environment:
//! - `SUITE_SHOW`: the number of failures to print (default 10).
//! - `SUITE_FILTER`: use only the suite records whose test name contains
//!   this text.
//! - `CLASSES_STEP`: use each Nth candidate for the sample (default 50).
//! - `SUITE_DUMP`: a directory. The tests `classes_all` and `full` write
//!   their output there when it is different (`NAME.expected.css` and
//!   `NAME.actual.css`).

use flate2::read::GzDecoder;
use serde::Deserialize;
use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};
use std::path::PathBuf;
use tailwind::compat::{CONFIG_NEEDS_JAVASCRIPT, NO_LOAD_MODULE, PLUGIN_NEEDS_JAVASCRIPT};
use tailwind::{compile, CompileOptions, Compiler, Error, Root, Stylesheet, POLYFILLS_ALL};

// ---------------------------------------------------------------------------
// The records.

#[derive(Deserialize)]
struct Options {
    base: Option<String>,
    polyfills: Option<u8>,
}

#[derive(Deserialize)]
struct LoadedStylesheet {
    path: String,
    base: String,
    content: String,
}

#[derive(Deserialize)]
struct StylesheetCall {
    id: String,
    base: String,
    result: Option<LoadedStylesheet>,
    error: Option<String>,
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
#[serde(untagged)]
enum RecordedRoot {
    /// `"none"`
    #[allow(dead_code)]
    Keyword(String),
    Pattern { base: Option<String>, pattern: String },
}

#[derive(Deserialize)]
struct Record {
    file: String,
    test: String,
    input: String,
    options: Options,
    stylesheets: Vec<StylesheetCall>,
    modules: usize,
    builds: Vec<Build>,
    error: Option<String>,
    sources: Option<Vec<RecordedSource>>,
    root: Option<RecordedRoot>,
    features: Option<u32>,
}

#[derive(Deserialize)]
struct ClassRecord {
    candidate: String,
    css: String,
}

#[derive(Deserialize)]
struct BuildRecord {
    input: String,
    candidates: Vec<String>,
    css: String,
}

// ---------------------------------------------------------------------------
// Helpers.

fn upstream(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/upstream").join(path)
}

fn gunzip(path: &str) -> BufReader<GzDecoder<std::fs::File>> {
    let file = std::fs::File::open(upstream(path)).unwrap_or_else(|e| panic!("cannot open {path}: {e}"));
    BufReader::new(GzDecoder::new(file))
}

fn read_lines<T: for<'de> Deserialize<'de>>(path: &str) -> Vec<T> {
    gunzip(path)
        .lines()
        .map(|line| serde_json::from_str(&line.expect("a line")).unwrap_or_else(|e| panic!("{path}: {e}")))
        .collect()
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &str) -> T {
    let mut text = String::new();
    gunzip(path).read_to_string(&mut text).expect("the text");
    serde_json::from_str(&text).unwrap_or_else(|e| panic!("{path}: {e}"))
}

fn env_number(name: &str, default: usize) -> usize {
    std::env::var(name).ok().and_then(|v| v.parse().ok()).unwrap_or(default)
}

/// The first line that is different, with some lines before and after it.
fn diff(expected: &str, actual: &str) -> String {
    const CONTEXT: usize = 3;
    let expected_lines: Vec<&str> = expected.split('\n').collect();
    let actual_lines: Vec<&str> = actual.split('\n').collect();
    let first = (0..expected_lines.len().max(actual_lines.len()))
        .find(|&i| expected_lines.get(i) != actual_lines.get(i))
        .unwrap_or(0);
    let start = first.saturating_sub(CONTEXT);

    let mut out = format!("  the first different line is line {}\n", first + 1);
    for line in &expected_lines[start.min(expected_lines.len())..first.min(expected_lines.len())] {
        out.push_str(&format!("      {line}\n"));
    }
    let side = |lines: &[&str]| -> String {
        lines.iter().skip(first).take(CONTEXT + 1).map(|line| format!("{line}\n        ")).collect::<String>().trim_end().to_string()
    };
    out.push_str(&format!("    - {}\n", side(&expected_lines)));
    out.push_str(&format!("    + {}\n", side(&actual_lines)));
    out
}

/// Write the two outputs to the directory of `SUITE_DUMP`.
fn dump(name: &str, expected: &str, actual: &str) {
    let Some(directory) = std::env::var_os("SUITE_DUMP").map(PathBuf::from) else { return };
    std::fs::write(directory.join(format!("{name}.expected.css")), expected).expect("the expected file");
    std::fs::write(directory.join(format!("{name}.actual.css")), actual).expect("the actual file");
}

/// The stylesheets of Tailwind CSS (`tests/upstream/css`), with the
/// virtual paths of the generator scripts.
fn load_tailwind_stylesheet(id: &str, _base: &str) -> Result<Stylesheet, Error> {
    let name = match id {
        "tailwindcss" => "index.css",
        id => id.strip_prefix("tailwindcss/").or_else(|| id.strip_prefix("./")).unwrap_or(id),
    };
    let content = std::fs::read_to_string(upstream("css").join(name))
        .map_err(|e| Error::new(format!("cannot read the stylesheet {id}: {e}")))?;
    Ok(Stylesheet { path: format!("/tailwindcss/{name}"), base: "/tailwindcss".to_string(), content })
}

fn compile_with_tailwind_stylesheets(input: &str) -> Compiler {
    let options = CompileOptions { load_stylesheet: Some(&load_tailwind_stylesheet), test_mode: true, ..Default::default() };
    compile(input, options).unwrap_or_else(|e| panic!("compile failed: {e}"))
}

// ---------------------------------------------------------------------------
// The suite.

/// The result of one record.
enum Outcome {
    Pass,
    Fail(String),
}

/// Replay one record of a stylesheet that needs no JavaScript module.
fn replay(record: &Record) -> Outcome {
    // The loader gives the recorded stylesheets. Upstream loads level by
    // level, as this compiler. The loader uses the first record that it
    // did not use and that has the same id and base.
    let used: Vec<Cell<bool>> = record.stylesheets.iter().map(|_| Cell::new(false)).collect();
    let loader_problem: RefCell<Option<String>> = RefCell::new(None);
    let loader = |id: &str, base: &str| -> Result<Stylesheet, Error> {
        let found = record
            .stylesheets
            .iter()
            .zip(&used)
            .find(|(call, used)| !used.get() && call.id == id && call.base == base);
        let Some((call, used)) = found else {
            let message = format!("the record has no stylesheet for id `{id}` and base `{base}`");
            loader_problem.borrow_mut().get_or_insert(message.clone());
            return Err(Error::new(message));
        };
        used.set(true);
        match (&call.result, &call.error) {
            (Some(result), _) => {
                Ok(Stylesheet { path: result.path.clone(), base: result.base.clone(), content: result.content.clone() })
            }
            (None, error) => Err(Error::new(error.clone().unwrap_or_default())),
        }
    };

    let options = CompileOptions {
        base: record.options.base.clone().unwrap_or_default(),
        polyfills: record.options.polyfills.unwrap_or(POLYFILLS_ALL),
        load_stylesheet: Some(&loader),
        test_mode: true,
        module_loader: None,
    };

    let compiled = compile(&record.input, options);
    if let Some(problem) = loader_problem.borrow().clone() {
        return Outcome::Fail(format!("  loader: {problem}\n"));
    }

    let mut compiler = match (compiled, &record.error) {
        (Err(error), Some(expected)) => {
            // Upstream has this error when the stylesheet needs a module
            // and the test gives no module loader.
            let needs_javascript = expected == "No `loadModule` function provided to `compile`"
                && ([PLUGIN_NEEDS_JAVASCRIPT, CONFIG_NEEDS_JAVASCRIPT, NO_LOAD_MODULE].contains(&error.message.as_str()));
            if error.message == *expected || needs_javascript {
                return Outcome::Pass;
            }
            return Outcome::Fail(format!("  the error is different\n    - {expected}\n    + {}\n", error.message));
        }
        (Err(error), None) => return Outcome::Fail(format!("  compile gave an error: {}\n", error.message)),
        (Ok(_), Some(expected)) => return Outcome::Fail(format!("  compile gave no error. Expected: {expected}\n")),
        (Ok(compiler), None) => compiler,
    };

    if let Some(index) = used.iter().position(|used| !used.get()) {
        return Outcome::Fail(format!("  the compiler did not load the stylesheet `{}`\n", record.stylesheets[index].id));
    }

    if Some(compiler.features) != record.features {
        return Outcome::Fail(format!("  features: expected {:?}, got {}\n", record.features, compiler.features));
    }

    let expected_root = match &record.root {
        None => Root::Default,
        Some(RecordedRoot::Keyword(_)) => Root::None,
        Some(RecordedRoot::Pattern { base, pattern }) => {
            Root::Pattern { base: base.clone().unwrap_or_default(), pattern: pattern.clone() }
        }
    };
    if compiler.root != expected_root {
        return Outcome::Fail(format!("  root: expected {expected_root:?}, got {:?}\n", compiler.root));
    }

    let expected_sources: Vec<(String, String, bool)> = record
        .sources
        .iter()
        .flatten()
        .map(|source| (source.base.clone().unwrap_or_default(), source.pattern.clone(), source.negated))
        .collect();
    let sources: Vec<(String, String, bool)> =
        compiler.sources.iter().map(|source| (source.base.clone(), source.pattern.clone(), source.negated)).collect();
    if sources != expected_sources {
        return Outcome::Fail(format!("  sources: expected {expected_sources:?}, got {sources:?}\n"));
    }

    for (index, build) in record.builds.iter().enumerate() {
        let css = compiler.build(&build.candidates);
        match (&build.css, &build.error) {
            (Some(expected), _) if *expected == css => {}
            (Some(expected), _) => return Outcome::Fail(format!("  build {index}:\n{}", diff(expected, &css))),
            (None, error) => {
                return Outcome::Fail(format!("  build {index}: upstream gave the error {error:?}, and this compiler gave CSS\n"))
            }
        }
    }

    Outcome::Pass
}

/// Replay one record of a stylesheet that needs a JavaScript module. This
/// file has no module for it, so the compiler must refuse it (the test
/// `plugin_suite.rs` replays these records with their modules).
fn replay_with_modules(record: &Record) -> Outcome {
    let loader = |id: &str, base: &str| -> Result<Stylesheet, Error> {
        match record.stylesheets.iter().find(|call| call.id == id && call.base == base).and_then(|call| call.result.as_ref()) {
            Some(result) => Ok(Stylesheet { path: result.path.clone(), base: result.base.clone(), content: result.content.clone() }),
            None => Err(Error::new(format!("no stylesheet for `{id}`"))),
        }
    };
    let options = CompileOptions {
        base: record.options.base.clone().unwrap_or_default(),
        polyfills: record.options.polyfills.unwrap_or(POLYFILLS_ALL),
        load_stylesheet: Some(&loader),
        test_mode: true,
        module_loader: None,
    };
    match compile(&record.input, options) {
        Err(error) if [PLUGIN_NEEDS_JAVASCRIPT, CONFIG_NEEDS_JAVASCRIPT, NO_LOAD_MODULE].contains(&error.message.as_str()) => Outcome::Pass,
        Err(error) => Outcome::Fail(format!("  expected the error for no module loader, got: {}\n", error.message)),
        Ok(_) => Outcome::Fail("  expected the error for no module loader, and compile gave no error\n".to_string()),
    }
}

/// Replay all the records of one file, and print a summary. The result is
/// the number of records that fail.
fn replay_file(path: &str) -> usize {
    let show = env_number("SUITE_SHOW", 10);
    let filter = std::env::var("SUITE_FILTER").ok();
    let records: Vec<Record> = read_lines(path);

    let (mut total, mut pass, mut module_total, mut module_pass) = (0, 0, 0, 0);
    let mut failures: Vec<String> = Vec::new();
    // For each test file: the number of failures and the number of records.
    let mut by_file: BTreeMap<String, (usize, usize)> = BTreeMap::new();

    for (line, record) in records.iter().enumerate() {
        if filter.as_ref().is_some_and(|filter| !record.test.contains(filter.as_str())) {
            continue;
        }

        let with_modules = record.modules > 0;
        let outcome = std::panic::catch_unwind(|| if with_modules { replay_with_modules(record) } else { replay(record) })
            .unwrap_or_else(|_| Outcome::Fail("  the compiler panicked\n".to_string()));

        let file = record.file.rsplit('/').next().unwrap_or("").to_string();
        let counts = by_file.entry(file.clone()).or_default();
        counts.1 += 1;
        if with_modules {
            module_total += 1;
        } else {
            total += 1;
        }
        match outcome {
            Outcome::Pass if with_modules => module_pass += 1,
            Outcome::Pass => pass += 1,
            Outcome::Fail(message) => {
                counts.0 += 1;
                failures.push(format!("line {} [{file}] {}\n{message}", line + 1, record.test));
            }
        }
    }

    for failure in failures.iter().take(show) {
        eprintln!("FAIL {failure}");
    }
    if failures.len() > show {
        eprintln!("... and {} more failures (set SUITE_SHOW)", failures.len() - show);
    }
    for (file, (failed, count)) in &by_file {
        if *failed > 0 {
            eprintln!("{file}: {failed} of {count} records fail");
        }
    }
    eprintln!(
        "{path}: {total} records, {pass} pass, {} fail. With JavaScript modules, for plugin_suite.rs: {module_total} ({module_pass} refused here with the correct error).",
        total - pass
    );

    failures.len()
}

/// The `compile` calls of the unit tests of Tailwind CSS.
#[test]
fn suite() {
    let failed = replay_file("suite.jsonl.gz");
    assert_eq!(failed, 0, "{failed} records of the suite fail");
}

/// More records, for the parts that have a different structure in this
/// port (`tests/upstream/tools/gen-extra.mjs`).
#[test]
fn suite_extra() {
    let failed = replay_file("suite-extra.jsonl.gz");
    assert_eq!(failed, 0, "{failed} records of the extra suite fail");
}

// ---------------------------------------------------------------------------
// The classes.

const CLASSES_INPUT: &str = "@layer theme, base, components, utilities;\n@import \"tailwindcss/theme.css\" layer(theme);\n@import \"tailwindcss/utilities.css\" layer(utilities);\n";

/// A sample of the candidates: a new compiler builds each candidate alone.
#[test]
fn classes_sample() {
    let show = env_number("SUITE_SHOW", 10);
    let step = env_number("CLASSES_STEP", 50).max(1);
    let records: Vec<ClassRecord> = read_lines("classes.jsonl.gz");

    let (mut total, mut failed) = (0, 0);
    for record in records.iter().step_by(step) {
        total += 1;
        let css = compile_with_tailwind_stylesheets(CLASSES_INPUT).build(std::slice::from_ref(&record.candidate));
        if css != record.css {
            failed += 1;
            if failed <= show {
                eprintln!("FAIL candidate `{}`\n{}", record.candidate, diff(&record.css, &css));
            }
        }
    }

    eprintln!("classes sample: {total} candidates, {} pass, {failed} fail", total - failed);
    assert_eq!(failed, 0, "{failed} candidates of the sample fail");
}

/// All the candidates in one build: this also checks the sort of the rules.
#[test]
fn classes_all() {
    let record: BuildRecord = read_json("classes-all.json.gz");
    let css = compile_with_tailwind_stylesheets(&record.input).build(&record.candidates);
    if css != record.css {
        eprintln!("FAIL classes-all ({} candidates)\n{}", record.candidates.len(), diff(&record.css, &css));
        dump("classes-all", &record.css, &css);
    }
    assert!(css == record.css, "the output for all the candidates is different");
}

// ---------------------------------------------------------------------------
// The full stylesheet.

#[test]
fn full() {
    let records: Vec<BuildRecord> = read_json("full.json.gz");
    let mut failed = 0;
    for (index, record) in records.iter().enumerate() {
        let css = compile_with_tailwind_stylesheets(&record.input).build(&record.candidates);
        if css != record.css {
            failed += 1;
            eprintln!("FAIL full entry {index} ({} candidates)\n{}", record.candidates.len(), diff(&record.css, &css));
            dump(&format!("full-{index}"), &record.css, &css);
        }
    }
    assert_eq!(failed, 0, "{failed} entries of full.json.gz fail");
}
