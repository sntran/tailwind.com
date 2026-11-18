//! The canonical form of candidates (`canonicalizeCandidates`) against the
//! record of the official compiler (`tests/fixtures/canonicalize.json.gz`,
//! from `tests/upstream/tools/gen-canonicalize.mjs`): each call of the unit
//! tests of `canonicalize-candidates.ts`, in their sequence, with the same
//! design system.
#![cfg(feature = "canonicalize")]

use serde::Deserialize;
use std::io::Read;
use tailwind::canonicalize::CanonicalizeOptions;
use tailwind::index::load_design_system;
use tailwind::{CompileOptions, Stylesheet};

#[derive(Deserialize)]
struct Loaded {
    id: String,
    base: String,
    result: LoadedResult,
}

#[derive(Deserialize)]
struct LoadedResult {
    path: String,
    base: String,
    content: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct Options {
    rem: Option<f64>,
    collapse: Option<bool>,
    logical_to_physical: Option<bool>,
}

#[derive(Deserialize)]
struct Call {
    candidates: Vec<String>,
    options: Option<Options>,
    result: Option<Vec<String>>,
}

#[derive(Deserialize)]
struct Record {
    test: Option<String>,
    css: String,
    base: String,
    stylesheets: Vec<Loaded>,
    modules: Vec<serde_json::Value>,
    calls: Vec<Call>,
}

fn records() -> Vec<Record> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/canonicalize.json.gz");
    let mut text = String::new();
    flate2::read::GzDecoder::new(std::fs::File::open(path).unwrap()).read_to_string(&mut text).unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn the_canonical_forms_are_the_official_ones() {
    let records = records();
    let mut failures = Vec::new();
    let mut calls = 0;
    for record in &records {
        // A record with plugins needs the JavaScript engine. A relative
        // module is a file of the root, and another one a package.
        if !record.modules.is_empty() && !cfg!(tw_js) {
            continue;
        }
        let files: Vec<(String, String)> = record
            .modules
            .iter()
            .map(|module| {
                let id = module["id"].as_str().unwrap();
                let path = match id.strip_prefix("./") {
                    Some(file) => format!("/{file}"),
                    None => format!("/node_modules/{id}/index.js"),
                };
                (path, module["source"].as_str().unwrap().to_string())
            })
            .collect();
        let files: Vec<(&str, &str)> = files.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let modules = std::rc::Rc::new(tailwind::MemoryLoader::new(&files));
        let base = if record.modules.is_empty() { record.base.clone() } else { "/".to_string() };
        let load = |id: &str, base: &str| -> tailwind::Result<Stylesheet> {
            let found = record.stylesheets.iter().find(|s| s.id == id && s.base == base).or_else(|| record.stylesheets.iter().find(|s| s.id == id));
            match found {
                Some(s) => Ok(Stylesheet { path: s.result.path.clone(), base: s.result.base.clone(), content: s.result.content.clone() }),
                None => Err(tailwind::Error::new(format!("no stylesheet {id}"))),
            }
        };
        let design_system =
            load_design_system(&record.css, CompileOptions { base, load_stylesheet: Some(&load), module_loader: Some(modules), ..CompileOptions::default() })
                .unwrap_or_else(|e| panic!("{:?}: {e}", record.test));
        for call in &record.calls {
            let Some(expected) = &call.result else { continue };
            let options = call.options.as_ref().map(|o| CanonicalizeOptions {
                rem: o.rem,
                collapse: o.collapse.unwrap_or(false),
                logical_to_physical: o.logical_to_physical.unwrap_or(false),
            });
            let got = design_system.canonicalize_candidates(&call.candidates, &options.unwrap_or_default());
            calls += 1;
            if &got != expected {
                failures.push(format!("{}\n  {:?}\n  got      {:?}\n  expected {:?}", record.test.as_deref().unwrap_or(""), call.candidates, got, expected));
            }
        }
    }
    assert!(calls > 1500, "{calls} calls");
    assert!(failures.is_empty(), "{} of {calls} calls differ:\n{}", failures.len(), failures[..failures.len().min(25)].join("\n"));
}
