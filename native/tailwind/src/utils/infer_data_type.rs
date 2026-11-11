//! The data type of a value: `utils/infer-data-type.ts` of Tailwind CSS.

use super::is_color::is_color;
use super::js::{is_js_line_terminator, is_js_whitespace, js_is_integer, js_number, js_number_to_string};
use super::math_operators::has_math_fn;
use super::segment::segment;

/// A CSS data type that `infer_data_type` can find.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DataType {
    Color,
    Length,
    Percentage,
    Ratio,
    Number,
    Integer,
    Url,
    Position,
    BgSize,
    LineWidth,
    Image,
    FamilyName,
    GenericName,
    AbsoluteSize,
    RelativeSize,
    Angle,
    Vector,
}

impl DataType {
    /// The upstream name of the type, for example `bg-size`.
    pub fn as_str(self) -> &'static str {
        match self {
            DataType::Color => "color",
            DataType::Length => "length",
            DataType::Percentage => "percentage",
            DataType::Ratio => "ratio",
            DataType::Number => "number",
            DataType::Integer => "integer",
            DataType::Url => "url",
            DataType::Position => "position",
            DataType::BgSize => "bg-size",
            DataType::LineWidth => "line-width",
            DataType::Image => "image",
            DataType::FamilyName => "family-name",
            DataType::GenericName => "generic-name",
            DataType::AbsoluteSize => "absolute-size",
            DataType::RelativeSize => "relative-size",
            DataType::Angle => "angle",
            DataType::Vector => "vector",
        }
    }

    /// The type for an upstream name.
    pub fn from_name(name: &str) -> Option<DataType> {
        Some(match name {
            "color" => DataType::Color,
            "length" => DataType::Length,
            "percentage" => DataType::Percentage,
            "ratio" => DataType::Ratio,
            "number" => DataType::Number,
            "integer" => DataType::Integer,
            "url" => DataType::Url,
            "position" => DataType::Position,
            "bg-size" => DataType::BgSize,
            "line-width" => DataType::LineWidth,
            "image" => DataType::Image,
            "family-name" => DataType::FamilyName,
            "generic-name" => DataType::GenericName,
            "absolute-size" => DataType::AbsoluteSize,
            "relative-size" => DataType::RelativeSize,
            "angle" => DataType::Angle,
            "vector" => DataType::Vector,
            _ => return None,
        })
    }

    /// `true` when the value has this type.
    pub fn check(self, value: &str) -> bool {
        match self {
            DataType::Color => is_color(value),
            DataType::Length => is_length(value),
            DataType::Percentage => is_percentage(value),
            DataType::Ratio => is_fraction(value),
            DataType::Number => is_number(value),
            DataType::Integer => is_positive_integer(value),
            DataType::Url => is_url(value),
            DataType::Position => is_background_position(value),
            DataType::BgSize => is_background_size(value),
            DataType::LineWidth => is_line_width(value),
            DataType::Image => is_image(value),
            DataType::FamilyName => is_family_name(value),
            DataType::GenericName => is_generic_name(value),
            DataType::AbsoluteSize => is_absolute_size(value),
            DataType::RelativeSize => is_relative_size(value),
            DataType::Angle => is_angle(value),
            DataType::Vector => is_vector(value),
        }
    }
}

/// Find the type of `value` with the syntax rules of the CSS specifications.
/// The result is the first type of `types` that the value has.
pub fn infer_data_type(value: &str, types: &[DataType]) -> Option<DataType> {
    if value.starts_with("var(") {
        return None;
    }
    types.iter().copied().find(|ty| ty.check(value))
}

/// `/^url\(.*\)$/`: `.` does not match a line terminator.
pub fn is_url(value: &str) -> bool {
    value.len() >= 5
        && value.starts_with("url(")
        && value.ends_with(')')
        && !value[4..value.len() - 1].chars().any(is_js_line_terminator)
}

pub fn is_line_width(value: &str) -> bool {
    segment(value, b' ')
        .iter()
        .all(|value| is_length(value) || is_number(value) || matches!(*value, "thin" | "medium" | "thick"))
}

/// `/^(?:element|image|cross-fade|image-set)\(/`
fn is_image_fn(value: &str) -> bool {
    ["element(", "image(", "cross-fade(", "image-set("].iter().any(|f| value.starts_with(f))
}

/// `/^(repeating-)?(conic|linear|radial)-gradient\(/`
fn is_gradient_fn(value: &str) -> bool {
    let value = value.strip_prefix("repeating-").unwrap_or(value);
    ["conic-gradient(", "linear-gradient(", "radial-gradient("].iter().any(|f| value.starts_with(f))
}

