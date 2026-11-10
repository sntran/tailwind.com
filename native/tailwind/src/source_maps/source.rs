//! The source of a node: `source-maps/source.ts` of Tailwind CSS.

use std::rc::Rc;

/// The text of one stylesheet. The text of the output has no file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub file: Option<String>,
    pub code: String,
}

/// A range in a source. The offsets are numbers of UTF-16 code units.
///
/// Two locations are equal when they are in the same `Source` object and
/// have the same offsets.
#[derive(Debug, Clone)]
pub struct SourceLocation {
    pub source: Rc<Source>,
    pub start: usize,
    pub end: usize,
}

impl PartialEq for SourceLocation {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.source, &other.source) && self.start == other.start && self.end == other.end
    }
}

impl Eq for SourceLocation {}

/// The two locations of a node: `src` is the text that made the node, and
/// `dst` is the text of the node in the output.
#[derive(Debug, Clone, Default)]
pub struct Locations {
    pub src: Option<SourceLocation>,
    pub dst: Option<SourceLocation>,
    /// The identity of a node that upstream has at more than one place of
    /// the tree (one JavaScript object). The copies of the node here have
    /// the same identity, and the printer gives them the same `dst`.
    pub identity: Option<Rc<()>>,
}

/// The length of a string of JavaScript: the number of UTF-16 code units.
#[inline]
pub fn utf16_len(text: &str) -> usize {
    if text.is_ascii() {
        text.len()
    } else {
        text.encode_utf16().count()
    }
}
