//! Tests of `utils`: the upstream test files `utils/*.test.ts` of Tailwind
//! CSS, tests of the JavaScript behavior in `utils::js`, and property tests.

use proptest::prelude::*;
use std::cmp::Ordering;
use tailwind::utils::brace_expansion::{expand, ExpandError};
use tailwind::utils::compare::compare;
use tailwind::utils::compare_breakpoints::{compare_breakpoints, Direction};
use tailwind::utils::decode_arbitrary_value::decode_arbitrary_value;
use tailwind::utils::default_map::DefaultMap;
use tailwind::utils::dimensions::dimensions;
use tailwind::utils::escape::{escape, unescape};
use tailwind::utils::infer_data_type::*;
use tailwind::utils::is_color::{is_color, is_named_color};
use tailwind::utils::is_valid_arbitrary::is_valid_arbitrary;
use tailwind::utils::js::*;
use tailwind::utils::math_operators::{add_whitespace_around_math_operators, has_math_fn};
use tailwind::utils::replace_shadow_colors::replace_shadow_colors;
use tailwind::utils::segment::segment;
use tailwind::utils::to_key_path::to_key_path;
use tailwind::utils::topological_sort::{topological_sort, CircularDependency};
use tailwind::utils::variables::extract_used_variables;

// ---------------------------------------------------------------------------
// segment.test.ts

mod segment_tests {
    use super::*;

    #[test]
    fn result_in_a_single_segment_when_the_separator_is_not_present() {
        assert_eq!(segment("foo", b':'), ["foo"]);
    }

    #[test]
    fn split_by_the_separator() {
        assert_eq!(segment("foo:bar:baz", b':'), ["foo", "bar", "baz"]);
    }

    #[test]
    fn not_split_inside_of_parens() {
        assert_eq!(segment("a:(b:c):d", b':'), ["a", "(b:c)", "d"]);
    }

    #[test]
    fn not_split_inside_of_brackets() {
        assert_eq!(segment("a:[b:c]:d", b':'), ["a", "[b:c]", "d"]);
    }

    #[test]
    fn not_split_inside_of_curlies() {
        assert_eq!(segment("a:{b:c}:d", b':'), ["a", "{b:c}", "d"]);
    }

    #[test]
    fn not_split_inside_of_double_quotes() {
        assert_eq!(segment("a:\"b:c\":d", b':'), ["a", "\"b:c\"", "d"]);
    }

    #[test]
    fn not_split_inside_of_single_quotes() {
        assert_eq!(segment("a:'b:c':d", b':'), ["a", "'b:c'", "d"]);
    }

    #[test]
    fn not_crash_when_double_quotes_are_unbalanced() {
        assert_eq!(segment("a:\"b:c:d", b':'), ["a", "\"b:c:d"]);
    }

    #[test]
    fn not_crash_when_single_quotes_are_unbalanced() {
        assert_eq!(segment("a:'b:c:d", b':'), ["a", "'b:c:d"]);
    }

