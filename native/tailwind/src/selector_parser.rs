//! The parser of selectors: `selector-parser.ts` of Tailwind CSS.

use crate::walk::Walkable;
use std::borrow::Cow;

/// A selector list: `.a, .b`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct SelectorListNode {
    pub nodes: Vec<SelectorAstNode>,
}

/// A combinator. The usual values are ` `, `>`, `+` and `~`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct SelectorCombinatorNode {
    pub value: String,
}

/// A complex selector: compound selectors with combinators.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct SelectorComplexNode {
    pub nodes: Vec<SelectorAstNode>,
}

/// A compound selector: `.a.b:hover`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct SelectorCompoundNode {
    pub nodes: Vec<SelectorAstNode>,
}

/// A simple selector: `.a`, `#a`, `:hover`, `[a]`, `&`, `*`, `div`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct SelectorNode {
    pub value: String,
}

/// A function: `:not(…)`. `value` is the name with the colon.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct SelectorFunctionNode {
    pub value: String,
    pub nodes: Vec<SelectorAstNode>,
}

/// The text in a function that is not a selector.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Default)]
pub struct SelectorValueNode {
    pub value: String,
}

/// A node of the selector syntax tree.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum SelectorAstNode {
    Combinator(SelectorCombinatorNode),
    Complex(SelectorComplexNode),
    Compound(SelectorCompoundNode),
    Function(SelectorFunctionNode),
    List(SelectorListNode),
    Selector(SelectorNode),
    Value(SelectorValueNode),
}

impl SelectorAstNode {
    /// The upstream `kind` of the node.
    pub fn kind(&self) -> &'static str {
        match self {
            SelectorAstNode::Combinator(_) => "combinator",
            SelectorAstNode::Complex(_) => "complex",
            SelectorAstNode::Compound(_) => "compound",
            SelectorAstNode::Function(_) => "function",
            SelectorAstNode::List(_) => "list",
            SelectorAstNode::Selector(_) => "selector",
            SelectorAstNode::Value(_) => "value",
        }
    }

    /// The `value` of the node, when this kind of node has a value.
    pub fn value(&self) -> Option<&str> {
        match self {
            SelectorAstNode::Combinator(n) => Some(&n.value),
            SelectorAstNode::Function(n) => Some(&n.value),
            SelectorAstNode::Selector(n) => Some(&n.value),
            SelectorAstNode::Value(n) => Some(&n.value),
            _ => None,
        }
    }

    /// The `value` of the node, for a change.
    pub fn value_mut(&mut self) -> Option<&mut String> {
        match self {
            SelectorAstNode::Combinator(n) => Some(&mut n.value),
            SelectorAstNode::Function(n) => Some(&mut n.value),
            SelectorAstNode::Selector(n) => Some(&mut n.value),
            SelectorAstNode::Value(n) => Some(&mut n.value),
            _ => None,
        }
    }

    /// The first byte of the value of a `selector` node.
    #[inline]
    fn selector_start(&self) -> Option<Option<u8>> {
        match self {
            SelectorAstNode::Selector(n) => Some(n.value.as_bytes().first().copied()),
            _ => None,
        }
    }
}

impl Walkable for SelectorAstNode {
    fn nodes(&self) -> Option<&Vec<Self>> {
        match self {
            SelectorAstNode::Complex(n) => Some(&n.nodes),
            SelectorAstNode::Compound(n) => Some(&n.nodes),
            SelectorAstNode::Function(n) => Some(&n.nodes),
            SelectorAstNode::List(n) => Some(&n.nodes),
            _ => None,
        }
    }

    fn nodes_mut(&mut self) -> Option<&mut Vec<Self>> {
        match self {
            SelectorAstNode::Complex(n) => Some(&mut n.nodes),
            SelectorAstNode::Compound(n) => Some(&mut n.nodes),
            SelectorAstNode::Function(n) => Some(&mut n.nodes),
            SelectorAstNode::List(n) => Some(&mut n.nodes),
            _ => None,
        }
    }

    fn shallow(&self) -> Self {
        match self {
            SelectorAstNode::Complex(_) => complex(Vec::new()),
            SelectorAstNode::Compound(_) => compound(Vec::new()),
            SelectorAstNode::Function(n) => fun(n.value.as_str(), Vec::new()),
            SelectorAstNode::List(_) => list(Vec::new()),
            other => other.clone(),
        }
    }
}

/// A combinator node.
pub fn combinator(value: impl Into<String>) -> SelectorAstNode {
    SelectorAstNode::Combinator(SelectorCombinatorNode { value: value.into() })
}

