//! The printer of the CSS syntax tree: `toCss` of `ast.ts` of Tailwind CSS.

use crate::ast::AstNode;
use crate::source_maps::{utf16_len, Source, SourceLocation};
use std::collections::HashMap;
use std::rc::Rc;

/// Print a syntax tree as CSS. Each level of depth adds two spaces.
pub fn to_css(ast: &[AstNode]) -> String {
    let mut css = String::new();
    for node in ast {
        stringify(node, 0, &mut css);
    }
    css
}

/// Print a syntax tree as CSS, and set the `dst` of each printed node:
/// `toCss(ast, true)`. The offsets are numbers of UTF-16 code units in the
/// result. The source of each `dst` is the result, with no file.
pub fn to_css_tracked(ast: &mut [AstNode]) -> String {
    let css = to_css(ast);
    let source = Rc::new(Source { file: None, code: css.clone() });
    let mut pos = 0;
    for node in ast.iter_mut() {
        track(node, 0, &mut pos, &source);
    }

    // Copies of one object of upstream have the `dst` of the last copy.
    let mut shared: HashMap<*const (), Option<SourceLocation>> = HashMap::new();
    collect_shared(ast, &mut shared);
    if !shared.is_empty() {
        apply_shared(ast, &shared);
    }
    css
}

fn collect_shared(nodes: &[AstNode], shared: &mut HashMap<*const (), Option<SourceLocation>>) {
    for node in nodes {
        if let Some(identity) = node.loc().and_then(|loc| loc.identity()) {
            shared.insert(identity, node.dst().cloned());
        }
        if let Some(children) = node.children() {
            collect_shared(children, shared);
        }
    }
}

fn apply_shared(nodes: &mut [AstNode], shared: &HashMap<*const (), Option<SourceLocation>>) {
    for node in nodes {
        if let Some(loc) = node.loc_mut() {
            if let Some(dst) = loc.identity().and_then(|identity| shared.get(&identity)) {
                loc.set_dst(dst.clone());
            }
        }
        if let Some(children) = node.children_mut() {
            apply_shared(children, shared);
        }
    }
}

/// Set the `dst` of a node and of its children. `pos` is the offset of
/// the node in the output.
fn track(node: &mut AstNode, depth: usize, pos: &mut usize, source: &Rc<Source>) {
    let at = |start: usize, end: usize| Some(SourceLocation { source: Rc::clone(source), start, end });
    let indent = depth * 2;
    match node {
        AstNode::Declaration(node) => {
            *pos += indent;
            let start = *pos;
            // `: ` and the value. Upstream adds no length for a value that
            // is not there, but prints `undefined`.
            *pos += utf16_len(&node.property) + 2 + node.value.as_deref().map_or(0, utf16_len);
            if node.important {
                *pos += 11;
            }
            let end = *pos;
            // `;\n`
            *pos += 2;
            node.loc.set_dst(at(start, end));
        }
        AstNode::Rule(node) => {
            *pos += indent;
            let start = *pos;
            // The selector and ` `.
            *pos += utf16_len(&node.selector) + 1;
            node.loc.set_dst(at(start, *pos));
            // `{\n`
            *pos += 2;
            for child in &mut node.nodes {
                track(child, depth + 1, pos, source);
            }
            // `}\n`
            *pos += indent + 2;
        }
        AstNode::AtRule(node) => {
            *pos += indent;
            let start = *pos;
            if node.nodes.is_empty() {
                // The name, ` `, and the params.
                *pos += utf16_len(&node.name) + 1 + utf16_len(&node.params);
                let end = *pos;
                // `;\n`
                *pos += 2;
                node.loc.set_dst(at(start, end));
                return;
            }
            *pos += utf16_len(&node.name);
            if !node.params.is_empty() {
                *pos += 1 + utf16_len(&node.params);
            }
            *pos += 1;
            node.loc.set_dst(at(start, *pos));
            // `{\n`
            *pos += 2;
            for child in &mut node.nodes {
                track(child, depth + 1, pos, source);
            }
            // `}\n`
            *pos += indent + 2;
        }
        AstNode::Comment(node) => {
            *pos += indent;
            let start = *pos;
            *pos += 2 + utf16_len(&node.value) + 2;
            node.loc.set_dst(at(start, *pos));
            // `\n`
            *pos += 1;
        }
        AstNode::Context(_) | AstNode::AtRoot(_) => {}
    }
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