    #[test]
    fn skip_escaped_double_quotes() {
        assert_eq!(segment(r#"a:"b:c\":d":e"#, b':'), ["a", r#""b:c\":d""#, "e"]);
    }

    #[test]
    fn skip_escaped_single_quotes() {
        assert_eq!(segment(r"a:'b:c\':d':e", b':'), ["a", r"'b:c\':d'", "e"]);
    }

    #[test]
    fn split_by_the_escape_sequence_which_is_escape_as_well() {
        assert_eq!(segment("a\\b\\c\\d", b'\\'), ["a", "b", "c", "d"]);
        assert_eq!(segment("a\\(b\\c)\\d", b'\\'), ["a", "(b\\c)", "d"]);
        assert_eq!(segment("a\\[b\\c]\\d", b'\\'), ["a", "[b\\c]", "d"]);
        assert_eq!(segment("a\\{b\\c}\\d", b'\\'), ["a", "{b\\c}", "d"]);
    }

    // The tests below are not in the upstream file.

    #[test]
    fn an_escape_hides_the_separator() {
        assert_eq!(segment("a\\:b:c", b':'), ["a\\:b", "c"]);
        assert_eq!(segment("a:b\\", b':'), ["a", "b\\"]);
    }

    #[test]
    fn empty_input_is_one_empty_segment() {
        assert_eq!(segment("", b','), [""]);
        assert_eq!(segment(",", b','), ["", ""]);
    }

    #[test]
    fn a_wrong_close_bracket_does_not_close_a_level() {
        assert_eq!(segment("(a],b),c", b','), ["(a],b)", "c"]);
        assert_eq!(segment("a),b", b','), ["a)", "b"]);
    }

    #[test]
    fn text_that_is_not_ascii_stays_complete() {
        assert_eq!(segment("é,😀\\,ü,(ö,ö)", b','), ["é", "😀\\,ü", "(ö,ö)"]);
    }

    #[test]
    fn a_level_deeper_than_the_upstream_stack_does_not_close() {
        let open = "(".repeat(257);
        let close = ")".repeat(257);
        let input = format!("a,{open}b{close},c");
        assert_eq!(segment(&input, b','), ["a", &input[2..]]);
        let open = "(".repeat(256);
        let close = ")".repeat(256);
        let input = format!("a,{open}b{close},c");
        assert_eq!(segment(&input, b',').len(), 3);
    }
}

// ---------------------------------------------------------------------------
// escape.test.ts

mod escape_tests {
    use super::*;

    #[test]
    fn escape_adds_backslashes() {
        assert_eq!(escape("red-1/2"), "red-1\\/2");
    }

    #[test]
    fn unescape_decodes_a_single_hexadecimal_digit() {
        for (input, expected) in [
            (r"\0", "\u{FFFD}"),
            (r"\1", "\u{0001}"),
            (r"\9", "\t"),
            (r"\a", "\n"),
            (r"\A", "\n"),
            (r"\c", "\u{000c}"),
            (r"\C", "\u{000c}"),
            (r"\f", "\u{000f}"),
            (r"\F", "\u{000f}"),
        ] {
            assert_eq!(unescape(input), expected, "{input}");
        }
    }

    #[test]
    fn unescape_keeps_the_escape_rules() {
        for (input, expected) in [
            (r"\a b", "\nb"),
            (r"\00000a", "\n"),
            (r"\41", "A"),
            (r"\0 ", "\u{FFFD}"),
            (r"\!", "!"),
            (r"\g", "g"),
            (r"\\", "\\"),
        ] {
            assert_eq!(unescape(input), expected, "{input}");
        }
    }

    #[test]
    fn unescape_removes_backslashes() {
        assert_eq!(unescape(r"red-1\/2"), "red-1/2");
    }

    #[test]
    fn unescape_replaces_out_of_range_escaped_code_points() {
        assert_eq!(
            unescape(
                r"--Coding-Projects-CharacterMapper-Master-Workspace\d8819554-4725-4235-9d22-2d0ed572e924"
            ),
            "--Coding-Projects-CharacterMapper-Master-Workspace\u{FFFD}54-4725-4235-9d22-2d0ed572e924"
        );
    }

    // The tests below are not in the upstream file.

    #[test]
    fn escape_follows_the_cssom_rules() {
        assert_eq!(escape(""), "");
        assert_eq!(escape("-"), "\\-");
        assert_eq!(escape("--"), "--");
        assert_eq!(escape("1a"), "\\31 a");
        assert_eq!(escape("-1a"), "-\\31 a");
        assert_eq!(escape("a1"), "a1");
        assert_eq!(escape("a\0b"), "a\u{FFFD}b");
        assert_eq!(escape("a\u{1}\u{1f}\u{7f}"), "a\\1 \\1f \\7f ");
        assert_eq!(escape("a b.c:d"), "a\\ b\\.c\\:d");
        assert_eq!(escape("é😀_-"), "é😀_-");
        assert_eq!(escape("é 😀"), "é\\ 😀");
        assert_eq!(escape("[&>*]:p-4"), "\\[\\&\\>\\*\\]\\:p-4");
    }

    #[test]
    fn unescape_keeps_a_backslash_at_the_end() {
        assert_eq!(unescape("a\\"), "a\\");
        assert_eq!(unescape("\\"), "\\");
        assert_eq!(unescape("a\\\\\\"), "a\\\\");
    }

    #[test]
    fn unescape_reads_six_digits_and_one_white_space() {
        assert_eq!(unescape(r"\0000411"), "A1");
        assert_eq!(unescape("\\41  b"), "A b");
        assert_eq!(unescape("\\41\tb\\41\nc\\41\u{c}d\\41\re"), "AbAcAdAe");
        assert_eq!(unescape(r"\10ffff"), "\u{10FFFF}");
        assert_eq!(unescape(r"\110000"), "\u{FFFD}");
        assert_eq!(unescape(r"\dfff"), "\u{FFFD}");
        assert_eq!(unescape(r"\1f600"), "😀");
        assert_eq!(unescape("\\😀\\é"), "😀é");
    }
}

// ---------------------------------------------------------------------------
// to-key-path.test.ts

#[test]
fn to_key_path_can_convert_key_paths_to_arrays() {
    assert_eq!(to_key_path("fontSize.xs"), ["fontSize", "xs"]);
    assert_eq!(to_key_path("fontSize.xs[1].lineHeight"), ["fontSize", "xs", "1", "lineHeight"]);
    assert_eq!(to_key_path("colors.red.500"), ["colors", "red", "500"]);
    assert_eq!(to_key_path("colors[red].500"), ["colors", "red", "500"]);
    assert_eq!(to_key_path("colors[red].[500]"), ["colors", "red", "500"]);
    assert_eq!(to_key_path("colors[red]500"), ["colors", "red", "500"]);
    assert_eq!(to_key_path("colors[red][500]"), ["colors", "red", "500"]);
    assert_eq!(to_key_path("colors[red]500[50]5"), ["colors", "red", "500", "50", "5"]);
}

#[test]
fn to_key_path_keeps_dots_in_brackets() {
    assert_eq!(to_key_path("a[b.c].e.f"), ["a", "b.c", "e", "f"]);
    assert_eq!(to_key_path(""), [""]);
    assert_eq!(to_key_path("a..b"), ["a", "", "b"]);
    assert_eq!(to_key_path("a[b"), ["a[b"]);
    assert_eq!(to_key_path("a]b[c]"), ["a]b", "c"]);
}

// ---------------------------------------------------------------------------
// brace-expansion.test.ts

mod brace_expansion_tests {
    use super::*;

    #[test]
    fn expand_patterns() {
        let table: Vec<(&str, Vec<&str>)> = vec![
            ("a/b/c", vec!["a/b/c"]),
            // Groups
            ("a/{x,y,z}/b", vec!["a/x/b", "a/y/b", "a/z/b"]),
            ("{a,b}/{x,y}", vec!["a/x", "a/y", "b/x", "b/y"]),
            ("{{xs,sm,md,lg}:,}hidden", vec!["xs:hidden", "sm:hidden", "md:hidden", "lg:hidden", "hidden"]),
            // Numeric ranges
            ("a/{0..5}/b", vec!["a/0/b", "a/1/b", "a/2/b", "a/3/b", "a/4/b", "a/5/b"]),
            ("a/{-5..0}/b", vec!["a/-5/b", "a/-4/b", "a/-3/b", "a/-2/b", "a/-1/b", "a/0/b"]),
            ("a/{0..-5}/b", vec!["a/0/b", "a/-1/b", "a/-2/b", "a/-3/b", "a/-4/b", "a/-5/b"]),
            ("a/{0..10..5}/b", vec!["a/0/b", "a/5/b", "a/10/b"]),
            ("a/{0..10..-5}/b", vec!["a/10/b", "a/5/b", "a/0/b"]),
            ("a/{10..0..5}/b", vec!["a/10/b", "a/5/b", "a/0/b"]),
            ("a/{10..0..-5}/b", vec!["a/0/b", "a/5/b", "a/10/b"]),
            // A numeric range with padding (padding is not supported)
            ("a/{00..05}/b", vec!["a/0/b", "a/1/b", "a/2/b", "a/3/b", "a/4/b", "a/5/b"]),
            ("a{001..9}b", vec!["a1b", "a2b", "a3b", "a4b", "a5b", "a6b", "a7b", "a8b", "a9b"]),
            // A numeric range with a step
            ("a/{0..5..2}/b", vec!["a/0/b", "a/2/b", "a/4/b"]),
            (
                "bg-red-{100..900..100}",
                vec![
                    "bg-red-100",
                    "bg-red-200",
                    "bg-red-300",
                    "bg-red-400",
                    "bg-red-500",
                    "bg-red-600",
                    "bg-red-700",
                    "bg-red-800",
                    "bg-red-900",
                ],
            ),
            // Nested braces
            ("a{b,c,/{x,y}}/e", vec!["ab/e", "ac/e", "a/x/e", "a/y/e"]),
            ("a{b,c,/{x,y},{z,w}}/e", vec!["ab/e", "ac/e", "a/x/e", "a/y/e", "az/e", "aw/e"]),
            ("a{b,c,/{x,y},{0..2}}/e", vec!["ab/e", "ac/e", "a/x/e", "a/y/e", "a0/e", "a1/e", "a2/e"]),
            (
                "bg-red-{50,{100..900..100},950}",
                vec![
                    "bg-red-50",
                    "bg-red-100",
                    "bg-red-200",
                    "bg-red-300",
                    "bg-red-400",
                    "bg-red-500",
                    "bg-red-600",
                    "bg-red-700",
                    "bg-red-800",
                    "bg-red-900",
                    "bg-red-950",
                ],
            ),
            // Ranges with decimals do not expand
            ("{1.1..2.2}", vec!["1.1..2.2"]),
        ];
        for (input, mut expected) in table {
            let mut actual = expand(input).unwrap();
            actual.sort();
            expected.sort();
            assert_eq!(actual, expected, "{input}");
        }
    }

    #[test]
    fn throws_on_unbalanced_braces() {
        let error = expand("a{b,c{d,e},{f,g}h}x{y,z").unwrap_err();
        assert_eq!(error, ExpandError::NotBalanced("x{y,z".to_string()));
        assert_eq!(error.to_string(), "The pattern `x{y,z` is not balanced.");
    }

    #[test]
    fn throws_when_step_is_set_to_zero() {
        let error = expand("a{0..5..0}/b").unwrap_err();
        assert_eq!(error, ExpandError::ZeroStep);
        assert_eq!(error.to_string(), "Step cannot be zero in sequence expansion.");
    }

    // The tests below are not in the upstream file.

    #[test]
    fn expand_keeps_the_upstream_order() {
        assert_eq!(expand("{a,b}/{x,y}").unwrap(), ["a/x", "b/x", "a/y", "b/y"]);
        assert_eq!(expand("a/{10..0..-5}/b").unwrap(), ["a/10/b", "a/5/b", "a/0/b"]);
        assert_eq!(expand("{3..3}").unwrap(), ["3"]);
        assert_eq!(expand("{-0..1}").unwrap(), ["0", "1"]);
    }

    #[test]
    fn expand_fails_when_the_sequence_has_no_end() {
        let error = expand("{9007199254740992..9007199254740999}").unwrap_err();
        assert!(matches!(error, ExpandError::EndlessSequence(_)));
    }
}

// ---------------------------------------------------------------------------
// decode-arbitrary-value.test.ts

mod decode_arbitrary_value_tests {
    use super::*;

    #[test]
    fn replace_an_underscore_with_a_space() {
        assert_eq!(decode_arbitrary_value("foo_bar"), "foo bar");
    }

    #[test]
    fn replace_multiple_underscores_with_spaces() {
        assert_eq!(decode_arbitrary_value("__foo__bar__"), "  foo  bar  ");
    }

    #[test]
    fn replace_escaped_underscores_with_a_normal_underscore() {
        assert_eq!(decode_arbitrary_value("foo\\_bar"), "foo_bar");
    }

    #[test]
    fn not_replace_underscores_in_url() {
        assert_eq!(decode_arbitrary_value("url(./my_file.jpg)"), "url(./my_file.jpg)");
        assert_eq!(decode_arbitrary_value("no-repeat_url(./my_file.jpg)"), "no-repeat url(./my_file.jpg)");
    }

    #[test]
    fn not_replace_underscores_in_the_first_argument_of_var() {
        assert_eq!(decode_arbitrary_value("var(--spacing-1_5)"), "var(--spacing-1_5)");
        assert_eq!(decode_arbitrary_value("var(--spacing-1_5,_1rem)"), "var(--spacing-1_5, 1rem)");
        assert_eq!(
            decode_arbitrary_value("var(--spacing-1_5,_var(--spacing-2_5,_1rem))"),
            "var(--spacing-1_5, var(--spacing-2_5, 1rem))"
        );
    }

    #[test]
    fn not_replace_underscores_in_the_first_argument_of_theme() {
        assert_eq!(decode_arbitrary_value("theme(--spacing-1_5)"), "theme(--spacing-1_5)");
        assert_eq!(decode_arbitrary_value("theme(--spacing-1_5,_1rem)"), "theme(--spacing-1_5, 1rem)");
        assert_eq!(
            decode_arbitrary_value("theme(--spacing-1_5,_theme(--spacing-2_5,_1rem))"),
            "theme(--spacing-1_5, theme(--spacing-2_5, 1rem))"
        );
    }

    #[test]
    fn leave_var_as_is() {
        assert_eq!(decode_arbitrary_value("var(--foo)"), "var(--foo)");
        assert_eq!(decode_arbitrary_value("var(--headings-h1-size)"), "var(--headings-h1-size)");
    }

    /// The upstream table `adds spaces around math operators`.
    const MATH_TABLE: &[(&str, &str)] = &[
        ("calc(1+2)", "calc(1 + 2)"),
        ("calc(100%+1rem)", "calc(100% + 1rem)"),
        ("calc(1+calc(100%-20px))", "calc(1 + calc(100% - 20px))"),
        ("calc(var(--headings-h1-size)*100)", "calc(var(--headings-h1-size) * 100)"),
        ("calc(var(--headings-h1-size)*calc(100%+50%))", "calc(var(--headings-h1-size) * calc(100% + 50%))"),
        ("min(1+2)", "min(1 + 2)"),
        ("max(1+2)", "max(1 + 2)"),
        ("clamp(1+2,1+3,1+4)", "clamp(1 + 2, 1 + 3, 1 + 4)"),
        ("var(--width, calc(100%+1rem))", "var(--width, calc(100% + 1rem))"),
        ("calc(1px*(7--12/24))", "calc(1px * (7 - -12 / 24))"),
        ("calc((7-32)/(1400-782))", "calc((7 - 32) / (1400 - 782))"),
        ("calc((7-3)/(1400-782))", "calc((7 - 3) / (1400 - 782))"),
        ("calc((7-32)/(1400-782))", "calc((7 - 32) / (1400 - 782))"),
        ("calc((70-3)/(1400-782))", "calc((70 - 3) / (1400 - 782))"),
        ("calc((70-32)/(1400-782))", "calc((70 - 32) / (1400 - 782))"),
        ("calc((704-3)/(1400-782))", "calc((704 - 3) / (1400 - 782))"),
        ("calc((704-320))", "calc((704 - 320))"),
        ("calc((704-320)/1)", "calc((704 - 320) / 1)"),
        ("calc((704-320)/(1400-782))", "calc((704 - 320) / (1400 - 782))"),
        ("calc(24px+-1rem)", "calc(24px + -1rem)"),
        ("calc(24px+(-1rem))", "calc(24px + (-1rem))"),
        ("calc(24px_+_-1rem)", "calc(24px + -1rem)"),
        ("calc(24px+(-1rem))", "calc(24px + (-1rem))"),
        ("calc(24px_+_(-1rem))", "calc(24px + (-1rem))"),
        (
            "calc(var(--10-10px,calc(-20px-(-30px--40px)-50px)))",
            "calc(var(--10-10px,calc(-20px - (-30px - -40px) - 50px)))",
        ),
        ("calc(theme(spacing.1-bar))", "calc(theme(spacing.1-bar))"),
        ("theme(spacing.1-bar)", "theme(spacing.1-bar)"),
        ("calc(theme(spacing.1-bar))", "calc(theme(spacing.1-bar))"),
        ("calc(1rem-theme(spacing.1-bar))", "calc(1rem - theme(spacing.1-bar))"),
        ("calc(theme(spacing.foo-2))", "calc(theme(spacing.foo-2))"),
        ("calc(theme(spacing.foo-bar))", "calc(theme(spacing.foo-bar))"),
        ("calc(100%-var(--foo))", "calc(100% - var(--foo))"),
        ("calc(100PX-theme(spacing.1))", "calc(100PX - theme(spacing.1))"),
        ("min(fit-content,calc(100dvh-4rem))", "min(fit-content, calc(100dvh - 4rem))"),
        (
            "min(theme(spacing.foo-bar),fit-content,calc(20*calc(40-30)))",
            "min(theme(spacing.foo-bar), fit-content, calc(20 * calc(40 - 30)))",
        ),
        (
            "min(fit-content,calc(100dvh-4rem)-calc(50dvh--2px))",
            "min(fit-content, calc(100dvh - 4rem) - calc(50dvh - -2px))",
        ),
        ("min(-3.4e-2-var(--foo),calc-size(auto))", "min(-3.4e-2 - var(--foo), calc-size(auto))"),
        (
            "clamp(-10e3-var(--foo),calc-size(max-content),var(--foo)+-10e3)",
            "clamp(-10e3 - var(--foo), calc-size(max-content), var(--foo) + -10e3)",
        ),
        ("clamp(-3px+4px,-3px+4px,-3px+4px)", "clamp(-3px + 4px, -3px + 4px, -3px + 4px)"),
        ("calc(var(--foo-bar-bar)*2)", "calc(var(--foo-bar-bar) * 2)"),
        ("calc(env(safe-area-inset-bottom)*2)", "calc(env(safe-area-inset-bottom) * 2)"),
        (
            "fit-content(min(max-content,max(min-content,calc(20px+1em))))",
            "fit-content(min(max-content, max(min-content, calc(20px + 1em))))",
        ),
        ("env(safe-area-inset-bottom,calc(10px+20px))", "env(safe-area-inset-bottom,calc(10px + 20px))"),
        (
            "calc(env(safe-area-inset-bottom,calc(10px+20px))+5px)",
            "calc(env(safe-area-inset-bottom,calc(10px + 20px)) + 5px)",
        ),
        ("minmax(min-content,25%)", "minmax(min-content,25%)"),
        (
            "radial-gradient(calc(1+2)),radial-gradient(calc(1+2))",
            "radial-gradient(calc(1 + 2)),radial-gradient(calc(1 + 2))",
        ),
        ("w-[calc(anchor-size(width)+8px)]", "w-[calc(anchor-size(width) + 8px)]"),
        ("w-[calc(anchor-size(foo(bar))+8px)]", "w-[calc(anchor-size(foo(bar)) + 8px)]"),
        (
            "[content-start]_calc(100%-1px)_[content-end]_minmax(1rem,1fr)",
            "[content-start] calc(100% - 1px) [content-end] minmax(1rem,1fr)",
        ),
        ("round(1+2,1+3)", "round(1 + 2, 1 + 3)"),
        ("round(to-zero,1+2,1+3)", "round(to-zero, 1 + 2, 1 + 3)"),
        ("env((safe-area-inset-bottom))", "env((safe-area-inset-bottom))"),
        ("atan(1 + -infinity)", "atan(1 + -infinity)"),
    ];

    #[test]
    fn adds_spaces_around_math_operators() {
        assert_eq!(MATH_TABLE.len(), 53);
        for (input, output) in MATH_TABLE {
            assert_eq!(decode_arbitrary_value(input), *output, "{input}");
        }
    }

    // The tests below are not in the upstream file.

    #[test]
    fn math_operators_have_no_effect_out_of_math_functions() {
        assert_eq!(add_whitespace_around_math_operators("1+2"), "1+2");
        assert_eq!(add_whitespace_around_math_operators("foo(1+2)"), "foo(1+2)");
        assert_eq!(add_whitespace_around_math_operators("calc(1+2)"), "calc(1 + 2)");
        assert_eq!(add_whitespace_around_math_operators("calc(é+1)"), "calc(é + 1)");
        assert_eq!(add_whitespace_around_math_operators("calc(1  +  2)"), "calc(1 + 2)");
    }

    #[test]
    fn has_math_fn_finds_a_call_of_a_math_function() {
        assert!(has_math_fn("calc(1+2)"));
        assert!(has_math_fn("1px + round(up, 1px, 2px)"));
        assert!(has_math_fn("mycalc(1)"));
        assert!(!has_math_fn("calc"));
        assert!(!has_math_fn("calc (1)"));
        assert!(!has_math_fn("var(--calc)"));
    }
}

// ---------------------------------------------------------------------------
// replace-shadow-colors.test.ts

mod replace_shadow_colors_tests {
    use super::*;

    fn replacer(color: &str) -> String {
        format!("var(--tw-shadow-color, {color})")
    }

    /// `replaceAlpha` of `utilities.ts`, for an alpha that is not a number.
    fn replace_alpha(value: &str, alpha: &str) -> String {
        format!("oklab(from {value} l a b / {alpha})")
    }

    fn intensity_replacer(color: &str) -> String {
        format!("var(--tw-shadow-color, {})", replace_alpha(color, "50%"))
    }

    #[test]
    fn handle_var_shadow() {
        assert_eq!(replace_shadow_colors("var(--my-shadow)", replacer), "var(--my-shadow)");
    }

    #[test]
    fn handle_var_shadow_with_offset() {
        assert_eq!(replace_shadow_colors("1px var(--my-shadow)", replacer), "1px var(--my-shadow)");
    }

    #[test]
    fn handle_var_color_with_offsets() {
        assert_eq!(
            replace_shadow_colors("1px 1px var(--my-color)", replacer),
            "1px 1px var(--tw-shadow-color, var(--my-color))"
        );
    }

    #[test]
    fn handle_var_color_with_zero_offsets() {
        assert_eq!(
            replace_shadow_colors("0 0 0 var(--my-color)", replacer),
            "0 0 0 var(--tw-shadow-color, var(--my-color))"
        );
    }

    #[test]
    fn handle_two_values_with_currentcolor() {
        assert_eq!(
            replace_shadow_colors("1px 2px", replacer),
            "1px 2px var(--tw-shadow-color, currentcolor)"
        );
    }

    #[test]
    fn handle_three_values_with_currentcolor() {
        assert_eq!(
            replace_shadow_colors("1px 2px 3px", replacer),
            "1px 2px 3px var(--tw-shadow-color, currentcolor)"
        );
    }

    #[test]
    fn handle_four_values_with_currentcolor() {
        assert_eq!(
            replace_shadow_colors("1px 2px 3px 4px", replacer),
            "1px 2px 3px 4px var(--tw-shadow-color, currentcolor)"
        );
    }

    #[test]
    fn find_the_color_regardless_of_its_position() {
        let xs = ["calc(var(--spacing) * 1)", "1", "--spacing(1)"];
        let ys = ["calc(var(--spacing) * 2)", "2", "--spacing(2)"];
        let blurs = ["calc(var(--spacing) * 3)", "3", "--spacing(3)"];
        let spreads = ["calc(var(--spacing) * 4)", "4", "--spacing(4)"];
        let colors = ["black", "rgb(0, 0, 0)", "#000", "--alpha(var(--color) / 50%)", "var(--uknown-color)"];
        let mut count = 0;
        for x in xs {
            for y in ys {
                for blur in blurs {
                    for spread in spreads {
                        for color in colors {
                            let expected_color = format!("var(--tw-shadow-color, {color})");
                            for input in [
                                format!("{x} {color} {y} {blur} {spread}"),
                                format!("{x} {y} {color} {blur} {spread}"),
                                format!("{x} {y} {blur} {color} {spread}"),
                            ] {
                                assert_eq!(
                                    replace_shadow_colors(&input, replacer),
                                    input.replacen(color, &expected_color, 1)
                                );
                                count += 1;
                            }
                        }
                    }
                }
            }
        }
        assert_eq!(count, 3 * 3 * 3 * 3 * 5 * 3);
    }

    // With `var(…)`, the types of the variables are not known, but the
    // color can be known.
    #[test]
    fn find_the_color() {
        for color in ["black", "#000", "rgb(0, 0, 0)", "--alpha(var(--color) / 50%)"] {
            let expected_color = format!("var(--tw-shadow-color, {color})");
            for input in [
                format!("var(--x) var(--y) {color}"),
                format!("var(--x) var(--y) var(--blur) {color}"),
                format!("var(--x) var(--y) var(--blur) var(--spread) {color}"),
            ] {
                assert_eq!(
                    replace_shadow_colors(&input, replacer),
                    input.replacen(color, &expected_color, 1)
                );
            }
        }
    }

    #[test]
    fn handle_multiple_shadows() {
        let input = ["var(--my-shadow)", "1px 1px var(--my-color)", "0 0 1px var(--my-color)"].join(", ");
        assert_eq!(
            replace_shadow_colors(&input, replacer),
            "var(--my-shadow), 1px 1px var(--tw-shadow-color, var(--my-color)), 0 0 1px var(--tw-shadow-color, var(--my-color))"
        );
    }

    #[test]
    fn handle_var_color_with_intensity() {
        assert_eq!(
            replace_shadow_colors("1px 1px var(--my-color)", intensity_replacer),
            "1px 1px var(--tw-shadow-color, oklab(from var(--my-color) l a b / 50%))"
        );
    }

    #[test]
    fn handle_box_shadow_with_intensity() {
        assert_eq!(
            replace_shadow_colors("1px 1px var(--my-color)", intensity_replacer),
            "1px 1px var(--tw-shadow-color, oklab(from var(--my-color) l a b / 50%))"
        );
    }

    #[test]
    fn handle_four_values_with_intensity_and_no_color_value() {
        assert_eq!(
            replace_shadow_colors("1px 2px 3px 4px", intensity_replacer),
            "1px 2px 3px 4px var(--tw-shadow-color, oklab(from currentcolor l a b / 50%))"
        );
    }

    #[test]
    fn handle_multiple_shadows_with_intensity() {
        let input = ["var(--my-shadow)", "1px 1px var(--my-color)", "0 0 1px var(--my-color)"].join(", ");
        assert_eq!(
            replace_shadow_colors(&input, intensity_replacer),
            "var(--my-shadow), 1px 1px var(--tw-shadow-color, oklab(from var(--my-color) l a b / 50%)), 0 0 1px var(--tw-shadow-color, oklab(from var(--my-color) l a b / 50%))"
        );
    }

    // The tests below are not in the upstream file.

    #[test]
    fn the_replacer_can_keep_state() {
        let mut colors = Vec::new();
        let result = replace_shadow_colors("inset 0 1px #fff,0 0 RED", |color| {
            colors.push(color.to_string());
            "x".to_string()
        });
        assert_eq!(result, "inset 0 1px x, 0 0 x");
        assert_eq!(colors, ["#fff", "RED"]);
    }
}

// ---------------------------------------------------------------------------
// compare.test.ts

mod compare_tests {
    use super::*;

    /// Some different orders of the items, in place of the upstream shuffle.
    fn orders<T: Clone>(items: &[T]) -> Vec<Vec<T>> {
        let mut result = Vec::new();
        let mut current = items.to_vec();
        for step in 0..items.len() {
            current.rotate_left(1);
            result.push(current.clone());
            let mut reversed = current.clone();
            reversed.reverse();
            result.push(reversed);
            let mut mixed = current.clone();
            mixed.swap(0, step);
            result.push(mixed);
        }
        result
    }

    #[test]
    fn compare_two_strings() {
        use Ordering::*;
        for (a, b, expected) in [
            // Same strings
            ("abc", "abc", Equal),
            // The shorter string comes first
            ("abc", "abcd", Less),
            // The longer string comes last
            ("abcd", "abc", Greater),
            // Numbers
            ("1", "1", Equal),
            ("1", "2", Less),
            ("2", "1", Greater),
            ("1", "10", Less),
            ("10", "1", Greater),
            // Numbers of different lengths
            ("75", "700", Less),
            ("700", "75", Greater),
            ("75", "770", Less),
            ("770", "75", Greater),
        ] {
            assert_eq!(compare(a, b), expected, "{a} {b}");
        }
    }

    #[test]
    fn sort_strings_with_numbers_consistently() {
        let classes = ["p-0", "p-0.5", "p-1", "p-1.5", "p-10", "p-12", "p-2", "p-20", "p-21"];
        for mut order in orders(&classes) {
            order.sort_by(|a, z| compare(a, z));
            assert_eq!(order, ["p-0", "p-0.5", "p-1", "p-1.5", "p-2", "p-10", "p-12", "p-20", "p-21"]);
        }
    }

    #[test]
    fn sort_strings_with_modifiers_consistently() {
        let classes = [
            "text-5xl",
            "text-6xl",
            "text-6xl/loose",
            "text-6xl/wide",
            "bg-red-500",
            "bg-red-500/50",
            "bg-red-500/70",
            "bg-red-500/60",
            "bg-red-50",
            "bg-red-50/50",
            "bg-red-50/70",
            "bg-red-50/60",
        ];
        for mut order in orders(&classes) {
            order.sort_by(|a, z| compare(a, z));
            assert_eq!(
                order,
                [
                    "bg-red-50",
                    "bg-red-50/50",
                    "bg-red-50/60",
                    "bg-red-50/70",
                    "bg-red-500",
                    "bg-red-500/50",
                    "bg-red-500/60",
                    "bg-red-500/70",
                    "text-5xl",
                    "text-6xl",
                    "text-6xl/loose",
                    "text-6xl/wide",
                ]
            );
        }
    }

    #[test]
    fn sort_strings_with_multiple_numbers_consistently() {
        let classes = [
            "foo-123-bar-456-baz-789",
            "foo-123-bar-456-baz-788",
            "foo-123-bar-456-baz-790",
            "foo-123-bar-455-baz-789",
            "foo-123-bar-456-baz-789",
            "foo-123-bar-457-baz-789",
            "foo-123-bar-456-baz-789",
            "foo-124-bar-456-baz-788",
            "foo-125-bar-456-baz-790",
            "foo-126-bar-455-baz-789",
            "foo-127-bar-456-baz-789",
            "foo-128-bar-457-baz-789",
            "foo-1-bar-2-baz-3",
            "foo-12-bar-34-baz-45",
            "foo-12-bar-34-baz-4",
            "foo-12-bar-34-baz-456",
        ];
        for mut order in orders(&classes) {
            order.sort_by(|a, z| compare(a, z));
            assert_eq!(
                order,
                [
                    "foo-1-bar-2-baz-3",
                    "foo-12-bar-34-baz-4",
                    "foo-12-bar-34-baz-45",
                    "foo-12-bar-34-baz-456",
                    "foo-123-bar-455-baz-789",
                    "foo-123-bar-456-baz-788",
                    "foo-123-bar-456-baz-789",
                    "foo-123-bar-456-baz-789",
                    "foo-123-bar-456-baz-789",
                    "foo-123-bar-456-baz-790",
                    "foo-123-bar-457-baz-789",
                    "foo-124-bar-456-baz-788",
                    "foo-125-bar-456-baz-790",
                    "foo-126-bar-455-baz-789",
                    "foo-127-bar-456-baz-789",
                    "foo-128-bar-457-baz-789",
                ]
            );
        }
    }

    /// All orders of the input: the algorithm of Heap, as upstream.
    fn permutations<T: Clone>(input: &[T]) -> Vec<Vec<T>> {
        let mut input = input.to_vec();
        let mut result = vec![input.clone()];
        let mut stack = vec![0; input.len()];
        let mut pos = 1;
        while pos < input.len() {
            if stack[pos] < pos {
                let k = if pos % 2 == 0 { 0 } else { stack[pos] };
                input.swap(k, pos);
                result.push(input.clone());
                stack[pos] += 1;
                pos = 1;
            } else {
                stack[pos] = 0;
                pos += 1;
            }
        }
        result
    }

    #[test]
    fn sort_is_stable() {
        let classes = ["duration-initial", "duration-75", "duration-150", "duration-700", "duration-1000"];
        let all = permutations(&classes);
        assert_eq!(all.len(), 120);
        for mut permutation in all {
            permutation.sort_by(|a, z| compare(a, z));
            assert_eq!(
                permutation,
                ["duration-75", "duration-150", "duration-700", "duration-1000", "duration-initial"]
            );
        }
    }

    // The tests below are not in the upstream file.

    #[test]
    fn equal_numbers_with_different_text_compare_as_text() {
        assert_eq!(compare("0123", "123"), Ordering::Less);
        assert_eq!(compare("a-01", "a-1"), Ordering::Less);
        assert_eq!(compare("9007199254740993", "9007199254740992"), Ordering::Greater);
        let big = "9".repeat(400);
        let bigger = format!("{}8", "9".repeat(400));
        assert_eq!(compare(&big, &bigger), Ordering::Less);
    }

    #[test]
    fn text_compares_by_utf16_code_units() {
        // U+E000 is one code unit. U+1F600 is two surrogates, which are lower.
        assert_eq!(compare("a\u{E000}", "a😀"), Ordering::Greater);
        assert_eq!(compare("aé", "aè"), Ordering::Greater);
        assert_eq!(compare("a", "é"), Ordering::Less);
        assert_eq!(compare("😀", "😁"), Ordering::Less);
    }
}

// ---------------------------------------------------------------------------
// The modules below have no upstream test file.

mod js_tests {
    use super::*;

    fn same(actual: f64, expected: f64) {
        assert!(
            actual.to_bits() == expected.to_bits() || (actual.is_nan() && expected.is_nan()),
            "{actual} is not {expected}"
        );
    }

    #[test]
    fn number_follows_javascript() {
        same(js_number(""), 0.0);
        same(js_number("   "), 0.0);
        same(js_number(" 12 "), 12.0);
        same(js_number("\n\t12\u{A0}\u{FEFF}\u{2028}"), 12.0);
        same(js_number("12\u{85}"), f64::NAN);
        same(js_number("1e3"), 1000.0);
        same(js_number("1E+3"), 1000.0);
        same(js_number("1e-3"), 0.001);
        same(js_number("1e"), f64::NAN);
        same(js_number("0x10"), 16.0);
        same(js_number("0X1f"), 31.0);
        same(js_number("0b101"), 5.0);
        same(js_number("0o17"), 15.0);
        same(js_number("0x"), f64::NAN);
        same(js_number("-0x10"), f64::NAN);
        same(js_number("0x1g"), f64::NAN);
        same(js_number(".5"), 0.5);
        same(js_number("5."), 5.0);
        same(js_number("+.5"), 0.5);
        same(js_number("."), f64::NAN);
        same(js_number("+"), f64::NAN);
        same(js_number("-0"), -0.0);
        same(js_number("0"), 0.0);
        same(js_number("Infinity"), f64::INFINITY);
        same(js_number("+Infinity"), f64::INFINITY);
        same(js_number("-Infinity"), f64::NEG_INFINITY);
        same(js_number("infinity"), f64::NAN);
        same(js_number("inf"), f64::NAN);
        same(js_number("nan"), f64::NAN);
        same(js_number("NaN"), f64::NAN);
        same(js_number("1_000"), f64::NAN);
        same(js_number("1,000"), f64::NAN);
        same(js_number("12px"), f64::NAN);
        same(js_number("1 2"), f64::NAN);
        same(js_number("--1"), f64::NAN);
        same(js_number("007"), 7.0);
        same(js_number("1e400"), f64::INFINITY);
        same(js_number("-1e400"), f64::NEG_INFINITY);
        same(js_number("9007199254740993"), 9007199254740992.0);
        same(js_number("0.1"), 0.1);
        same(js_number("0x1fffffffffffff"), 9007199254740991.0);
        same(js_number("0x20000000000001"), 9007199254740992.0);
        same(js_number("0x20000000000003"), 9007199254740996.0);
    }

    #[test]
    fn number_to_string_follows_javascript() {
        assert_eq!(js_number_to_string(0.0), "0");
        assert_eq!(js_number_to_string(-0.0), "0");
        assert_eq!(js_number_to_string(1.0), "1");
        assert_eq!(js_number_to_string(-1.5), "-1.5");
        assert_eq!(js_number_to_string(f64::NAN), "NaN");
        assert_eq!(js_number_to_string(f64::INFINITY), "Infinity");
        assert_eq!(js_number_to_string(f64::NEG_INFINITY), "-Infinity");
        assert_eq!(js_number_to_string(0.1 + 0.2), "0.30000000000000004");
        assert_eq!(js_number_to_string(1e21), "1e+21");
        assert_eq!(js_number_to_string(1e20), "100000000000000000000");
        assert_eq!(js_number_to_string(123456789012345680000.0), "123456789012345680000");
        assert_eq!(js_number_to_string(1.5e21), "1.5e+21");
        assert_eq!(js_number_to_string(1e-7), "1e-7");
        assert_eq!(js_number_to_string(1.5e-7), "1.5e-7");
        assert_eq!(js_number_to_string(1e-6), "0.000001");
        assert_eq!(js_number_to_string(0.00001234), "0.00001234");
        assert_eq!(js_number_to_string(-1e-7), "-1e-7");
        assert_eq!(js_number_to_string(100.0), "100");
        assert_eq!(js_number_to_string(1e15), "1000000000000000");
        assert_eq!(js_number_to_string(123.456), "123.456");
        assert_eq!(js_number_to_string(5e-324), "5e-324");
        assert_eq!(js_number_to_string(f64::MAX), "1.7976931348623157e+308");
        assert_eq!(js_number_to_string(9007199254740993.0), "9007199254740992");
        assert_eq!(js_number_to_string(1.0 / 3.0), "0.3333333333333333");
        assert_eq!(js_number_to_string(2.0 / 3.0), "0.6666666666666666");
        assert_eq!(js_number_to_string(0.25 * 100.0), "25");
        // The exact value is in the middle of two short decimal numbers.
        assert_eq!(js_number_to_string(f64::from_bits(0x4301_3b0c_ad59_6b62)), "606250029755756.2");
        assert_eq!(js_number_to_string(2f64.powi(-24)), "5.960464477539063e-8");
        assert_eq!(js_number_to_string(2f64.powi(-25)), "2.9802322387695312e-8");
    }

    #[test]
    fn parse_float_follows_javascript() {
        same(js_parse_float(""), f64::NAN);
        same(js_parse_float(" 12 "), 12.0);
        same(js_parse_float("12px"), 12.0);
        same(js_parse_float("1e3"), 1000.0);
        same(js_parse_float("1e3px"), 1000.0);
        same(js_parse_float("1e"), 1.0);
        same(js_parse_float("1em"), 1.0);
        same(js_parse_float("1e+"), 1.0);
        same(js_parse_float("0x10"), 0.0);
        same(js_parse_float(".5"), 0.5);
        same(js_parse_float("5."), 5.0);
        same(js_parse_float("5.rem"), 5.0);
        same(js_parse_float("."), f64::NAN);
        same(js_parse_float("-.5e1x"), -5.0);
        same(js_parse_float("-0"), -0.0);
        same(js_parse_float("Infinity"), f64::INFINITY);
        same(js_parse_float("-Infinityx"), f64::NEG_INFINITY);
        same(js_parse_float("infinity"), f64::NAN);
        same(js_parse_float("1_000"), 1.0);
        same(js_parse_float("1.2.3"), 1.2);
        same(js_parse_float("calc(1px)"), f64::NAN);
        same(js_parse_float("+ 1"), f64::NAN);
        same(js_parse_float("\u{FEFF}40rem"), 40.0);
    }

    #[test]
    fn parse_int_follows_javascript() {
        same(js_parse_int("", 10), f64::NAN);
        same(js_parse_int(" 12 ", 10), 12.0);
        same(js_parse_int("12px", 10), 12.0);
        same(js_parse_int("1e3", 10), 1.0);
        same(js_parse_int("0x10", 10), 0.0);
        same(js_parse_int("0x10", 16), 16.0);
        same(js_parse_int("0x10", 0), 16.0);
        same(js_parse_int("010", 0), 10.0);
        same(js_parse_int(".5", 10), f64::NAN);
        same(js_parse_int("5.", 10), 5.0);
        same(js_parse_int("-0", 10), -0.0);
        same(js_parse_int("-12", 10), -12.0);
        same(js_parse_int("+12", 10), 12.0);
        same(js_parse_int("Infinity", 10), f64::NAN);
        same(js_parse_int("1_000", 10), 1.0);
        same(js_parse_int("ff", 16), 255.0);
        same(js_parse_int("FF", 16), 255.0);
        same(js_parse_int("zz", 36), 1295.0);
        same(js_parse_int("102", 2), 2.0);
        same(js_parse_int("2", 2), f64::NAN);
        same(js_parse_int("10", 1), f64::NAN);
        same(js_parse_int("10", 37), f64::NAN);
        same(js_parse_int("9007199254740993", 10), 9007199254740992.0);
        same(js_parse_int("123456789012345678901234567890", 10), 1.2345678901234568e29);
    }

    #[test]
    fn to_fixed_follows_javascript() {
        assert_eq!(js_to_fixed(0.5, 0), "1");
        assert_eq!(js_to_fixed(1.5, 0), "2");
        assert_eq!(js_to_fixed(2.5, 0), "3");
        assert_eq!(js_to_fixed(-2.5, 0), "-3");
        assert_eq!(js_to_fixed(1.005, 2), "1.00");
        assert_eq!(js_to_fixed(1.45, 1), "1.4");
        assert_eq!(js_to_fixed(8.345, 2), "8.35");
        assert_eq!(js_to_fixed(0.0, 2), "0.00");
        assert_eq!(js_to_fixed(-0.0, 2), "0.00");
        assert_eq!(js_to_fixed(-0.0001, 2), "-0.00");
        assert_eq!(js_to_fixed(9.995, 2), "9.99");
        assert_eq!(js_to_fixed(99.99, 1), "100.0");
        assert_eq!(js_to_fixed(123.456, 0), "123");
        assert_eq!(js_to_fixed(1e21, 2), "1e+21");
        assert_eq!(js_to_fixed(f64::NAN, 2), "NaN");
        assert_eq!(js_to_fixed(0.1, 20), "0.10000000000000000555");
    }

    #[test]
    fn strings_follow_javascript() {
        assert_eq!(js_trim(" \t\n\r\u{B}\u{C}\u{A0}\u{FEFF}\u{2028}\u{2029}\u{3000}a b\u{1680} "), "a b");
        assert_eq!(js_trim("\u{85}a\u{85}"), "\u{85}a\u{85}");
        assert_eq!(js_trim_start(" a "), "a ");
        assert_eq!(js_trim_end(" a "), " a");
        assert_eq!(js_to_lower_case("ABC-é"), "abc-é");
        assert_eq!(js_to_lower_case("blac\u{212A}"), "black");
        assert_eq!(js_string_length("a😀é"), 4);
        assert_eq!(js_compare_strings("a", "b"), Ordering::Less);
        assert_eq!(js_compare_strings("a", "a"), Ordering::Equal);
        assert_eq!(js_compare_strings("ab", "a"), Ordering::Greater);
        assert_eq!(js_compare_strings("\u{E000}", "😀"), Ordering::Greater);
        assert_eq!(js_compare_strings("\u{D7FF}", "😀"), Ordering::Less);
        assert!(js_is_integer(5.0));
        assert!(!js_is_integer(5.5));
        assert!(!js_is_integer(f64::INFINITY));
        assert!(!js_is_integer(f64::NAN));
    }
}

mod infer_data_type_tests {
    use super::*;

    const ALL: [DataType; 17] = [
        DataType::Color,
        DataType::Length,
        DataType::Percentage,
        DataType::Ratio,
        DataType::Number,
        DataType::Integer,
        DataType::Url,
        DataType::Position,
        DataType::BgSize,
        DataType::LineWidth,
        DataType::Image,
        DataType::FamilyName,
        DataType::GenericName,
        DataType::AbsoluteSize,
        DataType::RelativeSize,
        DataType::Angle,
        DataType::Vector,
    ];

    #[test]
    fn names_of_data_types_go_in_the_two_directions() {
        for ty in ALL {
            assert_eq!(DataType::from_name(ty.as_str()), Some(ty));
        }
        assert_eq!(DataType::from_name("bg-size"), Some(DataType::BgSize));
        assert_eq!(DataType::from_name("size"), None);
    }

    #[test]
    fn infer_the_first_type_of_the_list() {
        assert_eq!(infer_data_type("var(--x)", &ALL), None);
        assert_eq!(infer_data_type("red", &ALL), Some(DataType::Color));
        assert_eq!(infer_data_type("#fff", &ALL), Some(DataType::Color));
        assert_eq!(infer_data_type("1px", &ALL), Some(DataType::Length));
        assert_eq!(infer_data_type("50%", &ALL), Some(DataType::Percentage));
        assert_eq!(infer_data_type("16/9", &ALL), Some(DataType::Ratio));
        assert_eq!(infer_data_type("1.5", &ALL), Some(DataType::Number));
        assert_eq!(infer_data_type("1.5", &[DataType::Integer, DataType::Number]), Some(DataType::Number));
        assert_eq!(infer_data_type("15", &[DataType::Integer, DataType::Number]), Some(DataType::Integer));
        assert_eq!(infer_data_type("url(a.png)", &ALL), Some(DataType::Url));
        assert_eq!(infer_data_type("top", &ALL), Some(DataType::Position));
        assert_eq!(infer_data_type("cover", &ALL), Some(DataType::BgSize));
        assert_eq!(infer_data_type("thin", &ALL), Some(DataType::LineWidth));
        assert_eq!(infer_data_type("linear-gradient(red, blue)", &ALL), Some(DataType::Image));
        assert_eq!(infer_data_type("serif", &ALL), Some(DataType::FamilyName));
        assert_eq!(infer_data_type("serif", &[DataType::GenericName]), Some(DataType::GenericName));
        assert_eq!(infer_data_type("small", &[DataType::AbsoluteSize]), Some(DataType::AbsoluteSize));
        assert_eq!(infer_data_type("larger", &[DataType::RelativeSize]), Some(DataType::RelativeSize));
        assert_eq!(infer_data_type("45deg", &[DataType::Angle]), Some(DataType::Angle));
        assert_eq!(infer_data_type("1 0 0", &[DataType::Vector]), Some(DataType::Vector));
        assert_eq!(infer_data_type("1px", &[]), None);
    }

    #[test]
    fn numbers_follow_the_upstream_expression() {
        for value in ["1", "-1", "+1", ".5", "1.5", "1e3", "1E-3", "-.5e+2", "007"] {
            assert!(is_number(value), "{value}");
        }
        for value in ["", "1.", ".", "1e", "1e+", "1 ", " 1", "1px", "--1", "1.2.3", "１"] {
            assert!(!is_number(value), "{value}");
        }
        assert!(is_number("calc(1px)"));
    }

    #[test]
    fn lengths_have_a_known_unit() {
        for value in
            ["1px", "1.5rem", "-2em", "1e3px", "1Q", "1cqmax", "--spacing(2)", "--SPACING(2)", "calc(1+2)"]
        {
            assert!(is_length(value), "{value}");
        }
        for value in ["1", "px", "1q", "1PX", "1 px", "1pxx", "1e", "--spacing", "1%"] {
            assert!(!is_length(value), "{value}");
        }
    }

    #[test]
    fn other_types_follow_the_upstream_expressions() {
        assert!(is_percentage("50%"));
        assert!(!is_percentage("50"));
        assert!(!is_percentage("50%%"));
        assert!(is_fraction("1/2"));
        assert!(is_fraction("1 / 2"));
        assert!(is_fraction("1\u{A0}/\n2"));
        assert!(!is_fraction("1/"));
        assert!(!is_fraction("1//2"));
        assert!(is_url("url()"));
        assert!(is_url("url(a)b)"));
        assert!(!is_url("url(a\nb)"));
        assert!(!is_url("url(a\u{2028}b)"));
        assert!(!is_url("URL(a)"));
        assert!(!is_url("url("));
        assert!(is_angle("1turn"));
        assert!(!is_angle("1"));
        assert!(is_vector("1 0  -1.5"));
        assert!(!is_vector("1 0"));
        assert!(!is_vector("1 0 0 0"));
        assert!(!is_vector("1\t0 0"));
        assert!(is_image("url(a),linear-gradient(red, blue),var(--x)"));
        // A space after the comma is a part of the next item.
        assert!(!is_image("url(a), url(b)"));
        assert!(is_image("repeating-conic-gradient(red, blue)"));
        assert!(is_image("image-set(a)"));
        assert!(!is_image("var(--x)"));
        assert!(!is_image("red"));
        assert!(is_background_position("top 1px var(--x)"));
        assert!(!is_background_position("var(--x)"));
        assert!(!is_background_position("top foo"));
        assert!(is_background_size("cover,1px auto"));
        assert!(!is_background_size("cover, 1px auto"));
        assert!(!is_background_size("1px 2px 3px"));
        assert!(!is_background_size("foo"));
        assert!(is_line_width("thin 1px 2"));
        assert!(!is_line_width("thin foo"));
        assert!(is_family_name("Inter, var(--x)"));
        assert!(is_family_name(""));
        assert!(!is_family_name("1nter"));
        assert!(!is_family_name("var(--x)"));
        assert!(is_generic_name("ui-sans-serif"));
        assert!(is_absolute_size("xxx-large"));
        assert!(is_relative_size("smaller"));
    }

    #[test]
    fn integers_and_multiples_have_the_usual_form() {
        for value in ["0", "1", "42", "1000000", "1e+21", "123456789012345680000"] {
            assert!(is_positive_integer(value), "{value}");
        }
        for value in
            ["", " 1", "01", "1.0", "-1", "+1", "1.5", "1e3", "1e21", "0x10", "-0", "abc", "Infinity"]
        {
            assert!(!is_positive_integer(value), "{value}");
        }
        assert!(is_strict_positive_integer("1"));
        assert!(!is_strict_positive_integer("0"));
        assert!(!is_strict_positive_integer("01"));
        for value in ["0", "0.25", "0.5", "1", "1.75", "100", "2.25"] {
            assert!(is_valid_spacing_multiplier(value), "{value}");
            assert!(is_valid_opacity_value(value), "{value}");
        }
        for value in ["", "0.1", ".5", "1.0", "0.250", "-1", "1e1", "Infinity", "NaN", "abc"] {
            assert!(!is_valid_spacing_multiplier(value), "{value}");
            assert!(!is_valid_opacity_value(value), "{value}");
        }
    }

    #[test]
    fn colors_are_hex_functions_or_names() {
        for value in [
            "#",
            "#fff",
            "red",
            "RED",
            "rgb(0 0 0)",
            "RGBA(",
            "oklch(1 0 0)",
            "light-dark(a, b)",
            "--alpha(red / 50%)",
            "currentColor",
            "transparent",
            "AccentColorText",
            "blac\u{212A}",
        ] {
            assert!(is_color(value), "{value}");
        }
        for value in ["", "rgb", "rgbx(0)", "foo", "var(--x)", " red", "okrgb(1)"] {
            assert!(!is_color(value), "{value}");
        }
        assert!(is_named_color("rebeccapurple"));
        assert!(is_named_color("LightGoldenRodYellow"));
        assert!(!is_named_color("#fff"));
        assert!(!is_named_color("rgb(0 0 0)"));
        assert!(!is_named_color("lightgoldenrodyellowx"));
    }
}

#[test]
fn is_valid_arbitrary_checks_the_balance() {
    for value in ["", "a", "(a)", "[a(b)]", "'('", "\\(", "a{b", "(a{b}c;)", "[a}]", "\"[\""] {
        assert!(is_valid_arbitrary(value), "{value}");
    }
    for value in ["(", ")", "(]", "a;b", "}", "a{b}", "&{color:red}", "'a')"] {
        assert!(!is_valid_arbitrary(value), "{value}");
    }
    assert!(is_valid_arbitrary(&format!("{}{}", "(".repeat(256), ")".repeat(256))));
    assert!(!is_valid_arbitrary(&format!("{}{}", "(".repeat(257), ")".repeat(257))));
}

#[test]
fn dimensions_parse_a_number_with_a_unit() {
    assert_eq!(dimensions("64rem"), Some((64.0, Some("rem"))));
    assert_eq!(dimensions("-1.5PX"), Some((-1.5, Some("PX"))));
    assert_eq!(dimensions("+.5%"), Some((0.5, Some("%"))));
    assert_eq!(dimensions("12"), Some((12.0, None)));
    assert_eq!(dimensions("007"), Some((7.0, None)));
    for value in ["", "rem", "1.", "1.rem", "1e3", "1 px", "1px2", "1%%", "1.2.3", ".", "--1", "1é"] {
        assert_eq!(dimensions(value), None, "{value}");
    }
}

#[test]
fn compare_breakpoints_sorts_by_unit_then_by_value() {
    use Direction::*;
    assert_eq!(compare_breakpoints("40rem", "40rem", Asc), Ordering::Equal);
    assert_eq!(compare_breakpoints("40rem", "48rem", Asc), Ordering::Less);
    assert_eq!(compare_breakpoints("40rem", "48rem", Desc), Ordering::Greater);
    assert_eq!(compare_breakpoints("100rem", "48rem", Asc), Ordering::Greater);
    // The unit is the bucket: `px` comes before `rem`.
    assert_eq!(compare_breakpoints("900px", "1rem", Asc), Ordering::Less);
    assert_eq!(compare_breakpoints("900px", "1rem", Desc), Ordering::Less);
    // The same value with different text
    assert_eq!(compare_breakpoints("1.0rem", "1rem", Asc), Ordering::Equal);
    // CSS functions are in the bucket of their name. The order is by text.
    assert_eq!(compare_breakpoints("calc(100% - 1rem)", "calc(100% - 2rem)", Asc), Ordering::Less);
    assert_eq!(compare_breakpoints("calc(100% - 2rem)", "calc(100% - 1rem)", Desc), Ordering::Greater);
    assert_eq!(compare_breakpoints("calc(1px)", "min(1px)", Asc), Ordering::Less);
    assert_eq!(compare_breakpoints("40rem", "calc(1px)", Asc), Ordering::Greater);
}

#[test]
fn topological_sort_puts_dependencies_first() {
    let graph = vec![("a", vec!["b", "c"]), ("b", vec!["c"]), ("c", vec![]), ("d", vec!["a", "missing"])];
    assert_eq!(topological_sort(&graph), Ok(vec!["c", "b", "a", "d"]));
    let empty: Vec<(&str, Vec<&str>)> = Vec::new();
    assert_eq!(topological_sort(&empty), Ok(vec![]));
}

#[test]
fn topological_sort_finds_a_circular_dependency() {
    let graph = vec![("a", vec!["b"]), ("b", vec!["c"]), ("c", vec!["a"])];
    assert_eq!(topological_sort(&graph), Err(CircularDependency { path: vec!["a", "b", "c"], start: "a" }));
    let graph = vec![("x", vec![]), ("a", vec!["x", "b"]), ("b", vec!["b"])];
    assert_eq!(topological_sort(&graph), Err(CircularDependency { path: vec!["a", "b"], start: "b" }));
    let graph = vec![("a".to_string(), vec!["a".to_string()])];
    assert_eq!(
        topological_sort(&graph),
        Err(CircularDependency { path: vec!["a".to_string()], start: "a".to_string() })
    );
}

#[test]
fn extract_used_variables_finds_the_variables_of_var() {
    assert_eq!(extract_used_variables("var(--a)"), ["--a"]);
    assert_eq!(extract_used_variables("var(--a, var(--b))"), ["--a", "--b"]);
    assert_eq!(extract_used_variables("calc(var(--a) + var(--b, 1px))"), ["--a", "--b"]);
    assert_eq!(extract_used_variables("var(--a, calc(--c * 2))"), ["--a", "--c"]);
    assert_eq!(extract_used_variables("theme(--a) --b"), Vec::<String>::new());
    assert_eq!(extract_used_variables("1px solid red"), Vec::<String>::new());
    assert_eq!(extract_used_variables("VAR(--a)"), Vec::<String>::new());
}

#[test]
fn default_map_makes_each_value_one_time() {
    let mut calls = 0;
    let mut map: DefaultMap<String, usize, _> = DefaultMap::new(|key: &String| {
        calls += 1;
        key.len()
    });
    assert!(map.is_empty());
    assert_eq!(*map.get("abc"), 3);
    assert_eq!(*map.get("abc"), 3);
    assert_eq!(*map.get("de"), 2);
    assert!(map.has("abc"));
    assert!(!map.has("x"));
    assert_eq!(map.peek("x"), None);
    assert_eq!(map.set("x".to_string(), 9), None);
    assert_eq!(*map.get("x"), 9);
    assert_eq!(map.delete("x"), Some(9));
    assert_eq!(map.len(), 2);
    assert_eq!(map.iter().count(), 2);
    map.clear();
    assert!(map.is_empty());
    drop(map);
    assert_eq!(calls, 2);
}

// ---------------------------------------------------------------------------
// Property tests

/// Text with the characters that have a function in the upstream code.
const SYNTAX: &str = "[a-c0-9()\\[\\]{}'\"\\\\,:;=<> /\n\r\t._%+*é😀\u{A0}\u{2028}-]{0,32}";

fn strings_for_order() -> impl Strategy<Value = String> {
    prop_oneof!["[0-9ab/-]{0,6}", "[0-2]{0,3}[ab]?[0-2]{0,3}", "(0|1|a|é|😀|\u{E000}|10|01|-){0,5}"]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(1500))]

