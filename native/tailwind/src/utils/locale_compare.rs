//! `String.prototype.localeCompare` of V8 with no locale: the collation of
//! ICU for the locale `en-US` (the root collation of CLDR).
//!
//! The comparison has two levels for ASCII text. First the primary
//! weights: the punctuation and the symbols come before the digits, and
//! the digits come before the letters, with no difference between a
//! lowercase and an uppercase letter. Then the tertiary weights: a
//! lowercase letter comes before an uppercase letter. The control
//! characters have no weight. A character that is not ASCII comes after
//! the ASCII characters, in the order of its code point: this is not the
//! order of ICU for such characters.

use std::cmp::Ordering;

/// The ASCII characters with a primary weight, in the order of ICU. An
/// uppercase letter has the weight of its lowercase letter.
const PRIMARY_ORDER: &[u8] =
    b"\t\n\x0b\x0c\r _-,;:!?.'\"()[]{}@*/\\&#%`^+<=>|~$0123456789abcdefghijklmnopqrstuvwxyz";

const fn primary_table() -> [u16; 128] {
    let mut table = [0u16; 128];
    let mut i = 0;
    while i < PRIMARY_ORDER.len() {
        table[PRIMARY_ORDER[i] as usize] = (i + 1) as u16;
        i += 1;
    }
    let mut c = b'A';
    while c <= b'Z' {
        table[c as usize] = table[(c + 32) as usize];
        c += 1;
    }
    table
}

const PRIMARY: [u16; 128] = primary_table();

/// The primary weight of a character, or 0 for a character with no weight.
fn primary(c: char) -> u32 {
    match u32::try_from(c).unwrap_or(0) {
        code @ 0..=127 => PRIMARY[code as usize] as u32,
        code => 1000 + code,
    }
}

/// The tertiary weight: 1 for an uppercase ASCII letter, else 0.
fn tertiary(c: char) -> u8 {
    c.is_ascii_uppercase() as u8
}

/// `a.localeCompare(b)` of V8.
pub fn locale_compare(a: &str, b: &str) -> Ordering {
    let weights = |s: &'_ str| s.chars().map(primary).filter(|w| *w != 0).collect::<Vec<u32>>();
    match weights(a).cmp(&weights(b)) {
        Ordering::Equal => {}
        other => return other,
    }
    let tertiaries = |s: &'_ str| s.chars().filter(|c| primary(*c) != 0).map(tertiary).collect::<Vec<u8>>();
    tertiaries(a).cmp(&tertiaries(b))
}
