//! Spaces around math operators: `utils/math-operators.ts` of Tailwind CSS.

use super::js::js_trim_end;
use std::borrow::Cow;

const MATH_FUNCTIONS: [&str; 19] = [
    "calc", "min", "max", "clamp", "mod", "rem", "sin", "cos", "tan", "asin", "acos", "atan", "atan2", "pow",
    "sqrt", "hypot", "log", "exp", "round",
];

/// `true` when the input has a call of a math function such as `calc(`.
pub fn has_math_fn(input: &str) -> bool {
    let bytes = input.as_bytes();
    let mut from = 0;
    while let Some(offset) = bytes[from..].iter().position(|&b| b == b'(') {
        let paren = from + offset;
        let before = &bytes[..paren];
        if MATH_FUNCTIONS.iter().any(|name| before.ends_with(name.as_bytes())) {
            return true;
        }
        from = paren + 1;
    }
    false
}

/// The last byte of `text`, when there is one.
#[inline]
fn byte_from_end(text: &str, offset: usize) -> Option<u8> {
    let bytes = text.as_bytes();
    bytes.len().checked_sub(offset).map(|i| bytes[i])
}

#[inline]
fn is_digit(b: Option<u8>) -> bool {
    matches!(b, Some(b'0'..=b'9'))
}

#[inline]
fn is_operator(b: Option<u8>) -> bool {
    matches!(b, Some(b'+' | b'*' | b'/' | b'-'))
}

/// As before 4.1.9: spaces around each operator in a math function, also
/// in a dashed name (`fit-content`). `to-zero` and `anchor-size(` are the
/// two exceptions.
fn add_whitespace_around_each_operator(input: &str) -> String {
    // `anchor-size(` gets a placeholder with no dash.
    let has_known_functions = input.contains("anchor-size(");
    let replaced;
    let input = if has_known_functions {
        replaced = input.replace("anchor-size(", "$0$(");
        replaced.as_str()
    } else {
        input
    };

    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut result = String::with_capacity(len + 16);
    // The top of the stack is the function at the cursor.
    let mut formattable: Vec<bool> = Vec::new();

    let mut i = 0;
    while i < len {
        let c = bytes[i];

        // A character that is not ASCII goes to the result as it is.
        if c >= 0x80 {
            let width = input[i..].chars().next().map_or(1, char::len_utf8);
            result.push_str(&input[i..i + width]);
            i += width;
            continue;
        }

        let in_math = formattable.last().copied().unwrap_or(false);

        if c == b'(' {
            result.push('(');

            // Find the function name: lower case letters and digits.
            let mut start = i;
            while start > 0 && (bytes[start - 1].is_ascii_digit() || bytes[start - 1].is_ascii_lowercase()) {
                start -= 1;
            }
            let name = &input[start..i];

            // A math function, or parentheses in a math function
            formattable.push(MATH_FUNCTIONS.contains(&name) || (in_math && name.is_empty()));
        } else if c == b')' {
            result.push(')');
            formattable.pop();
        } else if c == b',' && in_math {
            result.push_str(", ");
        } else if c == b' ' && in_math && result.as_bytes().last() == Some(&b' ') {
            // Only one space
        } else if is_operator(Some(c)) && in_math {
            let prev = byte_from_end(js_trim_end(&result), 1);
            let operator = c as char;

            if is_operator(prev) || matches!(prev, Some(b'(' | b',')) {
                // After an operator, or at the start of an argument
                result.push(operator);
            } else if i > 0 && bytes[i - 1] == b' ' {
                // There is a space before the operator already.
                result.push(operator);
                result.push(' ');
            } else {
                result.push(' ');
                result.push(operator);
                result.push(' ');
            }
        } else if in_math && input[i..].starts_with("to-zero") {
            // `to-zero` of `round(…)` stays, and the character after it
            // goes to the result as it is.
            let next = input[i + 7..].chars().next().map_or(0, char::len_utf8);
            result.push_str(&input[i..i + 7 + next]);
            i += 7 + next;
            continue;
        } else {
            result.push(c as char);
        }
        i += 1;
    }

    if !has_known_functions {
        return result;
    }

    // JavaScript `result.replace(/\$(\d+)\$/g, …)`: only the index 0 is a
    // known function.
    let mut out = String::with_capacity(result.len() + 16);
    let mut rest = result.as_str();
    while let Some(start) = rest.find('$') {
        let digits = rest[start + 1..].bytes().take_while(u8::is_ascii_digit).count();
        let end = start + 1 + digits;
        if digits > 0 && rest[end..].starts_with('$') {
            out.push_str(&rest[..start]);
            out.push_str(if &rest[start + 1..end] == "0" { "anchor-size" } else { &rest[start..=end] });
            rest = &rest[end + 1..];
        } else {
            out.push_str(&rest[..=start]);
            rest = &rest[start + 1..];
        }
    }
    out.push_str(rest);
    out
}

