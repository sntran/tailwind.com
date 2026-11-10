//! An alphanumeric order: `utils/compare.ts` of Tailwind CSS.

use super::js::{js_compare_chars, js_number};
use std::cmp::Ordering;

/// Compare two strings. Numbers in the strings compare as numbers.
///
/// Upstream returns a number. The result here is the sign of that number.
pub fn compare(a: &str, z: &str) -> Ordering {
    let (ab, zb) = (a.as_bytes(), z.as_bytes());
    let min_len = ab.len().min(zb.len());

    let mut i = 0;
    while i < min_len {
        let a_code = ab[i];
        let z_code = zb[i];

        // Two numbers compare as numbers.
        if a_code.is_ascii_digit() && z_code.is_ascii_digit() {
            let mut a_end = i + 1;
            while a_end < ab.len() && ab[a_end].is_ascii_digit() {
                a_end += 1;
            }
            let mut z_end = i + 1;
            while z_end < zb.len() && zb[z_end].is_ascii_digit() {
                z_end += 1;
            }

            let a_number = &ab[i..a_end];
            let z_number = &zb[i..z_end];

            if a_number != z_number {
                let ordering = compare_digits(a_number, z_number);
                if ordering != Ordering::Equal {
                    return ordering;
                }
            }

            // Go on with the next character. If not, short strings come
            // after long strings: `bg-red-500/70`, `bg-red-500`.
            i += 1;
            continue;
        }

        if a_code == z_code {
            i += 1;
            continue;
        }

        // Compare as strings: by UTF-16 code units.
        if a_code < 0x80 || z_code < 0x80 {
            return a_code.cmp(&z_code);
        }
        let mut start = i;
        while !a.is_char_boundary(start) {
            start -= 1;
        }
        return js_compare_chars(a[start..].chars().take(1), z[start..].chars().take(1));
    }

    // The strings are equal up to the length of the shortest string.
    ab.len().cmp(&zb.len())
}

/// Compare two different runs of digits: first as JavaScript numbers, then
/// as strings.
fn compare_digits(a: &[u8], z: &[u8]) -> Ordering {
    // Short runs are exact integers.
    if a.len() <= 15 && z.len() <= 15 {
        let value = |digits: &[u8]| digits.iter().fold(0u64, |n, d| n * 10 + (d - b'0') as u64);
        return value(a).cmp(&value(z)).then_with(|| a.cmp(z));
    }
    // The digits are ASCII, so the conversion is correct.
    let number = |digits: &[u8]| js_number(std::str::from_utf8(digits).unwrap_or(""));
    let diff = number(a) - number(z);
    if diff < 0.0 {
        Ordering::Less
    } else if diff > 0.0 {
        Ordering::Greater
    } else {
        // The difference is 0 or NaN (two infinite numbers).
        a.cmp(z)
    }
}
