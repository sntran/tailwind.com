//! Brace expansion: `utils/brace-expansion.ts` of Tailwind CSS.

use super::js::{js_number_to_string, js_parse_int};
use super::segment::segment;
use std::fmt;

/// An error of `expand`. `Display` gives the upstream message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExpandError {
    /// The braces of the pattern are not balanced.
    NotBalanced(String),
    /// The step of a sequence is zero.
    ZeroStep,
    /// The sequence has no end, because the step is too small for the
    /// numbers. Upstream has no memory left in this condition.
    EndlessSequence(String),
}

impl fmt::Display for ExpandError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExpandError::NotBalanced(pattern) => write!(f, "The pattern `{pattern}` is not balanced."),
            ExpandError::ZeroStep => f.write_str("Step cannot be zero in sequence expansion."),
            ExpandError::EndlessSequence(seq) => write!(f, "The sequence `{seq}` has no end."),
        }
    }
}

impl std::error::Error for ExpandError {}

/// Expand a pattern such as `a/{x,y}/{1..3}`.
pub fn expand(pattern: &str) -> Result<Vec<String>, ExpandError> {
    let Some(index) = pattern.find('{') else {
        return Ok(vec![pattern.to_string()]);
    };

    let pre = &pattern[..index];
    let rest = &pattern[index..];

    // Find the closing brace of the first brace.
    let mut depth = 0usize;
    let mut end_index = rest.rfind('}');
    for (i, b) in rest.bytes().enumerate() {
        if b == b'{' {
            depth += 1;
        } else if b == b'}' {
            // `rest` starts with `{`, so the depth is 1 or more here.
            depth -= 1;
            if depth == 0 {
                end_index = Some(i);
                break;
            }
        }
    }

    let Some(end_index) = end_index else {
        return Err(ExpandError::NotBalanced(pattern.to_string()));
    };

    let inside = &rest[1..end_index];
    let post = &rest[end_index + 1..];

    let mut parts: Vec<String> = Vec::new();
    match parse_sequence(inside) {
        Some(sequence) => {
            for part in expand_sequence(inside, sequence)? {
                parts.extend(expand(&part)?);
            }
        }
        None => {
            for part in segment(inside, b',') {
                parts.extend(expand(part)?);
            }
        }
    }

    let expanded_tail = expand(post)?;

    let mut result = Vec::with_capacity(parts.len() * expanded_tail.len());
    for tail in &expanded_tail {
        for part in &parts {
            let mut item = String::with_capacity(pre.len() + part.len() + tail.len());
            item.push_str(pre);
            item.push_str(part);
            item.push_str(tail);
            result.push(item);
        }
    }
    Ok(result)
}

/// The end of `-?\d+` at `start`.
fn integer_end(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    if bytes.get(i) == Some(&b'-') {
        i += 1;
    }
    let digits_start = i;
    while i < bytes.len() && bytes[i].is_ascii_digit() {
        i += 1;
    }
    (i > digits_start).then_some(i)
}

/// The parts of `/^(-?\d+)\.\.(-?\d+)(?:\.\.(-?\d+))?$/`.
fn parse_sequence(text: &str) -> Option<(&str, &str, Option<&str>)> {
    let bytes = text.as_bytes();
    let start_end = integer_end(bytes, 0)?;
    if !bytes[start_end..].starts_with(b"..") {
        return None;
    }
    let end_start = start_end + 2;
    let end_end = integer_end(bytes, end_start)?;
    let (start, end) = (&text[..start_end], &text[end_start..end_end]);
    if end_end == bytes.len() {
        return Some((start, end, None));
    }
    if !bytes[end_end..].starts_with(b"..") {
        return None;
    }
    let step_start = end_end + 2;
    let step_end = integer_end(bytes, step_start)?;
    (step_end == bytes.len()).then_some((start, end, Some(&text[step_start..])))
}

/// Expand a sequence such as `01..20`, with an optional step.
fn expand_sequence(seq: &str, parts: (&str, &str, Option<&str>)) -> Result<Vec<String>, ExpandError> {
    let (start, end, step) = parts;
    let start_num = js_parse_int(start, 10);
    let end_num = js_parse_int(end, 10);

    let mut step = match step {
        Some(text) => js_parse_int(text, 10),
        None if start_num <= end_num => 1.0,
        None => -1.0,
    };
    if step == 0.0 {
        return Err(ExpandError::ZeroStep);
    }

    let increasing = start_num < end_num;
    if increasing && step < 0.0 {
        step = -step;
    }
    if !increasing && step > 0.0 {
        step = -step;
    }

    let mut result = Vec::new();
    let mut i = start_num;
    while if increasing { i <= end_num } else { i >= end_num } {
        result.push(js_number_to_string(i));
        let next = i + step;
        if next == i {
            return Err(ExpandError::EndlessSequence(seq.to_string()));
        }
        i = next;
    }
    Ok(result)
}
