//! Tests of the CSS parser.
//!
//! The cases come from `css-parser.test.ts` of Tailwind CSS. Each case runs
//! one time with `\n` line ends and one time with `\r\n` line ends. The
//! upstream cases that test only source locations are not here.

use proptest::prelude::*;
use tailwind::ast::{
    at_root, at_rule, comment, context, decl, decl_full, style_rule, AstNode, ContextMap,
};
use tailwind::css_parser::{parse, parse_at_rule, parse_with_source};
use tailwind::to_css::to_css;

/// A declaration with the `!important` flag.
fn important(property: &str, value: &str) -> AstNode {
    decl_full(property, Some(value.to_string()), true)
}

/// The text with the line ends of the test run.
fn with_eol(text: &str, eol: &str) -> String {
    if eol == "\n" {
        text.to_string()
    } else {
        text.replace("\r\n", "\n").replace('\n', eol)
    }
}

/// The tree with the line ends of the test run in each text.
fn nodes_with_eol(nodes: Vec<AstNode>, eol: &str) -> Vec<AstNode> {
    nodes
        .into_iter()
        .map(|node| match node {
            AstNode::Rule(n) => {
                style_rule(with_eol(&n.selector, eol), nodes_with_eol(n.nodes, eol))
            }
            AstNode::AtRule(n) => at_rule(
                with_eol(&n.name, eol),
                with_eol(&n.params, eol),
                nodes_with_eol(n.nodes, eol),
            ),
            AstNode::Declaration(n) => decl_full(
                with_eol(&n.property, eol),
                n.value.map(|v| with_eol(&v, eol)),
                n.important,
            ),
            AstNode::Comment(n) => comment(with_eol(&n.value, eol)),
            other => other,
        })
        .collect()
}

fn check(eol: &str, input: &str, expected: Vec<AstNode>) {
    let input = with_eol(input, eol);
    assert_eq!(
        parse(&input),
        Ok(nodes_with_eol(expected, eol)),
        "input: {input:?}"
    );
}

fn check_error(eol: &str, input: &str, message: &str) {
    let input = with_eol(input, eol);
    let error = parse(&input).expect_err("the parse must fail");
    assert_eq!(error.message, with_eol(message, eol), "input: {input:?}");
    assert_eq!(error.to_string(), error.message);
}

/// The error without a source, and with the source `input.css`: the
/// message then starts with the position (`parseWithLoc` upstream).
fn check_error_at(eol: &str, input: &str, position: &str, message: &str) {
    check_error(eol, input, message);
    let input = with_eol(input, eol);
    let error = parse_with_source(&input, Some("input.css")).expect_err("the parse must fail");
    assert_eq!(error.message, format!("input.css:{position}: {}", with_eol(message, eol)), "input: {input:?}");
    let loc = error.loc.expect("the location of the error");
    assert_eq!(loc.source.file.as_deref(), Some("input.css"));
    assert_eq!(loc.source.code, input);
}

