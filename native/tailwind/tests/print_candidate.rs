//! The print functions of candidates and variants (`printCandidate` and
//! `printVariant` of `candidate.ts`) against the record of the official
//! compiler (`tests/fixtures/print_candidate.json.gz`, from
//! `tests/upstream/tools/gen-print-candidate.mjs`).

use serde::Deserialize;
use std::io::Read;
use tailwind::index::load_design_system;
use tailwind::{CompileOptions, Stylesheet};

#[derive(Deserialize)]
struct Record {
    css: String,
    candidates: Vec<(String, Vec<String>)>,
    variants: Vec<(String, String)>,
}

fn load(id: &str, _base: &str) -> tailwind::Result<Stylesheet> {
    let name = if id == "tailwindcss" { "index.css" } else { id.trim_start_matches("tailwindcss/").trim_start_matches("./") };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/upstream/css").join(name);
    let content = std::fs::read_to_string(&path).map_err(|e| tailwind::Error::new(e.to_string()))?;
    Ok(Stylesheet { path: format!("/tailwindcss/{name}"), base: "/tailwindcss".into(), content })
}

#[test]
fn the_print_functions_give_the_text_of_the_official_compiler() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/print_candidate.json.gz");
    let mut text = String::new();
    flate2::read::GzDecoder::new(std::fs::File::open(path).unwrap()).read_to_string(&mut text).unwrap();
    let records: Vec<Record> = serde_json::from_str(&text).unwrap();
    let loader = load;
    let mut failures = Vec::new();
    for record in &records {
        let design_system =
            load_design_system(&record.css, CompileOptions { base: "/".into(), load_stylesheet: Some(&loader), ..CompileOptions::default() })
                .unwrap();
        for (candidate, expected) in &record.candidates {
            let printed: Vec<String> =
                design_system.parse_candidate(candidate).iter().map(|c| design_system.print_candidate(c)).collect();
            if &printed != expected {
                failures.push(format!("{candidate}: {printed:?}, expected {expected:?}"));
            }
        }
        for (variant, expected) in &record.variants {
            let printed = design_system.parse_variant(variant).map(|v| design_system.print_variant(&v));
            if printed.as_ref() != Some(expected) {
                failures.push(format!("variant {variant}: {printed:?}, expected {expected:?}"));
            }
        }
        assert!(record.candidates.len() > 10_000);
    }
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures[..failures.len().min(40)].join("\n"));
}