    #[test]
    fn segments_join_to_the_input(input in SYNTAX, separator in prop::sample::select(vec![b',', b' ', b':', b'/', b'.', b'\\', b'='])) {
        let parts = segment(&input, separator);
        let joined = parts.join(&(separator as char).to_string());
        prop_assert_eq!(joined, input);
    }

    #[test]
    fn segments_join_to_the_input_for_all_strings(input in any::<String>(), separator in 0u8..128) {
        let parts = segment(&input, separator);
        let joined = parts.join(&(separator as char).to_string());
        prop_assert_eq!(joined, input);
    }

    #[test]
    fn unescape_is_the_inverse_of_escape(input in any::<String>()) {
        prop_assume!(!input.contains('\0'));
        let escaped = escape(&input);
        let decoded = unescape(&escaped);
        prop_assert_eq!(&*decoded, &*input);
    }

    #[test]
    fn unescape_is_the_inverse_of_escape_for_identifiers(input in "[a-f0-9 _.:/\\\\\t\n\u{1}\u{7f}é😀-]{0,16}") {
        let escaped = escape(&input);
        let decoded = unescape(&escaped);
        prop_assert_eq!(&*decoded, &*input);
    }

    #[test]
    fn unescape_does_not_panic(input in "([a-f0-9 \\\\\t\n]|é|😀|g){0,24}") {
        let _ = unescape(&input);
    }

