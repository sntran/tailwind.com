//! The limits of nesting (`src/nesting.rs`): an input that nests too deep
//! gives the error of the official program, and does not overflow the
//! stack. An input that the official program compiles compiles here too.
//!
//! The tests run on a thread with the stack of a run
//! (`nesting::with_stack`), as the program and the NIF do.

use tailwind::nesting::{self, MAX_BLOCKS, MAX_FUNCTIONS};
use tailwind::utils::variables::extract_used_variables;
use tailwind::{compile, CompileOptions};

/// `depth` nested rules with one declaration.
fn rules(depth: usize) -> String {
    format!("{}color: red;{}", ".a {".repeat(depth), "}".repeat(depth))
}

/// `open` `depth` times, then `inner`, then `)` `depth` times.
fn nested(open: &str, inner: &str, depth: usize) -> String {
    format!("{}{inner}{}", open.repeat(depth), ")".repeat(depth))
}

/// Compile and build with no candidate. `@tailwind utilities` makes the
/// build run each step of the compiler.
fn build(css: &str) -> tailwind::Result<String> {
    let mut compiler = compile(&format!("@tailwind utilities;\n{css}"), CompileOptions::default())?;
    compiler.try_build(&[])
}

fn assert_too_deep(result: tailwind::Result<String>) {
    let error = result.expect_err("an error for the deep input");
    assert_eq!(error.name, "RangeError");
    assert_eq!(error.message, "Maximum call stack size exceeded");
}

#[test]
fn rules_at_the_limit_compile() {
    let css = nesting::with_stack(|| build(&rules(MAX_BLOCKS))).unwrap();
    assert_eq!(css.matches(".a").count(), MAX_BLOCKS);
    assert!(css.contains("color: red;"));
}

#[test]
fn deeper_rules_give_the_error_of_the_official_program() {
    assert_too_deep(nesting::with_stack(|| build(&rules(MAX_BLOCKS + 1))));
    // The parser stops at the limit: a much deeper input is no problem.
    assert_too_deep(nesting::with_stack(|| build(&rules(200_000))));
    assert_too_deep(nesting::with_stack(|| build(&"@media (width >= 1px) {".repeat(MAX_BLOCKS + 1))));
}

#[test]
fn the_error_is_not_a_syntax_error() {
    let error = tailwind::css_parser::parse(&rules(MAX_BLOCKS + 1)).unwrap_err();
    assert!(error.too_deep);
    assert!(error.loc.is_none());
    assert_eq!(tailwind::Error::from(error).name, "RangeError");
}

#[test]
fn a_deep_variable_compiles_as_in_the_official_program() {
    // The official program reads `var()` with no recursion.
    let value = nested("var(--x, ", "1px", 200_000);
    let css = nesting::with_stack(|| build(&format!(".a {{ width: {value}; }}"))).unwrap();
    assert!(css.contains(&value));
}

#[test]
fn the_variables_of_a_deep_value_need_no_deep_stack() {
    // On the thread of the test, with its small stack.
    let variables = extract_used_variables(&nested("var(--x, ", "1px", 200_000));
    assert_eq!(variables.len(), 200_000);
    assert!(variables.iter().all(|name| name == "--x"));
    assert_eq!(extract_used_variables("calc(var(--a) + max(var(--b, var(--c)), 1px))"), ["--a", "--b", "--c"]);
}

#[test]
fn a_value_at_the_limit_compiles() {
    let value = nested("calc(", "--spacing(1)", MAX_FUNCTIONS - 1);
    let css = nesting::with_stack(|| build(&format!("@theme {{ --spacing: 4px; }} .a {{ width: {value}; }}"))).unwrap();
    assert!(css.contains(&nested("calc(", "var(--spacing)", MAX_FUNCTIONS - 1)));
}

#[test]
fn a_deeper_value_gives_the_error_of_the_official_program() {
    let value = nested("calc(", "--spacing(1)", MAX_FUNCTIONS);
    assert_too_deep(nesting::with_stack(|| build(&format!("@theme {{ --spacing: 4px; }} .a {{ width: {value}; }}"))));
    let value = nested("calc(", "--spacing(1)", 200_000);
    assert_too_deep(nesting::with_stack(|| build(&format!("@theme {{ --spacing: 4px; }} .a {{ width: {value}; }}"))));
}

#[test]
fn the_parsers_keep_a_deeper_text_flat() {
    nesting::take_error();
    let ast = tailwind::value_parser::parse(&nested("a(", "b", MAX_FUNCTIONS));
    assert!(nesting::take_error().is_none());
    let mut depth = 0;
    let mut nodes = &ast;
    while let Some(tailwind::value_parser::ValueAstNode::Function(function)) = nodes.first() {
        depth += 1;
        nodes = &function.nodes;
    }
    assert_eq!(depth, MAX_FUNCTIONS);
    nesting::with_stack(move || drop(ast));

    let deep = nested("a(", "b", MAX_FUNCTIONS + 5);
    let ast = tailwind::value_parser::parse(&deep);
    assert!(nesting::take_error().is_some());
    // The text stays the same.
    assert_eq!(nesting::with_stack(move || tailwind::value_parser::to_css(&ast)), deep);

    let selector = nested(":is(", ".a", MAX_FUNCTIONS + 5);
    let ast = tailwind::selector_parser::parse(&selector);
    assert!(nesting::take_error().is_some());
    nesting::with_stack(move || drop(ast));
    assert!(nesting::take_error().is_none());
}

