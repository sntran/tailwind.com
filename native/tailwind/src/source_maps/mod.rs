//! Source maps: `source-maps/` of Tailwind CSS.
//!
//! All offsets and columns are numbers of UTF-16 code units, as upstream
//! has them, so the maps have the same numbers.

pub mod line_table;
pub mod source;
pub mod source_map;

pub use line_table::{LineTable, Position};
pub use source::{utf16_len, Locations, Source, SourceLocation};
pub use source_map::{
    create_source_map, create_translation_map, DecodedMapping, DecodedSource, DecodedSourceMap, OriginalPosition,
};