/// A complex selector node.
pub fn complex(nodes: Vec<SelectorAstNode>) -> SelectorAstNode {
    SelectorAstNode::Complex(SelectorComplexNode { nodes })
}

/// A compound selector node.
pub fn compound(nodes: Vec<SelectorAstNode>) -> SelectorAstNode {
    SelectorAstNode::Compound(SelectorCompoundNode { nodes })
}

/// A function node.
pub fn fun(value: impl Into<String>, nodes: Vec<SelectorAstNode>) -> SelectorAstNode {
    SelectorAstNode::Function(SelectorFunctionNode { value: value.into(), nodes })
}

/// A selector list node.
pub fn list(nodes: Vec<SelectorAstNode>) -> SelectorAstNode {
    SelectorAstNode::List(SelectorListNode { nodes })
}

/// A simple selector node.
pub fn selector(value: impl Into<String>) -> SelectorAstNode {
    SelectorAstNode::Selector(SelectorNode { value: value.into() })
}

/// A value node.
pub fn value(value: impl Into<String>) -> SelectorAstNode {
    SelectorAstNode::Value(SelectorValueNode { value: value.into() })
}

/// `*`
pub fn is_universal_selector(node: &SelectorAstNode) -> bool {
    node.selector_start() == Some(Some(ASTERISK))
}

/// `&`
pub fn is_nesting_selector(node: &SelectorAstNode) -> bool {
    node.selector_start() == Some(Some(AMPERSAND))
}

/// `.foo`
pub fn is_class_selector(node: &SelectorAstNode) -> bool {
    node.selector_start() == Some(Some(DOT))
}

/// `#foo`
pub fn is_id_selector(node: &SelectorAstNode) -> bool {
    node.selector_start() == Some(Some(HASH))
}

/// `:hover` and `::before`
pub fn is_pseudo_selector(node: &SelectorAstNode) -> bool {
    node.selector_start() == Some(Some(COLON))
}

/// The pseudo-elements of CSS 1 and 2 that have one colon.
const LEGACY_PSEUDO_ELEMENTS: [&str; 4] = [":before", ":after", ":first-line", ":first-letter"];

/// `::before`, and the old pseudo-elements with one colon.
pub fn is_pseudo_element(node: &SelectorAstNode) -> bool {
    let value = match node {
        SelectorAstNode::Selector(n) => &n.value,
        SelectorAstNode::Function(n) => &n.value,
        _ => return false,
    };
    // No character outside ASCII has a lower case form in these names.
    value.starts_with("::") || LEGACY_PSEUDO_ELEMENTS.iter().any(|name| value.eq_ignore_ascii_case(name))
}

/// `[foo]`
pub fn is_attribute_selector(node: &SelectorAstNode) -> bool {
    node.selector_start() == Some(Some(OPEN_BRACKET))
}

/// `div`: a selector that is none of the other kinds.
pub fn is_type_selector(node: &SelectorAstNode) -> bool {
    match node.selector_start() {
        None => false,
        Some(Some(ASTERISK | AMPERSAND | DOT | HASH | COLON | OPEN_BRACKET)) => false,
        Some(_) => true,
    }
}

/// A deep copy of the node.
pub fn clone_ast_node(node: &SelectorAstNode) -> SelectorAstNode {
    node.clone()
}

/// Print the tree as CSS.
pub fn to_css(ast: &[SelectorAstNode], minify: bool) -> String {
    let mut css = String::new();
    write_css(ast, minify, &mut css);
    css
}

/// Print the tree as CSS at the end of `css`.
pub fn write_css(ast: &[SelectorAstNode], minify: bool, css: &mut String) {
    for node in ast {
        match node {
            SelectorAstNode::Selector(n) => css.push_str(&n.value),
            SelectorAstNode::Value(n) => css.push_str(&n.value),
            SelectorAstNode::Combinator(n) => {
                if minify || n.value == " " {
                    css.push_str(&n.value);
                } else {
                    css.push(' ');
                    css.push_str(&n.value);
                    css.push(' ');
                }
            }
            SelectorAstNode::Function(n) => {
                css.push_str(&n.value);
                css.push('(');
                write_css(&n.nodes, minify, css);
                css.push(')');
            }
            SelectorAstNode::Complex(n) => write_css(&n.nodes, minify, css),
            SelectorAstNode::Compound(n) => write_css(&n.nodes, minify, css),
            SelectorAstNode::List(n) => {
                for (i, item) in n.nodes.iter().enumerate() {
                    if i > 0 {
                        css.push_str(if minify { "," } else { ", " });
                    }
                    write_css(std::slice::from_ref(item), minify, css);
                }
            }
        }
    }
}