    #[test]
    fn compare_is_a_total_order(a in strings_for_order(), b in strings_for_order(), c in strings_for_order()) {
        prop_assert_eq!(compare(&a, &a), Ordering::Equal);
        // Antisymmetric
        prop_assert_eq!(compare(&a, &b), compare(&b, &a).reverse());
        prop_assert_eq!(compare(&a, &b) == Ordering::Equal, a == b);
        // Transitive
        let mut sorted = [&a, &b, &c];
        sorted.sort_by(|x, y| compare(x, y));
        prop_assert_ne!(compare(sorted[0], sorted[1]), Ordering::Greater);
        prop_assert_ne!(compare(sorted[1], sorted[2]), Ordering::Greater);
        prop_assert_ne!(compare(sorted[0], sorted[2]), Ordering::Greater);
        if compare(&a, &b) != Ordering::Greater && compare(&b, &c) != Ordering::Greater {
            prop_assert_ne!(compare(&a, &c), Ordering::Greater);
        }
    }

    #[test]
    fn compare_breakpoints_is_antisymmetric(a in "[0-9.]{0,3}(px|rem|em|%|é)?", b in "[0-9.]{0,3}(px|rem|em|%|é)?") {
        for direction in [Direction::Asc, Direction::Desc] {
            let forward = compare_breakpoints(&a, &b, direction);
            let backward = compare_breakpoints(&b, &a, direction);
            // Two values that are not numbers compare by text.
            prop_assert!(forward == backward.reverse() || a == b);
        }
    }

