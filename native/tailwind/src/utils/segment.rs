//! Split a string at a top-level character: `utils/segment.ts` of Tailwind CSS.

/// The size of the upstream stack of brackets. Upstream does not record a
/// bracket deeper than this, and such a bracket never closes.
const STACK_SIZE: usize = 256;

/// Split `input` at each `separator` that is not in parentheses, brackets,
/// braces or a string. `separator` must be an ASCII character.
///
/// ```text
/// var(--a, 0 0 1px rgb(0, 0, 0)), 0 0 1px rgb(0, 0, 0)
///                               ^ the only split for `,`
/// ```
pub fn segment(input: &str, separator: u8) -> Vec<&str> {
    debug_assert!(separator.is_ascii());
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut parts = Vec::new();

    // The usual input has no separator.
    if !bytes.contains(&separator) {
        parts.push(input);
        return parts;
    }

    let mut stack = [0u8; STACK_SIZE];
    let mut stack_pos = 0usize;
    let mut last_pos = 0usize;

    let mut idx = 0;
    while idx < len {
        let c = bytes[idx];

        if stack_pos == 0 && c == separator {
            parts.push(&input[last_pos..idx]);
            last_pos = idx + 1;
            idx += 1;
            continue;
        }

        match c {
            // The next character is escaped.
            b'\\' => idx += 1,
            // A string goes to its end quote. Brackets in it do not count.
            b'\'' | b'"' => {
                idx += 1;
                while idx < len {
                    let next = bytes[idx];
                    if next == b'\\' {
                        idx += 2;
                        continue;
                    }
                    if next == c {
                        break;
                    }
                    idx += 1;
                }
            }
            b'(' | b'[' | b'{' => {
                if stack_pos < STACK_SIZE {
                    stack[stack_pos] = match c {
                        b'(' => b')',
                        b'[' => b']',
                        _ => b'}',
                    };
                }
                stack_pos += 1;
            }
            b')' | b']' | b'}' => {
                if stack_pos > 0 && stack_pos <= STACK_SIZE && c == stack[stack_pos - 1] {
                    stack_pos -= 1;
                }
            }
            _ => {}
        }
        idx += 1;
    }

    // An escape or a string at the end can move the cursor after the end.
    parts.push(&input[last_pos.min(len)..]);

    parts
}
