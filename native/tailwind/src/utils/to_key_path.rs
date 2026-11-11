//! Parse a key path: `utils/to-key-path.ts` of Tailwind CSS.

use super::segment::segment;

/// Parse a path into its keys. Brackets `a[b]` escape dots.
///
/// ```text
/// a.b.c      -> a, b, c
/// a[b].c     -> a, b, c
/// a[b.c].e.f -> a, b.c, e, f
/// a[b][c][d] -> a, b, c, d
/// ```
pub fn to_key_path(path: &str) -> Vec<&str> {
    let mut keypath = Vec::new();

    for part in segment(path, b'.') {
        if !part.contains('[') {
            keypath.push(part);
            continue;
        }

        let mut current_index = 0;

        while let Some(bracket_l) = part[current_index..].find('[').map(|i| i + current_index) {
            let Some(bracket_r) = part[bracket_l..].find(']').map(|i| i + bracket_l) else {
                break;
            };

            // The part before the bracket is a key.
            if bracket_l > current_index {
                keypath.push(&part[current_index..bracket_l]);
            }

            // The part in the brackets is a key.
            keypath.push(&part[bracket_l + 1..bracket_r]);
            current_index = bracket_r + 1;
        }

        // The part after the last bracket is a key.
        if current_index < part.len() {
            keypath.push(&part[current_index..]);
        }
    }

    keypath
}
