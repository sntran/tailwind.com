//! `CSS.escape` and its inverse: `utils/escape.ts` of Tailwind CSS.

use std::borrow::Cow;

/// `true` for an ASCII byte that an identifier has with no escape.
#[inline]
fn is_plain(b: u8) -> bool {
    b >= 0x80 || b == b'-' || b == b'_' || b.is_ascii_alphanumeric()
}

/// Escape a value for use as a CSS identifier.
/// See <https://drafts.csswg.org/cssom/#serialize-an-identifier>.
pub fn escape(value: &str) -> Cow<'_, str> {
    let bytes = value.as_bytes();
    let first = bytes.first().copied();

    // The usual value needs no escape.
    let digit_start = match first {
        Some(b'0'..=b'9') => true,
        Some(b'-') => bytes.len() == 1 || bytes[1].is_ascii_digit(),
        _ => false,
    };
    if !digit_start && bytes.iter().all(|&b| is_plain(b)) {
        return Cow::Borrowed(value);
    }

    if bytes == b"-" {
        return Cow::Borrowed("\\-");
    }

    let mut result = String::with_capacity(bytes.len() + 8);
    // The start of the text that is not in `result` yet.
    let mut pending = 0;
    for (index, &code) in bytes.iter().enumerate() {
        // All bytes of a character that is not ASCII stay as they are.
        if code >= 0x80 {
            continue;
        }
        let plain = is_plain(code);
        let numeric_escape = (0x01..=0x1f).contains(&code)
            || code == 0x7f
            || (index == 0 && code.is_ascii_digit())
            || (index == 1 && code.is_ascii_digit() && first == Some(b'-'));
        if plain && !numeric_escape {
            continue;
        }
        result.push_str(&value[pending..index]);
        pending = index + 1;
        if code == 0 {
            result.push('\u{FFFD}');
        } else if numeric_escape {
            // The character as a code point, with a space at the end.
            result.push('\\');
            const HEX: &[u8; 16] = b"0123456789abcdef";
            if code >= 0x10 {
                result.push(HEX[(code >> 4) as usize] as char);
            }
            result.push(HEX[(code & 0xf) as usize] as char);
            result.push(' ');
        } else {
            result.push('\\');
            result.push(code as char);
        }
    }
    result.push_str(&value[pending..]);
    Cow::Owned(result)
}

/// Decode the escapes of a CSS identifier.
pub fn unescape(escaped: &str) -> Cow<'_, str> {
    let bytes = escaped.as_bytes();
    let Some(first) = bytes.iter().position(|&b| b == b'\\') else {
        return Cow::Borrowed(escaped);
    };
    // A backslash at the end stays.
    if first + 1 == bytes.len() {
        return Cow::Borrowed(escaped);
    }

    let len = bytes.len();
    let mut result = String::with_capacity(len);
    let mut pending = 0;
    let mut i = first;
    while i < len {
        if bytes[i] != b'\\' || i + 1 == len {
            i += 1;
            continue;
        }
        result.push_str(&escaped[pending..i]);
        i += 1;

        // One to six hexadecimal digits, then an optional white space.
        let mut code_point: u32 = 0;
        let mut digits = 0;
        while digits < 6 && i < len {
            let Some(d) = (bytes[i] as char).to_digit(16) else { break };
            code_point = code_point * 16 + d;
            digits += 1;
            i += 1;
        }
        if digits == 0 {
            // The escaped character stays: it is in the text after `pending`.
            pending = i;
            i += escaped[i..].chars().next().map_or(1, char::len_utf8);
            continue;
        }
        let white_space = i < len && matches!(bytes[i], b'\t' | b'\n' | 0x0c | b'\r' | b' ');
        if white_space {
            i += 1;
        }
        // Before 4.3.4, an escape of two characters (`\0` … `\f` with no
        // white space after it) was the second character itself.
        if digits == 1 && !white_space && !crate::switches::UNESCAPE_ONE_HEX_DIGIT.on() {
            pending = i - 1;
            continue;
        }
        // NUL, a surrogate, or a value after the last code point
        let c = match code_point {
            0 => '\u{FFFD}',
            n => char::from_u32(n).unwrap_or('\u{FFFD}'),
        };
        result.push(c);
        pending = i;
    }
    result.push_str(&escaped[pending..]);
    Cow::Owned(result)
}
