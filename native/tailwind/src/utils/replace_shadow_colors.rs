//! Replace the color of a shadow: `utils/replace-shadow-colors.ts` of Tailwind CSS.

use super::is_color::is_named_color;
use super::js::{is_js_line_terminator, js_to_lower_case, js_trim};
use super::segment::segment;
use crate::value_parser::{self, ValueAstNode};

const KEYWORDS: [&str; 5] = ["inset", "inherit", "initial", "revert", "unset"];
const LENGTH_FUNCTIONS: [&str; 5] = ["calc", "clamp", "max", "min", "--spacing"];
const COLOR_FUNCTIONS: [&str; 15] = [
    "color",
    "color-mix",
    "contrast-color",
    "device-cmyk",
    "hsl",
    "hsla",
    "hwb",
    "lab",
    "lch",
    "light-dark",
    "oklab",
    "oklch",
    "rgb",
    "rgba",
    "--alpha",
];

/// `/^-?(\d+|\.\d+)(.*?)$/`: `.` does not match a line terminator.
fn is_length(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut i = 0;
    if bytes.first() == Some(&b'-') {
        i = 1;
    }
    if bytes.get(i) == Some(&b'.') {
        i += 1;
    }
    if !bytes.get(i).is_some_and(u8::is_ascii_digit) {
        return false;
    }
    !value[i..].chars().any(is_js_line_terminator)
}

/// Replace the color of each shadow in `input` with the result of
/// `replacement`. A shadow with no color gets `replacement("currentcolor")`.
pub fn replace_shadow_colors(input: &str, mut replacement: impl FnMut(&str) -> String) -> String {
    let mut result = String::with_capacity(input.len() + 32);

    for (index, shadow) in segment(input, b',').into_iter().enumerate() {
        if index > 0 {
            result.push_str(", ");
        }
        let shadow = js_trim(shadow);
        let mut ast = value_parser::parse(shadow);

        // The walk of upstream never goes into a function, so a loop on the
        // top-level nodes is the same.
        let mut unknown: Option<usize> = None;
        let mut unknowns = 0;
        let mut lengths = 0;
        let mut color: Option<usize> = None;

        for (i, node) in ast.iter().enumerate() {
            match node {
                ValueAstNode::Word(word) => {
                    let lower = js_to_lower_case(&word.value);
                    if KEYWORDS.contains(&&*lower) {
                        continue;
                    }
                    if is_length(&lower) {
                        lengths += 1;
                        continue;
                    }
                    if word.value.starts_with('#') || is_named_color(&word.value) {
                        color = Some(i);
                        break;
                    }
                    unknown = Some(i);
                    unknowns += 1;
                }
                ValueAstNode::Function(function) => {
                    let lower = js_to_lower_case(&function.value);
                    if COLOR_FUNCTIONS.contains(&&*lower) {
                        color = Some(i);
                        break;
                    }
                    if LENGTH_FUNCTIONS.contains(&&*lower) {
                        lengths += 1;
                        continue;
                    }
                    unknown = Some(i);
                    unknowns += 1;
                }
                ValueAstNode::Separator(_) => {}
            }
        }

        // With one node that is not known, that node is the color. This
        // needs the x offset and the y offset.
        if color.is_none() && lengths >= 2 && unknowns == 1 {
            color = unknown;
        }

        if let Some(i) = color {
            let updated = replacement(&value_parser::to_css(&ast[i..=i]));
            ast.splice(i..=i, value_parser::parse(&updated));
            value_parser::write_css(&ast, &mut result);
        } else if lengths >= 2 && unknowns == 0 {
            // The shadow has the default color of the browser.
            result.push_str(shadow);
            result.push(' ');
            result.push_str(&replacement("currentcolor"));
        } else {
            // The shadow is not valid, or a variable has more than one field.
            result.push_str(shadow);
        }
    }

    result
}