    #[test]
    fn functions_on_text_do_not_panic(input in SYNTAX) {
        let _ = decode_arbitrary_value(&input);
        let _ = add_whitespace_around_math_operators(&input);
        let _ = has_math_fn(&input);
        let _ = is_valid_arbitrary(&input);
        let _ = to_key_path(&input);
        let _ = extract_used_variables(&input);
        let _ = dimensions(&input);
        let _ = replace_shadow_colors(&input, |color| format!("x({color})"));
        let _ = is_color(&input);
        let _ = infer_data_type(&input, &[DataType::Vector, DataType::Ratio, DataType::Image, DataType::BgSize, DataType::Position, DataType::LineWidth, DataType::FamilyName, DataType::Integer, DataType::Url]);
        let _ = is_valid_spacing_multiplier(&input);
        let _ = compare_breakpoints(&input, "1px", Direction::Asc);
        let _ = js_number(&input);
        let _ = js_parse_float(&input);
        let _ = js_parse_int(&input, 0);
    }

    #[test]
    fn functions_on_text_do_not_panic_for_all_strings(input in any::<String>()) {
        let _ = decode_arbitrary_value(&input);
        let _ = add_whitespace_around_math_operators(&input);
        let _ = is_valid_arbitrary(&input);
        let _ = to_key_path(&input);
        let _ = extract_used_variables(&input);
        let _ = dimensions(&input);
        let _ = replace_shadow_colors(&input, |color| format!("x({color})"));
        let _ = infer_data_type(&input, &[DataType::Color, DataType::Length, DataType::Vector, DataType::Ratio, DataType::Url]);
        let _ = compare(&input, "a1");
        let _ = js_number(&input);
        let _ = js_to_lower_case(&input);
    }