/// Add spaces around the operators in math functions:
/// `calc(1px+2px)` becomes `calc(1px + 2px)`.
pub fn add_whitespace_around_math_operators(input: &str) -> Cow<'_, str> {
    // With no math function in the input, there is no work.
    if !MATH_FUNCTIONS.iter().any(|name| input.contains(name)) {
        return Cow::Borrowed(input);
    }

    if !crate::switches::MATH_OPERATORS_NEED_VALUES.on() {
        return Cow::Owned(add_whitespace_around_each_operator(input));
    }

    // Before 4.1.10, only a lowercase letter was a part of a unit.
    let more_units = crate::switches::MATH_PERCENT_AND_UPPERCASE_UNITS.on();

    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut result = String::with_capacity(len + 16);
    // The top of the stack is the function at the cursor.
    let mut formattable: Vec<bool> = Vec::new();

    let mut value_pos: Option<usize> = None;
    let mut last_value_pos: Option<usize> = None;

    let mut i = 0;
    while i < len {
        let c = bytes[i];

        // A character that is not ASCII goes to the result as it is.
        if c >= 0x80 {
            last_value_pos = value_pos;
            value_pos = None;
            let width = input[i..].chars().next().map_or(1, char::len_utf8);
            result.push_str(&input[i..i + width]);
            i += width;
            continue;
        }

        // A number with a unit, such as `123px`, is not a function name.
        let is_unit = if more_units { c == b'%' || c.is_ascii_alphabetic() } else { c.is_ascii_lowercase() };
        if c.is_ascii_digit() || (value_pos.is_some() && is_unit) {
            value_pos = Some(i);
        } else {
            last_value_pos = value_pos;
            value_pos = None;
        }

        let in_math = formattable.last().copied().unwrap_or(false);

        if c == b'(' {
            result.push('(');

            // Find the function name: lower case letters and digits.
            let mut start = i;
            while start > 0 && (bytes[start - 1].is_ascii_digit() || bytes[start - 1].is_ascii_lowercase()) {
                start -= 1;
            }
            let name = &input[start..i];

            // A math function, or parentheses in a math function
            let format = MATH_FUNCTIONS.contains(&name) || (in_math && name.is_empty());
            formattable.push(format);
        } else if c == b')' {
            result.push(')');
            formattable.pop();
        } else if c == b',' && in_math {
            result.push_str(", ");
        } else if c == b' ' && in_math && result.as_bytes().last() == Some(&b' ') {
            // Only one space
        } else if is_operator(Some(c)) && in_math {
            let trimmed = js_trim_end(&result);
            let prev = byte_from_end(trimmed, 1);
            let prev_prev = byte_from_end(trimmed, 2);
            let next = bytes.get(i + 1).copied();
            let operator = c as char;

            if matches!(prev, Some(b'e' | b'E')) && is_digit(prev_prev) {
                // Scientific notation: `-3.4e-2`
                result.push(operator);
            } else if is_operator(prev) || matches!(prev, Some(b'(' | b',')) {
                // After an operator, or at the start of an argument
                result.push(operator);
            } else if i > 0 && bytes[i - 1] == b' ' {
                // There is a space before the operator already.
                result.push(operator);
                result.push(' ');
            } else if is_digit(prev)
                || is_digit(next)
                || prev == Some(b')')
                || next == Some(b'(')
                || is_operator(next)
                || (last_value_pos.is_some() && last_value_pos == i.checked_sub(1))
            {
                result.push(' ');
                result.push(operator);
                result.push(' ');
            } else {
                result.push(operator);
            }
        } else {
            result.push(c as char);
        }
        i += 1;
    }

    Cow::Owned(result)
}
