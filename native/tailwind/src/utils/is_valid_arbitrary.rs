//! A check of arbitrary values: `utils/is-valid-arbitrary.ts` of Tailwind CSS.

/// The size of the upstream stack of brackets. Upstream does not record a
/// bracket deeper than this.
const STACK_SIZE: usize = 256;

/// `true` when `input` can be a valid arbitrary value: the parentheses and
/// brackets are balanced, and there is no top-level `;`. An older version
/// has a part of this check (`switches::BALANCED_ARBITRARY_VALUE`,
/// `switches::ARBITRARY_VALUE_CHECK`).
pub fn is_valid_arbitrary(input: &str) -> bool {
    let bytes = input.as_bytes();
    let len = bytes.len();
    let mut stack = [0u8; STACK_SIZE];
    let mut stack_pos = 0usize;
    // Before 4.3.4, brackets that are not balanced were not an error.
    let balanced = crate::switches::BALANCED_ARBITRARY_VALUE.on();
    // Before 4.1.0, the official compiler had no check. The Rust compiler
    // keeps one part of the check for each version: a `}` with no open
    // bracket before it closes the rule of the output too early, and that
    // output is not valid CSS (docs/SWITCHES.md).
    let checked = crate::switches::ARBITRARY_VALUE_CHECK.on();

    let mut idx = 0;
    while idx < len {
        let c = bytes[idx];
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
            b'(' | b'[' => {
                if stack_pos < STACK_SIZE {
                    stack[stack_pos] = if c == b'(' { b')' } else { b']' };
                }
                stack_pos += 1;
            }
            // `{` does not open a level: `[&{color:red}]:flex` is not valid.
            // Before 4.1.0, it was valid, so the `{` opens a level.
            b'{' if !checked => {
                if stack_pos < STACK_SIZE {
                    stack[stack_pos] = b'}';
                }
                stack_pos += 1;
            }
            b')' | b']' | b'}' => {
                if stack_pos == 0 {
                    if checked || c == b'}' {
                        return false;
                    }
                } else if stack_pos <= STACK_SIZE && c == stack[stack_pos - 1] {
                    stack_pos -= 1;
                } else if c != b'}' && balanced {
                    return false;
                }
            }
            b';' if stack_pos == 0 && checked => return false,
            _ => {}
        }
        idx += 1;
    }

    !balanced || stack_pos == 0
}
