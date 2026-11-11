//! Offsets to lines and columns: `source-maps/line-table.ts` of Tailwind CSS.

/// A position in a text. The line starts at one. The column starts at zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct Position {
    pub line: usize,
    pub column: usize,
}

/// The offset of the start of each line of a text, in UTF-16 code units.
#[derive(Debug, Clone)]
pub struct LineTable {
    table: Vec<usize>,
}

impl LineTable {
    /// Make the table of a text.
    pub fn new(source: &str) -> Self {
        let mut table = vec![0];
        if source.is_ascii() {
            for (i, byte) in source.bytes().enumerate() {
                if byte == b'\n' {
                    table.push(i + 1);
                }
            }
        } else {
            let mut offset = 0;
            for c in source.chars() {
                offset += c.len_utf16();
                if c == '\n' {
                    table.push(offset);
                }
            }
        }
        LineTable { table }
    }

    /// Make the table of a text that is a list of UTF-16 code units.
    pub fn from_utf16(source: &[u16]) -> Self {
        let mut table = vec![0];
        for (i, unit) in source.iter().enumerate() {
            if *unit == 0x0a {
                table.push(i + 1);
            }
        }
        LineTable { table }
    }

    /// The position of an offset.
    pub fn find(&self, offset: usize) -> Position {
        // The number of line starts that are not after the offset. The
        // first line start is 0, so the result is 1 or more.
        let line = self.table.partition_point(|start| *start <= offset) - 1;
        Position { line: line + 1, column: offset - self.table[line] }
    }

    /// The offset of a position. A position that is not in the text gives
    /// the nearest offset.
    pub fn find_offset(&self, line: i64, column: i64) -> usize {
        let last = self.table.len() as i64 - 1;
        let line = (line - 1).clamp(0, last) as usize;
        let offset_a = self.table[line] as i64;
        let offset_b = self.table.get(line + 1).map_or(offset_a, |offset| *offset as i64);
        (offset_a + column).max(0).min(offset_b) as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEXT: &str = ".foo {\n  color: red;\n}\n";

    // `line tables` of `line-table.test.ts`.
    #[test]
    fn line_tables() {
        let table = LineTable::new(TEXT);
        let pos = |line, column| Position { line, column };

        // Line 1: `.foo {\n`
        for i in 0..=6 {
            assert_eq!(table.find(i), pos(1, i));
        }
        // Line 2: `  color: red;\n`
        for i in 1..=13 {
            assert_eq!(table.find(6 + i), pos(2, i - 1));
        }
        // Line 3: `}\n`
        assert_eq!(table.find(20 + 1), pos(3, 0));
        assert_eq!(table.find(20 + 2), pos(3, 1));
        // After the last line break.
        assert_eq!(table.find(22 + 1), pos(4, 0));
    }

    // `line tables findOffset` of `line-table.test.ts`.
    #[test]
    fn line_tables_find_offset() {
        let table = LineTable::new(TEXT);

        for i in 0..=6 {
            assert_eq!(table.find_offset(1, i), i as usize);
        }
        for i in 1..=13 {
            assert_eq!(table.find_offset(2, i - 1), 6 + i as usize);
        }
        assert_eq!(table.find_offset(3, 0), 20 + 1);
        assert_eq!(table.find_offset(3, 1), 20 + 2);
        assert_eq!(table.find_offset(4, 0), 22 + 1);
    }

    #[test]
    fn find_offset_keeps_the_result_in_the_text() {
        let table = LineTable::new(TEXT);
        assert_eq!(table.find_offset(0, 0), 0);
        assert_eq!(table.find_offset(-3, -4), 0);
        assert_eq!(table.find_offset(1, 100), 7);
        assert_eq!(table.find_offset(99, 0), 23);
        assert_eq!(table.find_offset(99, 5), 23);
    }

    #[test]
    fn the_columns_are_utf16_code_units() {
        let table = LineTable::new("a\u{1F600}b\n\u{e9}c");
        assert_eq!(table.find(4), Position { line: 1, column: 4 });
        assert_eq!(table.find(5), Position { line: 2, column: 0 });
        assert_eq!(table.find(7), Position { line: 2, column: 2 });
        let units: Vec<u16> = "a\u{1F600}b\n\u{e9}c".encode_utf16().collect();
        assert_eq!(LineTable::from_utf16(&units).find(6), Position { line: 2, column: 1 });
    }
}
