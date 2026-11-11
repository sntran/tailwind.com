//! The CSS parser: `css-parser.ts` of Tailwind CSS.
//!
//! The parser reads bytes. Each character of the CSS structure is ASCII, so
//! a byte offset at such a character is a correct limit of a slice.

use crate::ast::{comment, decl_full, rule, AstNode, AtRule, Loc};
use crate::source_maps::{LineTable, Source, SourceLocation};
use std::fmt;
use std::rc::Rc;

const BACKSLASH: u8 = b'\\';
const SLASH: u8 = b'/';
const ASTERISK: u8 = b'*';
const DOUBLE_QUOTE: u8 = b'"';
const SINGLE_QUOTE: u8 = b'\'';
const COLON: u8 = b':';
const SEMICOLON: u8 = b';';
const LINE_BREAK: u8 = b'\n';
const CARRIAGE_RETURN: u8 = b'\r';
const SPACE: u8 = b' ';
const TAB: u8 = b'\t';
const OPEN_CURLY: u8 = b'{';
const CLOSE_CURLY: u8 = b'}';
const OPEN_PAREN: u8 = b'(';
const CLOSE_PAREN: u8 = b')';
const OPEN_BRACKET: u8 = b'[';
const CLOSE_BRACKET: u8 = b']';
const DASH: u8 = b'-';
const AT_SIGN: u8 = b'@';
const EXCLAMATION_MARK: u8 = b'!';
const LESS_THAN: u8 = b'<';

const IMPORTANT: &str = "!important";

/// An error in the CSS input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CssSyntaxError {
    /// The message. With a location, it starts with `file:line:column: `.
    pub message: String,
    /// The location of the error, when the parser has a source.
    pub loc: Option<SourceLocation>,
}

impl CssSyntaxError {
    fn new(message: impl Into<String>, loc: Option<SourceLocation>) -> Self {
        let mut message = message.into();
        // Before 4.1.18, the message had no location.
        if let Some(loc) = loc.as_ref().filter(|_| crate::switches::CSS_ERROR_LOCATION.on()) {
            let start = LineTable::new(&loc.source.code).find(loc.start);
            let file = loc.source.file.as_deref().unwrap_or("null");
            message = format!("{}:{}:{}: {}", file, start.line, start.column + 1, message);
        }
        CssSyntaxError { message, loc }
    }
}

/// The source of the parser, and the offsets of UTF-16 code units for
/// byte offsets.
struct Tracker {
    source: Rc<Source>,
    /// The UTF-16 offset of each byte offset. An ASCII text has no table:
    /// the two offsets are the same.
    table: Option<Vec<u32>>,
}

impl Tracker {
    fn new(file: &str, input: &str) -> Self {
        let source = Rc::new(Source { file: Some(file.to_string()), code: input.to_string() });
        let table = if input.is_ascii() {
            None
        } else {
            let mut table = Vec::with_capacity(input.len() + 1);
            let mut units = 0u32;
            for c in input.chars() {
                for _ in 0..c.len_utf8() {
                    table.push(units);
                }
                units += c.len_utf16() as u32;
            }
            table.push(units);
            Some(table)
        };
        Tracker { source, table }
    }

    fn offset(&self, byte: usize) -> usize {
        match &self.table {
            Some(table) => table[byte.min(table.len() - 1)] as usize,
            None => byte,
        }
    }

    /// The location of a range of bytes.
    fn at(&self, start: usize, end: usize) -> SourceLocation {
        SourceLocation { source: Rc::clone(&self.source), start: self.offset(start), end: self.offset(end) }
    }
}

/// The location of a range of bytes, when the parser has a source.
#[inline]
fn location(tracker: &Option<Tracker>, start: usize, end: usize) -> Option<SourceLocation> {
    tracker.as_ref().map(|tracker| tracker.at(start, end))
}

/// Set the locations of a new node, when the parser has a source.
#[inline]
fn track(tracker: &Option<Tracker>, mut node: AstNode, start: usize, end: usize) -> AstNode {
    if let Some(tracker) = tracker {
        if let Some(loc) = node.loc_mut() {
            *loc = Loc::new(tracker.at(start, end));
        }
    }
    node
}

impl fmt::Display for CssSyntaxError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for CssSyntaxError {}

/// The byte at `index`, or `None` after the end of the input.
#[inline]
fn at(bytes: &[u8], index: usize) -> Option<u8> {
    bytes.get(index).copied()
}

