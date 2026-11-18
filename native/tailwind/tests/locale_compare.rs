//! `locale_compare` against `localeCompare` of Node.js
//! (`tests/fixtures/locale_compare.json.gz`, from
//! `tests/upstream/tools/gen-locale-compare.mjs`).

use std::cmp::Ordering;
use std::io::Read;
use tailwind::utils::locale_compare::locale_compare;

#[test]
fn locale_compare_gives_the_order_of_node_js() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/locale_compare.json.gz");
    let mut text = String::new();
    flate2::read::GzDecoder::new(std::fs::File::open(path).unwrap()).read_to_string(&mut text).unwrap();
    let pairs: Vec<(String, String, i8)> = serde_json::from_str(&text).unwrap();
    assert!(pairs.len() >= 40_000);
    let mut failures = Vec::new();
    for (a, b, expected) in &pairs {
        let got = match locale_compare(a, b) {
            Ordering::Less => -1,
            Ordering::Equal => 0,
            Ordering::Greater => 1,
        };
        if got != *expected {
            failures.push(format!("{a:?} {b:?}: {got}, expected {expected}"));
        }
    }
    assert!(failures.is_empty(), "{} failures:\n{}", failures.len(), failures[..failures.len().min(30)].join("\n"));
}
