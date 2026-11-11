//! Tests of the variants: `variants.ts` of Tailwind CSS.
//!
//! `tests/fixtures/variants.json` is a record of Tailwind CSS. The script
//! `tests/upstream/tools/gen-variants.mjs` writes it. It has the candidates
//! of `variants.test.ts`, and results of `substituteAtSlot`,
//! `substituteAtVariant`, `Variants.fromAst` and `Variants.compare`.

use serde::Deserialize;
use serde_json::{json, Value};
use tailwind::ast::{at_rule, decl, style_rule, AstNode, ContextValue};
use tailwind::candidate::Variant;
use tailwind::css_parser::parse;
use tailwind::design_system::{build_design_system, DesignSystem, COMPILE_RESPECT_IMPORTANT};
use tailwind::theme::{self, Theme};
use tailwind::to_css::to_css;
use tailwind::variants::{
    compounds_for_selectors, create_variants, is_valid_variant_name, substitute_at_slot, substitute_at_variant,
    VariantKind, COMPOUNDS_AT_RULES, COMPOUNDS_NEVER, COMPOUNDS_STYLE_RULES,
};
use tailwind::FEATURES_VARIANTS;

#[derive(Deserialize)]
struct Node {
    css: String,
    ast: Value,
    order: Vec<usize>,
    count: usize,
}