/// The length of the UTF-8 sequence that starts with `lead`.
#[inline]
fn utf8_len(lead: u8) -> usize {
    match lead {
        0xF0..=0xFF => 4,
        0xE0..=0xEF => 3,
        0xC0..=0xDF => 2,
        _ => 1,
    }
}

/// The index of the last byte of the comment that starts at `start`.
/// The result is `None` when the comment has no end.
#[inline]
fn comment_end(bytes: &[u8], start: usize) -> Option<usize> {
    // Before 4.3.4, a `\` in a comment escaped the next character.
    let escapes = !crate::switches::COMMENT_BACKSLASH_END.on();
    let mut j = start + 2;
    while j + 1 < bytes.len() {
        if escapes && bytes[j] == BACKSLASH {
            j += 1;
        } else if bytes[j] == ASTERISK && bytes[j + 1] == SLASH {
            return Some(j + 1);
        }
        j += 1;
    }
    None
}

/// Add a node to the open block, or to the root when no block is open.
#[inline]
fn attach(stack: &mut [AstNode], ast: &mut Vec<AstNode>, node: AstNode) {
    match stack.last_mut().and_then(AstNode::children_mut) {
        Some(nodes) => nodes.push(node),
        None => ast.push(node),
    }
}

/// Parse CSS into a syntax tree. The nodes have no source locations.
pub fn parse(input: &str) -> Result<Vec<AstNode>, CssSyntaxError> {
    parse_with_source(input, None)
}

