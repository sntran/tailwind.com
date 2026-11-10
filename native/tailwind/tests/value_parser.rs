//! Tests of the value parser: `value-parser.test.ts` of Tailwind CSS, and
//! property tests.

use proptest::prelude::*;
use tailwind::value_parser::{fun, parse, separator, to_css, word, ValueAstNode};
use tailwind::walk::{walk, WalkAction};

#[test]
fn parse_a_value() {
    assert_eq!(parse("123px"), vec![word("123px")]);
}

#[test]
fn parse_a_string_value() {
    assert_eq!(parse("'hello world'"), vec![word("'hello world'")]);
}

#[test]
fn parse_a_list() {
    assert_eq!(parse("hello world"), vec![word("hello"), separator(" "), word("world")]);
}

#[test]
fn parse_a_string_with_parentheses() {
    assert_eq!(parse("'hello ( world )'"), vec![word("'hello ( world )'")]);
}

#[test]
fn parse_a_function_with_no_arguments() {
    assert_eq!(parse("theme()"), vec![fun("theme", vec![])]);
}

#[test]
fn parse_a_function_with_a_single_argument() {
    assert_eq!(parse("theme(foo)"), vec![fun("theme", vec![word("foo")])]);
}

#[test]
fn parse_a_function_with_a_single_string_argument() {
    assert_eq!(parse("theme('foo')"), vec![fun("theme", vec![word("'foo'")])]);
}

#[test]
fn parse_a_function_with_multiple_arguments() {
    assert_eq!(parse("theme(foo, bar)"), vec![fun("theme", vec![word("foo"), separator(", "), word("bar")])]);
}

#[test]
fn parse_a_function_with_multiple_arguments_across_lines() {
    assert_eq!(
        parse("theme(\n\tfoo,\n\tbar\n)"),
        vec![fun(
            "theme",
            vec![separator("\n\t"), word("foo"), separator(",\n\t"), word("bar"), separator("\n")]
        )]
    );
}

#[test]
fn parse_a_function_with_nested_arguments() {
    assert_eq!(
        parse("theme(foo, theme(bar))"),
        vec![fun("theme", vec![word("foo"), separator(", "), fun("theme", vec![word("bar")])])]
    );
}

#[test]
fn parse_a_function_with_nested_arguments_separated_by() {
    assert_eq!(
        parse("theme(colors.red.500/var(--opacity))"),
        vec![fun("theme", vec![word("colors.red.500"), word("/"), fun("var", vec![word("--opacity")])])]
    );
}

#[test]
fn handle_calculations() {
    assert_eq!(
        parse("calc((1 + 2) * 3)"),
        vec![fun(
            "calc",
            vec![
                fun("", vec![word("1"), separator(" "), word("+"), separator(" "), word("2")]),
                separator(" "),
                word("*"),
                separator(" "),
                word("3")
            ]
        )]
    );
}

#[test]
fn handle_media_query_params_with_functions() {
    assert_eq!(parse("(min-width: 600px) and (max-width:theme(colors.red.500)) and (theme(--breakpoint-sm)<width<=theme(--breakpoint-md))"), vec![fun("", vec![word("min-width"), separator(": "), word("600px")]), separator(" "), word("and"), separator(" "), fun("", vec![word("max-width"), separator(":"), fun("theme", vec![word("colors.red.500")])]), separator(" "), word("and"), separator(" "), fun("", vec![fun("theme", vec![word("--breakpoint-sm")]), separator("<"), word("width"), separator("<="), fun("theme", vec![word("--breakpoint-md")])])]);
}

#[test]
fn not_error_when_extra_was_passed() {
    assert_eq!(
        parse("calc(1 + 2))"),
        vec![fun("calc", vec![word("1"), separator(" "), word("+"), separator(" "), word("2")])]
    );
}

#[test]
fn to_css_pretty_print_calculations() {
    assert_eq!(to_css(&parse("calc((1 + 2) * 3)")), "calc((1 + 2) * 3)");
}

#[test]
fn to_css_pretty_print_nested_function_calls() {
    assert_eq!(to_css(&parse("theme(foo, theme(bar))")), "theme(foo, theme(bar))");
}

#[test]
fn to_css_pretty_print_media_query_params_with_functions() {
    assert_eq!(
        to_css(&parse("(min-width: 600px) and (max-width:theme(colors.red.500))")),
        "(min-width: 600px) and (max-width:theme(colors.red.500))"
    );
}

#[test]
fn to_css_preserves_multiple_spaces() {
    assert_eq!(to_css(&parse("foo(   bar  )")), "foo(   bar  )");
}

#[test]
fn walk_can_replace_a_function_call() {
    let mut ast = parse("(min-width: 600px) and (max-width: theme(lg))");

    walk(&mut ast, |node, _| match node {
        ValueAstNode::Function(f) if f.value == "theme" => WalkAction::Replace(vec![word("64rem")]),
        _ => WalkAction::Continue,
    });

    assert_eq!(to_css(&ast), "(min-width: 600px) and (max-width: 64rem)");
}

