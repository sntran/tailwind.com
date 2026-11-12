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
    if !crate::switches::SHADOW_COLOR_DETECTION.on() {
        return replace_first_other_part(input, replacement);
    }

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

/// The function before 4.3.1 (`switches::SHADOW_COLOR_DETECTION`): the
/// color of a shadow is its first part that is not a keyword or a length.
fn replace_first_other_part(input: &str, mut replacement: impl FnMut(&str) -> String) -> String {
    let mut result = String::with_capacity(input.len() + 32);

    for (index, shadow) in segment(input, b',').into_iter().enumerate() {
        if index > 0 {
            result.push_str(", ");
        }
        let shadow = js_trim(shadow);
        let mut color: Option<&str> = None;
        let mut lengths = 0;
        for part in segment(shadow, b' ') {
            if js_trim(part).is_empty() || KEYWORDS.contains(&part) {
                continue;
            }
            if is_length(part) {
                lengths += 1;
            } else if color.is_none() {
                color = Some(part);
            }
        }

        // With no x offset or no y offset, the shadow is not valid, or a
        // variable has more than one field.
        if lengths < 2 {
            result.push_str(shadow);
            continue;
        }

        let replacement_color = replacement(color.unwrap_or("currentcolor"));
        match color {
            Some(color) => js_replace_first(shadow, color, &replacement_color, &mut result),
            None => {
                result.push_str(shadow);
                result.push(' ');
                result.push_str(&replacement_color);
            }
        }
    }

    result
}

/// `text.replace(search, replacement)` of JavaScript with a text to search
/// for: only the first place, and `$$`, `$&`, `` $` `` and `$'` in the
/// replacement have a special meaning.
fn js_replace_first(text: &str, search: &str, replacement: &str, result: &mut String) {
    let Some(at) = text.find(search) else {
        result.push_str(text);
        return;
    };
    let (before, after) = (&text[..at], &text[at + search.len()..]);
    result.push_str(before);
    let mut rest = replacement;
    while let Some(dollar) = rest.find('$') {
        result.push_str(&rest[..dollar]);
        let special = match rest[dollar + 1..].chars().next() {
            Some('$') => Some("$"),
            Some('&') => Some(search),
            Some('`') => Some(before),
            Some('\'') => Some(after),
            _ => None,
        };
        match special {
            Some(text) => {
                result.push_str(text);
                rest = &rest[dollar + 2..];
            }
            None => {
                result.push('$');
                rest = &rest[dollar + 1..];
            }
        }
    }
    result.push_str(rest);
    result.push_str(after);
}