/// Parse CSS into a syntax tree. With `from` (the path of the stylesheet,
/// not empty), each node has its source location, and the message of an
/// error starts with `file:line:column: `.
pub fn parse_with_source(input: &str, from: Option<&str>) -> Result<Vec<AstNode>, CssSyntaxError> {
    let bytes = input.as_bytes();
    let len = bytes.len();

    let source: Option<Tracker> = from.filter(|from| !from.is_empty()).map(|from| Tracker::new(from, input));
    let source = &source;
    // The start of the buffer. Upstream sets it only at an escape and at a
    // plain character.
    let mut buffer_start = 0;

    let mut ast: Vec<AstNode> = Vec::new();
    let mut license_comments: Vec<AstNode> = Vec::new();

    // The open blocks. The last one is the parent of the next node.
    let mut stack: Vec<AstNode> = Vec::new();

    let mut buffer = String::new();
    let mut closing_bracket_stack: Vec<u8> = Vec::new();

    // Upstream replaces a byte order mark with a space, and then ignores
    // the space. Thus the parse starts after the mark.
    let mut i = if input.starts_with('\u{FEFF}') { 3 } else { 0 };

    while i < len {
        let current = bytes[i];

        'step: {
            // Ignore the CR of a CRLF.
            if current == CARRIAGE_RETURN && at(bytes, i + 1) == Some(LINE_BREAK) {
                break 'step;
            }

            // An escape: keep the `\` and the next character.
            if current == BACKSLASH {
                let end = match at(bytes, i + 1) {
                    Some(next) => i + 1 + utf8_len(next),
                    None => i + 1,
                };
                if buffer.is_empty() {
                    buffer_start = i;
                }
                buffer.push_str(&input[i..end]);
                i = end - 1;
            }
            // A comment.
            else if current == SLASH && at(bytes, i + 1) == Some(ASTERISK) {
                // A comment with no end removes only its `/`.
                if let Some(end) = comment_end(bytes, i) {
                    if bytes[i + 2] == EXCLAMATION_MARK {
                        let node = comment(&input[i + 2..end - 1]);
                        license_comments.push(track(source, node, i, end + 1));
                    }
                    i = end;
                }
            }
            // A string.
            else if current == SINGLE_QUOTE || current == DOUBLE_QUOTE {
                let end = parse_string(input, i, current, source)?;
                buffer.push_str(&input[i..=end]);
                i = end;
            }
            // White space before more white space.
            else if (current == SPACE || current == LINE_BREAK || current == TAB)
                && match at(bytes, i + 1) {
                    Some(SPACE | LINE_BREAK | TAB) => true,
                    Some(CARRIAGE_RETURN) => at(bytes, i + 2) == Some(LINE_BREAK),
                    _ => false,
                }
            {
                break 'step;
            }
            // A new line becomes a space.
            else if current == LINE_BREAK {
                match buffer.as_bytes().last() {
                    None | Some(&(SPACE | LINE_BREAK | TAB)) => {}
                    Some(_) => buffer.push(' '),
                }
            }
            // `<!--` and `-->` between the nodes of the root.
            else if stack.is_empty()
                && buffer.is_empty()
                && crate::switches::HTML_COMMENT_MARKS.on()
                && ((current == DASH && bytes[i + 1..].starts_with(b"->"))
                    || (current == LESS_THAN && bytes[i + 1..].starts_with(b"!--")))
            {
                i += if current == DASH { 2 } else { 3 };
            }
            // A custom property. Its value can contain almost each
            // character, also `;` and `}` in balanced brackets.
            else if current == DASH && at(bytes, i + 1) == Some(DASH) && buffer.is_empty() {
                let start = i;
                let mut brackets: Vec<u8> = Vec::new();
                let mut colon_idx: Option<usize> = None;
                // The end of the value without the white space and the
                // comments after it.
                let mut value_end = 0;
                let mut end: Option<usize> = None;
                let complete_at_eof = crate::switches::CUSTOM_PROPERTY_AT_EOF.on();

                let mut j = i + 2;
                while j < len {
                    let peek = bytes[j];

                    if peek == BACKSLASH {
                        j += 1;
                    } else if peek == SINGLE_QUOTE || peek == DOUBLE_QUOTE {
                        j = parse_string(input, j, peek, source)?;
                    } else if peek == SLASH && at(bytes, j + 1) == Some(ASTERISK) {
                        if let Some(end) = comment_end(bytes, j) {
                            j = end;
                        }
                        j += 1;
                        continue;
                    } else if colon_idx.is_none() && peek == COLON {
                        colon_idx = Some(j - start);
                    } else if peek == SEMICOLON && brackets.is_empty() {
                        end = Some(j);
                        i = j;
                        break;
                    } else if peek == OPEN_PAREN {
                        brackets.push(CLOSE_PAREN);
                    } else if peek == OPEN_BRACKET {
                        brackets.push(CLOSE_BRACKET);
                    } else if peek == OPEN_CURLY {
                        brackets.push(CLOSE_CURLY);
                    } else if (peek == CLOSE_CURLY || (j == len - 1 && !complete_at_eof)) && brackets.is_empty() {
                        // Before 4.3.4, the last character of the input
                        // ended the property too, and was not a part of it.
                        end = Some(j);
                        i = j - 1;
                        break;
                    } else if (peek == CLOSE_PAREN || peek == CLOSE_BRACKET || peek == CLOSE_CURLY)
                        && brackets.last() == Some(&peek)
                    {
                        brackets.pop();
                    }

                    if peek != SPACE && peek != TAB && peek != LINE_BREAK && peek != CARRIAGE_RETURN
                    {
                        value_end = j + 1 - start;
                    }
                    j += 1;
                }

                // With no `;` and no `}`, the property goes to the end of
                // the input.
                let text = match end {
                    Some(end) => &input[start..end],
                    None if complete_at_eof => {
                        i = len;
                        &input[start..]
                    }
                    // Before 4.3.4: no text, and the parser reads the
                    // characters after the first `-` again.
                    None => "",
                };

                let Some(declaration) = parse_declaration(text, colon_idx, value_end) else {
                    return Err(CssSyntaxError::new(
                        "Invalid custom property, expected a value",
                        location(source, start, i),
                    ));
                };
                attach(&mut stack, &mut ast, track(source, declaration, start, i));
            }
            // The end of an at-rule with no block.
            else if current == SEMICOLON && buffer.as_bytes().first() == Some(&AT_SIGN) {
                let node = AstNode::AtRule(parse_at_rule(&buffer, Vec::new()));
                attach(&mut stack, &mut ast, track(source, node, buffer_start, i));
                buffer.clear();
            }
            // The end of a declaration.
            else if current == SEMICOLON && closing_bracket_stack.last() != Some(&CLOSE_PAREN) {
                match parse_declaration(&buffer, buffer.find(':'), buffer.len()) {
                    Some(declaration) => attach(&mut stack, &mut ast, track(source, declaration, buffer_start, i)),
                    None => {
                        if buffer.is_empty() {
                            break 'step;
                        }
                        return Err(invalid_declaration(&buffer, location(source, buffer_start, i)));
                    }
                }
                buffer.clear();
            }
            // The start of a block.
            else if current == OPEN_CURLY && closing_bracket_stack.last() != Some(&CLOSE_PAREN) {
                closing_bracket_stack.push(CLOSE_CURLY);
                stack.push(track(source, rule(js_trim(&buffer), Vec::new()), buffer_start, i));
                buffer.clear();
            }
            // The end of a block.
            else if current == CLOSE_CURLY && closing_bracket_stack.last() != Some(&CLOSE_PAREN) {
                if closing_bracket_stack.pop().is_none() {
                    return Err(CssSyntaxError::new("Missing opening {", location(source, i, i)));
                }

                // A filled buffer is a last node with no `;`.
                if !buffer.is_empty() {
                    let node = if buffer.as_bytes()[0] == AT_SIGN {
                        AstNode::AtRule(parse_at_rule(&buffer, Vec::new()))
                    } else {
                        match parse_declaration(&buffer, buffer.find(':'), buffer.len()) {
                            Some(declaration) => declaration,
                            None => return Err(invalid_declaration(&buffer, location(source, buffer_start, i))),
                        }
                    };
                    attach(&mut stack, &mut ast, track(source, node, buffer_start, i));
                    buffer.clear();
                }

                // The two stacks have the same number of blocks, so the
                // block is there.
                if let Some(done) = stack.pop() {
                    attach(&mut stack, &mut ast, done);
                }
            } else if current == OPEN_PAREN {
                closing_bracket_stack.push(CLOSE_PAREN);
                buffer.push('(');
            } else if current == CLOSE_PAREN {
                if closing_bracket_stack.last() != Some(&CLOSE_PAREN) {
                    return Err(CssSyntaxError::new("Missing opening (", location(source, i, i)));
                }
                closing_bracket_stack.pop();
                buffer.push(')');
            }
            // Each other character is a part of the node.
            else {
                // Ignore white space at the start of a node.
                if buffer.is_empty()
                    && (current == SPACE || current == LINE_BREAK || current == TAB)
                {
                    break 'step;
                }
                if buffer.is_empty() {
                    buffer_start = i;
                }
                if current < 0x80 {
                    buffer.push(current as char);
                } else {
                    let end = i + utf8_len(current);
                    buffer.push_str(&input[i..end]);
                    i = end - 1;
                }
            }
        }

        i += 1;
    }

    // An at-rule with no `;` at the end of the input.
    if buffer.as_bytes().first() == Some(&AT_SIGN) {
        let node = AstNode::AtRule(parse_at_rule(&buffer, Vec::new()));
        ast.push(track(source, node, buffer_start, len));
    }
    // A declaration with no `;` at the end of the input. Open brackets
    // make the declaration incomplete.
    else if !buffer.is_empty() && closing_bracket_stack.is_empty() && crate::switches::DECLARATION_AT_EOF.on() {
        if let Some(declaration) = parse_declaration(&buffer, buffer.find(':'), buffer.len()) {
            attach(&mut stack, &mut ast, track(source, declaration, buffer_start, len));
        }
    }

    // A block that is open at the end of the input is an error.
    match stack.last() {
        Some(AstNode::Rule(parent)) => {
            return Err(CssSyntaxError::new(
                format!("Missing closing }} at {}", parent.selector),
                parent.loc.src().map(|src| SourceLocation { end: src.start, ..src.clone() }),
            ));
        }
        Some(AstNode::AtRule(parent)) => {
            return Err(CssSyntaxError::new(
                format!("Missing closing }} at {} {}", parent.name, parent.params),
                parent.loc.src().map(|src| SourceLocation { end: src.start, ..src.clone() }),
            ));
        }
        _ => {}
    }

    if license_comments.is_empty() {
        return Ok(ast);
    }
    license_comments.append(&mut ast);
    Ok(license_comments)
}

