//! The parser of CSS values: `value-parser.ts` of Tailwind CSS.

use crate::walk::Walkable;
use std::borrow::Cow;

/// A word: text with no separator in it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ValueWordNode {
    pub value: String,
}

/// A function call. `value` is the name; it is empty for plain parentheses.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ValueFunctionNode {
    pub value: String,
    pub nodes: Vec<ValueAstNode>,
}

/// A run of separator characters: space, tab, newline, `,` `:` `=` `<` `>`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct ValueSeparatorNode {
    pub value: String,
}

/// A node of the value syntax tree.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum ValueAstNode {
    Word(ValueWordNode),
    Function(ValueFunctionNode),
    Separator(ValueSeparatorNode),
}

impl ValueAstNode {
    /// The upstream `kind` of the node.
    pub fn kind(&self) -> &'static str {
        match self {
            ValueAstNode::Word(_) => "word",
            ValueAstNode::Function(_) => "function",
            ValueAstNode::Separator(_) => "separator",
        }
    }

    /// The `value` of the node.
    pub fn value(&self) -> &str {
        match self {
            ValueAstNode::Word(n) => &n.value,
            ValueAstNode::Function(n) => &n.value,
            ValueAstNode::Separator(n) => &n.value,
        }
    }

    /// The `value` of the node, for a change.
    pub fn value_mut(&mut self) -> &mut String {
        match self {
            ValueAstNode::Word(n) => &mut n.value,
            ValueAstNode::Function(n) => &mut n.value,
            ValueAstNode::Separator(n) => &mut n.value,
        }
    }

    pub fn is_word(&self) -> bool {
        matches!(self, ValueAstNode::Word(_))
    }

    pub fn is_function(&self) -> bool {
        matches!(self, ValueAstNode::Function(_))
    }

    pub fn is_separator(&self) -> bool {
        matches!(self, ValueAstNode::Separator(_))
    }
}

impl Walkable for ValueAstNode {
    fn nodes(&self) -> Option<&Vec<Self>> {
        match self {
            ValueAstNode::Function(n) => Some(&n.nodes),
            _ => None,
        }
    }

    fn nodes_mut(&mut self) -> Option<&mut Vec<Self>> {
        match self {
            ValueAstNode::Function(n) => Some(&mut n.nodes),
            _ => None,
        }
    }

    fn shallow(&self) -> Self {
        match self {
            ValueAstNode::Function(n) => fun(n.value.as_str(), Vec::new()),
            other => other.clone(),
        }
    }
}

/// A word node.
pub fn word(value: impl Into<String>) -> ValueAstNode {
    ValueAstNode::Word(ValueWordNode { value: value.into() })
}

/// A function node.
pub fn fun(value: impl Into<String>, nodes: Vec<ValueAstNode>) -> ValueAstNode {
    ValueAstNode::Function(ValueFunctionNode { value: value.into(), nodes })
}

/// A separator node.
pub fn separator(value: impl Into<String>) -> ValueAstNode {
    ValueAstNode::Separator(ValueSeparatorNode { value: value.into() })
}

/// Print the tree as CSS.
pub fn to_css(ast: &[ValueAstNode]) -> String {
    let mut css = String::new();
    write_css(ast, &mut css);
    css
}

/// Print the tree as CSS at the end of `css`.
pub fn write_css(ast: &[ValueAstNode], css: &mut String) {
    for node in ast {
        match node {
            ValueAstNode::Word(n) => css.push_str(&n.value),
            ValueAstNode::Separator(n) => css.push_str(&n.value),
            ValueAstNode::Function(n) => {
                css.push_str(&n.value);
                css.push('(');
                write_css(&n.nodes, css);
                css.push(')');
            }
        }
    }
}

const BACKSLASH: u8 = b'\\';
const CLOSE_PAREN: u8 = b')';
const COLON: u8 = b':';
const COMMA: u8 = b',';
const DOUBLE_QUOTE: u8 = b'"';
const EQUALS: u8 = b'=';
const GREATER_THAN: u8 = b'>';
const LESS_THAN: u8 = b'<';
const NEWLINE: u8 = b'\n';
const OPEN_PAREN: u8 = b'(';
const SINGLE_QUOTE: u8 = b'\'';
const SLASH: u8 = b'/';
const SPACE: u8 = b' ';
const TAB: u8 = b'\t';

#[inline]
fn is_separator(b: u8) -> bool {
    matches!(b, COLON | COMMA | EQUALS | GREATER_THAN | LESS_THAN | NEWLINE | SPACE | TAB)
}

