//! The rules of each candidate before the last pass of the compiler,
//! against the record of Tailwind CSS (`tests/upstream/nodes.jsonl.gz`).
//!
//! Environment:
//! - `NODES_KIND`: the kinds of candidates to check, with commas (default
//!   `class`; others: `variant`, `random`, `edge`).
//! - `NODES_FILTER`: check only the candidates that contain this text.
//! - `NODES_SHOW`: the number of failures to print (default 20).
//! - `NODES_STRICT=1`: fail also when a candidate has no rule here and has
//!   rules upstream.

use flate2::read::GzDecoder;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader};
use tailwind::ast::AstNode;
use tailwind::design_system::{build_design_system, DesignSystem, COMPILE_RESPECT_IMPORTANT};
use tailwind::theme::{self, Theme};

#[derive(Deserialize)]
struct Node {
    css: String,
    order: Vec<usize>,
    count: usize,
}

#[derive(Deserialize)]
struct Line {
    candidate: String,
    kind: String,
    nodes: Vec<Node>,
}

fn upstream(path: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/upstream").join(path)
}

/// The design system of the default theme (`theme.css` of Tailwind CSS).
fn default_design_system() -> DesignSystem {
    let css = std::fs::read_to_string(upstream("css/theme.css")).unwrap();
    let ast = tailwind::css_parser::parse(&css).unwrap();
    let theme = Theme::new();
    for node in &ast {
        let AstNode::AtRule(rule) = node else { continue };
        if rule.name != "@theme" {
            continue;
        }
        let mut options = theme::NONE;
        for option in rule.params.split(' ') {
            match option {
                "reference" => options |= theme::REFERENCE,
                "inline" => options |= theme::INLINE,
                "default" => options |= theme::DEFAULT,
                "static" => options |= theme::STATIC,
                _ => {}
            }
        }
        for child in &rule.nodes {
            match child {
                AstNode::Declaration(d) if d.property.starts_with("--") => {
                    let key = tailwind::utils::escape::unescape(&d.property);
                    theme.add(&key, d.value.as_deref().unwrap_or(""), options).unwrap();
                }
                AstNode::AtRule(k) if k.name == "@keyframes" => theme.add_keyframes(k.clone()),
                _ => {}
            }
        }
    }
    build_design_system(theme)
}

#[test]
fn rules_of_each_candidate() {
    let kinds = std::env::var("NODES_KIND").unwrap_or_else(|_| "class".into());
    let kinds: Vec<&str> = kinds.split(',').collect();
    let filter = std::env::var("NODES_FILTER").ok();
    let show: usize = std::env::var("NODES_SHOW").ok().and_then(|v| v.parse().ok()).unwrap_or(20);
    let strict = std::env::var("NODES_STRICT").is_ok_and(|v| v == "1");

    let ds = default_design_system();
    let file = std::fs::File::open(upstream("nodes.jsonl.gz")).unwrap();
    let (mut total, mut same, mut different, mut missing) = (0, 0, 0, 0);
    let mut missing_roots: BTreeMap<String, usize> = BTreeMap::new();
    let mut shown = 0;

    for line in BufReader::new(GzDecoder::new(file)).lines() {
        let line: Line = serde_json::from_str(&line.unwrap()).unwrap();
        if !kinds.contains(&line.kind.as_str()) {
            continue;
        }
        if filter.as_ref().is_some_and(|f| !line.candidate.contains(f.as_str())) {
            continue;
        }
        total += 1;

        let mut ours: Vec<(String, Vec<usize>, usize)> = Vec::new();
        for candidate in ds.parse_candidate(&line.candidate).iter() {
            for compiled in ds.compile_ast_nodes(candidate, COMPILE_RESPECT_IMPORTANT).iter() {
                ours.push((
                    tailwind::to_css::to_css(std::slice::from_ref(&compiled.node)),
                    compiled.property_sort.order.clone(),
                    compiled.property_sort.count,
                ));
            }
        }
        let theirs: Vec<(String, Vec<usize>, usize)> =
            line.nodes.into_iter().map(|n| (n.css, n.order, n.count)).collect();

        if ours == theirs {
            same += 1;
        } else if ours.is_empty() {
            missing += 1;
            let root = line.candidate.trim_start_matches('-').split(['-', '/']).next().unwrap_or("").to_string();
            *missing_roots.entry(root).or_default() += 1;
        } else {
            different += 1;
            if shown < show {
                shown += 1;
                eprintln!("DIFFERENT: {}\n--- upstream\n{:?}\n--- here\n{:?}\n", line.candidate, theirs, ours);
            }
        }
    }

    eprintln!("candidates: {total}, same: {same}, different: {different}, no rule here: {missing}");
    if missing > 0 {
        eprintln!("no rule here, by the first part of the name: {missing_roots:?}");
    }
    assert_eq!(different, 0, "some candidates give other rules than upstream");
    if strict {
        assert_eq!(missing, 0, "some candidates give no rule");
    }
}