/// Make two tests for each case: one for each type of line end.
macro_rules! line_end_tests {
    ($($name:ident,)*) => {
        mod unix {
            $(#[test] fn $name() { super::$name("\n"); })*
        }
        mod windows {
            $(#[test] fn $name() { super::$name("\r\n"); })*
        }
    };
}

// Upstream group: comments.

fn parse_a_comment_and_ignore_it(eol: &str) {
    check(eol, "\n          /*Hello, world!*/\n        ", vec![]);
}

fn parse_a_comment_with_an_escaped_end_and_ignore_it(eol: &str) {
    check(
        eol,
        "\n          /*Hello, \\*\\/ world!*/\n        ",
        vec![],
    );
}

fn end_a_comment_at_comment_end_even_when_it_is_preceded_by_a_backslash(eol: &str) {
    check(
        eol,
        "\n          /* C:\\temp\\*/\n          .foo {\n            color: red;\n          }\n        ",
        vec![
            style_rule(".foo", vec![
                decl("color", "red"),
            ]),
        ],
    );
}

fn parse_a_comment_inside_of_a_selector_and_ignore_it(eol: &str) {
    check(
        eol,
        "\n          .foo {\n            /*Example comment*/\n          }\n        ",
        vec![style_rule(".foo", vec![])],
    );
}

fn remove_comments_in_between_selectors_and_keep_the_correct_whitespace(eol: &str) {
    check(
        eol,
        "\n          .foo/*.bar*/.baz {\n          }\n          .foo/*.bar*//*.baz*/.qux\n          {\n          }\n          .foo/*.bar*/ /*.baz*/.qux {\n            /*        ^ whitespace */\n          }\n          .foo /*.bar*/.baz {\n            /*^ whitespace */\n          }\n          .foo/*.bar*/ .baz {\n            /*        ^ whitespace */\n          }\n          .foo/*.bar*/\n          .baz {\n          }\n        ",
        vec![
            style_rule(".foo.baz", vec![]),
            style_rule(".foo.qux", vec![]),
            style_rule(".foo .qux", vec![]),
            style_rule(".foo .baz", vec![]),
            style_rule(".foo .baz", vec![]),
            style_rule(".foo .baz", vec![]),
        ],
    );
}

fn collect_license_comments(eol: &str) {
    check(
        eol,
        "\n          /*! License #1 */\n          /*!\n            * License #2\n            */\n        ",
        vec![
            comment("! License #1 "),
            comment("!\n            * License #2\n            "),
        ],
    );
}

fn hoist_all_license_comments(eol: &str) {
    check(
        eol,
        "\n          /*! License #1 */\n          .foo {\n            color: red; /*! License #1.5 */\n          }\n          /*! License #2 */\n          .bar {\n            /*! License #2.5 */\n            color: blue;\n          }\n          /*! License #3 */\n        ",
        vec![
            comment("! License #1 "),
            comment("! License #1.5 "),
            comment("! License #2 "),
            comment("! License #2.5 "),
            comment("! License #3 "),
            style_rule(".foo", vec![
                decl("color", "red"),
            ]),
            style_rule(".bar", vec![
                decl("color", "blue"),
            ]),
        ],
    );
}

fn handle_comments_before_element_selectors(eol: &str) {
    check(
        eol,
        "\n          .dark /* comment */p {\n            color: black;\n          }\n        ",
        vec![style_rule(".dark p", vec![decl("color", "black")])],
    );
}

// Upstream group: declarations.

fn parse_a_simple_declaration(eol: &str) {
    check(
        eol,
        "\n          color: red;\n        ",
        vec![decl("color", "red")],
    );
}

fn parse_a_declaration_with_no_last_semicolon(eol: &str) {
    check(eol, "color: red", vec![decl("color", "red")]);
}

fn parse_a_declaration_with_no_last_semicolon_and_a_newline_at_the_end(eol: &str) {
    check(eol, "color: red\n", vec![decl("color", "red")]);
}

fn parse_a_declaration_with_no_last_semicolon_and_important(eol: &str) {
    check(
        eol,
        "color: red !important",
        vec![important("color", "red")],
    );
}

fn parse_multiple_declarations_where_the_last_one_has_no_last_semicolon(eol: &str) {
    check(eol, "a: 1; b: 2", vec![decl("a", "1"), decl("b", "2")]);
}

fn parse_a_declaration_with_no_last_semicolon_after_a_custom_property(eol: &str) {
    check(
        eol,
        "--a: 1;\ncolor: red",
        vec![decl("--a", "1"), decl("color", "red")],
    );
}

fn not_parse_a_declaration_with_unbalanced_brackets_and_no_last_semicolon(eol: &str) {
    check(eol, "color: rgb(1, 2", vec![]);
}

fn parse_declarations_with_strings(eol: &str) {
    check(
        eol,
        "\n          content: 'Hello, world!';\n        ",
        vec![decl("content", "'Hello, world!'")],
    );
}

fn parse_declarations_with_nested_strings(eol: &str) {
    check(
        eol,
        "\n          content: 'Good, \"monday\", morning!';\n        ",
        vec![decl("content", "'Good, \"monday\", morning!'")],
    );
}

fn parse_declarations_with_nested_strings_that_are_not_balanced(eol: &str) {
    check(
        eol,
        "\n          content: \"It's a beautiful day!\";\n        ",
        vec![decl("content", "\"It's a beautiful day!\"")],
    );
}

fn parse_declarations_with_strings_and_escaped_string_ends(eol: &str) {
    check(
        eol,
        "\n          content: 'These are not the end \"\\' of the string';\n        ",
        vec![decl(
            "content",
            "'These are not the end \"\\' of the string'",
        )],
    );
}

// Upstream group: declarations / important.

fn parse_declarations_with_important(eol: &str) {
    check(
        eol,
        "\n            width: 123px !important;\n          ",
        vec![important("width", "123px")],
    );
}

fn parse_declarations_with_important_when_a_comment_follows(eol: &str) {
    check(
        eol,
        "\n            width: 123px !important /* Very important */;\n          ",
        vec![important("width", "123px")],
    );
}

fn parse_custom_properties_with_important_when_a_comment_follows(eol: &str) {
    check(
        eol,
        "\n            --foo: red !important /* Very important */;\n            --bar: red /* not important */;\n          ",
        vec![
            important("--foo", "red"),
            decl("--bar", "red /* not important */"),
        ],
    );
}

fn not_treat_important_inside_of_a_comment_as_the_important_flag(eol: &str) {
    check(
        eol,
        "\n            --foo: red /* !important /* example */;\n            --bar: red /* example */ !important /* example */ /* example */;\n            --baz: '/* example' !important;\n            --quux: red !important\n              /* example */;\n          ",
        vec![
            decl("--foo", "red /* !important /* example */"),
            important("--bar", "red /* example */"),
            important("--baz", "'/* example'"),
            important("--quux", "red"),
        ],
    );
}

fn not_treat_important_inside_of_a_value_as_the_important_flag(eol: &str) {
    check(
        eol,
        "\n            content: 'a !important b';\n            background: url('x!important.png') !important;\n            --foo: '!important';\n          ",
        vec![
            decl("content", "'a !important b'"),
            important("background", "url('x!important.png')"),
            decl("--foo", "'!important'"),
        ],
    );
}

fn keep_an_escaped_whitespace_at_the_end_of_the_value(eol: &str) {
    check(
        eol,
        "\n            content: a\\ ;\n            font-family: a\\ !important;\n            --foo: a\\ ;\n            --bar: a\\ !important;\n            --baz: a\\\\ !important;\n          ",
        vec![
            decl("content", "a\\ "),
            important("font-family", "a\\ "),
            decl("--foo", "a\\ "),
            important("--bar", "a\\ "),
            important("--baz", "a\\\\"),
        ],
    );
}

fn not_treat_an_escaped_exclamation_mark_as_the_important_flag(eol: &str) {
    check(
        eol,
        "\n            content: \\!important;\n            --foo: red\\!important;\n            --bar: red\\\\!important;\n          ",
        vec![
            decl("content", "\\!important"),
            decl("--foo", "red\\!important"),
            important("--bar", "red\\\\"),
        ],
    );
}

// Upstream group: declarations / Custom properties.

fn parse_a_custom_property(eol: &str) {
    check(
        eol,
        "\n            --foo: bar;\n          ",
        vec![decl("--foo", "bar")],
    );
}

fn parse_a_minified_custom_property(eol: &str) {
    check(
        eol,
        ":root{--foo:bar;}",
        vec![style_rule(":root", vec![decl("--foo", "bar")])],
    );
}

fn parse_a_minified_custom_property_with_no_semicolon(eol: &str) {
    check(
        eol,
        ":root{--foo:bar}",
        vec![style_rule(":root", vec![decl("--foo", "bar")])],
    );
}

fn parse_a_custom_property_with_no_last_semicolon(eol: &str) {
    check(
        eol,
        "\n            --foo: bar\n          ",
        vec![decl("--foo", "bar")],
    );
}

fn parse_a_custom_property_with_no_last_semicolon_and_important(eol: &str) {
    check(
        eol,
        "\n            --foo: bar !important\n          ",
        vec![important("--foo", "bar")],
    );
}

fn parse_a_custom_property_with_no_last_semicolon_at_the_end_of_the_input(eol: &str) {
    check(eol, "--foo: bar", vec![decl("--foo", "bar")]);
    check(
        eol,
        "--foo: bar !important",
        vec![important("--foo", "bar")],
    );
    check(eol, "--foo: var(--bar)", vec![decl("--foo", "var(--bar)")]);
    check(eol, "--foo: 'bar'", vec![decl("--foo", "'bar'")]);
    check(eol, "--foo: {}", vec![decl("--foo", "{}")]);
    check(
        eol,
        "--foo: bar /* baz */",
        vec![decl("--foo", "bar /* baz */")],
    );
    check(
        eol,
        "--foo: bar;\n--baz: rgb(0 0 0)",
        vec![decl("--foo", "bar"), decl("--baz", "rgb(0 0 0)")],
    );
}

fn parse_a_custom_property_with_an_unclosed_bracket_at_the_end_of_the_input(eol: &str) {
    check(eol, "--foo: [bar", vec![decl("--foo", "[bar")]);
    check(
        eol,
        "color: red; --foo: (bar",
        vec![decl("color", "red"), decl("--foo", "(bar")],
    );
}

fn parse_a_custom_property_with_code_of_a_different_language(eol: &str) {
    check(
        eol,
        "\n            --foo: if(x > 5) this.width = 10;\n          ",
        vec![decl("--foo", "if(x > 5) this.width = 10")],
    );
}

fn parse_a_custom_property_with_an_empty_block_as_the_value(eol: &str) {
    check(eol, "--foo: {};", vec![decl("--foo", "{}")]);
}

fn parse_a_custom_property_with_an_empty_value(eol: &str) {
    check(eol, "--foo:;", vec![decl("--foo", "")]);
}

fn parse_a_custom_property_with_a_space_value(eol: &str) {
    check(eol, "--foo: ;", vec![decl("--foo", "")]);
}

fn parse_a_custom_property_with_a_block_with_nested_css(eol: &str) {
    check(
        eol,
        "\n            --foo: {\n              background-color: red;\n              /* A comment */\n              content: 'Hello, world!';\n            };\n          ",
        vec![
            decl("--foo", "{\n              background-color: red;\n              /* A comment */\n              content: 'Hello, world!';\n            }"),
        ],
    );
}

fn parse_a_custom_property_with_a_block_with_nested_css_and_comments_with_end_characters_inside_them(
    eol: &str,
) {
    check(
        eol,
        "\n            --foo: {\n              background-color: red;\n              /* A comment ; */\n              content: 'Hello, world!';\n            };\n            --bar: {\n              background-color: red;\n              /* A comment } */\n              content: 'Hello, world!';\n            };\n          ",
        vec![
            decl("--foo", "{\n              background-color: red;\n              /* A comment ; */\n              content: 'Hello, world!';\n            }"),
            decl("--bar", "{\n              background-color: red;\n              /* A comment } */\n              content: 'Hello, world!';\n            }"),
        ],
    );
}

fn parse_a_custom_property_with_escaped_characters_in_the_value(eol: &str) {
    check(
        eol,
        "\n            --foo: This is not the end \\;, but this is;\n          ",
        vec![decl("--foo", "This is not the end \\;, but this is")],
    );
}

fn parse_a_custom_property_with_escaped_characters_inside_a_comment_in_the_value(eol: &str) {
    check(
        eol,
        "\n            --foo: /* This is not the end \\; this is also not the end ; */ but this is;\n          ",
        vec![
            decl("--foo", "/* This is not the end \\; this is also not the end ; */ but this is"),
        ],
    );
}

fn end_a_comment_in_a_custom_property_at_comment_end_even_when_it_is_preceded_by_a_backslash(
    eol: &str,
) {
    check(
        eol,
        "\n            --foo: /* C:\\temp\\*/ bar;\n            --bar: /* baz */ qux;\n          ",
        vec![
            decl("--foo", "/* C:\\temp\\*/ bar"),
            decl("--bar", "/* baz */ qux"),
        ],
    );
}

fn parse_empty_custom_properties(eol: &str) {
    check(
        eol,
        "\n            --foo: ;\n          ",
        vec![decl("--foo", "")],
    );
}

fn parse_custom_properties_with_important(eol: &str) {
    check(
        eol,
        "\n            --foo: bar !important;\n          ",
        vec![important("--foo", "bar")],
    );
}

fn parse_custom_properties_with_data_url_value(eol: &str) {
    check(
        eol,
        "\n            --foo: 'data:text/plain;base64,SGVsbG8sIFdvcmxkIQ==';\n          ",
        vec![decl(
            "--foo",
            "'data:text/plain;base64,SGVsbG8sIFdvcmxkIQ=='",
        )],
    );
}

// Upstream group: declarations.

fn parse_multiple_declarations(eol: &str) {
    check(
        eol,
        "\n          color: red;\n          background-color: blue;\n        ",
        vec![decl("color", "red"), decl("background-color", "blue")],
    );
}

fn correctly_parse_comments_with_colon_inside_of_them(eol: &str) {
    check(
        eol,
        "\n          color/* color: #f00; */: red;\n          font-weight:/* font-size: 12px */ bold;\n\n          .foo {\n            background-color/* background-color: #f00; */: red;\n          }\n        ",
        vec![
            decl("color", "red"),
            decl("font-weight", "bold"),
            style_rule(".foo", vec![
                decl("background-color", "red"),
            ]),
        ],
    );
}

fn parse_multi_line_declarations(eol: &str) {
    check(
        eol,
        "\n          .foo {\n            grid-template-areas:\n              'header header header'\n              'sidebar main main'\n              'footer footer footer';\n          }\n        ",
        vec![
            style_rule(".foo", vec![
                decl("grid-template-areas", "'header header header' 'sidebar main main' 'footer footer footer'"),
            ]),
        ],
    );
}

fn parse_url_without_quotes_and_special_characters_such_as_semicolon_braces_and_brackets(
    eol: &str,
) {
    check(
        eol,
        "\n          .foo {\n            /* ';' should be valid inside the 'url(\\u2026)' function */\n            background: url(data:image/png;base64,abc==);\n\n            /* '{', '}', '[' and ']' should be valid inside the 'url(\\u2026)' function */\n            /* '{' and '}' should not start a new block (nesting) */\n            background: url(https://example-image-search.org?q={query;limit=5}&ids=[1,2,3]);\n\n            /* '{' and '}' don't need to be balanced */\n            background: url(https://example-image-search.org?curlies=}});\n\n            /* '(' and ')' are not valid, unless we are in a string with quotes */\n            background: url('https://example-image-search.org?q={query;limit=5}&ids=[1,2,3]&format=(png|jpg)');\n          }\n        ",
        vec![
            style_rule(".foo", vec![
                decl("background", "url(data:image/png;base64,abc==)"),
                decl("background", "url(https://example-image-search.org?q={query;limit=5}&ids=[1,2,3])"),
                decl("background", "url(https://example-image-search.org?curlies=}})"),
                decl("background", "url('https://example-image-search.org?q={query;limit=5}&ids=[1,2,3]&format=(png|jpg)')"),
            ]),
        ],
    );
}

// Upstream group: selectors.

fn parse_a_simple_selector(eol: &str) {
    check(
        eol,
        "\n          .foo {\n          }\n        ",
        vec![style_rule(".foo", vec![])],
    );
}

fn parse_selectors_with_escaped_characters(eol: &str) {
    check(
        eol,
        "\n          .hover\\:foo:hover {\n          }\n          .\\32 xl\\:foo {\n          }\n        ",
        vec![
            style_rule(".hover\\:foo:hover", vec![]),
            style_rule(".\\32 xl\\:foo", vec![]),
        ],
    );
}

fn parse_multiple_simple_selectors(eol: &str) {
    check(
        eol,
        "\n          .foo,\n          .bar {\n          }\n        ",
        vec![style_rule(".foo, .bar", vec![])],
    );
}

fn parse_multiple_declarations_inside_of_a_selector(eol: &str) {
    check(
        eol,
        "\n          .foo {\n            color: red;\n            font-size: 16px;\n          }\n        ",
        vec![
            style_rule(".foo", vec![
                decl("color", "red"),
                decl("font-size", "16px"),
            ]),
        ],
    );
}

fn parse_rules_with_declarations_that_end_with_no_semicolon(eol: &str) {
    check(
        eol,
        "\n          .foo {\n            color: red;\n            font-size: 16px\n          }\n        ",
        vec![
            style_rule(".foo", vec![
                decl("color", "red"),
                decl("font-size", "16px"),
            ]),
        ],
    );
}

fn parse_rules_with_declarations_that_end_with_no_semicolon_and_important(eol: &str) {
    check(
        eol,
        "\n          .foo {\n            color: red;\n            font-size: 16px !important\n          }\n        ",
        vec![
            style_rule(".foo", vec![
                decl("color", "red"),
                important("font-size", "16px"),
            ]),
        ],
    );
}

fn parse_a_multi_line_selector(eol: &str) {
    check(
        eol,
        ".foo,\n.bar,\n.baz\n{\ncolor:red;\n}",
        vec![style_rule(".foo, .bar, .baz", vec![decl("color", "red")])],
    );
}

fn parse_a_multi_line_selector_and_preserves_important_whitespace(eol: &str) {
    check(
        eol,
        ".foo,\n.bar,\n.baz\t\n \n .qux\n{\ncolor:red;\n}",
        vec![style_rule(
            ".foo, .bar, .baz .qux",
            vec![decl("color", "red")],
        )],
    );
}

// Upstream group: at-rules.

fn parse_an_at_rule_without_a_block(eol: &str) {
    check(
        eol,
        "\n          @charset \"UTF-8\";\n        ",
        vec![at_rule("@charset", "\"UTF-8\"", vec![])],
    );
}

fn parse_an_at_rule_with_whitespace_in_the_params_1(eol: &str) {
    check(
        eol,
        "@apply bg-red-500;",
        vec![at_rule("@apply", "bg-red-500", vec![])],
    );
}

fn parse_an_at_rule_with_whitespace_in_the_params_2(eol: &str) {
    check(
        eol,
        "@apply  bg-red-500;",
        vec![at_rule("@apply", "bg-red-500", vec![])],
    );
}

fn parse_an_at_rule_with_whitespace_in_the_params_3(eol: &str) {
    check(
        eol,
        "@apply\tbg-red-500;",
        vec![at_rule("@apply", "bg-red-500", vec![])],
    );
}

fn parse_an_at_rule_with_whitespace_in_the_params_4(eol: &str) {
    check(
        eol,
        "@apply \tbg-red-500;",
        vec![at_rule("@apply", "bg-red-500", vec![])],
    );
}

fn parse_an_at_rule_with_whitespace_in_the_params_5(eol: &str) {
    check(
        eol,
        "@apply\t bg-red-500;",
        vec![at_rule("@apply", "bg-red-500", vec![])],
    );
}

fn parse_an_at_rule_with_whitespace_in_the_params_6(eol: &str) {
    check(
        eol,
        "@apply\t\tbg-red-500;",
        vec![at_rule("@apply", "bg-red-500", vec![])],
    );
}

fn parse_an_at_rule_with_whitespace_in_the_params_7(eol: &str) {
    check(
        eol,
        "@apply\nbg-red-500;",
        vec![at_rule("@apply", "bg-red-500", vec![])],
    );
}

fn parse_an_at_rule_with_whitespace_in_the_params_8(eol: &str) {
    check(
        eol,
        "@apply \nbg-red-500;",
        vec![at_rule("@apply", "bg-red-500", vec![])],
    );
}

fn parse_an_at_rule_with_whitespace_in_the_params_9(eol: &str) {
    check(
        eol,
        "@apply\n bg-red-500;",
        vec![at_rule("@apply", "bg-red-500", vec![])],
    );
}

fn parse_an_at_rule_with_whitespace_in_the_params_10(eol: &str) {
    check(
        eol,
        "@apply\n\nbg-red-500;",
        vec![at_rule("@apply", "bg-red-500", vec![])],
    );
}

fn parse_an_at_rule_with_whitespace_in_the_params_11(eol: &str) {
    check(
        eol,
        "@apply\nbg-red-500;",
        vec![at_rule("@apply", "bg-red-500", vec![])],
    );
}

fn parse_an_at_rule_with_whitespace_in_the_params_12(eol: &str) {
    check(
        eol,
        "@apply \nbg-red-500;",
        vec![at_rule("@apply", "bg-red-500", vec![])],
    );
}

fn parse_an_at_rule_with_whitespace_in_the_params_13(eol: &str) {
    check(
        eol,
        "@apply\n bg-red-500;",
        vec![at_rule("@apply", "bg-red-500", vec![])],
    );
}

fn parse_an_at_rule_without_a_block_or_semicolon(eol: &str) {
    check(
        eol,
        "\n          @tailwind utilities\n        ",
        vec![at_rule("@tailwind", "utilities", vec![])],
    );
}

fn parse_an_at_rule_without_a_block_or_semicolon_when_it_s_the_last_rule_in_a_block(eol: &str) {
    check(
        eol,
        "\n          @layer utilities {\n            @tailwind utilities\n          }\n        ",
        vec![at_rule(
            "@layer",
            "utilities",
            vec![at_rule("@tailwind", "utilities", vec![])],
        )],
    );
}

fn parse_a_nested_at_rule_without_a_block(eol: &str) {
    check(
        eol,
        "\n          @layer utilities {\n            @charset \"UTF-8\";\n          }\n\n          .foo {\n            @apply font-bold hover:text-red-500;\n          }\n        ",
        vec![
            at_rule("@layer", "utilities", vec![
                at_rule("@charset", "\"UTF-8\"", vec![]),
            ]),
            style_rule(".foo", vec![
                at_rule("@apply", "font-bold hover:text-red-500", vec![]),
            ]),
        ],
    );
}

fn parse_custom_at_rules_without_a_block(eol: &str) {
    check(
        eol,
        "\n          @tailwind;\n          @tailwind base;\n        ",
        vec![
            at_rule("@tailwind", "", vec![]),
            at_rule("@tailwind", "base", vec![]),
        ],
    );
}

fn parse_nested_media_queries(eol: &str) {
    check(
        eol,
        "\n          @media (width >= 600px) {\n            .foo {\n              color: red;\n              @media (width >= 800px) {\n                color: blue;\n              }\n              @media (width >= 1000px) {\n                color: green;\n              }\n            }\n          }\n        ",
        vec![
            at_rule("@media", "(width >= 600px)", vec![
                style_rule(".foo", vec![
                    decl("color", "red"),
                    at_rule("@media", "(width >= 800px)", vec![
                        decl("color", "blue"),
                    ]),
                    at_rule("@media", "(width >= 1000px)", vec![
                        decl("color", "green"),
                    ]),
                ]),
            ]),
        ],
    );
}

fn parse_at_rules_that_span_multiple_lines(eol: &str) {
    check(
        eol,
        "\n          .foo {\n            @apply hover:text-red-100\n                   sm:hover:text-red-200\n                   md:hover:text-red-300\n                   lg:hover:text-red-400\n                   xl:hover:text-red-500;\n          }\n        ",
        vec![
            style_rule(".foo", vec![
                at_rule("@apply", "hover:text-red-100 sm:hover:text-red-200 md:hover:text-red-300 lg:hover:text-red-400 xl:hover:text-red-500", vec![]),
            ]),
        ],
    );
}

// Upstream group: nesting.

fn parse_nested_rules(eol: &str) {
    check(
        eol,
        "\n          .foo {\n            .bar {\n              .baz {\n                color: red;\n              }\n            }\n          }\n        ",
        vec![
            style_rule(".foo", vec![
                style_rule(".bar", vec![
                    style_rule(".baz", vec![
                        decl("color", "red"),
                    ]),
                ]),
            ]),
        ],
    );
}

fn parse_nested_selector_with_ampersand(eol: &str) {
    check(
        eol,
        "\n          .foo {\n            color: red;\n\n            &:hover {\n              color: blue;\n            }\n          }\n        ",
        vec![
            style_rule(".foo", vec![
                decl("color", "red"),
                style_rule("&:hover", vec![
                    decl("color", "blue"),
                ]),
            ]),
        ],
    );
}

fn parse_nested_sibling_selectors(eol: &str) {
    check(
        eol,
        "\n          .foo {\n            .bar {\n              color: red;\n            }\n\n            .baz {\n              color: blue;\n            }\n          }\n        ",
        vec![
            style_rule(".foo", vec![
                style_rule(".bar", vec![
                    decl("color", "red"),
                ]),
                style_rule(".baz", vec![
                    decl("color", "blue"),
                ]),
            ]),
        ],
    );
}

fn parse_nested_sibling_selectors_and_sibling_declarations(eol: &str) {
    check(
        eol,
        "\n          .foo {\n            font-weight: bold;\n            text-declaration-line: underline;\n\n            .bar {\n              color: red;\n            }\n\n            --in-between: 1;\n\n            .baz {\n              color: blue;\n            }\n\n            --at-the-end: 2;\n          }\n        ",
        vec![
            style_rule(".foo", vec![
                decl("font-weight", "bold"),
                decl("text-declaration-line", "underline"),
                style_rule(".bar", vec![
                    decl("color", "red"),
                ]),
                decl("--in-between", "1"),
                style_rule(".baz", vec![
                    decl("color", "blue"),
                ]),
                decl("--at-the-end", "2"),
            ]),
        ],
    );
}

// Upstream group: complex.

fn parse_complex_examples(eol: &str) {
    check(
        eol,
        "\n          @custom \\{ {\n            foo: bar;\n          }\n        ",
        vec![at_rule("@custom", "\\{", vec![decl("foo", "bar")])],
    );
}

fn parse_minified_nested_css(eol: &str) {
    check(
        eol,
        ".foo{color:red;@media(width>=600px){.bar{color:blue;font-weight:bold}}}",
        vec![style_rule(
            ".foo",
            vec![
                decl("color", "red"),
                at_rule(
                    "@media",
                    "(width>=600px)",
                    vec![style_rule(
                        ".bar",
                        vec![decl("color", "blue"), decl("font-weight", "bold")],
                    )],
                ),
            ],
        )],
    );
}

fn ignore_all_text_inside_of_comments(eol: &str) {
    check(
        eol,
        "\n          .foo:has(.bar /* instead \\*\\/ of .baz { */) {\n            color: red;\n          }\n        ",
        vec![
            style_rule(".foo:has(.bar )", vec![
                decl("color", "red"),
            ]),
        ],
    );
}

fn ignore_consecutive_semicolons(eol: &str) {
    check(eol, ";;;", vec![]);
}

fn ignore_semicolons_after_an_at_rule_with_a_body(eol: &str) {
    check(
        eol,
        "@plugin \"foo\" {} ;",
        vec![at_rule("@plugin", "\"foo\"", vec![])],
    );
}

fn ignore_consecutive_semicolons_a_declaration(eol: &str) {
    check(
        eol,
        ".foo { color: red;;; }",
        vec![style_rule(".foo", vec![decl("color", "red")])],
    );
}

// Upstream group: CDO and CDC tokens.

fn ignore_cdo_and_cdc_on_their_own(eol: &str) {
    check(eol, "<!--", vec![]);
    check(eol, "-->", vec![]);
    check(eol, "<!-- -->", vec![]);
}

fn ignore_cdo_and_cdc_around_rules(eol: &str) {
    check(
        eol,
        "\n          <!--\n          a {\n            color: red;\n          }\n          -->\n          b {\n            color: blue;\n          }\n          -->\n        ",
        vec![
            style_rule("a", vec![
                decl("color", "red"),
            ]),
            style_rule("b", vec![
                decl("color", "blue"),
            ]),
        ],
    );
}

fn ignore_cdo_and_cdc_without_whitespace_around_them(eol: &str) {
    check(
        eol,
        "<!--a{color:red}-->b{color:blue}",
        vec![
            style_rule("a", vec![decl("color", "red")]),
            style_rule("b", vec![decl("color", "blue")]),
        ],
    );
}

fn ignore_cdo_and_cdc_next_to_at_rules_and_declarations(eol: &str) {
    check(
        eol,
        "<!-- @charset \"UTF-8\"; --> @media print { a { color: red } }",
        vec![
            at_rule("@charset", "\"UTF-8\"", vec![]),
            at_rule(
                "@media",
                "print",
                vec![style_rule("a", vec![decl("color", "red")])],
            ),
        ],
    );
    check(
        eol,
        "--foo: bar; --> --baz: qux;",
        vec![decl("--foo", "bar"), decl("--baz", "qux")],
    );
}

fn keep_cdo_and_cdc_inside_of_a_selector(eol: &str) {
    check(
        eol,
        "a <!-- b{color:blue}",
        vec![style_rule("a <!-- b", vec![decl("color", "blue")])],
    );
    check(
        eol,
        "a --> b{color:blue}",
        vec![style_rule("a --> b", vec![decl("color", "blue")])],
    );
}

fn keep_cdo_and_cdc_inside_of_a_custom_property(eol: &str) {
    check(eol, "--foo: <!-- -->;", vec![decl("--foo", "<!-- -->")]);
}

fn not_treat_a_name_that_starts_with_three_dashes_as_cdc(eol: &str) {
    check(eol, "---a: b;", vec![decl("---a", "b")]);
}

fn only_ignore_cdo_and_cdc_at_the_top_level(eol: &str) {
    check_error(
        eol,
        "a { --> }",
        "Invalid custom property, expected a value",
    );
}

// Upstream group: errors.

fn error_when_curly_brackets_are_unbalanced_open(eol: &str) {
    check_error_at(eol, "\n          .foo {\n            color: red;\n          }\n\n          .bar\n            /* ^ Missing opening { */\n            color: blue;\n          }\n        ", "9:11", "Missing opening {");
}

fn error_when_curly_brackets_are_unbalanced_close(eol: &str) {
    check_error_at(eol, "\n          .foo {\n            color: red;\n          }\n\n          .bar {\n            color: blue;\n\n       /* ^ Missing closing } */\n        ", "6:11", "Missing closing } at .bar");
}

fn error_when_an_unterminated_string_is_used(eol: &str) {
    check_error_at(eol, "\n        .foo {\n          content: \"Hello world!\n          /*                    ^ missing \" */\n          font-weight: bold;\n        }\n      ", "3:20", "Unterminated string: \"Hello world!\"");
}

fn error_when_an_unterminated_string_is_used_with_a_semicolon(eol: &str) {
    check_error_at(eol, "\n        .foo {\n          content: \"Hello world!;\n          /*                    ^ missing \" */\n          font-weight: bold;\n        }\n      ", "3:20", "Unterminated string: \"Hello world!;\"");
}

fn error_when_incomplete_custom_properties_are_used(eol: &str) {
    check_error_at(eol, "--foo", "1:1", "Invalid custom property, expected a value");
}

fn error_when_incomplete_custom_properties_are_used_inside_rules(eol: &str) {
    check_error_at(
        eol,
        ".foo { --bar }",
        "1:8", "Invalid custom property, expected a value",
    );
}

fn error_when_an_unterminated_string_is_used_in_a_custom_property(eol: &str) {
    check_error_at(eol, "\n        .foo {\n          --bar: 'Hello world!\n          /*                  ^ missing ' * /;\n        }\n      ", "3:18", "Unterminated string: 'Hello world!'");
}

fn error_when_a_declaration_is_incomplete(eol: &str) {
    check_error_at(eol, ".foo { bar }", "1:8", "Invalid declaration: `bar`");
}

// Upstream group: top level.

fn ignore_a_bom_at_the_start_of_a_file(eol: &str) {
    check(
        eol,
        "\u{feff}@reference 'tailwindcss';",
        vec![at_rule("@reference", "'tailwindcss'", vec![])],
    );
}

line_end_tests! {
    parse_a_comment_and_ignore_it,
    parse_a_comment_with_an_escaped_end_and_ignore_it,
    end_a_comment_at_comment_end_even_when_it_is_preceded_by_a_backslash,
    parse_a_comment_inside_of_a_selector_and_ignore_it,
    remove_comments_in_between_selectors_and_keep_the_correct_whitespace,
    collect_license_comments,
    hoist_all_license_comments,
    handle_comments_before_element_selectors,
    parse_a_simple_declaration,
    parse_a_declaration_with_no_last_semicolon,
    parse_a_declaration_with_no_last_semicolon_and_a_newline_at_the_end,
    parse_a_declaration_with_no_last_semicolon_and_important,
    parse_multiple_declarations_where_the_last_one_has_no_last_semicolon,
    parse_a_declaration_with_no_last_semicolon_after_a_custom_property,
    not_parse_a_declaration_with_unbalanced_brackets_and_no_last_semicolon,
    parse_declarations_with_strings,
    parse_declarations_with_nested_strings,
    parse_declarations_with_nested_strings_that_are_not_balanced,
    parse_declarations_with_strings_and_escaped_string_ends,
    parse_declarations_with_important,
    parse_declarations_with_important_when_a_comment_follows,
    parse_custom_properties_with_important_when_a_comment_follows,
    not_treat_important_inside_of_a_comment_as_the_important_flag,
    not_treat_important_inside_of_a_value_as_the_important_flag,
    keep_an_escaped_whitespace_at_the_end_of_the_value,
    not_treat_an_escaped_exclamation_mark_as_the_important_flag,
    parse_a_custom_property,
    parse_a_minified_custom_property,
    parse_a_minified_custom_property_with_no_semicolon,
    parse_a_custom_property_with_no_last_semicolon,
    parse_a_custom_property_with_no_last_semicolon_and_important,
    parse_a_custom_property_with_no_last_semicolon_at_the_end_of_the_input,
    parse_a_custom_property_with_an_unclosed_bracket_at_the_end_of_the_input,
    parse_a_custom_property_with_code_of_a_different_language,
    parse_a_custom_property_with_an_empty_block_as_the_value,
    parse_a_custom_property_with_an_empty_value,
    parse_a_custom_property_with_a_space_value,
    parse_a_custom_property_with_a_block_with_nested_css,
    parse_a_custom_property_with_a_block_with_nested_css_and_comments_with_end_characters_inside_them,
    parse_a_custom_property_with_escaped_characters_in_the_value,
    parse_a_custom_property_with_escaped_characters_inside_a_comment_in_the_value,
    end_a_comment_in_a_custom_property_at_comment_end_even_when_it_is_preceded_by_a_backslash,
    parse_empty_custom_properties,
    parse_custom_properties_with_important,
    parse_custom_properties_with_data_url_value,
    parse_multiple_declarations,
    correctly_parse_comments_with_colon_inside_of_them,
    parse_multi_line_declarations,
    parse_url_without_quotes_and_special_characters_such_as_semicolon_braces_and_brackets,
    parse_a_simple_selector,
    parse_selectors_with_escaped_characters,
    parse_multiple_simple_selectors,
    parse_multiple_declarations_inside_of_a_selector,
    parse_rules_with_declarations_that_end_with_no_semicolon,
    parse_rules_with_declarations_that_end_with_no_semicolon_and_important,
    parse_a_multi_line_selector,
    parse_a_multi_line_selector_and_preserves_important_whitespace,
    parse_an_at_rule_without_a_block,
    parse_an_at_rule_with_whitespace_in_the_params_1,
    parse_an_at_rule_with_whitespace_in_the_params_2,
    parse_an_at_rule_with_whitespace_in_the_params_3,
    parse_an_at_rule_with_whitespace_in_the_params_4,
    parse_an_at_rule_with_whitespace_in_the_params_5,
    parse_an_at_rule_with_whitespace_in_the_params_6,
    parse_an_at_rule_with_whitespace_in_the_params_7,
    parse_an_at_rule_with_whitespace_in_the_params_8,
    parse_an_at_rule_with_whitespace_in_the_params_9,
    parse_an_at_rule_with_whitespace_in_the_params_10,
    parse_an_at_rule_with_whitespace_in_the_params_11,
    parse_an_at_rule_with_whitespace_in_the_params_12,
    parse_an_at_rule_with_whitespace_in_the_params_13,
    parse_an_at_rule_without_a_block_or_semicolon,
    parse_an_at_rule_without_a_block_or_semicolon_when_it_s_the_last_rule_in_a_block,
    parse_a_nested_at_rule_without_a_block,
    parse_custom_at_rules_without_a_block,
    parse_nested_media_queries,
    parse_at_rules_that_span_multiple_lines,
    parse_nested_rules,
    parse_nested_selector_with_ampersand,
    parse_nested_sibling_selectors,
    parse_nested_sibling_selectors_and_sibling_declarations,
    parse_complex_examples,
    parse_minified_nested_css,
    ignore_all_text_inside_of_comments,
    ignore_consecutive_semicolons,
    ignore_semicolons_after_an_at_rule_with_a_body,
    ignore_consecutive_semicolons_a_declaration,
    ignore_cdo_and_cdc_on_their_own,
    ignore_cdo_and_cdc_around_rules,
    ignore_cdo_and_cdc_without_whitespace_around_them,
    ignore_cdo_and_cdc_next_to_at_rules_and_declarations,
    keep_cdo_and_cdc_inside_of_a_selector,
    keep_cdo_and_cdc_inside_of_a_custom_property,
    not_treat_a_name_that_starts_with_three_dashes_as_cdc,
    only_ignore_cdo_and_cdc_at_the_top_level,
    error_when_curly_brackets_are_unbalanced_open,
    error_when_curly_brackets_are_unbalanced_close,
    error_when_an_unterminated_string_is_used,
    error_when_an_unterminated_string_is_used_with_a_semicolon,
    error_when_incomplete_custom_properties_are_used,
    error_when_incomplete_custom_properties_are_used_inside_rules,
    error_when_an_unterminated_string_is_used_in_a_custom_property,
    error_when_a_declaration_is_incomplete,
    ignore_a_bom_at_the_start_of_a_file,
}

// Cases that are not in the upstream test file.

#[test]
fn split_an_at_rule_into_name_and_params() {
    let cases = [
        ("@media (width >= 40rem)", "@media", "(width >= 40rem)"),
        ("@media(width >= 40rem)", "@media", "(width >= 40rem)"),
        ("@layer\tbase", "@layer", "base"),
        ("@tailwind utilities ", "@tailwind", "utilities"),
        ("@charset", "@charset", ""),
        // Upstream starts the search at the index 5, so a short name keeps
        // its params.
        ("@x y", "@x y", ""),
        ("@abc (y)", "@abc", "(y)"),
        // The index 5 is an index of UTF-16 code units.
        ("@\u{1D4B3}\u{1D4B3} a b", "@\u{1D4B3}\u{1D4B3}", "a b"),
        ("@\u{e9}\u{e9}\u{e9} a b", "@\u{e9}\u{e9}\u{e9} a", "b"),
    ];
    for (input, name, params) in cases {
        let node = parse_at_rule(input, vec![]);
        assert_eq!(
            (node.name.as_str(), node.params.as_str()),
            (name, params),
            "input: {input:?}"
        );
        assert!(node.nodes.is_empty());
    }
}

#[test]
fn keep_the_nodes_of_an_at_rule() {
    let node = parse_at_rule("@media print", vec![decl("color", "red")]);
    assert_eq!(node.nodes, vec![decl("color", "red")]);
}

#[test]
fn trim_the_white_space_of_javascript() {
    // JavaScript removes U+FEFF and keeps U+0085. Rust does the opposite.
    assert_eq!(parse("a:\u{feff}b\u{feff};"), Ok(vec![decl("a", "b")]));
    assert_eq!(
        parse("a:\u{85}b\u{85};"),
        Ok(vec![decl("a", "\u{85}b\u{85}")])
    );
    assert_eq!(
        parse("\u{a0}.a\u{2028}{}"),
        Ok(vec![style_rule(".a", vec![])])
    );
}

#[test]
fn ignore_only_the_slash_of_a_comment_with_no_end() {
    assert_eq!(parse("a: b /* c"), Ok(vec![decl("a", "b * c")]));
}

#[test]
fn keep_characters_that_are_not_ascii() {
    assert_eq!(
        parse(".\u{e9}\\\u{1D4B3} { --\u{e9}: \"\u{1D4B3}\" \\\u{e9} }"),
        Ok(vec![style_rule(
            ".\u{e9}\\\u{1D4B3}",
            vec![decl("--\u{e9}", "\"\u{1D4B3}\" \\\u{e9}")]
        )])
    );
}

/// Inputs with the output of the upstream parser and printer: the printed
/// tree, or the message of the error.
const UPSTREAM_SAMPLES: &[(&str, Result<&str, &str>)] = &[
    ("/*!;-:'\u{a0}\r\n/*", Err("Invalid declaration: `*!`")),
    (
        "\r\u{b}.foo!important\r\\ \")-\\@x\r}a0\t}*&",
        Err("Missing opening ("),
    ),
    (
        "{red\\ ,-->\r/*\u{85}:\u{feff}.foo\r\n",
        Err("Missing closing } at "),
    ),
    ("-\".foo\r\n\r", Err("Unterminated string: \".foo\"")),
    (
        "*//*!/*-->\\\\\u{feff}!/--foo@[{'",
        Err("Missing closing } at */*!*-->\\\\\u{feff}!/--foo@["),
    ),
    (
        "@)\u{b}!important<!--</*\r\n,!>@)",
        Err("Missing opening ("),
    ),
    (
        "\u{feff}};\r*/'0red/*!'\u{a0}*\r\n\n",
        Err("Missing opening {"),
    ),
    (
        ",𝒳]:-->: }\u{b}\\ !important& \u{feff}\u{c},\r,\u{b}\n",
        Err("Missing opening {"),
    ),
    ("!\n\\ \r\n-}\t\u{0}-color", Err("Missing opening {")),
    (
        "!important;\"@x-é",
        Err("Invalid declaration: `!important`"),
    ),
    ("@media>&-[:", Ok("@media>&-[: ;\n")),
    (
        "@media\\\\!important\\ a:",
        Ok("@media\\\\!important\\ a:;\n"),
    ),
    ("@layercolor-{", Err("Missing closing } at @layercolor- ")),
    (
        "--foo\u{85}@layer}@\u{feff}--foo\u{c}\t.foo",
        Err("Invalid custom property, expected a value"),
    ),
    ("&{\r\n \t-}", Err("Invalid declaration: `-`")),
    ("\t@\u{0}\"}*]/*", Err("Missing opening {")),
    (
        "\u{feff}--foo}/*",
        Err("Invalid custom property, expected a value"),
    ),
    (
        "\u{feff}:--/*é<!--/*[\u{85}\\\\]",
        Ok(": --*é<!--*[\u{85}\\\\];\n"),
    ),
    (
        "color\u{a0}{\u{2028}\"\\ ]\\\\\u{0},red",
        Err("Missing closing } at color"),
    ),
    (
        "[\r\u{2028}\r\n\\\\/*@xé;\u{2028}/*",
        Err("Invalid declaration: `[\r\u{2028} \\\\*@xé`"),
    ),
    ("@layer/*@\u{feff}", Ok("@layer*@ ;\n")),
    ("\n@]*/\\'𝒳(\r-->:[", Ok("@]*/\\'𝒳 (\r-->:[;\n")),
    ("/*{}[@layer\u{2028}\u{a0}", Ok("* {\n}\n")),
    (
        "--foo\u{2028}\\!important𝒳acolor",
        Err("Invalid custom property, expected a value"),
    ),
    ("color\\ 𝒳\u{85}/:colora", Ok("color\\ 𝒳\u{85}/: colora;\n")),
    (
        "color\\@\"\u{a0}/]\\<\n",
        Err("Unterminated string: \"\u{a0}/]\\<\""),
    ),
    ("\u{feff};\\\\!important:", Ok("\\\\!important: ;\n")),
    (":-->\ré\u{0} 𝒳\\\r\u{c}", Ok(": -->\ré\u{0} 𝒳\\\r;\n")),
    (
        "--@x!important\r\u{a0}<!--\\!important0é",
        Err("Invalid custom property, expected a value"),
    ),
    ("]:\n--foocolor/red*/\"@", Ok("]: --foocolor/red*/\"@;\n")),
    (":<>\u{a0}\r !)*/\"𝒳red\u{c}", Err("Missing opening (")),
    (
        ">color-)!\u{a0}red/*color\" \u{b}'\\\\",
        Err("Missing opening ("),
    ),
    (
        "\"(\u{a0}𝒳]\u{feff}.foo<!--\n/*{\\a",
        Err("Unterminated string: \"(\u{a0}𝒳]\u{feff}.foo<!--\""),
    ),
    ("--foo\u{85}<:*/", Ok("--foo\u{85}<: */;\n")),
    ("\"\r\n}*'\\\\url(", Err("Unterminated string: \"\"")),
    (
        ";/:!important\u{b}--foo\u{a0};a\\",
        Ok("/: !important\u{b}--foo;\n"),
    ),
    ("--<*/\u{0}\u{c}:/*([\r\n", Ok("--<*/\u{0}: /*([;\n")),
    (
        "/*!!𝒳red:\"/<!--!important\u{c}",
        Ok("*!!𝒳red: \"/<!-- !important;\n"),
    ),
    ("@layer\t@x", Ok("@layer @x;\n")),
    (
        ":\"*/color/*!é\\ @media:&",
        Ok(": \"*/color*!é\\ @media:&;\n"),
    ),
    ("\r\nred*]:", Ok("red*]: ;\n")),
    (
        "]\u{0}&:é:a-/*]\u{a0}red\\",
        Ok("]\u{0}&: é:a-*]\u{a0}red\\;\n"),
    ),
    (
        "@media&!important@media/*!é\u{b}",
        Ok("@media&!important@media*!é ;\n"),
    ),
    (
        "\u{feff}\u{2028}:/\t\u{85}&!!important<!--0",
        Ok(": /\t\u{85}&!!important<!--0;\n"),
    ),
    (".foo0:\u{c}':\u{85}", Ok(".foo0: ':\u{85};\n")),
    (":.foo>\r\n ", Ok(": .foo>;\n")),
    ("----\\\\(:𝒳}-*/", Ok("----\\\\(: 𝒳}-*/;\n")),
    (
        ",/*\"color<<!--:𝒳<!--éred\u{b}</*",
        Ok(",*\"color<<!--: 𝒳<!--éred\u{b}<*;\n"),
    ),
    (
        "\u{feff}\u{feff}\u{feff}\\ \\ >::*&;\\!important--foo,",
        Ok("\\ \\ >: :*&;\n"),
    ),
    (
        "@media𝒳0\u{85}]>\u{2028}>\u{c}\t",
        Ok("@media𝒳0\u{85}]>\u{2028}> ;\n"),
    ),
    ("\u{2028}\u{0}: *𝒳color", Ok("\u{0}: *𝒳color;\n")),
    ("\u{c}\u{0}!importanta:", Ok("\u{0}!importanta: ;\n")),
    (
        ";@x/*𝒳,>\u{b}--foo0\u{a0}\\;]",
        Ok("@x*𝒳,>\u{b}--foo0\u{a0}\\;] ;\n"),
    ),
    (
        "\"--foo\u{c}\\\\:\\ \u{0}>/*'",
        Ok("\"--foo\u{c}\\\\: \\ \u{0}>*';\n"),
    ),
    ("-->\n::\u{a0}\\\\@layer&𝒳", Ok(": :\u{a0}\\\\@layer&𝒳;\n")),
    ("@--;/-->", Ok("@-- ;\n")),
    (":@<\\\\\n\\ !;\u{c}\u{c}", Ok(": @<\\\\ \\ !;\n")),
    (
        "\r\n@media\t-->a{ \r.foo𝒳:!\u{c}}",
        Ok("@media -->a {\n  .foo𝒳: !;\n}\n"),
    ),
    ("aé]@x:\\\\@x\u{0}]", Ok("aé]@x: \\\\@x\u{0}];\n")),
    (
        "𝒳 !importanta]red:.foo! ",
        Ok("𝒳 !importanta]red: .foo!;\n"),
    ),
    (
        "--\u{85}{\\\\{\u{feff}&a:!important\"",
        Ok("--\u{85}{\\\\{\u{feff}&a: !important\";\n"),
    ),
    ("@media.foo\\0", Ok("@media.foo\\0 ;\n")),
    ("@media/*𝒳", Ok("@media*𝒳 ;\n")),
    (
        "@x.foo]!important\u{85}é/:",
        Ok("@x.foo]!important\u{85}é/: ;\n"),
    ),
    (
        "@layer/*<!--&@layer\u{0}é",
        Ok("@layer*<!--&@layer\u{0}é ;\n"),
    ),
    (
        "@a0!(@media.foo\u{a0}\r{.foo",
        Ok("@a0!(@media.foo\u{a0}\r{.foo ;\n"),
    ),
    ("\u{2028}\t:color", Ok(": color;\n")),
    (
        "@@media@--foo\u{c}@media\u{a0}(-\u{b}",
        Ok("@@media@--foo\u{c}@media (-;\n"),
    ),
    ("@x&\u{c}-- @x", Ok("@x&\u{c}-- @x;\n")),
];

#[test]
fn agree_with_upstream_on_recorded_inputs() {
    for (input, expected) in UPSTREAM_SAMPLES {
        let result = parse(input);
        let actual = match &result {
            Ok(ast) => Ok(to_css(ast)),
            Err(error) => Err(error.message.clone()),
        };
        let expected = expected.map(str::to_string).map_err(str::to_string);
        assert_eq!(actual, expected, "input: {input:?}");
    }
}

// Property tests.

/// One of the given texts.
fn one_of(texts: &'static [&'static str]) -> impl Strategy<Value = String> {
    prop::sample::select(texts).prop_map(str::to_string)
}

/// Zero to three of the given texts, with one space between them.
fn words(texts: &'static [&'static str]) -> impl Strategy<Value = String> {
    prop::collection::vec(prop::sample::select(texts), 0..=3).prop_map(|words| words.join(" "))
}

