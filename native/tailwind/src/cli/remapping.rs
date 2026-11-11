//! A text replacement that keeps a source map correct: the parts of the
//! packages `magic-string` (`replaceAll`, and `generateMap` with
//! `hires: 'boundary'`) and `@jridgewell/remapping` that `optimize.ts` of
//! `@tailwindcss/node` uses.
//!
//! Upstream makes a source map of the replacement (from the new text to
//! the old text), and then traces each of its segments through the source
//! map of the old text. This file does the two steps in one pass, with
//! the same result.
//!
//! All columns are numbers of UTF-16 code units, as in JavaScript.

use super::source_maps::{json_string, vlq};

/// A segment of a line of a source map: the generated column, and the
/// source, the line and the column when the segment has a source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Segment {
    column: i64,
    original: Option<(i64, i64, i64)>,
    name: Option<i64>,
}

/// The decoded form of the `mappings` field: `decode` of
/// `@jridgewell/sourcemap-codec`. Each line is sorted by column.
fn decode(mappings: &str) -> Vec<Vec<Segment>> {
    fn digit(byte: u8) -> Option<i64> {
        Some(match byte {
            b'A'..=b'Z' => byte - b'A',
            b'a'..=b'z' => byte - b'a' + 26,
            b'0'..=b'9' => byte - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as i64)
    }

    let mut lines = Vec::new();
    let (mut source, mut line, mut column, mut name) = (0i64, 0i64, 0i64, 0i64);
    for group in mappings.split(';') {
        let mut segments: Vec<Segment> = Vec::new();
        let mut generated_column = 0i64;
        for text in group.split(',').filter(|text| !text.is_empty()) {
            let mut fields = [0i64; 5];
            let mut count = 0;
            let (mut value, mut shift) = (0i64, 0u32);
            for byte in text.bytes() {
                let Some(digit) = digit(byte) else { break };
                value |= (digit & 31) << shift;
                if digit & 32 != 0 {
                    shift += 5;
                } else {
                    if count < 5 {
                        fields[count] = if value & 1 == 1 { -(value >> 1) } else { value >> 1 };
                        count += 1;
                    }
                    value = 0;
                    shift = 0;
                }
            }
            if count == 0 {
                continue;
            }
            generated_column += fields[0];
            let mut segment = Segment { column: generated_column, original: None, name: None };
            if count >= 4 {
                source += fields[1];
                line += fields[2];
                column += fields[3];
                segment.original = Some((source, line, column));
                if count == 5 {
                    name += fields[4];
                    segment.name = Some(name);
                }
            }
            segments.push(segment);
        }
        // The sort keeps the sequence of the segments of one column.
        segments.sort_by_key(|segment| segment.column);
        lines.push(segments);
    }
    lines
}

/// The segment of a position: `traceSegment` of
/// `@jridgewell/trace-mapping`. It is the last segment that is not after
/// the column. Of the segments with that column, it is the first.
fn trace_segment(lines: &[Vec<Segment>], line: usize, column: i64) -> Option<&Segment> {
    let segments = lines.get(line)?;
    let after = segments.partition_point(|segment| segment.column <= column);
    if after == 0 {
        return None;
    }
    let found = segments[after - 1].column;
    let first = segments.partition_point(|segment| segment.column < found);
    Some(&segments[first])
}

/// `resolve(input, '')` of `@jridgewell/resolve-uri` for a path: the path
/// without `.` parts, and without `..` parts where that is possible. A
/// URL with a scheme stays as it is.
fn resolve_source(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }
    let scheme_end = input.find("://");
    let has_scheme = scheme_end.is_some_and(|end| {
        end > 0 && input[..end].chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '.' | '-'))
    });
    if has_scheme || input.starts_with("//") || input.starts_with("file:") {
        return input.to_string();
    }

    let absolute = input.starts_with('/');
    let split = input.find(['?', '#']).unwrap_or(input.len());
    let (path, query_hash) = input.split_at(split);
    if path.is_empty() {
        return query_hash.to_string();
    }

    let mut pieces: Vec<&str> = Vec::new();
    let mut positive = 0;
    let mut add_trailing_slash = false;
    for piece in path.split('/').skip(usize::from(absolute)) {
        if piece.is_empty() {
            add_trailing_slash = true;
            continue;
        }
        add_trailing_slash = false;
        match piece {
            "." => {}
            ".." => {
                if positive > 0 {
                    add_trailing_slash = true;
                    positive -= 1;
                    pieces.pop();
                } else if !absolute {
                    pieces.push(piece);
                }
            }
            _ => {
                pieces.push(piece);
                positive += 1;
            }
        }
    }
    let mut normalized: String = pieces.iter().map(|piece| format!("/{piece}")).collect();
    if normalized.is_empty() || (add_trailing_slash && !normalized.ends_with("/..")) {
        normalized.push('/');
    }

    if absolute {
        return format!("{normalized}{query_hash}");
    }
    let relative = &normalized[1..];
    if relative.is_empty() {
        return if query_hash.is_empty() { ".".to_string() } else { query_hash.to_string() };
    }
    if input.starts_with('.') && !relative.starts_with('.') {
        return format!("./{relative}{query_hash}");
    }
    format!("{relative}{query_hash}")
}