#[test]
fn a_new_compile_does_not_keep_the_mark_of_a_build() {
    nesting::with_stack(|| {
        let value = nested("calc(", "--spacing(1)", MAX_FUNCTIONS + 1);
        let mut compiler = compile("@theme { --spacing: 4px; } @tailwind utilities;", CompileOptions::default()).unwrap();
        // `build` has no error: it drops the mark.
        compiler.build(&[format!("w-[{value}]")]);
        assert!(nesting::take_error().is_none());
        assert!(build(".a { color: red; }").is_ok());
    });
}

#[cfg(feature = "cli")]
mod optimizer {
    use super::*;
    use tailwind::cli::optimize_css::optimize;

    fn optimized(css: &str) -> (String, String) {
        let optimized = nesting::with_stack(|| optimize(css, "input.css", false, None)).unwrap();
        (optimized.code, optimized.warnings.unwrap_or_default())
    }

    #[test]
    fn a_deep_declaration_gives_the_warning_of_lightning_css() {
        // Lightning CSS reads this value as a list of tokens.
        for value in [nested("var(--x, ", "1px", 200_000), nested("(", "1px", 200_000)] {
            let (code, warnings) = optimized(&format!(".a {{\n  width: {value};\n}}\n.b {{\n  color: red;\n}}\n"));
            assert_eq!(code, ".b {\n  color: red;\n}\n");
            assert!(warnings.starts_with("Found 1 warning while optimizing generated CSS:"));
            assert!(warnings.contains("Overflowed the maximum nesting depth"));
            // The lines of the warning are the lines of the input.
            assert!(warnings.contains("width: var(--x, ") || warnings.contains("width: (((("));
        }
    }

    #[test]
    fn a_deep_math_function_gives_a_warning_and_no_overflow() {
        // The official program overflows its stack here.
        let (code, warnings) = optimized(&format!(".a {{\n  width: {};\n}}\n", nested("calc(", "1px", 200_000)));
        assert_eq!(code.trim(), "");
        assert!(warnings.contains("width: calc(calc(") && warnings.contains("Overflowed the maximum nesting depth"));
        // At this depth, Lightning CSS reads the value as the official
        // program does.
        let (code, warnings) = optimized(&format!(".a {{\n  width: {};\n}}\n", nested("calc(", "1px", 4_000)));
        assert_eq!(code, ".a {\n  width: 1px;\n}\n");
        assert_eq!(warnings, "");
    }

    #[test]
    fn a_deep_custom_property_gives_the_warning_of_lightning_css() {
        let (code, warnings) = optimized(&format!(".a {{\n  --x: {};\n}}\n", nested("(", "1", 200_000)));
        assert_eq!(code.trim(), "");
        assert!(warnings.contains("Overflowed the maximum nesting depth"));
    }

    #[test]
    fn a_deep_selector_or_at_rule_is_an_error() {
        for css in [
            format!("{} {{\n  color: red;\n}}\n", nested(":is(", ".a", 20_000)),
            format!("@media {} {{\n  .a {{\n    color: red;\n  }}\n}}\n", nested("(", "width >= 1px", 20_000)),
            format!("@supports {} {{\n  .a {{\n    color: red;\n  }}\n}}\n", nested("(", "display: grid", 20_000)),
            format!("@import url({});\n", nested("(", "a", 20_000)),
        ] {
            let error = nesting::with_stack(|| optimize(&css, "input.css", true, None)).err().expect("an error");
            assert_eq!((error.name.as_ref(), error.message.as_str()), ("RangeError", "Maximum call stack size exceeded"));
        }
    }

    #[test]
    fn a_selector_below_the_limit_is_for_lightning_css() {
        let css = format!("{} {{\n  color: red;\n}}\n", nested(":is(", ".a", 9_990));
        let (code, _) = optimized(&css);
        assert!(code.ends_with("{\n  color: red;\n}\n"));
    }

    #[test]
    fn a_deep_if_is_not_lowered() {
        // Deeper than the limit of Lightning CSS for a list of tokens:
        // Lightning CSS gives its warning, as for the official program.
        let (code, warnings) = optimized(&format!(".a {{\n  color: {};\n}}\n", nested("if(media(width >= 1px): ", "red", 600)));
        assert_eq!(code.trim(), "");
        assert!(warnings.contains("color: if(media(") && warnings.contains("Overflowed the maximum nesting depth"));
        // A small one becomes a rule in `@media`.
        let (code, warnings) = optimized(&format!(".a {{\n  color: {};\n}}\n", nested("if(media(width >= 1px): ", "red", 3)));
        assert!(code.contains("@media"));
        assert_eq!(warnings, "");
    }
}
