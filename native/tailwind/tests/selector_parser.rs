//! Tests of the selector parser: `selector-parser.test.ts` of Tailwind CSS,
//! and property tests.

use proptest::prelude::*;
use tailwind::selector_parser::*;
use tailwind::walk::{walk, WalkAction};

#[test]
fn parse_a_simple_selector() {
    assert_eq!(parse(".foo"), vec![selector(".foo")]);
}

#[test]
fn parse_a_compound_selector() {
    assert_eq!(
        parse(".foo.bar:hover#id"),
        vec![compound(vec![selector(".foo"), selector(".bar"), selector(":hover"), selector("#id")])]
    );
}

#[test]
fn parse_a_pseudo_element_selector_with_double() {
    assert_eq!(parse(".foo::before"), vec![compound(vec![selector(".foo"), selector("::before")])]);
}

#[test]
fn parse_a_selector_list() {
    assert_eq!(parse(".foo,.bar"), vec![list(vec![selector(".foo"), selector(".bar")])]);
}

#[test]
fn safely_parse_an_invalid_selector_list() {
    assert_eq!(parse(".foo,"), vec![list(vec![selector(".foo"), compound(vec![])])]);
    assert_eq!(parse(",.foo"), vec![list(vec![compound(vec![]), selector(".foo")])]);
    assert_eq!(parse(",.foo,"), vec![list(vec![compound(vec![]), selector(".foo"), compound(vec![])])]);
    assert_eq!(parse(".foo,,.bar"), vec![list(vec![selector(".foo"), compound(vec![]), selector(".bar")])]);
}

#[test]
fn parse_selector_lists_with_whitespace_around_the_comma() {
    assert_eq!(parse(".foo, .bar"), vec![list(vec![selector(".foo"), selector(".bar")])]);
    assert_eq!(parse(".foo , .bar"), vec![list(vec![selector(".foo"), selector(".bar")])]);
    assert_eq!(parse(".foo ,.bar"), vec![list(vec![selector(".foo"), selector(".bar")])]);
}

#[test]
fn parse_a_list_of_attribute_selectors() {
    assert_eq!(parse("[foo],[bar]"), vec![list(vec![selector("[foo]"), selector("[bar]")])]);
}

#[test]
fn parse_a_list_of_selectors_with_just_functions() {
    assert_eq!(
        parse(":is(.a),:is(.b)"),
        vec![list(vec![fun(":is", vec![selector(".a")]), fun(":is", vec![selector(".b")])])]
    );
}

#[test]
fn parse_selector_lists_with_more_than_two_selectors() {
    assert_eq!(
        parse(".foo,.bar,.baz"),
        vec![list(vec![selector(".foo"), selector(".bar"), selector(".baz")])]
    );
}

#[test]
fn group_selectors_with_combinators_in_a_selector_list() {
    assert_eq!(
        parse(".a+.b, .c .d[attr], .e"),
        vec![list(vec![
            complex(vec![selector(".a"), combinator("+"), selector(".b")]),
            complex(vec![
                selector(".c"),
                combinator(" "),
                compound(vec![selector(".d"), selector("[attr]")])
            ]),
            selector(".e")
        ])]
    );
}

#[test]
fn parse_complex_selectors_in_a_selector_list() {
    assert_eq!(
        parse("#a.b > .c, .d"),
        vec![list(vec![
            complex(vec![compound(vec![selector("#a"), selector(".b")]), combinator(">"), selector(".c")]),
            selector(".d")
        ])]
    );
}

#[test]
fn combine_everything_within_attribute_selectors() {
    assert_eq!(parse(".foo[bar=\"baz\"]"), vec![compound(vec![selector(".foo"), selector("[bar=\"baz\"]")])]);
}

#[test]
fn parse_functions() {
    assert_eq!(
        parse(".foo:hover:not(.bar:focus)"),
        vec![compound(vec![
            selector(".foo"),
            selector(":hover"),
            fun(":not", vec![compound(vec![selector(".bar"), selector(":focus")])])
        ])]
    );
}

#[test]
fn parse_selector_lists_in_functions() {
    assert_eq!(
        parse(":is(.foo, .bar)"),
        vec![fun(":is", vec![list(vec![selector(".foo"), selector(".bar")])])]
    );
}