pub fn is_image(value: &str) -> bool {
    let mut count = 0;
    for part in segment(value, b',') {
        if part.starts_with("var(") {
            continue;
        }
        if is_url(part) || is_gradient_fn(part) || is_image_fn(part) {
            count += 1;
            continue;
        }
        return false;
    }
    count > 0
}

pub fn is_generic_name(value: &str) -> bool {
    matches!(
        value,
        "serif"
            | "sans-serif"
            | "monospace"
            | "cursive"
            | "fantasy"
            | "system-ui"
            | "ui-serif"
            | "ui-sans-serif"
            | "ui-monospace"
            | "ui-rounded"
            | "math"
            | "emoji"
            | "fangsong"
    )
}

pub fn is_family_name(value: &str) -> bool {
    let mut count = 0;
    for part in segment(value, b',') {
        // A family name does not start with a digit.
        if part.as_bytes().first().is_some_and(u8::is_ascii_digit) {
            return false;
        }
        if part.starts_with("var(") {
            continue;
        }
        count += 1;
    }
    count > 0
}

pub fn is_absolute_size(value: &str) -> bool {
    matches!(
        value,
        "xx-small" | "x-small" | "small" | "medium" | "large" | "x-large" | "xx-large" | "xxx-large"
    )
}

pub fn is_relative_size(value: &str) -> bool {
    value == "larger" || value == "smaller"
}

/// The end of a number at `start`: `/[+-]?\d*\.?\d+(?:[eE][+-]?\d+)?/`.
///
/// Each use has an anchor at the two ends, and the text after a number never
/// starts with a digit or a `.`. Thus the longest match is the only match
/// that can be correct.
fn number_end(bytes: &[u8], start: usize) -> Option<usize> {
    let len = bytes.len();
    let mut i = start;
    if i < len && (bytes[i] == b'+' || bytes[i] == b'-') {
        i += 1;
    }
    let int_start = i;
    while i < len && bytes[i].is_ascii_digit() {
        i += 1;
    }
    let has_int = i > int_start;
    if i + 1 < len && bytes[i] == b'.' && bytes[i + 1].is_ascii_digit() {
        i += 2;
        while i < len && bytes[i].is_ascii_digit() {
            i += 1;
        }
    } else if !has_int {
        return None;
    }
    if i < len && (bytes[i] == b'e' || bytes[i] == b'E') {
        let mut j = i + 1;
        if j < len && (bytes[j] == b'+' || bytes[j] == b'-') {
            j += 1;
        }
        let digits_start = j;
        while j < len && bytes[j].is_ascii_digit() {
            j += 1;
        }
        if j > digits_start {
            i = j;
        }
    }
    Some(i)
}

/// The position after the JavaScript white space (`\s*`) at `start`.
fn skip_whitespace(value: &str, start: usize) -> usize {
    let mut i = start;
    for c in value[start..].chars() {
        if !is_js_whitespace(c) {
            break;
        }
        i += c.len_utf8();
    }
    i
}

pub fn is_number(value: &str) -> bool {
    number_end(value.as_bytes(), 0) == Some(value.len()) || has_math_fn(value)
}

pub fn is_percentage(value: &str) -> bool {
    let bytes = value.as_bytes();
    let matches = match number_end(bytes, 0) {
        Some(end) => end + 1 == bytes.len() && bytes[end] == b'%',
        None => false,
    };
    matches || has_math_fn(value)
}

/// `<number> / <number>`, with optional white space around the `/`.
pub fn is_fraction(value: &str) -> bool {
    fn matches(value: &str) -> bool {
        let bytes = value.as_bytes();
        let Some(end) = number_end(bytes, 0) else { return false };
        let slash = skip_whitespace(value, end);
        if bytes.get(slash) != Some(&b'/') {
            return false;
        }
        let start = skip_whitespace(value, slash + 1);
        number_end(bytes, start) == Some(bytes.len())
    }
    matches(value) || has_math_fn(value)
}

/// See <https://developer.mozilla.org/en-US/docs/Learn/CSS/Building_blocks/Values_and_units>.
const LENGTH_UNITS: [&str; 49] = [
    "cm", "mm", "Q", "in", "pc", "pt", "px", "em", "ex", "ch", "cap", "ic", "rem", "rex", "rch", "rcap",
    "ric", "lh", "rlh", "vw", "vh", "vmin", "vmax", "vb", "vi", "svw", "svh", "svi", "svb", "svmin", "svmax",
    "lvw", "lvh", "lvi", "lvb", "lvmin", "lvmax", "dvw", "dvh", "dvi", "dvb", "dvmin", "dvmax", "cqw", "cqh",
    "cqi", "cqb", "cqmin", "cqmax",
];

