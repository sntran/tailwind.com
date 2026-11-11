//! Tests of `optimize_ast` and `handle_nesting` against the record of the
//! upstream `optimizeAst` and `handleNesting` of `ast.ts` of Tailwind CSS.
//!
//! The record is `tests/fixtures/optimize.json`. The script
//! `tests/upstream/tools/gen-optimize.mjs` writes it, and its first comment
//! gives the format.
//!
//! Environment:
//! - `OPTIMIZE_SHOW`: the number of failures to print (default 5).

use serde::Deserialize;
use serde_json::{json, Value};
use tailwind::ast::{at_root, at_rule, context, decl_full, AstNode, ContextMap, ContextValue};
use tailwind::css_parser::parse;
use tailwind::design_system::build_design_system;
use tailwind::optimize::{handle_nesting, optimize_ast};
use tailwind::theme::Theme;
use tailwind::to_css::to_css;

#[derive(Deserialize)]
struct ThemeRecord {
    prefix: Option<String>,
    entries: Vec<(String, String, u8)>,
}

#[derive(Deserialize)]
struct NestingCase {
    css: String,
    tree: Value,
}

#[derive(Deserialize)]
struct OptimizeCase {
    theme: usize,
    used: Vec<String>,
    polyfills: u8,
    css: String,
    tree: Value,
}

#[derive(Deserialize)]
struct Fixture {
    themes: Vec<ThemeRecord>,
    nesting: Vec<NestingCase>,
    optimize: Vec<OptimizeCase>,
}

