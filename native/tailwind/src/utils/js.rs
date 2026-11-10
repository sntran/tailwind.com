//! JavaScript behavior of numbers and strings.
//!
//! Tailwind CSS has no file for this. The upstream code uses `Number(x)`,
//! `parseFloat`, `parseInt`, `String(number)`, `trim` and `toLowerCase`.
//! These functions give the same results as JavaScript (V8).

use std::borrow::Cow;
use std::cmp::Ordering;

/// `true` for the characters that JavaScript `trim` and `\s` see as white
/// space: `WhiteSpace` and `LineTerminator` of ECMAScript.
#[inline]
pub fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{9}'..='\u{D}'
            | ' '
            | '\u{A0}'
            | '\u{1680}'
            | '\u{2000}'..='\u{200A}'
            | '\u{2028}'
            | '\u{2029}'
            | '\u{202F}'
            | '\u{205F}'
            | '\u{3000}'
            | '\u{FEFF}'
    )
}

/// `true` for the characters that `.` of a JavaScript regular expression
/// does not match (without the `s` flag).
#[inline]
pub fn is_js_line_terminator(c: char) -> bool {
    matches!(c, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// `String.prototype.trim`.
#[inline]
pub fn js_trim(s: &str) -> &str {
    s.trim_matches(is_js_whitespace)
}

/// `String.prototype.trimStart`.
#[inline]
pub fn js_trim_start(s: &str) -> &str {
    s.trim_start_matches(is_js_whitespace)
}

/// `String.prototype.trimEnd`.
#[inline]
pub fn js_trim_end(s: &str) -> &str {
    s.trim_end_matches(is_js_whitespace)
}

/// `String.prototype.toLowerCase`. ASCII text has no allocation when it has
/// no upper case letter.
pub fn js_to_lower_case(s: &str) -> Cow<'_, str> {
    if s.is_ascii() {
        if s.bytes().any(|b| b.is_ascii_uppercase()) {
            Cow::Owned(s.to_ascii_lowercase())
        } else {
            Cow::Borrowed(s)
        }
    } else {
        Cow::Owned(s.to_lowercase())
    }
}

/// `String.prototype.length`: the number of UTF-16 code units.
pub fn js_string_length(s: &str) -> usize {
    if s.is_ascii() {
        s.len()
    } else {
        s.chars().map(char::len_utf16).sum()
    }
}

/// A key that sorts characters as their UTF-16 code units sort.
#[inline]
fn utf16_key(c: char) -> u32 {
    let c = c as u32;
    if c < 0x10000 {
        c << 16
    } else {
        let v = c - 0x10000;
        ((0xD800 + (v >> 10)) << 16) | (0xDC00 + (v & 0x3FF))
    }
}

/// The order of two sequences of characters by UTF-16 code units.
pub fn js_compare_chars(
    mut a: impl Iterator<Item = char>,
    mut z: impl Iterator<Item = char>,
) -> Ordering {
    loop {
        match (a.next(), z.next()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) => {
                if x != y {
                    return utf16_key(x).cmp(&utf16_key(y));
                }
            }
        }
    }
}

/// The order of two strings as the JavaScript operators `<` and `>` see
/// them: by UTF-16 code units.
pub fn js_compare_strings(a: &str, z: &str) -> Ordering {
    let (ab, zb) = (a.as_bytes(), z.as_bytes());
    let min = ab.len().min(zb.len());
    let mut i = 0;
    while i < min && ab[i] == zb[i] {
        i += 1;
    }
    if i == min {
        return ab.len().cmp(&zb.len());
    }
    if ab[i] < 0x80 || zb[i] < 0x80 {
        return ab[i].cmp(&zb[i]);
    }
    // The two different characters are not ASCII. Find their start.
    while !a.is_char_boundary(i) {
        i -= 1;
    }
    js_compare_chars(a[i..].chars(), z[i..].chars())
}

/// `Number.isInteger`.
#[inline]
pub fn js_is_integer(x: f64) -> bool {
    x.is_finite() && x.trunc() == x
}

/// The end of an unsigned decimal literal at the start of `b`:
/// digits, an optional `.` with digits, and an optional exponent.
/// The result is `None` when there is no digit in the mantissa.
fn decimal_literal_end(b: &[u8]) -> Option<usize> {
    let mut i = 0;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let int_digits = i;
    let mut frac_digits = 0;
    if i < b.len() && b[i] == b'.' {
        let mut j = i + 1;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        frac_digits = j - i - 1;
        if int_digits > 0 || frac_digits > 0 {
            i = j;
        }
    }
    if int_digits == 0 && frac_digits == 0 {
        return None;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        let mut j = i + 1;
        if j < b.len() && (b[j] == b'+' || b[j] == b'-') {
            j += 1;
        }
        let digits_start = j;
        while j < b.len() && b[j].is_ascii_digit() {
            j += 1;
        }
        if j > digits_start {
            i = j;
        }
    }
    Some(i)
}