#[derive(Deserialize)]
struct CandidateRecord {
    candidate: String,
    #[serde(default)]
    nodes: Vec<Node>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct Case {
    test: String,
    input: String,
    prefix: Option<String>,
    theme: Vec<(String, String, u8)>,
    candidates: Vec<CandidateRecord>,
}

#[derive(Deserialize)]
struct SlotRecord {
    ast: String,
    nodes: String,
    expected: Value,
}

#[derive(Deserialize)]
struct AtVariantRecord {
    input: String,
    features: Option<u32>,
    expected: Option<Value>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct FromAstRecord {
    name: String,
    body: String,
    compounds: u8,
    candidates: Vec<CandidateRecord>,
}

#[derive(Deserialize)]
struct Order {
    input: Vec<String>,
    sorted: Vec<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Fixture {
    cases: Vec<Case>,
    slot: Vec<SlotRecord>,
    at_variant: Vec<AtVariantRecord>,
    from_ast: Vec<FromAstRecord>,
    order: Order,
}

fn fixture() -> Fixture {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/variants.json");
    serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap()
}

/// The design system of the default theme (`theme.css` of Tailwind CSS).
fn default_design_system() -> DesignSystem {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/upstream/css/theme.css");
    let ast = parse(&std::fs::read_to_string(path).unwrap()).unwrap();
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

/// A design system with an empty theme.
fn empty_design_system() -> DesignSystem {
    build_design_system(Theme::new())
}

/// The node in the form of the record: the fields of the upstream node.
fn plain(node: &AstNode) -> Value {
    let list = |nodes: &[AstNode]| Value::Array(nodes.iter().map(plain).collect());
    match node {
        AstNode::Rule(n) => json!({ "kind": "rule", "selector": n.selector, "nodes": list(&n.nodes) }),
        AstNode::AtRule(n) => {
            json!({ "kind": "at-rule", "name": n.name, "params": n.params, "nodes": list(&n.nodes) })
        }
        AstNode::Declaration(n) => {
            json!({ "kind": "declaration", "property": n.property, "value": n.value, "important": n.important })
        }
        AstNode::Comment(n) => json!({ "kind": "comment", "value": n.value }),
        AstNode::Context(n) => {
            let mut context = serde_json::Map::new();
            for (key, value) in &n.context {
                let value = match value {
                    ContextValue::Str(s) => Value::String(s.clone()),
                    ContextValue::Bool(b) => Value::Bool(*b),
                };
                context.insert(key.clone(), value);
            }
            json!({ "kind": "context", "context": context, "nodes": list(&n.nodes) })
        }
        AstNode::AtRoot(n) => json!({ "kind": "at-root", "nodes": list(&n.nodes) }),
    }
}

fn plain_list(nodes: &[AstNode]) -> Value {
    Value::Array(nodes.iter().map(plain).collect())
}

/// The rules of a candidate, in the form of the record.
fn compile(ds: &DesignSystem, candidate: &str) -> Vec<(String, Value, Vec<usize>, usize)> {
    let mut rules = Vec::new();
    for parsed in ds.parse_candidate(candidate).iter() {
        for compiled in ds.compile_ast_nodes(parsed, COMPILE_RESPECT_IMPORTANT).iter() {
            rules.push((
                to_css(std::slice::from_ref(&compiled.node)),
                plain(&compiled.node),
                compiled.property_sort.order.clone(),
                compiled.property_sort.count,
            ));
        }
    }
    rules
}

fn recorded(record: &CandidateRecord) -> Vec<(String, Value, Vec<usize>, usize)> {
    record.nodes.iter().map(|n| (n.css.clone(), n.ast.clone(), n.order.clone(), n.count)).collect()
}

/// The rules of the first candidate, as CSS.
fn css_of(ds: &DesignSystem, candidate: &str) -> String {
    compile(ds, candidate).into_iter().map(|rule| rule.0).collect()
}

#[test]
fn candidates_of_the_upstream_tests() {
    let fixture = fixture();
    let mut total = 0;
    let mut failures = Vec::new();

    for case in &fixture.cases {
        let mut theme = Theme::new();
        theme.prefix = case.prefix.clone();
        for (key, value, options) in &case.theme {
            theme.add(key, value, *options).unwrap();
        }
        let ds = build_design_system(theme);

        for record in &case.candidates {
            total += 1;
            assert!(record.error.is_none(), "{}: the record has an error", record.candidate);
            let ours = compile(&ds, &record.candidate);
            if ours != recorded(record) {
                failures.push(format!(
                    "test `{}`, candidate `{}`, input `{}`\n--- upstream\n{}\n--- here\n{}",
                    case.test,
                    record.candidate,
                    case.input.trim(),
                    record.nodes.iter().map(|n| n.css.as_str()).collect::<String>(),
                    ours.iter().map(|n| n.0.as_str()).collect::<String>(),
                ));
            }
        }
    }

    eprintln!("cases: {}, candidates: {total}, different: {}", fixture.cases.len(), failures.len());
    assert!(failures.is_empty(), "{}", failures.join("\n\n"));
}

#[test]
fn substitute_at_slot_gives_the_upstream_tree() {
    for record in fixture().slot {
        let mut ast = parse(&record.ast).unwrap();
        let nodes = parse(&record.nodes).unwrap();
        substitute_at_slot(&mut ast, &nodes);
        assert_eq!(plain_list(&ast), record.expected, "ast `{}`, nodes `{}`", record.ast, record.nodes);
    }
}

#[test]
fn substitute_at_slot_puts_the_nodes_at_each_slot() {
    let mut ast = vec![style_rule("&:hover", vec![at_rule("@slot", "", vec![])]), at_rule("@slot", "", vec![])];
    substitute_at_slot(&mut ast, &[decl("color", "red")]);
    assert_eq!(ast, vec![style_rule("&:hover", vec![decl("color", "red")]), decl("color", "red")]);
}

#[test]
fn substitute_at_slot_does_not_visit_the_new_nodes() {
    let mut ast = vec![at_rule("@slot", "", vec![])];
    let nodes = [at_rule("@slot", "", vec![]), at_rule("@keyframes", "a", vec![])];
    substitute_at_slot(&mut ast, &nodes);
    assert_eq!(ast, nodes.to_vec());
}

#[test]
fn substitute_at_variant_gives_the_upstream_tree() {
    let ds = default_design_system();
    for record in fixture().at_variant {
        let mut ast = parse(&record.input).unwrap();
        match substitute_at_variant(&mut ast, &ds) {
            Ok(features) => {
                assert_eq!(record.error, None, "input `{}`", record.input);
                assert_eq!(Some(features), record.features, "input `{}`", record.input);
                assert_eq!(Some(plain_list(&ast)), record.expected, "input `{}`", record.input);
            }
            Err(error) => assert_eq!(Some(error.message), record.error, "input `{}`", record.input),
        }
    }
}

#[test]
fn substitute_at_variant_reports_the_feature() {
    let ds = empty_design_system();
    let mut ast = parse(".a { color: red; }").unwrap();
    assert_eq!(substitute_at_variant(&mut ast, &ds), Ok(tailwind::FEATURES_NONE));

    let mut ast = parse(".a { @variant focus { color: red; } }").unwrap();
    assert_eq!(substitute_at_variant(&mut ast, &ds), Ok(FEATURES_VARIANTS));
    assert_eq!(to_css(&ast), ".a {\n  &:focus {\n    color: red;\n  }\n}\n");
}

#[test]
fn substitute_at_variant_gives_the_upstream_errors() {
    let ds = empty_design_system();
    for (input, message) in [
        ("@variant { color: red; }", "Cannot use `@variant` with empty variant"),
        ("@variant foo { color: red; }", "Cannot use `@variant` with unknown variant: foo"),
        ("@variant max-foo { color: red; }", "Cannot use `@variant` with variant: max-foo"),
    ] {
        let mut ast = parse(input).unwrap();
        assert_eq!(substitute_at_variant(&mut ast, &ds).unwrap_err().message, message);
    }
}

#[test]
fn from_ast_gives_the_upstream_rules() {
    let mut ds = default_design_system();
    let fixture = fixture();
    for record in &fixture.from_ast {
        ds.variants.from_ast(record.name.clone(), parse(&record.body).unwrap());
    }

    let mut total = 0;
    for record in &fixture.from_ast {
        let info = ds.variants.get(&record.name).unwrap();
        assert_eq!(info.kind, VariantKind::Static, "body `{}`", record.body);
        assert_eq!(info.compounds, record.compounds, "body `{}`", record.body);

        for candidate in &record.candidates {
            total += 1;
            ds.variants.take_error();
            let ours = compile(&ds, &candidate.candidate);
            let error = ds.variants.take_error().map(|e| e.message);
            assert_eq!(error, candidate.error, "body `{}`, candidate `{}`", record.body, candidate.candidate);
            assert_eq!(ours, recorded(candidate), "body `{}`, candidate `{}`", record.body, candidate.candidate);
        }
    }
    eprintln!("variants from a tree: {}, candidates: {total}", fixture.from_ast.len());
}

#[test]
fn from_ast_keeps_the_order_of_a_variant_that_is_there() {
    let mut ds = empty_design_system();
    let order = ds.variants.get("hover").unwrap().order;
    ds.variants.from_ast("hover", parse("&:where(.hover) { @slot; }").unwrap());
    assert_eq!(ds.variants.get("hover").unwrap().order, order);
    assert_eq!(css_of(&ds, "hover:[color:red]"), ".hover\\:\\[color\\:red\\] {\n  &:where(.hover) {\n    color: red;\n  }\n}\n");
}

#[test]
fn from_ast_gets_the_design_system_at_the_time_of_the_call() {
    let mut ds = empty_design_system();
    ds.variants.from_ast("first", parse("@variant second { @slot; }").unwrap());
    // The variant `second` comes after the variant that uses it.
    ds.variants.from_ast("second", parse("&:where(.second) { @slot; }").unwrap());
    assert_eq!(css_of(&ds, "first:[color:red]"), ".first\\:\\[color\\:red\\] {\n  &:where(.second) {\n    color: red;\n  }\n}\n");
}

#[test]
fn from_ast_keeps_the_error_of_an_at_variant() {
    let mut ds = empty_design_system();
    ds.variants.from_ast("broken", parse("@variant no-such-variant { @slot; }").unwrap());
    assert_eq!(css_of(&ds, "broken:[color:red]"), "");
    assert_eq!(
        ds.variants.take_error().map(|e| e.message).as_deref(),
        Some("Cannot use `@variant` with unknown variant: no-such-variant")
    );
    assert_eq!(ds.variants.take_error(), None);

    // `@variant` gives the error of the variant that it uses.
    let mut ast = parse(".a { @variant broken { color: red; } }").unwrap();
    assert_eq!(
        substitute_at_variant(&mut ast, &ds).unwrap_err().message,
        "Cannot use `@variant` with unknown variant: no-such-variant"
    );
}

#[test]
fn compare_gives_the_upstream_order() {
    let ds = default_design_system();
    let order = fixture().order;
    assert!(order.input.len() >= 300);

    let mut variants: Vec<(&String, std::rc::Rc<Variant>)> = order
        .input
        .iter()
        .map(|name| (name, ds.parse_variant(name).unwrap_or_else(|| panic!("no variant `{name}`"))))
        .collect();
    variants.sort_by(|a, z| ds.variants.compare(Some(&a.1), Some(&z.1), &ds).cmp(&0));

    let sorted: Vec<&String> = variants.iter().map(|v| v.0).collect();
    let expected: Vec<&String> = order.sorted.iter().collect();
    assert_eq!(sorted, expected);
}

#[test]
fn compare_with_no_variant() {
    let ds = empty_design_system();
    let hover = ds.parse_variant("hover").unwrap();
    assert_eq!(ds.variants.compare(None, None, &ds), 0);
    assert_eq!(ds.variants.compare(None, Some(&hover), &ds), -1);
    assert_eq!(ds.variants.compare(Some(&hover), None, &ds), 1);
    assert_eq!(ds.variants.compare(Some(&hover), Some(&hover), &ds), 0);
}

#[test]
fn breakpoints_read_the_theme_as_upstream() {
    let theme = Theme::new();
    theme.add("--breakpoint-sm", "40rem", theme::NONE).unwrap();
    theme.add("--container-sm", "24rem", theme::NONE).unwrap();
    let ds = build_design_system(theme);

    // `sm` keeps the value of the time of the registration. `min-*` and
    // `@*` read the theme at the time of the call.
    ds.theme.add("--breakpoint-sm", "50rem", theme::NONE).unwrap();
    ds.theme.add("--breakpoint-md", "60rem", theme::NONE).unwrap();
    ds.theme.add("--container-sm", "30rem", theme::NONE).unwrap();

    let class = |name: &str| format!(".{}", tailwind::utils::escape::escape(name));
    let media = |name: &str, params: &str| format!("{} {{\n  @media {params} {{\n    color: red;\n  }}\n}}\n", class(name));

    assert_eq!(css_of(&ds, "sm:[color:red]"), media("sm:[color:red]", "(width >= 40rem)"));
    assert_eq!(css_of(&ds, "min-sm:[color:red]"), media("min-sm:[color:red]", "(width >= 50rem)"));
    assert_eq!(css_of(&ds, "max-md:[color:red]"), media("max-md:[color:red]", "(width < 60rem)"));
    assert_eq!(css_of(&ds, "md:[color:red]"), "");
    assert_eq!(
        css_of(&ds, "@sm:[color:red]"),
        format!("{} {{\n  @container (width >= 30rem) {{\n    color: red;\n  }}\n}}\n", class("@sm:[color:red]"))
    );
}

#[test]
fn group_and_peer_use_the_prefix_of_the_theme() {
    let mut theme = Theme::new();
    theme.prefix = Some("tw".into());
    let ds = build_design_system(theme);
    let rules = compile(&ds, "tw:group-focus/item:peer-focus:[color:red]");
    assert_eq!(rules.len(), 1);
    assert_eq!(
        rules[0].0,
        ".tw\\:group-focus\\/item\\:peer-focus\\:\\[color\\:red\\] {\n  :is(:where(.tw\\:group\\/item):focus &) {\n    :is(:where(.tw\\:peer):focus ~ &) {\n      color: red;\n    }\n  }\n}\n"
    );
}

#[test]
fn the_built_in_variants_are_in_the_upstream_sequence() {
    let theme = Theme::new();
    theme.add("--breakpoint-sm", "40rem", theme::NONE).unwrap();
    theme.add("--breakpoint-md", "48rem", theme::NONE).unwrap();
    let variants = create_variants(&theme);

    let names: Vec<&str> = variants.keys().map(String::as_str).collect();
    assert_eq!(&names[..6], ["*", "**", "not", "group", "peer", "first-letter"]);
    assert_eq!(names.last(), Some(&"noscript"));

    let order = |name: &str| variants.get(name).unwrap().order;
    assert_eq!(order("*"), 1);
    assert_eq!(order("**"), 2);
    // The variants of one group have one order.
    assert_eq!(order("sm"), order("md"));
    assert_eq!(order("sm"), order("min"));
    assert_eq!(order("max") + 1, order("min"));
    assert_eq!(order("@max") + 1, order("@"));
    assert_eq!(order("@"), order("@min"));
    assert_eq!(order("@min") + 1, order("portrait"));
    assert_eq!(variants.compare_fns.len(), 4);

    assert_eq!(variants.kind("hover"), Some(VariantKind::Static));
    assert_eq!(variants.kind("aria"), Some(VariantKind::Functional));
    assert_eq!(variants.kind("not"), Some(VariantKind::Compound));
    assert_eq!(variants.kind("sm"), Some(VariantKind::Static));
    assert_eq!(variants.kind("lg"), None);

    let compounds = |name: &str| variants.get(name).unwrap().compounds;
    assert_eq!(compounds("*"), COMPOUNDS_NEVER);
    assert_eq!(compounds("before"), COMPOUNDS_NEVER);
    assert_eq!(compounds("marker"), COMPOUNDS_NEVER);
    assert_eq!(compounds("starting"), COMPOUNDS_NEVER);
    assert_eq!(compounds("hover"), COMPOUNDS_STYLE_RULES);
    assert_eq!(compounds("dark"), COMPOUNDS_AT_RULES);
    assert_eq!(compounds("supports"), COMPOUNDS_AT_RULES);
    assert_eq!(compounds("sm"), COMPOUNDS_AT_RULES);

    assert!(variants.compounds_with_name("not", "dark"));
    assert!(!variants.compounds_with_name("group", "dark"));
    assert!(variants.compounds_with_name("group", "hover"));
    assert!(!variants.compounds_with_name("hover", "hover"));
    assert_eq!(variants.entries().count(), names.len());
}

#[test]
fn compounds_for_selectors_as_the_upstream_test() {
    let cases: [(&[&str], u8); 10] = [
        // Style rules
        (&[".foo"], COMPOUNDS_STYLE_RULES),
        (&["&:is(:hover)"], COMPOUNDS_STYLE_RULES),
        // Conditional at-rules
        (&["@media foo"], COMPOUNDS_AT_RULES),
        (&["@supports foo"], COMPOUNDS_AT_RULES),
        (&["@container foo"], COMPOUNDS_AT_RULES),
        // The two kinds
        (&[".foo", "@media foo"], COMPOUNDS_STYLE_RULES | COMPOUNDS_AT_RULES),
        // A pseudo-element or an at-rule with no condition: never
        (&[".foo::before"], COMPOUNDS_NEVER),
        (&["@starting-style"], COMPOUNDS_NEVER),
        (&[".foo", "@media foo", ".foo::before"], COMPOUNDS_NEVER),
        (&[".foo", "@media foo", "@starting-style"], COMPOUNDS_NEVER),
    ];
    for (selectors, expected) in cases {
        assert_eq!(compounds_for_selectors(selectors), expected, "{selectors:?}");
    }
}

#[test]
fn valid_variant_names() {
    for name in ["a", "0", "hover", "foo-bar", "foo_bar", "fooBar", "a-B_c9", "@", "@foo", "@3xl", "2xl", "a--b", "@a"] {
        assert_eq!(is_valid_variant_name(name), name != "@", "{name}");
    }
    for name in [
        "", "@", "@@foo", "Foo", "-foo", "_foo", "foo-", "foo_", "foo--", "@foo-", "@-foo", "foo bar", "foo:bar",
        "foo.bar", "foo/bar", "foo@", "fo\u{e9}", "@Foo", "foo\n", "[foo]", "*",
    ] {
        assert!(!is_valid_variant_name(name), "{name:?}");
    }
}