#[test]
fn parse_selector_lists_in_vendor_prefixed_functions_moz_any() {
    assert_eq!(
        parse(":-moz-any(&, .foo)"),
        vec![fun(":-moz-any", vec![list(vec![selector("&"), selector(".foo")])])]
    );
}

#[test]
fn parse_selector_lists_in_vendor_prefixed_functions_webkit_any() {
    assert_eq!(
        parse(":-webkit-any(&, .foo)"),
        vec![fun(":-webkit-any", vec![list(vec![selector("&"), selector(".foo")])])]
    );
}

#[test]
fn handle_next_children_combinator() {
    assert_eq!(parse(".foo + p"), vec![complex(vec![selector(".foo"), combinator("+"), selector("p")])]);
}

#[test]
fn normalize_combinators() {
    assert_eq!(
        parse(".foo + .bar"),
        vec![complex(vec![selector(".foo"), combinator("+"), selector(".bar")])]
    );
    assert_eq!(
        parse(".foo  \n\t .bar"),
        vec![complex(vec![selector(".foo"), combinator(" "), selector(".bar")])]
    );
}

#[test]
fn handle_escaped_characters() {
    assert_eq!(parse("foo\\.bar"), vec![selector("foo\\.bar")]);
}

#[test]
fn parses_nth_child() {
    assert_eq!(parse(":nth-child(n+1)"), vec![fun(":nth-child", vec![value("n+1")])]);
    assert_eq!(parse(":nth-child(2 of &)"), vec![fun(":nth-child", vec![value("2 of "), selector("&")])]);
    assert_eq!(
        parse(":nth-child(2n + 1 of .foo, .bar)"),
        vec![fun(":nth-child", vec![value("2n + 1 of "), list(vec![selector(".foo"), selector(".bar")])])]
    );
}

#[test]
fn parses_has_child_nth_child_2() {
    assert_eq!(
        parse("&:has(.child:nth-child(2))"),
        vec![compound(vec![
            selector("&"),
            fun(":has", vec![compound(vec![selector(".child"), fun(":nth-child", vec![value("2")])])])
        ])]
    );
}

#[test]
fn parses_has_nth_child_2() {
    assert_eq!(
        parse("&:has(:nth-child(2))"),
        vec![compound(vec![selector("&"), fun(":has", vec![fun(":nth-child", vec![value("2")])])])]
    );
}

#[test]
fn parses_nesting_selector_before_attribute_selector() {
    assert_eq!(parse("&[data-foo]"), vec![compound(vec![selector("&"), selector("[data-foo]")])]);
}

#[test]
fn parses_nesting_selector_after_an_attribute_selector() {
    assert_eq!(parse("[data-foo]&"), vec![compound(vec![selector("[data-foo]"), selector("&")])]);
}

#[test]
fn parses_universal_selector_before_attribute_selector() {
    assert_eq!(parse("*[data-foo]"), vec![compound(vec![selector("*"), selector("[data-foo]")])]);
}

#[test]
fn parses_the_universal_selector_after_an_attribute_selector() {
    assert_eq!(parse("[data-foo]*"), vec![compound(vec![selector("[data-foo]"), selector("*")])]);
}

#[test]
fn parses_another_attribute_selector_after_an_attribute_selector() {
    assert_eq!(
        parse("[data-foo][data-bar]"),
        vec![compound(vec![selector("[data-foo]"), selector("[data-bar]")])]
    );
}

#[test]
fn parses_a_type_selector_before_an_attribute_selector() {
    assert_eq!(parse("div[data-foo]"), vec![compound(vec![selector("div"), selector("[data-foo]")])]);
}

#[test]
fn parses_a_type_selector_after_an_attribute_selector() {
    assert_eq!(parse("[data-foo]div"), vec![compound(vec![selector("[data-foo]"), selector("div")])]);
}

#[test]
fn parse_selector_lists_as_real_selectors() {
    assert_eq!(
        parse(".foo[attr], .bar#id, .baz + .qux"),
        vec![list(vec![
            compound(vec![selector(".foo"), selector("[attr]")]),
            compound(vec![selector(".bar"), selector("#id")]),
            complex(vec![selector(".baz"), combinator("+"), selector(".qux")])
        ])]
    );
}