fn invalid_declaration(buffer: &str, loc: Option<SourceLocation>) -> CssSyntaxError {
    CssSyntaxError::new(format!("Invalid declaration: `{}`", js_trim(buffer)), loc)
}

/// An at-rule from its text, for example `@media (width >= 40rem)`.
pub fn parse_at_rule(buffer: &str, nodes: Vec<AstNode>) -> AtRule {
    let bytes = buffer.as_bytes();
    let mut name = buffer;
    let mut params = "";

    // Upstream starts the search at the UTF-16 index 5: the length of
    // `@page`, the shortest at-rule of CSS.
    let start = if bytes.len() >= 5 && bytes[..5].is_ascii() {
        5
    } else {
        let mut units = 0;
        let mut start = bytes.len();
        for (offset, c) in buffer.char_indices() {
            if units >= 5 {
                start = offset;
                break;
            }
            units += c.len_utf16();
        }
        start
    };

    for (i, &current) in bytes.iter().enumerate().skip(start) {
        if current == SPACE || current == TAB || current == OPEN_PAREN {
            name = &buffer[..i];
            params = &buffer[i..];
            break;
        }
    }

    AtRule {
        name: js_trim(name).to_string(),
        params: js_trim(params).to_string(),
        nodes,
        loc: Loc::default(),
    }
}