const CSS_TOKENS: &[&str] = &[
    "{",
    "}",
    ";",
    ":",
    "(",
    ")",
    "[",
    "]",
    "\"",
    "'",
    "/*",
    "*/",
    "/*!",
    "\\",
    "@",
    "@media",
    "--",
    "--x",
    "-->",
    "<!--",
    "!important",
    " ",
    "\n",
    "\r\n",
    "\r",
    "\t",
    "a",
    "color",
    ".foo",
    "&",
    "\u{e9}",
    "\u{1D4B3}",
    "\u{a0}",
    "\u{feff}",
    "\u{85}",
    "\u{0}",
    "url(",
    ",",
];

const SELECTORS: &[&str] = &[
    ".foo",
    "&:hover",
    "&",
    ".a > .b",
    "a, b",
    ".hover\\:flex",
    "[data-x=\"a;b{c}\"]",
    ":is(.a, .b)",
    "div::before",
    ".\u{e9}",
];

const AT_RULE_NAMES: &[&str] = &[
    "@media",
    "@layer",
    "@supports",
    "@container",
    "@custom-variant",
    "@tailwind",
    "@apply",
];

const AT_RULE_PARAMS: &[&str] = &[
    "",
    "(width >= 40rem)",
    "base, components",
    "screen and (min-width: 1px)",
    "utilities",
    "\"a;b{c}\"",
];

