//! Tests of the printer of the CSS syntax tree.
//!
//! The first cases come from `ast.test.ts` of Tailwind CSS: the cases that
//! use only the parser and the printer. The other cases compare the output
//! with the output of the upstream `toCss` for the same tree.

use tailwind::ast::{
    at_root, at_rule, comment, context, decl, decl_full, style_rule, AstNode, ContextMap,
};
use tailwind::css_parser::parse;
use tailwind::to_css::to_css;

fn print(input: &str) -> String {
    to_css(&parse(input).expect("the input is correct CSS"))
}

#[test]
fn pretty_print_an_ast() {
    assert_eq!(
        print(".foo{color:red;&:hover{color:blue;}}"),
        ".foo {\n  color: red;\n  &:hover {\n    color: blue;\n  }\n}\n"
    );
}

#[test]
fn print_empty_rules_before_the_optimization() {
    let input = r#"
    /* Empty rule */
    .foo {
    }

    /* Empty rule, with nesting */
    .foo {
      .bar {
      }
      .baz {
      }
    }

    /* Empty rule, with special case '&' rules */
    .foo {
      & {
        &:hover {
        }
        &:focus {
        }
      }
    }

    /* Empty at-rule */
    @media (min-width: 768px) {
    }

    /* Empty at-rule with nesting*/
    @media (min-width: 768px) {
      .foo {
      }

      @media (min-width: 1024px) {
        .bar {
        }
      }
    }

    /* Exceptions: */
    @charset "UTF-8";
    @layer foo, bar, baz;
    @layer foo, bar, baz; /* Will be deduped */
    @custom-media --modern (color), (hover);
    @namespace 'http://www.w3.org/1999/xhtml';
    @import url('https://fonts.googleapis.com/css2?family=Cedarville+Cursive&display=swap');
  "#;
    let expected = r#".foo {
}
.foo {
  .bar {
  }
  .baz {
  }
}
.foo {
  & {
    &:hover {
    }
    &:focus {
    }
  }
}
@media (min-width: 768px);
@media (min-width: 768px) {
  .foo {
  }
  @media (min-width: 1024px) {
    .bar {
    }
  }
}
@charset "UTF-8";
@layer foo, bar, baz;
@layer foo, bar, baz;
@custom-media --modern (color), (hover);
@namespace 'http://www.w3.org/1999/xhtml';
@import url('https://fonts.googleapis.com/css2?family=Cedarville+Cursive&display=swap');
"#;
    assert_eq!(print(input), expected);
}

#[test]
fn print_duplicate_declarations_before_the_optimization() {
    let input = "
    .foo {
      color: red;
      .bar {
        color: green;
        color: blue;
        color: green;
      }
      color: red;
    }
    .foo {
      color: red;
      & {
        color: green;
        & {
          color: red;
          color: green;
          color: blue;
        }
        color: red;
      }
      background: blue;
      .bar {
        color: green;
        color: blue;
        color: green;
      }
      caret-color: orange;
    }
  ";
    let expected = "\
.foo {
  color: red;
  .bar {
    color: green;
    color: blue;
    color: green;
  }
  color: red;
}
.foo {
  color: red;
  & {
    color: green;
    & {
      color: red;
      color: green;
      color: blue;
    }
    color: red;
  }
  background: blue;
  .bar {
    color: green;
    color: blue;
    color: green;
  }
  caret-color: orange;
}
";
    assert_eq!(print(input), expected);
}

#[test]
fn print_nothing_for_an_empty_tree() {
    assert_eq!(to_css(&[]), "");
}

#[test]
fn print_an_at_rule_with_no_nodes_as_a_statement() {
    let ast = [
        at_rule("@layer", "base, components", vec![]),
        at_rule("@tailwind", "", vec![]),
    ];
    assert_eq!(to_css(&ast), "@layer base, components;\n@tailwind ;\n");
}

#[test]
fn print_an_at_rule_with_nodes_as_a_block() {
    let ast = [
        at_rule("@media", "", vec![decl("color", "red")]),
        at_rule("@media", "print", vec![decl("color", "red")]),
    ];
    assert_eq!(
        to_css(&ast),
        "@media {\n  color: red;\n}\n@media print {\n  color: red;\n}\n"
    );
}

#[test]
fn print_each_type_of_node_with_the_indent_of_its_depth() {
    let ast = [at_rule(
        "@media",
        "print",
        vec![style_rule(
            ".a",
            vec![
                decl_full("color", Some("red".to_string()), true),
                comment(" note "),
                at_rule("@apply", "flex", vec![]),
            ],
        )],
    )];
    assert_eq!(
        to_css(&ast),
        "@media print {\n  .a {\n    color: red !important;\n    /* note */\n    @apply flex;\n  }\n}\n"
    );
}

#[test]
fn print_a_comment_with_its_text_unchanged() {
    assert_eq!(
        to_css(&[comment("! license "), comment("")]),
        "/*! license */\n/**/\n"
    );
}

#[test]
fn print_a_declaration_with_an_empty_value() {
    assert_eq!(to_css(&[decl("--x", "")]), "--x: ;\n");
}

#[test]
fn print_undefined_for_a_declaration_with_no_value() {
    // JavaScript prints `undefined` in a template string.
    assert_eq!(to_css(&[decl_full("x", None, false)]), "x: undefined;\n");
    assert_eq!(
        to_css(&[decl_full("x", None, true)]),
        "x: undefined !important;\n"
    );
}

#[test]
fn print_nothing_for_context_nodes_and_at_root_nodes() {
    let hidden =
        |nodes: Vec<AstNode>| vec![context(ContextMap::new(), nodes.clone()), at_root(nodes)];
    assert_eq!(to_css(&hidden(vec![decl("a", "b")])), "");
    // An at-rule with only such nodes has nodes, so it prints a block.
    let ast = [
        style_rule(".b", hidden(vec![decl("a", "b")])),
        at_rule("@supports", "(a: b)", hidden(vec![decl("a", "b")])),
    ];
    assert_eq!(to_css(&ast), ".b {\n}\n@supports (a: b) {\n}\n");
}