#[test]
fn to_css_print_a_simple_selector() {
    assert_eq!(to_css(&parse(".foo"), false), ".foo");
}

#[test]
fn to_css_print_a_compound_selector() {
    assert_eq!(to_css(&parse(".foo.bar:hover#id"), false), ".foo.bar:hover#id");
}

#[test]
fn to_css_print_a_selector_list() {
    assert_eq!(to_css(&parse(".foo, .bar"), false), ".foo, .bar");
}

#[test]
fn to_css_print_a_selector_list_with_normalized_commas() {
    assert_eq!(to_css(&parse(".foo,.bar"), false), ".foo, .bar");
    assert_eq!(to_css(&parse(".foo, .bar"), false), ".foo, .bar");
    assert_eq!(to_css(&parse(".foo , .bar"), false), ".foo, .bar");
    assert_eq!(to_css(&parse(".foo ,.bar"), false), ".foo, .bar");
}

#[test]
fn to_css_print_a_selector_list_but_minimized() {
    assert_eq!(to_css(&parse(".foo, .bar"), true), ".foo,.bar");
    assert_eq!(to_css(&parse(".foo , .bar"), true), ".foo,.bar");
    assert_eq!(to_css(&parse(".foo ,.bar"), true), ".foo,.bar");
}

#[test]
fn to_css_print_an_attribute_selectors() {
    assert_eq!(to_css(&parse(".foo[bar=\"baz\"]"), false), ".foo[bar=\"baz\"]");
}

#[test]
fn to_css_print_a_function() {
    assert_eq!(to_css(&parse(".foo:hover:not(.bar:focus)"), false), ".foo:hover:not(.bar:focus)");
}

#[test]
fn to_css_print_escaped_characters() {
    assert_eq!(to_css(&parse("foo\\.bar"), false), "foo\\.bar");
}

#[test]
fn to_css_print_nth_child() {
    assert_eq!(to_css(&parse(":nth-child(n+1)"), false), ":nth-child(n+1)");
    assert_eq!(to_css(&parse(":nth-child(+2)"), false), ":nth-child(+2)");
    assert_eq!(to_css(&parse(":nth-child(2n + 1 of .foo,.bar)"), false), ":nth-child(2n + 1 of .foo, .bar)");
}

#[test]
fn to_css_pretty_print_a_complex_selector() {
    assert_eq!(to_css(&parse(".a+.b, .c .d[attr], .e"), false), ".a + .b, .c .d[attr], .e");
}

#[test]
fn to_css_minify_a_complex_selector() {
    assert_eq!(to_css(&parse(".a+.b, .c .d[attr], .e"), true), ".a+.b,.c .d[attr],.e");
}

#[test]
fn walk_can_replace_a_function_call() {
    let mut ast = parse(".foo:hover:not(.bar:focus)");
    walk(&mut ast, |node, _| match node {
        SelectorAstNode::Function(f) if f.value == ":not" => {
            WalkAction::Replace(vec![selector(".inverted-bar")])
        }
        _ => WalkAction::Continue,
    });
    assert_eq!(to_css(&ast, false), ".foo:hover.inverted-bar");
}

// The tests below are not in the upstream file.

#[test]
fn predicates_find_the_kind_of_a_selector() {
    assert!(is_universal_selector(&selector("*")));
    assert!(is_nesting_selector(&selector("&")));
    assert!(is_class_selector(&selector(".a")));
    assert!(is_id_selector(&selector("#a")));
    assert!(is_pseudo_selector(&selector(":hover")));
    assert!(is_pseudo_selector(&selector("::before")));
    assert!(is_attribute_selector(&selector("[a]")));
    assert!(is_type_selector(&selector("div")));
    assert!(is_type_selector(&selector("")));
    assert!(!is_type_selector(&selector(".a")));
    assert!(!is_type_selector(&fun("div", vec![])));
    assert!(!is_class_selector(&value(".a")));
    assert!(!is_class_selector(&fun(".a", vec![])));
}