/// The result of `replace_all`.
pub struct Replaced {
    pub code: String,
    /// The new source map as JSON text. `None`: the text has no match,
    /// and the map stays as it is.
    pub map: Option<String>,
}

/// The sources and the mappings of the new map: `GenMapping` of
/// `@jridgewell/gen-mapping`, with `maybeAddSegment`.
#[derive(Default)]
struct Generator {
    lines: Vec<Vec<Segment>>,
    sources: Vec<String>,
    contents: Vec<Option<String>>,
    names: Vec<String>,
}

impl Generator {
    fn index(list: &mut Vec<String>, value: &str) -> usize {
        match list.iter().position(|known| known == value) {
            Some(index) => index,
            None => {
                list.push(value.to_string());
                list.len() - 1
            }
        }
    }

    /// Add a segment, unless it says the same as the segment before it.
    fn maybe_add(&mut self, line: usize, column: i64, traced: Option<(&str, i64, i64, Option<&str>)>) {
        while self.lines.len() <= line {
            self.lines.push(Vec::new());
        }
        // The segments come in the sequence of their columns.
        match traced {
            None => {
                let segments = &mut self.lines[line];
                if segments.last().is_none_or(|previous| previous.original.is_none()) {
                    return;
                }
                segments.push(Segment { column, original: None, name: None });
            }
            Some((source, source_line, source_column, name)) => {
                let source_index = Self::index(&mut self.sources, source) as i64;
                if self.contents.len() < self.sources.len() {
                    self.contents.push(None);
                }
                let name = name.filter(|name| !name.is_empty()).map(|name| Self::index(&mut self.names, name) as i64);
                let segment =
                    Segment { column, original: Some((source_index, source_line, source_column)), name };
                let segments = &mut self.lines[line];
                if let Some(previous) = segments.last() {
                    if previous.original == segment.original && previous.name == segment.name {
                        return;
                    }
                }
                segments.push(segment);
            }
        }
    }

    /// The JSON text: `toEncodedMap`, and `SourceMap.toString()` of
    /// `@jridgewell/remapping`.
    fn into_json(mut self) -> String {
        while self.lines.last().is_some_and(Vec::is_empty) {
            self.lines.pop();
        }

        let mut out = String::from("{\"version\":3,\"mappings\":\"");
        let (mut source, mut line, mut column, mut name) = (0i64, 0i64, 0i64, 0i64);
        for (index, segments) in self.lines.iter().enumerate() {
            if index > 0 {
                out.push(';');
            }
            let mut generated_column = 0i64;
            for (j, segment) in segments.iter().enumerate() {
                if j > 0 {
                    out.push(',');
                }
                vlq(segment.column - generated_column, &mut out);
                generated_column = segment.column;
                if let Some(original) = segment.original {
                    vlq(original.0 - source, &mut out);
                    source = original.0;
                    vlq(original.1 - line, &mut out);
                    line = original.1;
                    vlq(original.2 - column, &mut out);
                    column = original.2;
                    if let Some(index) = segment.name {
                        vlq(index - name, &mut out);
                        name = index;
                    }
                }
            }
        }
        out.push_str("\",\"names\":[");
        for (index, name) in self.names.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            json_string(name, &mut out);
        }
        out.push_str("],\"ignoreList\":[],\"sources\":[");
        for (index, source) in self.sources.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            json_string(source, &mut out);
        }
        out.push_str("],\"sourcesContent\":[");
        for (index, content) in self.contents.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            match content {
                Some(content) => json_string(content, &mut out),
                None => out.push_str("null"),
            }
        }
        out.push_str("]}");
        out
    }
}

/// The parts of a source map as JSON text that the trace uses.
struct InputMap {
    lines: Vec<Vec<Segment>>,
    sources: Vec<String>,
    contents: Vec<Option<String>>,
    names: Vec<String>,
}

fn parse_map(map: &str) -> Option<InputMap> {
    let json: serde_json::Value = serde_json::from_str(map).ok()?;
    let strings = |key: &str| -> Vec<Option<String>> {
        json.get(key)
            .and_then(|value| value.as_array())
            .map(|list| list.iter().map(|item| item.as_str().map(String::from)).collect())
            .unwrap_or_default()
    };
    let root = json.get("sourceRoot").and_then(|root| root.as_str()).filter(|root| !root.is_empty());
    let sources = strings("sources")
        .into_iter()
        .map(|source| {
            let source = source.unwrap_or_default();
            resolve_source(&match root {
                Some(root) => format!("{root}/{source}"),
                None => source,
            })
        })
        .collect();
    Some(InputMap {
        lines: decode(json.get("mappings")?.as_str()?),
        sources,
        contents: strings("sourcesContent"),
        names: strings("names").into_iter().map(Option::unwrap_or_default).collect(),
    })
}

