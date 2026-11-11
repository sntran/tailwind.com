//! Tests of the parser of attribute selectors:
//! `attribute-selector-parser.test.ts` of Tailwind CSS, and property tests.

use proptest::prelude::*;
use tailwind::attribute_selector_parser::{parse, AttributeSelector};

fn attr<'a>(
    attribute: &'a str,
    operator: Option<&'static str>,
    quote: Option<char>,
    value: Option<&'a str>,
    sensitivity: Option<char>,
) -> Option<AttributeSelector<'a>> {
    Some(AttributeSelector { attribute, operator, quote, value, sensitivity })
}

#[test]
fn parse_an_invalid_attribute_selector_as_none() {
    for input in [
        "",
        "]",
        "[]",
        "[",
        "=\"value\"",
        "data-foo]",
        "[data-foo",
        "[data-foo=\"foo]",
        "[data-foo * = foo]",
        "[data-foo*=]",
        "[data-foo=value x]",
        "[data-foo=value ix]",
    ] {
        assert_eq!(parse(input), None, "{input}");
    }
}

#[test]
fn parse_correctly() {
    assert_eq!(parse("[data-foo]"), attr("data-foo", None, None, None, None));
    assert_eq!(parse("[ data-foo ]"), attr("data-foo", None, None, None, None));
    // Code points that are not ASCII are valid in a CSS identifier.
    assert_eq!(parse("[data-état]"), attr("data-état", None, None, None, None));
    assert_eq!(parse("[data-state=expanded]"), attr("data-state", Some("="), None, Some("expanded"), None));
    assert_eq!(
        parse("[data-state = expanded ]"),
        attr("data-state", Some("="), None, Some("expanded"), None)
    );
    assert_eq!(
        parse("[data-state*=\"expanded\"]"),
        attr("data-state", Some("*="), Some('"'), Some("expanded"), None)
    );
    assert_eq!(
        parse("[data-state*=\"expanded\"i]"),
        attr("data-state", Some("*="), Some('"'), Some("expanded"), Some('i'))
    );
    assert_eq!(
        parse("[data-state*=expanded i]"),
        attr("data-state", Some("*="), None, Some("expanded"), Some('i'))
    );
}

#[test]
fn work_with_a_real_world_example() {
    assert_eq!(
        parse("[data-url$=\".com\"i]"),
        attr("data-url", Some("$="), Some('"'), Some(".com"), Some('i'))
    );
}

// The tests below are not in the upstream file.

#[test]
fn parse_each_operator() {
    for operator in ["=", "~=", "|=", "^=", "$=", "*="] {
        let input = format!("[a{operator}b]");
        assert_eq!(parse(&input), attr("a", Some(operator), None, Some("b"), None));
    }
    assert_eq!(parse("[a!=b]"), None);
    assert_eq!(parse("[a~b]"), None);
}

#[test]
fn parse_the_sensitivity_in_the_two_cases() {
    assert_eq!(parse("[a=b I]"), attr("a", Some("="), None, Some("b"), Some('i')));
    assert_eq!(parse("[a='b' S ]"), attr("a", Some("="), Some('\''), Some("b"), Some('s')));
    assert_eq!(parse("[a=b\ts]"), attr("a", Some("="), None, Some("b"), Some('s')));
}

#[test]
fn a_value_with_quotes_ends_at_the_last_quote() {
    assert_eq!(parse("[a=\"b\"c\"]"), attr("a", Some("="), Some('"'), Some("b\"c"), None));
    assert_eq!(parse("[a=\"b\\\"c\"]"), attr("a", Some("="), Some('"'), Some("b\\\"c"), None));
    assert_eq!(parse("[a=\"]"), attr("a", Some("="), Some('"'), Some(""), None));
    assert_eq!(parse("[a=\"i]"), attr("a", Some("="), Some('"'), Some(""), Some('i')));
}

#[test]
fn an_escape_in_the_name_keeps_the_next_character() {
    assert_eq!(parse("[a\\:b]"), attr("a\\:b", None, None, None, None));
    assert_eq!(parse("[a\\😀b=c]"), attr("a\\😀b", Some("="), None, Some("c"), None));
    assert_eq!(parse("[a\\]"), None);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn parse_does_not_panic(input in any::<String>()) {
        let _ = parse(&input);
    }

    #[test]
    fn parse_does_not_panic_with_syntax_characters(input in "\\[?[a-c \t\n=~|^$*'\"\\\\isIS\\[\\]é😀_-]{0,16}\\]?") {
        let _ = parse(&input);
    }

    #[test]
    fn the_parts_of_a_simple_selector_come_back(
        attribute in "[a-z][a-z0-9_-]{0,8}",
        operator in prop::sample::select(vec!["=", "~=", "|=", "^=", "$=", "*="]),
        value in "[a-z0-9]{1,8}",
        quote in prop::sample::select(vec![None, Some('"'), Some('\'')]),
        sensitivity in prop::sample::select(vec![None, Some('i'), Some('s')]),
    ) {
        let q = quote.map(String::from).unwrap_or_default();
        let s = sensitivity.map(|c| format!(" {c}")).unwrap_or_default();
        let input = format!("[{attribute}{operator}{q}{value}{q}{s}]");
        prop_assert_eq!(
            parse(&input),
            Some(AttributeSelector { attribute: &attribute, operator: Some(operator), quote, value: Some(&value), sensitivity })
        );
    }
}
