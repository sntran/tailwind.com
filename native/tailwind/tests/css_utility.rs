//! The utilities of `@utility`, against the record of Tailwind CSS
//! (`tests/fixtures/css_utility.json`). The script
//! `tests/upstream/tools/gen-css-utility.mjs` writes the record.
//!
//! Environment:
//! - `CSS_UTILITY_FILTER`: check only the cases whose name contains this text.
//! - `CSS_UTILITY_SHOW`: the number of failures to print (default 20).

use serde::Deserialize;
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::rc::Rc;
use tailwind::ast::{AstNode, AtRule};
use tailwind::design_system::{build_design_system, DesignSystem, COMPILE_RESPECT_IMPORTANT};
use tailwind::theme::{self, Theme};
use tailwind::utilities::css_utility::{
    create_css_utility, create_css_utility_shared, is_valid_functional_utility_name, is_valid_static_utility_name,
};

#[derive(Deserialize)]
struct Name {
    name: String,
    r#static: bool,
    functional: bool,
}

/// The CSS of a rule, its property order, and its count of declarations.
type Rule = (String, Vec<usize>, usize);

#[derive(Deserialize)]
struct Case {
    name: String,
    input: String,
    important: bool,
    /// For each `@utility` rule of the input: `true` for a valid name.
    valid: Vec<bool>,
    /// The message when upstream rejects the input.
    error: Option<String>,
    /// The candidates with rules.
    rules: BTreeMap<String, Vec<Rule>>,
    /// The candidates with no rule, with a space between them.
    none: String,
}

#[derive(Deserialize)]
struct Fixture {
    names: Vec<Name>,
    cases: Vec<Case>,
}

fn fixture() -> Fixture {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/css_utility.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The theme of the `@theme` rules of the input.
fn theme_of(ast: &[AstNode]) -> Theme {
    let theme = Theme::new();
    for node in ast {
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
    theme
}

fn utilities_of(ast: &[AstNode]) -> Vec<&AtRule> {
    ast.iter().filter_map(|node| node.as_at_rule()).filter(|rule| rule.name == "@utility").collect()
}

fn rules_of(ds: &DesignSystem, candidate: &str) -> Vec<Rule> {
    let mut rules = Vec::new();
    for parsed in ds.parse_candidate(candidate).iter() {
        for compiled in ds.compile_ast_nodes(parsed, COMPILE_RESPECT_IMPORTANT).iter() {
            rules.push((
                tailwind::to_css::to_css(std::slice::from_ref(&compiled.node)),
                compiled.property_sort.order.clone(),
                compiled.property_sort.count,
            ));
        }
    }
    rules
}

#[test]
fn utility_names() {
    let fixture = fixture();
    assert!(fixture.names.len() >= 100);
    for name in &fixture.names {
        assert_eq!(is_valid_static_utility_name(&name.name), name.r#static, "static name {:?}", name.name);
        assert_eq!(is_valid_functional_utility_name(&name.name), name.functional, "functional name {:?}", name.name);
    }
}

#[test]
fn rules_of_each_candidate() {
    let filter = std::env::var("CSS_UTILITY_FILTER").ok();
    let show: usize = std::env::var("CSS_UTILITY_SHOW").ok().and_then(|v| v.parse().ok()).unwrap_or(20);
    let fixture = fixture();
    assert!(fixture.cases.len() >= 150);

    let (mut cases, mut failed_cases, mut total, mut same, mut with_rules, mut shown) = (0, 0, 0, 0, 0, 0);

    for case in &fixture.cases {
        if filter.as_ref().is_some_and(|f| !case.name.contains(f.as_str())) {
            continue;
        }
        cases += 1;
        let mut failed = false;

        let ast = tailwind::css_parser::parse(&case.input).unwrap();
        let utilities = utilities_of(&ast);
        assert_eq!(utilities.len(), case.valid.len(), "{}", case.name);

        // As upstream, the utility and the stylesheet share the node.
        let shared: Vec<_> = utilities.iter().map(|rule| Rc::new(RefCell::new((*rule).clone()))).collect();
        let registrations: Vec<_> = shared.iter().map(|node| create_css_utility_shared(node.clone())).collect();
        for (rule, registration) in utilities.iter().zip(&registrations) {
            assert_eq!(create_css_utility(rule).is_some(), registration.is_some(), "{}", case.name);
        }
        for ((rule, registration), valid) in utilities.iter().zip(&registrations).zip(&case.valid) {
            if registration.is_some() != *valid {
                failed = true;
                eprintln!("NAME: {}: `@utility {}` must be {}", case.name, rule.params, if *valid { "valid" } else { "invalid" });
            }
        }

        if case.error.is_none() {
            let mut ds = build_design_system(theme_of(&ast));
            ds.important = case.important;
            for registration in registrations.iter().flatten() {
                registration(&mut ds);
            }

            // Upstream replaces the functions of Tailwind CSS in the full
            // stylesheet after the registration. The `@utility` nodes are a
            // part of the stylesheet at that time.
            for node in &shared {
                let mut nodes = vec![AstNode::AtRule(node.borrow().clone())];
                tailwind::css_functions::substitute_functions(&mut nodes, &ds).unwrap();
                let Some(AstNode::AtRule(changed)) = nodes.pop() else { panic!("{}", case.name) };
                *node.borrow_mut() = changed;
            }

            let none = case.none.split(' ').filter(|c| !c.is_empty()).map(|c| (c, None));
            let rules = case.rules.iter().map(|(c, rules)| (c.as_str(), Some(rules)));
            for (candidate, theirs) in rules.chain(none) {
                total += 1;
                with_rules += usize::from(theirs.is_some());
                let ours = rules_of(&ds, candidate);
                if Some(&ours) == theirs.or(Some(&Vec::new())) {
                    same += 1;
                    continue;
                }
                failed = true;
                if shown < show {
                    shown += 1;
                    eprintln!(
                        "DIFFERENT: {}\n{}\ncandidate: {}\n--- upstream\n{:?}\n--- here\n{:?}\n",
                        case.name, case.input, candidate, theirs, ours
                    );
                }
            }
        }

        failed_cases += usize::from(failed);
    }

    eprintln!(
        "cases: {cases}, failed cases: {failed_cases}, candidates: {total} ({with_rules} with rules), same: {same}, different: {}",
        total - same
    );
    assert_eq!(failed_cases, 0, "some cases give other rules than upstream");
}