fn is_word_code(unit: u16) -> bool {
    matches!(unit, 0x61..=0x7a | 0x41..=0x5a | 0x30..=0x39 | 0x5f)
}

/// Replace each `search` in `code` with `replacement`, and give the source
/// map of the new text. `map` is the source map of `code` as JSON text.
/// `search` and `replacement` are ASCII texts with no line break.
pub fn replace_all(code: &str, search: &str, replacement: &str, map: Option<&str>) -> Replaced {
    if !code.contains(search) {
        return Replaced { code: code.to_string(), map: None };
    }
    let new_code = code.replace(search, replacement);
    let Some(input) = map.and_then(parse_map) else {
        return Replaced { code: new_code, map: None };
    };

    let mut generator = Generator::default();
    // The position in the old text, and the column in the new text. The
    // replacement has no line break, so the line is the same in the two.
    let (mut line, mut column, mut generated_column) = (0usize, 0i64, 0i64);

    // Trace one segment of the map of the replacement.
    let add = |generator: &mut Generator, line: usize, generated_column: i64, column: i64| {
        let Some(segment) = trace_segment(&input.lines, line, column) else { return };
        match segment.original {
            None => generator.maybe_add(line, generated_column, None),
            Some((source, source_line, source_column)) => {
                let index = source as usize;
                let url = input.sources.get(index).map_or("", String::as_str);
                // An empty source name is a segment with no source.
                if url.is_empty() {
                    generator.maybe_add(line, generated_column, None);
                    return;
                }
                let name = segment.name.and_then(|name| input.names.get(name as usize)).map(String::as_str);
                generator.maybe_add(line, generated_column, Some((url, source_line, source_column, name)));
                if let Some(Some(content)) = input.contents.get(index) {
                    let at = generator.sources.iter().position(|known| known == url);
                    if let Some(at) = at {
                        generator.contents[at] = Some(content.clone());
                    }
                }
            }
        }
    };

    let mut rest = code;
    loop {
        let (chunk, found) = match rest.find(search) {
            Some(at) => (&rest[..at], true),
            None => (rest, false),
        };

        // A part with no change: one segment at the start of each word,
        // and one at each character that is not a part of a word.
        let mut in_word = false;
        for unit in chunk.encode_utf16() {
            if unit == 0x0a {
                line += 1;
                column = 0;
                generated_column = 0;
                in_word = false;
                continue;
            }
            if is_word_code(unit) {
                if !in_word {
                    add(&mut generator, line, generated_column, column);
                    in_word = true;
                }
            } else {
                add(&mut generator, line, generated_column, column);
                in_word = false;
            }
            column += 1;
            generated_column += 1;
        }

        if !found {
            break;
        }
        // The replacement: one segment at its start.
        add(&mut generator, line, generated_column, column);
        column += search.len() as i64;
        generated_column += replacement.len() as i64;
        rest = &rest[chunk.len() + search.len()..];
    }

    Replaced { code: new_code, map: Some(generator.into_json()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_match_keeps_the_map() {
        let replaced = replace_all("a{b:c}", "@media not (", "@media not all and (", Some("{}"));
        assert_eq!(replaced.code, "a{b:c}");
        assert!(replaced.map.is_none());
    }

    #[test]
    fn decode_reads_the_segments_of_each_line() {
        let lines = decode(";AAAA,CAAC;;EAAE");
        assert_eq!(lines.len(), 4);
        assert!(lines[0].is_empty());
        assert_eq!(lines[1][1], Segment { column: 1, original: Some((0, 0, 1)), name: None });
        assert_eq!(lines[3][0], Segment { column: 2, original: Some((0, 0, 3)), name: None });
    }

    #[test]
    fn trace_gives_the_first_segment_of_the_last_column() {
        let segment = |column, source_column| Segment { column, original: Some((0, 0, source_column)), name: None };
        let lines = vec![vec![segment(2, 1), segment(5, 2), segment(5, 3), segment(9, 4)]];
        assert_eq!(trace_segment(&lines, 0, 1), None);
        assert_eq!(trace_segment(&lines, 0, 2), Some(&segment(2, 1)));
        assert_eq!(trace_segment(&lines, 0, 7), Some(&segment(5, 2)));
        assert_eq!(trace_segment(&lines, 0, 100), Some(&segment(9, 4)));
        assert_eq!(trace_segment(&lines, 1, 0), None);
    }

    #[test]
    fn resolve_source_removes_dot_parts() {
        assert_eq!(resolve_source("a/b.css"), "a/b.css");
        assert_eq!(resolve_source("a/./c/../b.css"), "a/b.css");
        assert_eq!(resolve_source("./a.css"), "./a.css");
        assert_eq!(resolve_source("../a.css"), "../a.css");
        assert_eq!(resolve_source("/a//b/../c.css"), "/a/c.css");
        assert_eq!(resolve_source("https://example.com/a/../b.css"), "https://example.com/a/../b.css");
        assert_eq!(resolve_source(""), "");
    }
}