/// A declaration from its text. `colon_idx` is the offset of the `:`.
/// `value_end` is the end of the value without comments after it.
fn parse_declaration(buffer: &str, colon_idx: Option<usize>, value_end: usize) -> Option<AstNode> {
    let colon_idx = colon_idx?;

    // Before 4.3.4: the first `!important` after the colon is the flag,
    // and it ends the value. (The text can be shorter than the place of
    // the colon: see `CUSTOM_PROPERTY_AT_EOF`.)
    if !crate::switches::IMPORTANT_AT_THE_END.on() {
        let property = js_trim(buffer.get(..colon_idx).unwrap_or(buffer));
        let rest = buffer.get(colon_idx + 1..).unwrap_or("");
        let important = rest.find(IMPORTANT);
        let value = js_trim(&rest[..important.unwrap_or(rest.len())]);
        return Some(decl_full(property, Some(value.to_string()), important.is_some()));
    }

    let property = js_trim(&buffer[..colon_idx]);
    let value_start = colon_idx + 1;
    let value_end = value_end.clamp(value_start, buffer.len());

    // `!important` is a flag only at the end of the value, and only when
    // no `\` escapes the `!`.
    let value = trim_value(&buffer[value_start..value_end]);
    if let Some(rest) = value.strip_suffix(IMPORTANT) {
        if !is_escaped(value, rest.len()) {
            return Some(decl_full(
                property,
                Some(trim_value(rest).to_string()),
                true,
            ));
        }
    }

    Some(decl_full(
        property,
        Some(trim_value(&buffer[value_start..]).to_string()),
        false,
    ))
}

/// Remove the white space around a value. An escaped white space
/// character at the end is a part of the value.
fn trim_value(value: &str) -> &str {
    let value = js_trim_start(value);
    let trimmed = js_trim_end(value);

    if trimmed.len() < value.len() && is_escaped(value, trimmed.len()) {
        let next = value[trimmed.len()..]
            .chars()
            .next()
            .map_or(0, char::len_utf8);
        return &value[..trimmed.len() + next];
    }

    trimmed
}

/// True when an odd number of `\` is before the character at `idx`.
fn is_escaped(value: &str, idx: usize) -> bool {
    let backslashes = value.as_bytes()[..idx]
        .iter()
        .rev()
        .take_while(|&&b| b == BACKSLASH)
        .count();
    backslashes % 2 == 1
}

/// The index of the quote that ends the string that starts at `start_idx`.
/// The result is `start_idx` when the input ends before the string ends.
fn parse_string(
    input: &str,
    start_idx: usize,
    quote_char: u8,
    source: &Option<Tracker>,
) -> Result<usize, CssSyntaxError> {
    let bytes = input.as_bytes();
    let mut i = start_idx + 1;

    while i < bytes.len() {
        let peek = bytes[i];

        if peek == BACKSLASH {
            i += 1;
        } else if peek == quote_char {
            return Ok(i);
        }
        // A `;` at the end of the line, and no end of the string.
        else if peek == SEMICOLON
            && (at(bytes, i + 1) == Some(LINE_BREAK)
                || (at(bytes, i + 1) == Some(CARRIAGE_RETURN)
                    && at(bytes, i + 2) == Some(LINE_BREAK)))
        {
            return Err(unterminated_string(&input[start_idx..=i], quote_char, location(source, start_idx, i + 1)));
        }
        // The end of the line, and no end of the string.
        else if peek == LINE_BREAK
            || (peek == CARRIAGE_RETURN && at(bytes, i + 1) == Some(LINE_BREAK))
        {
            return Err(unterminated_string(&input[start_idx..i], quote_char, location(source, start_idx, i + 1)));
        }

        i += 1;
    }

    Ok(start_idx)
}

fn unterminated_string(text: &str, quote_char: u8, loc: Option<SourceLocation>) -> CssSyntaxError {
    CssSyntaxError::new(format!("Unterminated string: {}{}", text, quote_char as char), loc)
}

/// True for the characters that `String.prototype.trim` of JavaScript
/// removes: the white space and the line terminators of ECMAScript.
#[inline]
fn is_js_whitespace(c: char) -> bool {
    matches!(
        c,
        '\u{0009}'..='\u{000D}'
            | '\u{0020}'
            | '\u{00A0}'
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

fn js_trim_start(value: &str) -> &str {
    value.trim_start_matches(is_js_whitespace)
}

fn js_trim_end(value: &str) -> &str {
    value.trim_end_matches(is_js_whitespace)
}

fn js_trim(value: &str) -> &str {
    js_trim_end(js_trim_start(value))
}
