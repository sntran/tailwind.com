//! The printer of the CSS syntax tree: `toCss` of `ast.ts` of Tailwind CSS.

use crate::ast::AstNode;

/// Print a syntax tree as CSS. Each level of depth adds two spaces.
pub fn to_css(ast: &[AstNode]) -> String {
    let mut css = String::new();
    for node in ast {
        stringify(node, 0, &mut css);
    }
    css
}

fn indent(depth: usize, css: &mut String) {
    for _ in 0..depth {
        css.push_str("  ");
    }
}

fn stringify(node: &AstNode, depth: usize, css: &mut String) {
    match node {
        AstNode::Declaration(node) => {
            indent(depth, css);
            css.push_str(&node.property);
            css.push_str(": ");
            // JavaScript prints a value that is not there as `undefined`.
            css.push_str(node.value.as_deref().unwrap_or("undefined"));
            if node.important {
                css.push_str(" !important");
            }
            css.push_str(";\n");
        }
        AstNode::Rule(node) => {
            indent(depth, css);
            css.push_str(&node.selector);
            css.push_str(" {\n");
            for child in &node.nodes {
                stringify(child, depth + 1, css);
            }
            indent(depth, css);
            css.push_str("}\n");
        }
        AstNode::AtRule(node) => {
            indent(depth, css);
            css.push_str(&node.name);
            // An at-rule with no nodes ends with `;` and has no block.
            if node.nodes.is_empty() {
                css.push(' ');
                css.push_str(&node.params);
                css.push_str(";\n");
                return;
            }
            if !node.params.is_empty() {
                css.push(' ');
                css.push_str(&node.params);
            }
            css.push_str(" {\n");
            for child in &node.nodes {
                stringify(child, depth + 1, css);
            }
            indent(depth, css);
            css.push_str("}\n");
        }
        AstNode::Comment(node) => {
            indent(depth, css);
            css.push_str("/*");
            css.push_str(&node.value);
            css.push_str("*/\n");
        }
        // These nodes print nothing, and their children print nothing.
        AstNode::Context(_) | AstNode::AtRoot(_) => {}
    }
}