/// The value of a valid unsigned decimal literal.
#[inline]
fn decimal_value(s: &str) -> f64 {
    // The Rust parser rounds correctly, as V8 does.
    s.parse::<f64>().unwrap_or(f64::NAN)
}

/// The value of a digit in a radix up to 36.
#[inline]
fn digit_value(b: u8, radix: u32) -> Option<u32> {
    let d = match b {
        b'0'..=b'9' => (b - b'0') as u32,
        b'a'..=b'z' => (b - b'a') as u32 + 10,
        b'A'..=b'Z' => (b - b'A') as u32 + 10,
        _ => return None,
    };
    (d < radix).then_some(d)
}

/// The value of digits in a radix that is a power of two, with correct
/// rounding. All bytes of `digits` must be digits of the radix.
fn power_of_two_radix_value(digits: &[u8], radix: u32) -> f64 {
    let bits = radix.trailing_zeros();
    let mut i = 0;
    while i < digits.len() && digits[i] == b'0' {
        i += 1;
    }
    let mut value: u128 = 0;
    let mut dropped: u32 = 0;
    let mut sticky = false;
    for &b in &digits[i..] {
        let d = digit_value(b, radix).unwrap_or(0) as u128;
        if value >> (128 - 8) == 0 {
            value = (value << bits) | d;
        } else {
            dropped = dropped.saturating_add(bits);
            sticky |= d != 0;
        }
    }
    if sticky {
        value |= 1;
    }
    let v = value as f64;
    if dropped == 0 {
        v
    } else if dropped > 1100 {
        f64::INFINITY
    } else {
        v * 2f64.powi(dropped as i32)
    }
}

/// The value of digits in a radix that is not 10 and not a power of two.
/// V8 has an approximation for long inputs: this is the same procedure.
fn generic_radix_value(digits: &[u8], radix: u32) -> f64 {
    const MAXIMUM_MULTIPLIER: u32 = 0xFFFF_FFFF / 36;
    let mut value = 0f64;
    let mut i = 0;
    while i < digits.len() {
        let mut part: u32 = 0;
        let mut multiplier: u32 = 1;
        while i < digits.len() {
            let m = multiplier * radix;
            if m > MAXIMUM_MULTIPLIER {
                break;
            }
            part = part * radix + digit_value(digits[i], radix).unwrap_or(0);
            multiplier = m;
            i += 1;
        }
        value = value * multiplier as f64 + part as f64;
    }
    value
}

/// `Number(string)`.
pub fn js_number(input: &str) -> f64 {
    let s = js_trim(input);
    let b = s.as_bytes();
    if b.is_empty() {
        return 0.0;
    }
    // The usual input is a small integer.
    if b.len() <= 15 && b.iter().all(u8::is_ascii_digit) {
        let mut n: u64 = 0;
        for &d in b {
            n = n * 10 + (d - b'0') as u64;
        }
        return n as f64;
    }
    if b.len() >= 2 && b[0] == b'0' {
        let radix = match b[1] {
            b'x' | b'X' => 16,
            b'o' | b'O' => 8,
            b'b' | b'B' => 2,
            _ => 0,
        };
        if radix != 0 {
            let digits = &b[2..];
            if digits.is_empty() || !digits.iter().all(|&d| digit_value(d, radix).is_some()) {
                return f64::NAN;
            }
            return power_of_two_radix_value(digits, radix);
        }
    }
    let (negative, rest) = match b[0] {
        b'-' => (true, &s[1..]),
        b'+' => (false, &s[1..]),
        _ => (false, s),
    };
    let magnitude = if rest == "Infinity" {
        f64::INFINITY
    } else if decimal_literal_end(rest.as_bytes()) == Some(rest.len()) {
        decimal_value(rest)
    } else {
        return f64::NAN;
    };
    if negative {
        -magnitude
    } else {
        magnitude
    }
}

/// `parseFloat(string)`.
pub fn js_parse_float(input: &str) -> f64 {
    let s = js_trim_start(input);
    let b = s.as_bytes();
    let (negative, rest) = match b.first() {
        Some(b'-') => (true, &s[1..]),
        Some(b'+') => (false, &s[1..]),
        _ => (false, s),
    };
    let magnitude = if rest.starts_with("Infinity") {
        f64::INFINITY
    } else if let Some(end) = decimal_literal_end(rest.as_bytes()) {
        decimal_value(&rest[..end])
    } else {
        return f64::NAN;
    };
    if negative {
        -magnitude
    } else {
        magnitude
    }
}

