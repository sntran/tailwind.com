//! The decoded source map of a tree: `source-maps/source-map.ts` of
//! Tailwind CSS.

use super::line_table::{LineTable, Position};
use super::source::Source;
use crate::ast::AstNode;
use std::rc::Rc;

/// A source of a decoded source map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedSource {
    pub url: Option<String>,
    pub content: Option<String>,
    pub ignore: bool,
}

/// A position in a source. `source` is an index into the sources of the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OriginalPosition {
    pub source: usize,
    pub line: usize,
    pub column: usize,
}

/// One mapping of a decoded source map.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DecodedMapping {
    pub original_position: Option<OriginalPosition>,
    pub generated_position: Position,
    pub name: Option<String>,
}

/// A decoded source map (ECMA-426).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DecodedSourceMap {
    pub file: Option<String>,
    pub sources: Vec<DecodedSource>,
    pub mappings: Vec<DecodedMapping>,
}

/// The data of one source that the builder keeps.
struct SourceEntry {
    source: Rc<Source>,
    /// The text as UTF-16 code units: the offsets are indices into it.
    units: Vec<u16>,
    table: LineTable,
}

#[derive(Default)]
struct Builder {
    /// The sources that have a line table, in the sequence of first use.
    entries: Vec<SourceEntry>,
    mappings: Vec<DecodedMapping>,
}

/// True for a UTF-16 code unit that `String.prototype.trim` removes.
fn is_js_whitespace_unit(unit: u16) -> bool {
    char::from_u32(u32::from(unit)).is_some_and(crate::utils::js::is_js_whitespace)
}

impl Builder {
    fn entry(&mut self, source: &Rc<Source>) -> usize {
        if let Some(index) = self.entries.iter().position(|entry| Rc::ptr_eq(&entry.source, source)) {
            return index;
        }
        let units: Vec<u16> = source.code.encode_utf16().collect();
        let table = LineTable::from_utf16(&units);
        self.entries.push(SourceEntry { source: Rc::clone(source), units, table });
        self.entries.len() - 1
    }

    fn visit(&mut self, node: &AstNode) {
        self.add(node);
        if let Some(children) = node.children() {
            for child in children {
                self.visit(child);
            }
        }
    }

    fn add(&mut self, node: &AstNode) {
        let (Some(src), Some(dst)) = (node.src(), node.dst()) else { return };
        if src.source.code.is_empty() {
            return;
        }

        let original = self.entry(&src.source);
        let generated = self.entry(&dst.source);

        let generated_start = self.entries[generated].table.find(dst.start);
        let generated_end = self.entries[generated].table.find(dst.end);

        let entry = &self.entries[original];
        let len = entry.units.len();
        let slice_start = src.start.min(len);
        let slice_end = src.end.min(len).max(slice_start);

        // A source map has no ranges. Each line of the source that is not
        // empty gets a mapping to the start of the node in the output.
        let mut offset = 0;
        for line in entry.units[slice_start..slice_end].split(|unit| *unit == 0x0a) {
            if !line.iter().all(|unit| is_js_whitespace_unit(*unit)) {
                let start = entry.table.find(src.start + offset);
                self.mappings.push(DecodedMapping {
                    name: None,
                    original_position: Some(OriginalPosition { source: original, line: start.line, column: start.column }),
                    generated_position: generated_start,
                });
            }
            offset += line.len() + 1;
        }

        let end = entry.table.find(src.end);
        self.mappings.push(DecodedMapping {
            name: None,
            original_position: Some(OriginalPosition { source: original, line: end.line, column: end.column }),
            generated_position: generated_end,
        });
    }
}

/// The source map of a printed tree. Each node needs its `src`, and its
/// `dst` from the printer with locations.
pub fn create_source_map(ast: &[AstNode]) -> DecodedSourceMap {
    let mut builder = Builder::default();
    for node in ast {
        builder.visit(node);
    }

    let sources = builder
        .entries
        .iter()
        .map(|entry| DecodedSource {
            url: entry.source.file.clone(),
            content: Some(entry.source.code.clone()),
            ignore: false,
        })
        .collect();

    // The sort keeps the sequence of equal mappings, as upstream.
    let mut mappings = builder.mappings;
    mappings.sort_by_key(|mapping| {
        let original = mapping.original_position;
        (
            mapping.generated_position.line,
            mapping.generated_position.column,
            original.map_or(0, |position| position.line),
            original.map_or(0, |position| position.column),
        )
    });

    DecodedSourceMap { file: None, sources, mappings }
}

/// The positions of a node in its source and in the output:
/// `(original start, original end, generated start, generated end)`.
pub type Translation = (Position, Position, Option<Position>, Option<Position>);

/// The translation of the offsets of a node to positions.
pub struct TranslationMap {
    original: LineTable,
    generated: LineTable,
}

impl TranslationMap {
    /// The translations of one node. A node with no `src` has none.
    pub fn translate(&self, node: &AstNode) -> Vec<Translation> {
        let Some(src) = node.src() else { return Vec::new() };
        let dst = node.dst();
        vec![(
            self.original.find(src.start),
            self.original.find(src.end),
            dst.map(|dst| self.generated.find(dst.start)),
            dst.map(|dst| self.generated.find(dst.end)),
        )]
    }
}

/// The translation map of an original text and a generated text.
pub fn create_translation_map(original: &str, generated: &str) -> TranslationMap {
    TranslationMap { original: LineTable::new(original), generated: LineTable::new(generated) }
}
