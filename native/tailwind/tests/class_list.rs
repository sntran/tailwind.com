//! The class list and the completions of the variants (`intellisense.ts`)
//! against the record of the official compiler
//! (`tests/fixtures/class_list.json.gz`, from
//! `tests/upstream/tools/gen-class-list.mjs`).
#![cfg(feature = "canonicalize")]

use serde::Deserialize;
use std::io::Read;
use tailwind::index::load_design_system;
use tailwind::{CompileOptions, Stylesheet};

#[derive(Deserialize)]
struct Record {
    css: String,
    /// The files of the plugins, by path.
    files: std::collections::BTreeMap<String, String>,
    classes: Vec<(String, Vec<String>)>,
    variants: Vec<(String, String, Vec<String>)>,
}

fn load(id: &str, _base: &str) -> tailwind::Result<Stylesheet> {
    let name = if id == "tailwindcss" { "index.css" } else { id.trim_start_matches("tailwindcss/").trim_start_matches("./") };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/upstream/css").join(name);
    let content = std::fs::read_to_string(&path).map_err(|e| tailwind::Error::new(e.to_string()))?;
    Ok(Stylesheet { path: format!("/tailwindcss/{name}"), base: "/tailwindcss".into(), content })
}

fn records() -> Vec<Record> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/class_list.json.gz");
    let mut text = String::new();
    flate2::read::GzDecoder::new(std::fs::File::open(path).unwrap()).read_to_string(&mut text).unwrap();
    serde_json::from_str(&text).unwrap()
}

/// The records to test: the records with plugins need the JavaScript
/// engine.
fn records_to_test() -> Vec<Record> {
    let records: Vec<Record> = records().into_iter().filter(|record| cfg!(tw_js) || !record.css.contains("@plugin")).collect();
    assert_eq!(records.len(), if cfg!(tw_js) { 7 } else { 5 });
    records
}

fn design_system(record: &Record) -> tailwind::design_system::DesignSystem {
    let loader = load;
    let files: Vec<(&str, &str)> = record.files.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let modules = std::rc::Rc::new(tailwind::MemoryLoader::new(&files));
    load_design_system(
        &record.css,
        CompileOptions { base: "/".into(), load_stylesheet: Some(&loader), module_loader: Some(modules), ..CompileOptions::default() },
    )
    .unwrap()
}

/// The first differences of two lists, as text.
fn differences<T: std::fmt::Debug + PartialEq>(got: &[T], expected: &[T]) -> String {
    let mut lines = Vec::new();
    for i in 0..got.len().max(expected.len()) {
        if got.get(i) != expected.get(i) {
            lines.push(format!("  {i}: {:?}\n     expected {:?}", got.get(i), expected.get(i)));
            if lines.len() == 12 {
                break;
            }
        }
    }
    format!("{} items, expected {}\n{}", got.len(), expected.len(), lines.join("\n"))
}

#[test]
fn the_class_list_is_the_official_class_list() {
    for record in records_to_test() {
        let classes = design_system(&record).get_class_list();
        assert!(classes == record.classes, "the class list of\n{}\n{}", record.css, differences(&classes, &record.classes));
    }
}

#[test]
fn the_completions_of_the_variants_are_the_official_ones() {
    for record in records_to_test() {
        let design_system = design_system(&record);
        let variants: Vec<(String, String, Vec<String>)> = design_system
            .variants
            .entries()
            .map(|(root, info)| {
                let kind = format!("{:?}", info.kind).to_lowercase();
                (root.clone(), kind, design_system.variants.get_completions(root, &design_system))
            })
            .collect();
        assert!(variants == record.variants, "the variants of\n{}\n{}", record.css, differences(&variants, &record.variants));
    }
}