// The tests below are not in the upstream file. They record upstream
// behavior for input that is not correct CSS.

#[test]
fn a_backslash_at_the_end_gets_the_text_undefined() {
    assert_eq!(parse("a\\"), vec![word("a\\undefined")]);
}

#[test]
fn a_function_with_no_end_gets_the_remainder_after_it() {
    assert_eq!(parse("foo(a b"), vec![fun("foo", vec![word("a"), separator(" ")]), word("b")]);
    assert_eq!(to_css(&parse("foo(a b")), "foo(a )b");
    assert_eq!(parse("a(b(c"), vec![fun("a", vec![fun("b", vec![])]), word("c")]);
}

#[test]
fn an_extra_close_parenthesis_drops_the_word_before_it() {
    assert_eq!(parse("a)b"), vec![word("b")]);
    assert_eq!(parse("a b)"), vec![word("a"), separator(" ")]);
}

#[test]
fn a_string_with_no_end_is_only_the_quote() {
    assert_eq!(parse("'a b"), vec![word("'a"), separator(" "), word("b")]);
    assert_eq!(parse("\"a\\\" b"), vec![word("\"a\\\""), separator(" "), word("b")]);
}

#[test]
fn a_windows_line_end_becomes_a_newline() {
    assert_eq!(parse("a\r\nb"), vec![word("a"), separator("\n"), word("b")]);
    assert_eq!(parse("a\rb"), vec![word("a\rb")]);
}

#[test]
fn an_escape_keeps_the_next_character() {
    assert_eq!(parse("a\\(b\\ c\\😀d"), vec![word("a\\(b\\ c\\😀d")]);
}

#[test]
fn text_that_is_not_ascii_stays_in_words() {
    assert_eq!(parse("é(ü, 😀)"), vec![fun("é", vec![word("ü"), separator(", "), word("😀")])]);
}

/// `true` when upstream prints the input again with no change.
///
/// Upstream changes the input in these conditions:
/// - `\r\n` becomes `\n`.
/// - A `)` with no `(` goes away, with the word before it.
/// - A `(` with no `)` gets a `)`.
/// - A backslash at the end gets the text `undefined`.
fn is_round_trip_input(input: &str) -> bool {
    if input.contains("\r\n") {
        return false;
    }
    let bytes = input.as_bytes();
    let mut depth = 0usize;
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => {
                if i + 1 == bytes.len() {
                    return false;
                }
                i += 2;
            }
            quote @ (b'"' | b'\'') => {
                // A string with no end is only the quote.
                let mut j = i + 1;
                let mut end = i;
                while j < bytes.len() {
                    if bytes[j] == b'\\' {
                        j += 2;
                    } else if bytes[j] == quote {
                        end = j;
                        break;
                    } else {
                        j += 1;
                    }
                }
                i = end + 1;
            }
            b'(' => {
                depth += 1;
                i += 1;
            }
            b')' => {
                if depth == 0 {
                    return false;
                }
                depth -= 1;
                i += 1;
            }
            _ => i += 1,
        }
    }
    depth == 0
}

/// A value with balanced parentheses, complete strings and complete escapes.
fn balanced_value() -> impl Strategy<Value = String> {
    let leaf = prop_oneof![
        "[a-z0-9.%#é😀-]{1,6}",
        "[ ,:=<>\n\t/]{1,3}",
        "\"([a-z ()']|\\\\.){0,6}\"",
        "'([a-z ()\"]|\\\\.){0,6}'",
        "\\\\[a-z()\\\\ é]",
    ];
    leaf.prop_recursive(4, 32, 6, |inner| {
        prop_oneof![
            prop::collection::vec(inner.clone(), 0..6).prop_map(|parts| parts.concat()),
            ("[a-z-]{0,5}", prop::collection::vec(inner, 0..4))
                .prop_map(|(name, parts)| format!("{name}({})", parts.concat())),
        ]
    })
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(2000))]

    #[test]
    fn parse_does_not_panic(input in any::<String>()) {
        let _ = to_css(&parse(&input));
    }

    #[test]
    fn parse_does_not_panic_with_syntax_characters(input in "[a-c0-9()'\"\\\\,:=<> /\n\r\té😀-]{0,40}") {
        let _ = to_css(&parse(&input));
    }

    #[test]
    fn balanced_input_prints_with_no_change(input in balanced_value()) {
        prop_assert!(is_round_trip_input(&input));
        prop_assert_eq!(to_css(&parse(&input)), input);
    }

    #[test]
    fn input_prints_with_no_change_when_upstream_does_not_normalize(
        input in "[a-c0-9()'\"\\\\,:=<> /\n\r\té😀-]{0,24}"
    ) {
        if is_round_trip_input(&input) {
            prop_assert_eq!(to_css(&parse(&input)), input);
        }
    }

    #[test]
    fn print_is_stable_after_one_round(input in "[a-c0-9()'\"\\\\,:=<> /\n\té😀-]{0,24}") {
        let once = to_css(&parse(&input));
        prop_assert_eq!(to_css(&parse(&once)), once);
    }
}