/// `true` when the value is a number with one of the units.
fn is_number_with_unit(value: &str, units: &[&str]) -> bool {
    match number_end(value.as_bytes(), 0) {
        Some(end) => units.contains(&&value[end..]),
        None => false,
    }
}

/// `/^(--spacing)\(/i`
fn is_length_fn(value: &str) -> bool {
    let bytes = value.as_bytes();
    crate::switches::SPACING_FN_IS_LENGTH.on()
        && bytes.len() >= 10
        && bytes[..9].eq_ignore_ascii_case(b"--spacing")
        && bytes[9] == b'('
}

/// The units of 4.3.4.
const NEW_LENGTH_UNITS: [&str; 18] = [
    "cap", "ic", "rex", "rch", "rcap", "ric", "svi", "svb", "svmin", "svmax", "lvi", "lvb", "lvmin", "lvmax", "dvi", "dvb", "dvmin",
    "dvmax",
];

pub fn is_length(value: &str) -> bool {
    let unit = is_number_with_unit(value, &LENGTH_UNITS)
        && (crate::switches::MORE_LENGTH_UNITS.on() || !is_number_with_unit(value, &NEW_LENGTH_UNITS));
    unit || is_length_fn(value) || has_math_fn(value)
}

pub fn is_background_position(value: &str) -> bool {
    let mut count = 0;
    for part in segment(value, b' ') {
        if matches!(part, "center" | "top" | "right" | "bottom" | "left") {
            count += 1;
            continue;
        }
        if part.starts_with("var(") {
            continue;
        }
        if is_length(part) || is_percentage(part) {
            count += 1;
            continue;
        }
        return false;
    }
    count > 0
}

/// `true` when the value is valid for `background-size`.
/// See <https://developer.mozilla.org/en-US/docs/Web/CSS/background-size#formal_syntax>.
pub fn is_background_size(value: &str) -> bool {
    let mut count = 0;
    for size in segment(value, b',') {
        if size == "cover" || size == "contain" {
            count += 1;
            continue;
        }
        let values = segment(size, b' ');
        // A size has one value or two values.
        if values.len() != 1 && values.len() != 2 {
            return false;
        }
        if values.iter().all(|value| *value == "auto" || is_length(value) || is_percentage(value)) {
            count += 1;
        }
    }
    count > 0
}

const ANGLE_UNITS: [&str; 4] = ["deg", "rad", "grad", "turn"];

/// `<number>` with `deg`, `rad`, `grad` or `turn`.
pub fn is_angle(value: &str) -> bool {
    is_number_with_unit(value, &ANGLE_UNITS)
}

/// `<number> <number> <number>`: the vector of `rotate`.
pub fn is_vector(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut pos = 0;
    for part in 0..3 {
        if part > 0 {
            let start = pos;
            while pos < bytes.len() && bytes[pos] == b' ' {
                pos += 1;
            }
            if pos == start {
                return false;
            }
        }
        match number_end(bytes, pos) {
            Some(end) => pos = end,
            None => return false,
        }
    }
    pos == bytes.len()
}

/// `true` when the text is the usual form of a whole number that is 0 or more.
pub fn is_positive_integer(value: &str) -> bool {
    // The usual value is a short run of digits.
    let bytes = value.as_bytes();
    if !bytes.is_empty() && bytes.len() <= 15 && bytes.iter().all(u8::is_ascii_digit) {
        return bytes.len() == 1 || bytes[0] != b'0';
    }
    let num = js_number(value);
    js_is_integer(num) && num >= 0.0 && js_number_to_string(num) == value
}

/// `true` when the text is the usual form of a whole number that is 1 or more.
pub fn is_strict_positive_integer(value: &str) -> bool {
    let num = js_number(value);
    js_is_integer(num) && num > 0.0 && js_number_to_string(num) == value
}

pub fn is_valid_spacing_multiplier(value: &str) -> bool {
    is_multiple_of(value, 0.25)
}

pub fn is_valid_opacity_value(value: &str) -> bool {
    is_multiple_of(value, 0.25)
}

/// `true` when the text is the usual form of a number that is a multiple of
/// `divisor`: no zeros that are not necessary.
fn is_multiple_of(value: &str, divisor: f64) -> bool {
    let num = js_number(value);
    num >= 0.0 && num % divisor == 0.0 && js_number_to_string(num) == value
}
