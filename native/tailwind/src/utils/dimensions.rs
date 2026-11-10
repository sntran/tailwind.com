//! Parse a dimension: `utils/dimensions.ts` of Tailwind CSS.

use super::js::js_number;

/// Parse a dimension such as `64rem` into `(64.0, Some("rem"))`.
///
/// `/^(?<value>[-+]?(?:\d*\.)?\d+)(?<unit>[a-z]+|%)?$/i`
///
/// Upstream keeps the results in a map. This function is fast with no map.
pub fn dimensions(input: &str) -> Option<(f64, Option<&str>)> {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut i = 0;
    if i < len && (bytes[i] == b'-' || bytes[i] == b'+') {
        i += 1;
    }
    let int_start = i;
    while i < len && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let has_int = i > int_start;
    if i < len && bytes[i] == b'.' {
        // A `.` needs a digit after it.
        i += 1;
        let frac_start = i;
        while i < len && bytes[i].is_ascii_digit() {
            i += 1;
        }
        if i == frac_start {
            return None;
        }
    } else if !has_int {
        return None;
    }

    let value = js_number(&input[..i]);
    if value.is_nan() {
        return None;
    }

    let unit = &input[i..];
    if unit.is_empty() {
        return Some((value, None));
    }
    if unit == "%" || unit.bytes().all(|b| b.is_ascii_alphabetic()) {
        return Some((value, Some(unit)));
    }
    None
}
