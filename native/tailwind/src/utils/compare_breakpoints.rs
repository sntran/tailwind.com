//! The order of breakpoints: `utils/compare-breakpoints.ts` of Tailwind CSS.

use super::js::{js_compare_chars, js_compare_strings, js_parse_float};
use std::cmp::Ordering;

/// The direction of the order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Direction {
    Asc,
    Desc,
}

/// The bucket of a value with no CSS function: the value without digits
/// and dots (`/[\d.]+/g`). This is the unit.
fn unit_bucket(value: &str) -> impl Iterator<Item = char> + '_ {
    value.chars().filter(|c| !c.is_ascii_digit() && *c != '.')
}

/// Compare two breakpoints such as `40rem` and `calc(100% - 1rem)`.
///
/// Upstream returns a number. The result here is the sign of that number.
pub fn compare_breakpoints(a: &str, z: &str, direction: Direction) -> Ordering {
    if a == z {
        return Ordering::Equal;
    }

    // A `(` means that the value is a CSS function. The bucket is then the
    // name of the function.
    let bucket_order = match (a.find('('), z.find('(')) {
        (None, None) => js_compare_chars(unit_bucket(a), unit_bucket(z)),
        (None, Some(zi)) => js_compare_chars(unit_bucket(a), z[..zi].chars()),
        (Some(ai), None) => js_compare_chars(a[..ai].chars(), unit_bucket(z)),
        (Some(ai), Some(zi)) => js_compare_strings(&a[..ai], &z[..zi]),
    };
    if bucket_order != Ordering::Equal {
        return bucket_order;
    }

    // In the same bucket, compare by value.
    // Before 4.3.4: `parseInt`.
    let number = |text: &str| {
        if crate::switches::BREAKPOINT_DECIMAL_SORT.on() {
            js_parse_float(text)
        } else {
            crate::utils::js::js_parse_int(text, 10)
        }
    };
    let order = match direction {
        Direction::Asc => number(a) - number(z),
        Direction::Desc => number(z) - number(a),
    };

    // The order is NaN when the contents are not numbers, for example with
    // `calc`. The string order then makes the sort stable.
    if order.is_nan() {
        return if js_compare_strings(a, z) == Ordering::Less { Ordering::Less } else { Ordering::Greater };
    }

    order.partial_cmp(&0.0).unwrap_or(Ordering::Equal)
}
