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

/// Add spaces around the operators in math functions:
/// `calc(1px+2px)` becomes `calc(1px + 2px)`.
pub fn add_whitespace_around_math_operators(input: &str) -> Cow<'_, str> {
    // With no math function in the input, there is no work.
    if !MATH_FUNCTIONS.iter().any(|name| input.contains(name)) {
        return Cow::Borrowed(input);
    }

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
        if c.is_ascii_digit() || (value_pos.is_some() && (c == b'%' || c.is_ascii_alphabetic())) {
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