const PROPERTIES: &[&str] = &[
    "color",
    "margin-top",
    "font-family",
    "-webkit-box-orient",
    "content",
];

const VALUES: &[&str] = &[
    "red",
    "1px",
    "var(--x)",
    "\"a;b}\"",
    "'{'",
    "url(a;b{c})",
    "calc(1px + 2px)",
    "rgb(0 0 0 / 50%)",
    "a\\;b",
    "\u{1D4B3}",
];

const CUSTOM_PROPERTIES: &[&str] = &["--x", "--tw-ring-color", "--\u{e9}"];

const CUSTOM_VALUES: &[&str] = &[
    "red",
    "var(--y, 1px)",
    "{ a: b; c: d }",
    "[x;y]",
    "\"a;b}\"",
    "/* note; } */",
    "a\\;b",
    "url(a;b)",
];

const COMMENTS: &[&str] = &[" note ", "", "! license ", "!", " a; b { c } "];

fn declaration() -> impl Strategy<Value = AstNode> {
    let value = prop_oneof![
        (one_of(PROPERTIES), words(VALUES)),
        (one_of(CUSTOM_PROPERTIES), words(CUSTOM_VALUES)),
    ];
    // A declaration with no value is rare.
    (value, any::<bool>(), 0..20u8).prop_map(|((property, value), important, no_value)| {
        decl_full(property, (no_value != 0).then_some(value), important)
    })
}

