//! The parser of attribute selectors: `attribute-selector-parser.ts` of Tailwind CSS.

/// The parts of an attribute selector such as `[data-url$=".com" i]`.
/// The text parts are slices of the input.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AttributeSelector<'a> {
    pub attribute: &'a str,
    /// One of `=`, `~=`, `|=`, `^=`, `$=`, `*=`.
    pub operator: Option<&'static str>,
    /// `"` or `'`.
    pub quote: Option<char>,
    /// The value without the quotes.
    pub value: Option<&'a str>,
    /// `i` or `s`.
    pub sensitivity: Option<char>,
}

#[inline]
fn is_ascii_whitespace(b: Option<&u8>) -> bool {
    matches!(b, Some(b' ' | b'\t' | b'\n' | b'\r'))
}

/// Parse an attribute selector. The result is `None` when the input is not
/// a valid attribute selector.
pub fn parse(input: &str) -> Option<AttributeSelector<'_>> {
    let bytes = input.as_bytes();

    // Must start with `[` and end with `]`
    if bytes.first() != Some(&b'[') || bytes.last() != Some(&b']') {
        return None;
    }

    let mut i = 1;
    let end = bytes.len() - 1;

    while is_ascii_whitespace(bytes.get(i)) {
        i += 1;
    }

    // The attribute name: [data-foo]
    let mut start = i;
    while i < end {
        let c = bytes[i];
        if c == b'\\' {
            // An escaped character
            i += 2;
            continue;
        }
        if c.is_ascii_alphanumeric() || c == b'-' || c == b'_' || c >= 0x80 {
            i += 1;
            continue;
        }
        break;
    }
    if start == i {
        return None;
    }
    // An escape before the `]` moves the cursor after the end.
    let attribute = &input[start..i];

    while is_ascii_whitespace(bytes.get(i)) {
        i += 1;
    }

    if i == end {
        return Some(AttributeSelector {
            attribute,
            operator: None,
            quote: None,
            value: None,
            sensitivity: None,
        });
    }

    // The operator: [data-foo*=value]
    let operator = match bytes.get(i) {
        Some(b'=') => {
            i += 1;
            "="
        }
        Some(c @ (b'~' | b'|' | b'^' | b'$' | b'*')) if bytes.get(i + 1) == Some(&b'=') => {
            i += 2;
            match c {
                b'~' => "~=",
                b'|' => "|=",
                b'^' => "^=",
                b'$' => "$=",
                _ => "*=",
            }
        }
        _ => return None,
    };

    while is_ascii_whitespace(bytes.get(i)) {
        i += 1;
    }

    // An operator with no value: [data-foo*=]
    if i == end {
        return None;
    }

    let value;
    let mut quote = None;
    let current = bytes[i];
    if current == b'\'' || current == b'"' {
        // A value with quotes. The value ends at the last quote that is the
        // same as the first.
        quote = Some(current as char);
        i += 1;
        start = i;
        let mut j = i;
        while j < end {
            if bytes[j] == current {
                i = j + 1;
            } else if bytes[j] == b'\\' {
                j += 1;
            }
            j += 1;
        }
        // With no last quote, the value is empty.
        value = if i > start { &input[start..i - 1] } else { "" };
    } else {
        // A value with no quotes ends at white space.
        start = i;
        while i < end && !is_ascii_whitespace(bytes.get(i)) {
            i += 1;
        }
        value = &input[start..i];
    }

    while is_ascii_whitespace(bytes.get(i)) {
        i += 1;
    }

    if i == end {
        return Some(AttributeSelector {
            attribute,
            operator: Some(operator),
            quote,
            value: Some(value),
            sensitivity: None,
        });
    }

    // The sensitivity: [data-foo=value i]
    let sensitivity = match bytes.get(i) {
        Some(b'i' | b'I') => 'i',
        Some(b's' | b'S') => 's',
        _ => return None,
    };
    i += 1;

    while is_ascii_whitespace(bytes.get(i)) {
        i += 1;
    }

    // Each other character after the sensitivity is not valid.
    if i != end {
        return None;
    }

    Some(AttributeSelector {
        attribute,
        operator: Some(operator),
        quote,
        value: Some(value),
        sensitivity: Some(sensitivity),
    })
}