    #[test]
    fn math_calls_do_not_panic(input in "(calc\\(|min\\(|\\(|\\)|[0-9a-z%+*/ ,.é-]){0,24}") {
        let formatted = add_whitespace_around_math_operators(&input);
        // The formatter only adds and removes spaces.
        let strip = |text: &str| text.replace(' ', "");
        prop_assert_eq!(strip(&formatted), strip(&input));
    }

    #[test]
    fn expand_does_not_panic(input in "[a-c{},.0-2-]{0,12}") {
        let _ = expand(&input);
    }

    #[test]
    fn numbers_print_and_parse_to_the_same_number(bits in any::<u64>()) {
        let x = f64::from_bits(bits);
        let text = js_number_to_string(x);
        let back = js_number(&text);
        prop_assert!(back == x || (back.is_nan() && x.is_nan()));
        prop_assert_eq!(js_parse_float(&text).to_bits(), back.to_bits());
    }

    #[test]
    fn whole_numbers_print_as_integers(n in -9007199254740991i64..=9007199254740991i64) {
        prop_assert_eq!(js_number_to_string(n as f64), n.to_string());
        prop_assert_eq!(js_number(&n.to_string()), n as f64);
        prop_assert_eq!(js_parse_int(&n.to_string(), 10), n as f64);
        prop_assert_eq!(is_positive_integer(&n.to_string()), n >= 0);
    }
}