/// A list of nodes that the printer and the parser can process. With
/// `only_printed_nodes`, the tree has no comments and no nodes that the
/// printer ignores.
fn tree(only_printed_nodes: bool) -> impl Strategy<Value = Vec<AstNode>> {
    let statement = (one_of(AT_RULE_NAMES), one_of(AT_RULE_PARAMS))
        .prop_map(|(name, params)| at_rule(name, params, vec![]));
    let leaf = if only_printed_nodes {
        prop_oneof![4 => declaration(), 1 => statement].boxed()
    } else {
        prop_oneof![4 => declaration(), 1 => statement, 2 => one_of(COMMENTS).prop_map(comment)]
            .boxed()
    };
    let node = leaf.prop_recursive(4, 24, 4, move |inner| {
        let nodes = prop::collection::vec(inner, 0..4);
        let hidden = if only_printed_nodes { 0 } else { 1 };
        prop_oneof![
            4 => (one_of(SELECTORS), nodes.clone()).prop_map(|(selector, nodes)| style_rule(selector, nodes)),
            3 => (one_of(AT_RULE_NAMES), one_of(AT_RULE_PARAMS), nodes.clone())
                .prop_map(|(name, params, nodes)| at_rule(name, params, nodes)),
            hidden => nodes.clone().prop_map(|nodes| context(ContextMap::new(), nodes)),
            hidden => nodes.prop_map(at_root),
        ]
    });
    prop::collection::vec(node, 0..4)
}

