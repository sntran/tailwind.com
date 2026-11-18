//! The constant fold, the order of `calc()` operands and the expansion of
//! declarations against the record of the official compiler
//! (`tests/fixtures/fold.json.gz`, from `tests/upstream/tools/gen-fold.mjs`).

use serde::Deserialize;
use std::io::Read;
use tailwind::ast::{AstNode, Declaration};
use tailwind::canonicalize::canonicalize_calc_expressions::canonicalize_calc_expressions;
use tailwind::canonicalize::constant_fold_declaration::{constant_fold_declaration, constant_fold_declaration_with};
use tailwind::canonicalize::expand_declaration::expand_declaration;

#[derive(Deserialize)]
struct Record {
    fold: Vec<(String, String, String, String, String)>,
    #[allow(clippy::type_complexity)]
    expand: Vec<(String, String, bool, u8, Option<Vec<(String, String, bool)>>)>,
}

fn record() -> Record {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/fold.json.gz");
    let mut text = String::new();
    flate2::read::GzDecoder::new(std::fs::File::open(path).unwrap()).read_to_string(&mut text).unwrap();
    serde_json::from_str(&text).unwrap()
}

#[test]
fn the_constant_fold_and_the_order_of_calc_operands_are_the_official_ones() {
    let record = record();
    assert!(record.fold.len() > 5_000);
    let mut failures = Vec::new();
    for (value, folded, folded_rem, folded_units, calc) in &record.fold {
        let got = (
            constant_fold_declaration(value, None),
            constant_fold_declaration(value, Some(16.0)),
            constant_fold_declaration_with(value, None, false),
            canonicalize_calc_expressions(value),
        );
        let expected = (folded.clone(), folded_rem.clone(), folded_units.clone(), calc.clone());
        if got != expected {
            failures.push(format!("{value:?}: {got:?}, expected {expected:?}"));
        }
    }
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures[..failures.len().min(30)].join("\n"));
}

#[test]
fn the_expansion_of_declarations_is_the_official_one() {
    let record = record();
    let mut failures = Vec::new();
    for (property, value, important, features, expected) in &record.expand {
        let node = Declaration { property: property.clone(), value: Some(value.clone()), important: *important, loc: Default::default() };
        let got = expand_declaration(&node, *features).map(|nodes| {
            nodes
                .into_iter()
                .map(|node| match node {
                    AstNode::Declaration(d) => (d.property, d.value.unwrap_or_default(), d.important),
                    _ => panic!("not a declaration"),
                })
                .collect::<Vec<_>>()
        });
        if &got != expected {
            failures.push(format!("{property}: {value:?} {features}: {got:?}, expected {expected:?}"));
        }
    }
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures[..failures.len().min(30)].join("\n"));
}
