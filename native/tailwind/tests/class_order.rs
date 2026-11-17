//! The order of classes (`getClassOrder` of the design system) against the
//! record of the official compiler (`tests/fixtures/class_order.json`, from
//! `tests/upstream/tools/gen-class-order.mjs`): each class of each list has
//! the same place, or no place, as upstream.

use serde::Deserialize;
use std::collections::HashMap;
use tailwind::sort::sort_classes;
use tailwind::index::load_design_system;
use tailwind::{CompileOptions, Stylesheet};

#[derive(Deserialize)]
struct Record {
    css: String,
    classes: Vec<String>,
    order: Vec<(String, Option<u64>)>,
}

fn load(id: &str, _base: &str) -> tailwind::Result<Stylesheet> {
    let name = if id == "tailwindcss" { "index.css" } else { id.trim_start_matches("tailwindcss/").trim_start_matches("./") };
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/upstream/css").join(name);
    let content = std::fs::read_to_string(&path).map_err(|e| tailwind::Error::new(e.to_string()))?;
    Ok(Stylesheet { path: format!("/tailwindcss/{name}"), base: "/tailwindcss".into(), content })
}

fn records() -> Vec<Record> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/class_order.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn the_order_of_classes_is_the_order_of_the_official_compiler() {
    let records = records();
    assert!(records.len() >= 250);
    let loader = load;
    let mut systems = HashMap::new();
    let mut classes = 0;
    for record in &records {
        let design_system = systems.entry(record.css.clone()).or_insert_with(|| {
            load_design_system(&record.css, CompileOptions { base: "/".into(), load_stylesheet: Some(&loader), ..CompileOptions::default() })
                .unwrap()
        });
        let order = design_system.get_class_order(&record.classes);
        assert_eq!(order, record.order, "the order of {:?}\nfor\n{}", record.classes, record.css);
        classes += record.classes.len();
    }
    assert!(classes > 2_000);
}

#[test]
fn the_sort_of_the_tests_of_tailwind_css() {
    // The table of `sort.test.ts` of Tailwind CSS.
    let css = "@theme {\n  --spacing-1: 0.25rem;\n  --spacing-3: 0.75rem;\n  --spacing-4: 1rem;\n  --color-red-500: red;\n  --color-blue-500: blue;\n}\n";
    let design_system = load_design_system(css, CompileOptions::default()).unwrap();
    let table = [
        ("py-3 p-1 px-3", "p-1 px-3 py-3"),
        ("px-3 focus:hover:p-3 hover:p-1 py-3", "px-3 py-3 hover:p-1 focus:hover:p-3"),
        ("px-3 py-4! p-1", "p-1 px-3 py-4!"),
        ("py-4! px-3 p-1", "p-1 px-3 py-4!"),
        ("b p-1 a", "b a p-1"),
        ("hover:b focus:p-1 a", "hover:b a focus:p-1"),
        ("a-class px-3 p-1 b-class py-3 bg-red-500 bg-blue-500", "a-class b-class bg-blue-500 bg-red-500 p-1 px-3 py-3"),
        ("px-3 b-class p-1 py-3 bg-blue-500 a-class bg-red-500", "b-class a-class bg-blue-500 bg-red-500 p-1 px-3 py-3"),
        ("[--bg:#111] [--bg_hover:#000] [--fg:#fff]", "[--bg:#111] [--bg_hover:#000] [--fg:#fff]"),
    ];
    for (input, expected) in table {
        let classes: Vec<String> = input.split(' ').map(String::from).collect();
        assert_eq!(sort_classes(design_system.get_class_order(&classes)).join(" "), expected, "{input}");
    }
}