#[test]
fn pseudo_elements_have_two_colons_or_an_old_name() {
    assert!(is_pseudo_element(&selector("::before")));
    assert!(is_pseudo_element(&selector(":before")));
    assert!(is_pseudo_element(&selector(":AFTER")));
    assert!(is_pseudo_element(&selector(":First-Line")));
    assert!(is_pseudo_element(&selector(":first-letter")));
    assert!(is_pseudo_element(&fun("::part", vec![value("a")])));
    assert!(!is_pseudo_element(&selector(":hover")));
    assert!(!is_pseudo_element(&value("::before")));
    assert!(!is_pseudo_element(&combinator("::before")));
}

#[test]
fn clone_ast_node_makes_an_equal_node() {
    let ast = parse(".a > :is(.b, .c):nth-child(2n of .d)");
    let copy: Vec<_> = ast.iter().map(clone_ast_node).collect();
    assert_eq!(ast, copy);
}

#[test]
fn a_backslash_at_the_end_gets_the_text_undefined() {
    assert_eq!(parse(".a\\"), vec![selector(".a\\undefined")]);
}

#[test]
fn a_function_with_no_end_stays_in_the_tree() {
    assert_eq!(parse(":is(.a"), vec![fun(":is", vec![selector(".a")])]);
    assert_eq!(parse(".a > :is(.b"), vec![selector(".a"), combinator(">"), fun(":is", vec![selector(".b")])]);
    // The outer list loses the selector that is not complete.
    assert_eq!(parse(".a, :is(.b"), vec![list(vec![selector(".a")])]);
    assert_eq!(parse("foo(bar"), vec![compound(vec![fun("foo", vec![value("")]), selector("bar")])]);
}

#[test]
fn an_extra_close_parenthesis_ends_the_selector_at_the_root() {
    assert_eq!(parse(".a)"), vec![selector(".a")]);
    assert_eq!(
        parse(".a .b).c"),
        vec![compound(vec![complex(vec![selector(".a"), combinator(" "), selector(".b")]), selector(".c")])]
    );
    assert_eq!(parse(".a,.b).c"), vec![list(vec![selector(".a"), selector(".b")]), selector(".c")]);
}

#[test]
fn an_attribute_selector_with_no_end_is_only_the_bracket() {
    assert_eq!(parse("[a"), vec![compound(vec![selector("["), selector("a")])]);
}

#[test]
fn a_windows_line_end_is_a_descendant_combinator() {
    assert_eq!(parse(".a\r\n.b"), vec![complex(vec![selector(".a"), combinator(" "), selector(".b")])]);
}

#[test]
fn combinators_with_no_selector_between_are_one_combinator() {
    assert_eq!(parse(".a > + .b"), vec![complex(vec![selector(".a"), combinator("> +"), selector(".b")])]);
    assert_eq!(to_css(&parse(".a > + .b"), false), ".a > + .b");
    assert_eq!(to_css(&parse(".a > + .b"), true), ".a> +.b");
}

#[test]
fn nth_child_finds_the_first_of_in_the_contents() {
    assert_eq!(
        parse(":nth-last-child(odd of .a)"),
        vec![fun(":nth-last-child", vec![value("odd of "), selector(".a")])]
    );
    assert_eq!(parse(":nth-of-type(2 of .a)"), vec![fun(":nth-of-type", vec![value("2 of .a")])]);
}

const SYNTAX: &str = "([a-c.#:&*>+~, \n\t\\[\\]()'\"\\\\é😀-]|:is\\(|:not\\(|:nth-child\\(|of |\r\n){0,24}";

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn parse_does_not_panic(input in any::<String>()) {
        let ast = parse(&input);
        let _ = to_css(&ast, false);
        let _ = to_css(&ast, true);
    }

    #[test]
    fn parse_does_not_panic_with_syntax_characters(input in SYNTAX) {
        let ast = parse(&input);
        let _ = to_css(&ast, false);
        let _ = to_css(&ast, true);
    }

    #[test]
    fn print_is_a_fixed_point_on_the_second_round(input in SYNTAX) {
        let first = to_css(&parse(&input), false);
        let second = to_css(&parse(&first), false);
        prop_assert_eq!(to_css(&parse(&second), false), second);
    }

    #[test]
    fn minified_print_is_a_fixed_point_on_the_second_round(input in SYNTAX) {
        let first = to_css(&parse(&input), true);
        let second = to_css(&parse(&first), true);
        prop_assert_eq!(to_css(&parse(&second), true), second);
    }
}