const BACKSLASH: u8 = b'\\';
const CLOSE_BRACKET: u8 = b']';
const CLOSE_PAREN: u8 = b')';
const COLON: u8 = b':';
const COMMA: u8 = b',';
const DOUBLE_QUOTE: u8 = b'"';
const DOT: u8 = b'.';
const GREATER_THAN: u8 = b'>';
const NEWLINE: u8 = b'\n';
const HASH: u8 = b'#';
const OPEN_BRACKET: u8 = b'[';
const OPEN_PAREN: u8 = b'(';
const PLUS: u8 = b'+';
const SINGLE_QUOTE: u8 = b'\'';
const SPACE: u8 = b' ';
const TAB: u8 = b'\t';
const TILDE: u8 = b'~';
const AMPERSAND: u8 = b'&';
const ASTERISK: u8 = b'*';

#[inline]
fn is_combinator_char(b: u8) -> bool {
    matches!(b, GREATER_THAN | NEWLINE | SPACE | PLUS | TAB | TILDE)
}

/// `true` for each byte that has its own case in the parser.
const SPECIAL: [bool; 256] = {
    let mut table = [false; 256];
    let bytes = b"\\,>\n +\t~():.#['\"&*";
    let mut i = 0;
    while i < bytes.len() {
        table[bytes[i] as usize] = true;
        i += 1;
    }
    table
};

/// The state of one level of the parser: the root or an open function.
#[derive(Default)]
struct Frame {
    /// The name of the function. The root has no name.
    name: String,
    /// The nodes of the selector at the cursor.
    target: Vec<SelectorAstNode>,
    /// The items of the selector list, after the first comma.
    list: Option<Vec<SelectorAstNode>>,
    contains_combinator: bool,
}

impl Frame {
    /// The nodes of the level when the level ends correctly.
    fn finish(self) -> Vec<SelectorAstNode> {
        match self.list {
            Some(mut items) => {
                items.push(current(self.target, self.contains_combinator));
                vec![list(items)]
            }
            None if self.contains_combinator => vec![complex(self.target)],
            None => self.target,
        }
    }
}

/// One node for the nodes of a selector.
fn current(mut nodes: Vec<SelectorAstNode>, contains_combinator: bool) -> SelectorAstNode {
    if nodes.len() == 1 {
        nodes.pop().unwrap_or_else(|| compound(Vec::new()))
    } else if contains_combinator {
        complex(nodes)
    } else {
        compound(nodes)
    }
}

/// Add a node. Two nodes with no combinator between make a compound.
fn append(target: &mut Vec<SelectorAstNode>, node: SelectorAstNode) {
    match target.last_mut() {
        Some(SelectorAstNode::Compound(existing)) => existing.nodes.push(node),
        None | Some(SelectorAstNode::List(_)) | Some(SelectorAstNode::Combinator(_)) => target.push(node),
        Some(_) => {
            if let Some(existing) = target.pop() {
                target.push(compound(vec![existing, node]));
            }
        }
    }
}

/// Parse a selector.
pub fn parse(input: &str) -> Vec<SelectorAstNode> {
    let input: Cow<'_, str> = if input.contains("\r\n") {
        Cow::Owned(input.replace("\r\n", "\n"))
    } else {
        Cow::Borrowed(input)
    };
    parse_normalized(&input)
}