/// `parseInt(string, radix)`. A radix of 0 is the same as no radix.
pub fn js_parse_int(input: &str, radix: u32) -> f64 {
    let s = js_trim_start(input);
    let mut b = s.as_bytes();
    let mut negative = false;
    match b.first() {
        Some(b'-') => {
            negative = true;
            b = &b[1..];
        }
        Some(b'+') => b = &b[1..],
        _ => {}
    }
    let mut radix = radix;
    let mut strip_prefix = true;
    if radix != 0 {
        if !(2..=36).contains(&radix) {
            return f64::NAN;
        }
        if radix != 16 {
            strip_prefix = false;
        }
    } else {
        radix = 10;
    }
    if strip_prefix && b.len() >= 2 && b[0] == b'0' && (b[1] == b'x' || b[1] == b'X') {
        b = &b[2..];
        radix = 16;
    }
    let mut end = 0;
    while end < b.len() && digit_value(b[end], radix).is_some() {
        end += 1;
    }
    if end == 0 {
        return f64::NAN;
    }
    let digits = &b[..end];
    let magnitude = if radix == 10 {
        if end <= 15 {
            let mut n: u64 = 0;
            for &d in digits {
                n = n * 10 + (d - b'0') as u64;
            }
            n as f64
        } else {
            // The digits are ASCII, so this is valid UTF-8.
            decimal_value(std::str::from_utf8(digits).unwrap_or("NaN"))
        }
    } else if radix.is_power_of_two() {
        power_of_two_radix_value(digits, radix)
    } else {
        generic_radix_value(digits, radix)
    };
    if negative {
        -magnitude
    } else {
        magnitude
    }
}

/// `String(number)`: `Number.prototype.toString` with radix 10.
pub fn js_number_to_string(x: f64) -> String {
    if x.is_nan() {
        return "NaN".to_string();
    }
    if x == 0.0 {
        return "0".to_string();
    }
    if x.is_infinite() {
        return if x > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
    }
    // The usual number is a small integer.
    if x.trunc() == x && x.abs() < 1e15 {
        return (x as i64).to_string();
    }
    // `{:e}` gives the shortest digits that give the same number again.
    let scientific = format!("{:e}", x.abs());
    let (mantissa, exponent) = scientific.split_once('e').unwrap_or((&scientific, "0"));
    let exponent: i32 = exponent.parse().unwrap_or(0);
    let mut digits = String::with_capacity(mantissa.len());
    digits.extend(mantissa.chars().filter(|c| *c != '.'));
    let k = digits.len() as i32;
    let n = exponent + 1;

    let mut out = String::with_capacity(digits.len() + 8);
    if x < 0.0 {
        out.push('-');
    }
    if k <= n && n <= 21 {
        out.push_str(&digits);
        out.extend(std::iter::repeat_n('0', (n - k) as usize));
    } else if 0 < n && n <= 21 {
        out.push_str(&digits[..n as usize]);
        out.push('.');
        out.push_str(&digits[n as usize..]);
    } else if -6 < n && n <= 0 {
        out.push_str("0.");
        out.extend(std::iter::repeat_n('0', (-n) as usize));
        out.push_str(&digits);
    } else {
        let e = n - 1;
        out.push_str(&digits[..1]);
        if k > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        out.push(if e < 0 { '-' } else { '+' });
        out.push_str(&e.unsigned_abs().to_string());
    }
    out
}

/// `Number.prototype.toFixed(digits)`, for `digits` from 0 to 100.
/// JavaScript rounds a tie away from zero; Rust formats a tie to even.
pub fn js_to_fixed(x: f64, digits: usize) -> String {
    if !x.is_finite() || x.abs() >= 1e21 {
        return js_number_to_string(x);
    }
    let negative = x < 0.0;
    let x = x.abs();
    // The number of fraction digits of the exact decimal value.
    let exponent = ((x.to_bits() >> 52) & 0x7FF) as i64 - 1023;
    let exact = if x == 0.0 { 0 } else { (52 - exponent).clamp(0, 1074) as usize };
    let precision = exact.max(digits + 1);
    let text = format!("{:.*}", precision, x);
    let (int_part, frac_part) = text.split_once('.').unwrap_or((&text, ""));
    let round_up = frac_part.as_bytes().get(digits).is_some_and(|d| *d >= b'5');

    let mut number: Vec<u8> = Vec::with_capacity(int_part.len() + digits + 2);
    number.extend_from_slice(int_part.as_bytes());
    number.extend_from_slice(&frac_part.as_bytes()[..digits]);
    if round_up {
        let mut i = number.len();
        loop {
            if i == 0 {
                number.insert(0, b'1');
                break;
            }
            i -= 1;
            if number[i] == b'9' {
                number[i] = b'0';
            } else {
                number[i] += 1;
                break;
            }
        }
    }
    let int_len = number.len() - digits;
    let mut out = String::with_capacity(number.len() + 2);
    if negative {
        out.push('-');
    }
    for (i, d) in number.iter().enumerate() {
        if i == int_len {
            out.push('.');
        }
        out.push(*d as char);
    }
    out
}
