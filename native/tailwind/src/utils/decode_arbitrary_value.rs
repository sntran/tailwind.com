//! Decode an arbitrary value: `utils/decode-arbitrary-value.ts` of Tailwind CSS.

use super::math_operators::add_whitespace_around_math_operators;
use crate::value_parser::{self, ValueAstNode};
use std::borrow::Cow;

/// Decode the value of a candidate such as `[calc(1px+2px)]`: `_` becomes a
/// space, `\_` becomes `_`, and math operators get spaces.
pub fn decode_arbitrary_value(input: &str) -> Cow<'_, str> {
    // With no function in the input, the underscores are the only work.
    if !input.contains('(') {
        return convert_underscores_to_whitespace(input, false);
    }

    let mut ast = value_parser::parse(input);
    recursively_decode_arbitrary_values(&mut ast);
    let css = value_parser::to_css(&ast);

    match add_whitespace_around_math_operators(&css) {
        Cow::Borrowed(_) => Cow::Owned(css),
        Cow::Owned(formatted) => Cow::Owned(formatted),
    }
}

/// Convert `_` to a space, and `\_` to `_`. With `skip_underscore_to_space`,
/// only `\_` changes.
fn convert_underscores_to_whitespace(input: &str, skip_underscore_to_space: bool) -> Cow<'_, str> {
    let bytes = input.as_bytes();
    let needs_change = if skip_underscore_to_space { input.contains("\\_") } else { bytes.contains(&b'_') };
    if !needs_change {
        return Cow::Borrowed(input);
    }

    let mut output = String::with_capacity(bytes.len());
    let mut pending = 0;
    let mut i = 0;
    while i < bytes.len() {
        let c = bytes[i];
        if c == b'\\' && bytes.get(i + 1) == Some(&b'_') {
            output.push_str(&input[pending..i]);
            output.push('_');
            i += 2;
            pending = i;
        } else if c == b'_' && !skip_underscore_to_space {
            output.push_str(&input[pending..i]);
            output.push(' ');
            i += 1;
            pending = i;
        } else {
            i += 1;
        }
    }
    output.push_str(&input[pending..]);
    Cow::Owned(output)
}

/// Do the conversion on a `String`.
fn convert_in_place(value: &mut String, skip_underscore_to_space: bool) {
    if let Cow::Owned(converted) = convert_underscores_to_whitespace(value, skip_underscore_to_space) {
        *value = converted;
    }
}

fn recursively_decode_arbitrary_values(ast: &mut [ValueAstNode]) {
    for node in ast {
        decode_node(node);
    }
}

fn decode_node(node: &mut ValueAstNode) {
    match node {
        ValueAstNode::Function(node) => {
            // `url()` keeps its underscores. The name of the function does not.
            if node.value == "url" || node.value.ends_with("_url") {
                convert_in_place(&mut node.value, false);
                return;
            }

            if node.value == "var"
                || node.value.ends_with("_var")
                || node.value == "theme"
                || node.value.ends_with("_theme")
            {
                convert_in_place(&mut node.value, false);
                for (i, child) in node.nodes.iter_mut().enumerate() {
                    // The first argument of `var()` keeps its underscores.
                    // (Before 4.0.4, it also kept `\_`.)
                    if i == 0 {
                        if let ValueAstNode::Word(word) = child {
                            if crate::switches::VAR_NAME_UNESCAPES_UNDERSCORE.on() {
                                convert_in_place(&mut word.value, true);
                            }
                            continue;
                        }
                    }
                    decode_node(child);
                }
                return;
            }

            convert_in_place(&mut node.value, false);
            recursively_decode_arbitrary_values(&mut node.nodes);
        }
        ValueAstNode::Separator(node) => convert_in_place(&mut node.value, false),
        ValueAstNode::Word(node) => convert_in_place(&mut node.value, false),
    }
}