fn parse_normalized(input: &str) -> Vec<SelectorAstNode> {
    let bytes = input.as_bytes();
    let len = bytes.len();

    // Upstream adds a function to its parent at `(`; this parser adds it at
    // `)`. The parent does not change between, so the tree is the same.
    let mut stack: Vec<Frame> = Vec::new();
    let mut frame = Frame::default();
    let mut buffer = String::new();

    macro_rules! flush {
        () => {
            if !buffer.is_empty() {
                append(&mut frame.target, selector(std::mem::take(&mut buffer)));
            }
        };
    }

    let mut i = 0;
    while i < len {
        let current_char = bytes[i];
        match current_char {
            // A selector list: `.foo, .bar`
            COMMA => {
                flush!();
                i += 1;
                while i < len && matches!(bytes[i], NEWLINE | SPACE | TAB) {
                    i += 1;
                }
                let nodes = std::mem::take(&mut frame.target);
                let item = current(nodes, frame.contains_combinator);
                match &mut frame.list {
                    Some(items) => items.push(item),
                    None => frame.list = Some(vec![item]),
                }
                frame.contains_combinator = false;
            }

            // A combinator: `.foo .bar`, `.foo > .bar`
            GREATER_THAN | NEWLINE | SPACE | PLUS | TAB | TILDE => {
                flush!();
                let mut end = i + 1;
                while end < len && is_combinator_char(bytes[end]) {
                    end += 1;
                }
                let value = input[i..end].trim_matches(|c| c == ' ' || c == '\n' || c == '\t');
                i = end;
                if value.is_empty() && (frame.target.is_empty() || end >= len || bytes[end] == COMMA) {
                    continue;
                }
                frame.target.push(combinator(if value.is_empty() { " " } else { value }));
                frame.contains_combinator = true;
            }

            // A function: `.foo:not(.bar)`
            OPEN_PAREN => {
                let name = std::mem::take(&mut buffer);
                if matches!(name.as_str(), ":not" | ":where" | ":has" | ":is" | ":-moz-any" | ":-webkit-any") {
                    stack.push(std::mem::replace(&mut frame, Frame { name, ..Frame::default() }));
                    i += 1;
                    continue;
                }

                // The contents of each other function are one value node.
                let start = i + 1;
                let mut nesting = 0usize;
                let mut close = None;
                for (j, &peek) in bytes.iter().enumerate().skip(start) {
                    if peek == OPEN_PAREN {
                        nesting += 1;
                    } else if peek == CLOSE_PAREN {
                        if nesting == 0 {
                            close = Some(j);
                            break;
                        }
                        nesting -= 1;
                    }
                }
                // With no `)`, upstream has empty contents and goes on after
                // the `(`.
                let (contents, next) = match close {
                    Some(j) => (&input[start..j], j + 1),
                    None => ("", i + 1),
                };
                i = next;

                let mut nodes = Vec::new();
                // `:nth-child(2n + 1 of .foo, .bar)`: the part after `of` is
                // a selector list.
                let of = if name == ":nth-child" || name == ":nth-last-child" { contents.find("of ") } else { None };
                match of {
                    Some(idx) => {
                        nodes.push(value(&contents[..idx + 3]));
                        nodes.extend(parse_normalized(&contents[idx + 3..]));
                    }
                    None => nodes.push(value(contents)),
                }
                append(&mut frame.target, fun(name, nodes));
            }

            // The end of a function
            CLOSE_PAREN => {
                flush!();
                i += 1;
                match stack.pop() {
                    Some(parent) => {
                        let mut done = std::mem::replace(&mut frame, parent);
                        let name = std::mem::take(&mut done.name);
                        append(&mut frame.target, fun(name, done.finish()));
                    }
                    // A `)` with no `(`: the root starts again with its nodes.
                    None => {
                        let done = std::mem::take(&mut frame);
                        frame.target = done.finish();
                    }
                }
            }

            // A compound selector: `.foo.bar`
            DOT | COLON | HASH => {
                if current_char == COLON && buffer == ":" {
                    buffer.push(':');
                } else {
                    flush!();
                    buffer.push(current_char as char);
                }
                i += 1;
            }

            // An attribute selector, to the `]` at the same level
            OPEN_BRACKET => {
                flush!();
                let mut end = i;
                let mut nesting = 0usize;
                for (j, &peek) in bytes.iter().enumerate().skip(i + 1) {
                    if peek == OPEN_BRACKET {
                        nesting += 1;
                    } else if peek == CLOSE_BRACKET {
                        if nesting == 0 {
                            end = j;
                            break;
                        }
                        nesting -= 1;
                    }
                }
                append(&mut frame.target, selector(&input[i..=end]));
                i = end + 1;
            }

            // A string. It ends at the same quote.
            SINGLE_QUOTE | DOUBLE_QUOTE => {
                let mut end = i;
                let mut j = i + 1;
                while j < len {
                    let peek = bytes[j];
                    if peek == BACKSLASH {
                        j += 2;
                    } else if peek == current_char {
                        end = j;
                        break;
                    } else {
                        j += 1;
                    }
                }
                buffer.push_str(&input[i..=end]);
                i = end + 1;
            }

            // `&` and `*` are selectors of their own.
            AMPERSAND | ASTERISK => {
                flush!();
                append(&mut frame.target, selector(if current_char == AMPERSAND { "&" } else { "*" }));
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

    flush!();

    // Only the level at the cursor ends correctly.
    let mut name = std::mem::take(&mut frame.name);
    let mut nodes = frame.finish();

    // A function with no `)`: upstream leaves each outer level as it is.
    // An outer level with a list loses the selector that is not complete.
    while let Some(mut parent) = stack.pop() {
        append(&mut parent.target, fun(name, nodes));
        name = std::mem::take(&mut parent.name);
        nodes = match parent.list {
            Some(items) => vec![list(items)],
            None => parent.target,
        };
    }

    nodes
}