/// `true` for each byte that has its own case in the parser.
const SPECIAL: [bool; 256] = {
    let mut table = [false; 256];
    let bytes = b"\\/:,=><\n \t'\"()";
    let mut i = 0;
    while i < bytes.len() {
        table[bytes[i] as usize] = true;
        i += 1;
    }
    table
};

/// Parse a CSS value.
pub fn parse(input: &str) -> Vec<ValueAstNode> {
    let input: Cow<'_, str> =
        if input.contains("\r\n") { Cow::Owned(input.replace("\r\n", "\n")) } else { Cow::Borrowed(input) };
    let input: &str = &input;
    let bytes = input.as_bytes();
    let len = bytes.len();

    let mut ast: Vec<ValueAstNode> = Vec::new();
    // The functions that are open. Upstream adds a function to its parent at
    // `(`; this parser adds it at `)`. No sibling comes between, so the tree
    // is the same.
    let mut stack: Vec<ValueFunctionNode> = Vec::new();
    let mut buffer = String::new();

    macro_rules! push {
        ($node:expr) => {{
            let node = $node;
            match stack.last_mut() {
                Some(parent) => parent.nodes.push(node),
                None => ast.push(node),
            }
        }};
    }

    let mut i = 0;
    while i < len {
        let current = bytes[i];
        match current {
            // Before 4.0.5, `\` was a normal character. (But `\)` with no
            // open function stopped the old parser with an exception: this
            // parser keeps the two characters there, as 4.0.5 does.)
            BACKSLASH
                if !crate::switches::VALUE_PARSER_ESCAPES.on()
                    && !(stack.is_empty() && bytes.get(i + 1) == Some(&CLOSE_PAREN)) =>
            {
                buffer.push('\\');
                i += 1;
            }

            // The next character is escaped: keep the two characters.
            BACKSLASH => {
                buffer.push('\\');
                match input[i + 1..].chars().next() {
                    Some(c) => {
                        buffer.push(c);
                        i += 1 + c.len_utf8();
                    }
                    None => {
                        // JavaScript adds `undefined` to the text here.
                        buffer.push_str("undefined");
                        i += 1;
                    }
                }
            }

            // `/` is a word of its own, for example `theme(colors.red.500/10)`.
            SLASH => {
                if !buffer.is_empty() {
                    push!(word(std::mem::take(&mut buffer)));
                }
                push!(word("/"));
                i += 1;
            }

            COLON | COMMA | EQUALS | GREATER_THAN | LESS_THAN | NEWLINE | SPACE | TAB => {
                if !buffer.is_empty() {
                    push!(word(std::mem::take(&mut buffer)));
                }
                let mut end = i + 1;
                while end < len && is_separator(bytes[end]) {
                    end += 1;
                }
                push!(separator(&input[i..end]));
                i = end;
            }

            // A string. It ends at the same quote. A string with no end is
            // only the quote.
            SINGLE_QUOTE | DOUBLE_QUOTE => {
                let mut end = i;
                let mut j = i + 1;
                while j < len {
                    let peek = bytes[j];
                    if peek == BACKSLASH {
                        j += 2;
                    } else if peek == current {
                        end = j;
                        break;
                    } else {
                        j += 1;
                    }
                }
                buffer.push_str(&input[i..=end]);
                i = end + 1;
            }

            OPEN_PAREN => {
                stack.push(ValueFunctionNode { value: std::mem::take(&mut buffer), nodes: Vec::new() });
                i += 1;
            }

            CLOSE_PAREN => {
                match stack.pop() {
                    Some(mut tail) => {
                        if !buffer.is_empty() {
                            tail.nodes.push(word(std::mem::take(&mut buffer)));
                        }
                        push!(ValueAstNode::Function(tail));
                    }
                    // A `)` with no `(`: upstream drops the text before it.
                    None => buffer.clear(),
                }
                i += 1;
            }

            _ => {
                let mut end = i + 1;
                while end < len && !SPECIAL[bytes[end] as usize] {
                    end += 1;
                }
                buffer.push_str(&input[i..end]);
                i = end;
            }
        }
    }

    // Close the functions that have no `)`.
    while let Some(node) = stack.pop() {
        push!(ValueAstNode::Function(node));
    }

    // Upstream adds the remainder to the root.
    if !buffer.is_empty() {
        ast.push(word(buffer));
    }

    ast
}