fn fixture() -> Fixture {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/optimize.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// Change the special at-rules of the input to the nodes that CSS text
/// cannot give. This is `special` of `gen-optimize.mjs`.
fn special(nodes: Vec<AstNode>) -> Vec<AstNode> {
    let mut out = Vec::with_capacity(nodes.len());
    for node in nodes {
        match node {
            AstNode::Rule(mut rule) => {
                rule.nodes = special(rule.nodes);
                out.push(AstNode::Rule(rule));
            }
            AstNode::AtRule(rule) => {
                let nodes = special(rule.nodes);
                match rule.name.as_str() {
                    "@in-variant" => {
                        let mut map = ContextMap::new();
                        map.insert("source".to_string(), ContextValue::from("variant"));
                        out.push(context(map, nodes));
                    }
                    "@x-context" => {
                        let mut map = ContextMap::new();
                        for word in rule.params.split(' ') {
                            let (key, value) = match word {
                                "theme" => ("theme", ContextValue::from(true)),
                                "theme-false" => ("theme", ContextValue::from(false)),
                                "reference" => ("reference", ContextValue::from(true)),
                                "keyframes" => ("keyframes", ContextValue::from(true)),
                                "supports-color-mix" => ("supportsColorMix", ContextValue::from(true)),
                                "source-variant" => ("source", ContextValue::from("variant")),
                                "source-user" => ("source", ContextValue::from("user")),
                                _ => continue,
                            };
                            map.insert(key.to_string(), value);
                        }
                        out.push(context(map, nodes));
                    }
                    "@x-at-root" => out.push(at_root(nodes)),
                    "@x-undefined" => out.push(decl_full(rule.params, None, false)),
                    _ => out.push(at_rule(rule.name, rule.params, nodes)),
                }
            }
            other => out.push(other),
        }
    }
    out
}

/// The tree in the format of the record.
fn encode(nodes: &[AstNode]) -> Value {
    Value::Array(
        nodes
            .iter()
            .map(|node| match node {
                AstNode::Rule(n) => json!(["r", n.selector, encode(&n.nodes)]),
                AstNode::AtRule(n) => json!(["a", n.name, n.params, encode(&n.nodes)]),
                AstNode::Declaration(n) => json!(["d", n.property, n.value, n.important]),
                AstNode::Comment(n) => json!(["c", n.value]),
                AstNode::Context(n) => {
                    let map: serde_json::Map<String, Value> = n
                        .context
                        .iter()
                        .map(|(key, value)| {
                            let value = match value {
                                ContextValue::Str(s) => json!(s),
                                ContextValue::Bool(b) => json!(b),
                            };
                            (key.clone(), value)
                        })
                        .collect();
                    json!(["x", map, encode(&n.nodes)])
                }
                AstNode::AtRoot(n) => json!(["t", encode(&n.nodes)]),
            })
            .collect(),
    )
}

/// The record as nodes, to print it as CSS in a failure message.
fn decode(tree: &Value) -> Vec<AstNode> {
    let text = |value: &Value| value.as_str().unwrap().to_string();
    tree.as_array()
        .unwrap()
        .iter()
        .map(|node| match node[0].as_str().unwrap() {
            "r" => tailwind::ast::style_rule(text(&node[1]), decode(&node[2])),
            "a" => at_rule(text(&node[1]), text(&node[2]), decode(&node[3])),
            "d" => decl_full(text(&node[1]), node[2].as_str().map(String::from), node[3].as_bool().unwrap()),
            "c" => tailwind::ast::comment(text(&node[1])),
            "x" => context(ContextMap::new(), decode(&node[2])),
            _ => at_root(decode(&node[1])),
        })
        .collect()
}

struct Report {
    name: &'static str,
    total: usize,
    failures: usize,
    show: usize,
}

impl Report {
    fn new(name: &'static str) -> Self {
        let show = std::env::var("OPTIMIZE_SHOW").ok().and_then(|v| v.parse().ok()).unwrap_or(5);
        Report { name, total: 0, failures: 0, show }
    }

    fn check(&mut self, index: usize, input: &str, actual: &[AstNode], expected: &Value) {
        self.total += 1;
        if encode(actual) == *expected {
            return;
        }
        self.failures += 1;
        if self.failures <= self.show {
            eprintln!(
                "--- {} case {index}\n{input}\n--- expected\n{}\n{expected}\n--- actual\n{}\n{}",
                self.name,
                to_css(&decode(expected)),
                to_css(actual),
                encode(actual)
            );
        }
    }

    fn finish(self, minimum: usize) {
        eprintln!("{}: {} of {} cases are correct", self.name, self.total - self.failures, self.total);
        assert!(self.total >= minimum, "{}: the record has only {} cases", self.name, self.total);
        assert_eq!(self.failures, 0, "{}: {} of {} cases are different", self.name, self.failures, self.total);
    }
}

#[test]
fn handle_nesting_gives_the_upstream_tree() {
    let fixture = fixture();
    let mut report = Report::new("handle_nesting");
    for (index, case) in fixture.nesting.iter().enumerate() {
        let ast = special(parse(&case.css).expect("upstream parses the input"));
        let actual = handle_nesting(ast);
        report.check(index, &case.css, &actual, &case.tree);
    }
    report.finish(400);
}

#[test]
fn optimize_ast_gives_the_upstream_tree() {
    let fixture = fixture();
    let mut report = Report::new("optimize_ast");
    for (index, case) in fixture.optimize.iter().enumerate() {
        let record = &fixture.themes[case.theme];
        let mut theme = Theme::new();
        theme.prefix = record.prefix.clone();
        for (key, value, options) in &record.entries {
            theme.add(key, value, *options).unwrap();
        }
        let design_system = build_design_system(theme);
        for raw in &case.used {
            design_system.track_used_variables(raw);
        }

        let ast = special(parse(&case.css).expect("upstream parses the input"));
        let before = ast.clone();
        let actual = optimize_ast(&ast, &design_system, case.polyfills);
        assert_eq!(ast, before);
        report.check(index, &case.css, &actual, &case.tree);
    }
    report.finish(150);
}

#[test]
fn optimize_ast_with_no_nodes() {
    let design_system = build_design_system(Theme::new());
    assert!(optimize_ast(&[], &design_system, tailwind::POLYFILLS_ALL).is_empty());
    assert!(handle_nesting(Vec::new()).is_empty());
}