/// Move the license comments of the printed nodes to `licenses`, and give
/// the other nodes in the form that the parser gives for the printed tree.
fn normalize_nodes(nodes: &[AstNode], licenses: &mut Vec<AstNode>) -> Vec<AstNode> {
    let mut result = Vec::new();
    for node in nodes {
        match node {
            AstNode::Rule(n) => result.push(style_rule(
                n.selector.clone(),
                normalize_nodes(&n.nodes, licenses),
            )),
            AstNode::AtRule(n) => {
                result.push(at_rule(
                    n.name.clone(),
                    n.params.clone(),
                    normalize_nodes(&n.nodes, licenses),
                ));
            }
            // The printer writes `undefined` for a value that is not there.
            AstNode::Declaration(n) => result.push(decl_full(
                n.property.clone(),
                Some(n.value.clone().unwrap_or_else(|| "undefined".to_string())),
                n.important,
            )),
            // The parser keeps only the license comments.
            AstNode::Comment(n) => {
                if n.value.starts_with('!') {
                    licenses.push(node.clone());
                }
            }
            // The printer prints nothing for these nodes.
            AstNode::Context(_) | AstNode::AtRoot(_) => {}
        }
    }
    result
}

/// The tree that the parser gives for the printed tree.
fn normalize(nodes: &[AstNode]) -> Vec<AstNode> {
    let mut result = Vec::new();
    let mut rest = normalize_nodes(nodes, &mut result);
    result.append(&mut rest);
    result
}

proptest! {
    #[test]
    fn parse_does_not_panic_on_arbitrary_text(input in any::<String>()) {
        if let Ok(ast) = parse(&input) {
            to_css(&ast);
        }
    }

    #[test]
    fn parse_does_not_panic_on_text_with_css_characters(
        input in "[ab:;{}()\\[\\]'\"/*\\\\@!<> \\n\\r\\t\\-\u{e9}\u{1D4B3}\u{feff}]{0,64}"
    ) {
        if let Ok(ast) = parse(&input) {
            to_css(&ast);
        }
    }

    #[test]
    fn parse_does_not_panic_on_a_sequence_of_css_tokens(
        tokens in prop::collection::vec(prop::sample::select(CSS_TOKENS), 0..40)
    ) {
        let input = tokens.concat();
        if let Ok(ast) = parse(&input) {
            to_css(&ast);
        }
    }

    #[test]
    fn at_rule_parse_does_not_panic(input in any::<String>()) {
        parse_at_rule(&input, vec![]);
    }

    #[test]
    fn parse_gives_back_the_printed_tree(ast in tree(false)) {
        let expected = normalize(&ast);
        let css = to_css(&ast);
        prop_assert_eq!(parse(&css), Ok(expected.clone()), "css: {}", css);

        // The print of the result is a fixed point.
        let printed = to_css(&expected);
        let parsed = parse(&printed);
        prop_assert_eq!(parsed.as_ref(), Ok(&expected), "css: {}", printed);
        prop_assert_eq!(to_css(&parsed.unwrap()), printed);
    }

    #[test]
    fn print_of_a_tree_of_printed_nodes_is_a_fixed_point(ast in tree(true)) {
        // The parser moves or removes comments, and the printer ignores
        // some nodes. With none of these, the print does not change.
        let css = to_css(&ast);
        let parsed = parse(&css);
        prop_assert!(parsed.is_ok(), "css: {}", css);
        prop_assert_eq!(to_css(&parsed.unwrap()), css);
    }
}
