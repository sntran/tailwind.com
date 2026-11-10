//! The text of a source map: `source-maps.ts` of `@tailwindcss/node`, with
//! the part of the package `source-map-js` that it uses
//! (`SourceMapGenerator`).

use crate::source_maps::DecodedSourceMap;

/// A source map as text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceMap {
    /// The JSON text of the map.
    pub raw: String,
}

impl SourceMap {
    /// The comment that holds the map as a data URL.
    pub fn inline(&self) -> String {
        comment(&format!("data:application/json;base64,{}", base64(self.raw.as_bytes())))
    }

    /// The comment that gives the URL of the map.
    pub fn comment(&self, url: &str) -> String {
        comment(url)
    }
}

/// The comment that gives the URL of a source map.
pub fn comment(url: &str) -> String {
    format!("/*# sourceMappingURL={url} */\n")
}

/// The source map of the compiler as text (`toSourceMap` with a map).
pub fn to_source_map(map: &DecodedSourceMap) -> SourceMap {
    SourceMap { raw: serialize_source_map(map) }
}

/// A source map that is text already (`toSourceMap` with a string).
pub fn raw_source_map(raw: impl Into<String>) -> SourceMap {
    SourceMap { raw: raw.into() }
}

/// One mapping of the generator.
struct Mapping {
    generated_line: usize,
    generated_column: usize,
    /// The source, the line (it starts at one) and the column.
    original: Option<(usize, usize, usize)>,
}

/// The JSON text of a decoded source map, as `SourceMapGenerator` of
/// `source-map-js` writes it.
pub fn serialize_source_map(map: &DecodedSourceMap) -> String {
    // The URL of each decoded source. A source with no URL gets a name
    // with a number, in the sequence of first use.
    let mut urls: Vec<Option<String>> = vec![None; map.sources.len()];
    let mut unknown_url: Option<String> = None;
    let mut id = 1;

    // The sources of the generator, in the sequence of first use, each
    // with its content.
    let mut sources: Vec<(String, String)> = Vec::new();
    let mut mappings: Vec<Mapping> = Vec::with_capacity(map.mappings.len());

    for mapping in &map.mappings {
        let (url, content) = match mapping.original_position {
            Some(position) => {
                let source = &map.sources[position.source];
                let url = urls[position.source].get_or_insert_with(|| match &source.url {
                    Some(url) => url.clone(),
                    None => {
                        id += 1;
                        format!("<unknown {}>", id - 1)
                    }
                });
                (url.clone(), source.content.clone().unwrap_or_else(|| "<none>".to_string()))
            }
            None => {
                let url = unknown_url.get_or_insert_with(|| {
                    id += 1;
                    format!("<unknown {}>", id - 1)
                });
                (url.clone(), "<none>".to_string())
            }
        };

        // The generator needs a mapping that is valid. Upstream throws an
        // error for the other mappings. The compiler makes none.
        let valid = mapping.generated_position.line > 0 && mapping.original_position.is_none_or(|p| p.line > 0);
        if !valid {
            continue;
        }

        let index = match sources.iter().position(|(source, _)| *source == url) {
            Some(index) => {
                sources[index].1 = content;
                index
            }
            None => {
                sources.push((url, content));
                sources.len() - 1
            }
        };

        mappings.push(Mapping {
            generated_line: mapping.generated_position.line,
            generated_column: mapping.generated_position.column,
            original: mapping.original_position.map(|position| (index, position.line, position.column)),
        });
    }

    // The generator sorts the mappings when one comes before the mapping
    // before it. The sort keeps the sequence of equal mappings.
    let sorted = mappings.windows(2).all(|pair| {
        (pair[0].generated_line, pair[0].generated_column) <= (pair[1].generated_line, pair[1].generated_column)
    });
    if !sorted {
        let key = |mapping: &Mapping| (mapping.generated_line, mapping.generated_column);
        let strings: Vec<&str> = sources.iter().map(|(url, _)| url.as_str()).collect();
        mappings.sort_by(|a, b| {
            key(a).cmp(&key(b)).then_with(|| match (a.original, b.original) {
                (Some(a), Some(b)) => strings[a.0].cmp(strings[b.0]).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2)),
                (None, None) => std::cmp::Ordering::Equal,
                (None, Some(_)) => std::cmp::Ordering::Less,
                (Some(_), None) => std::cmp::Ordering::Greater,
            })
        });
    }

    let mut out = String::from("{\"version\":3,\"sources\":[");
    for (index, (url, _)) in sources.iter().enumerate() {
        if index > 0 {
            out.push(',');
        }
        json_string(url, &mut out);
    }
    out.push_str("],\"names\":[],\"mappings\":\"");
    serialize_mappings(&mappings, &mut out);
    out.push('"');
    // The generator has the contents only after the first mapping.
    if !map.mappings.is_empty() {
        out.push_str(",\"sourcesContent\":[");
        for (index, (_, content)) in sources.iter().enumerate() {
            if index > 0 {
                out.push(',');
            }
            json_string(content, &mut out);
        }
        out.push(']');
    }
    out.push('}');
    out
}

/// The `mappings` field: `_serializeMappings` of `source-map-js`.
fn serialize_mappings(mappings: &[Mapping], out: &mut String) {
    let mut previous_generated_column = 0i64;
    let mut previous_generated_line = 1;
    let mut previous_original_column = 0i64;
    let mut previous_original_line = 0i64;
    let mut previous_source = 0i64;

    for (i, mapping) in mappings.iter().enumerate() {
        if mapping.generated_line != previous_generated_line {
            previous_generated_column = 0;
            while mapping.generated_line != previous_generated_line {
                out.push(';');
                previous_generated_line += 1;
            }
        } else if i > 0 {
            // A mapping that is equal to the mapping before it has no text.
            let previous = &mappings[i - 1];
            if previous.generated_line == mapping.generated_line
                && previous.generated_column == mapping.generated_column
                && previous.original == mapping.original
            {
                continue;
            }
            out.push(',');
        }

        vlq(mapping.generated_column as i64 - previous_generated_column, out);
        previous_generated_column = mapping.generated_column as i64;

        if let Some((source, line, column)) = mapping.original {
            vlq(source as i64 - previous_source, out);
            previous_source = source as i64;
            // The lines of a source map start at zero.
            vlq(line as i64 - 1 - previous_original_line, out);
            previous_original_line = line as i64 - 1;
            vlq(column as i64 - previous_original_column, out);
            previous_original_column = column as i64;
        }
    }
}

const BASE64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Add a number as a Base64 VLQ.
pub fn vlq(value: i64, out: &mut String) {
    let mut vlq: u64 = if value < 0 { ((value.unsigned_abs()) << 1) + 1 } else { (value as u64) << 1 };
    loop {
        let mut digit = (vlq & 0b11111) as usize;
        vlq >>= 5;
        if vlq > 0 {
            digit |= 0b100000;
        }
        out.push(BASE64[digit] as char);
        if vlq == 0 {
            break;
        }
    }
}

/// Bytes as Base64 text, with padding.
pub fn base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], chunk.get(1).copied().unwrap_or(0), chunk.get(2).copied().unwrap_or(0)];
        out.push(BASE64[(b[0] >> 2) as usize] as char);
        out.push(BASE64[(((b[0] & 0b11) << 4) | (b[1] >> 4)) as usize] as char);
        out.push(if chunk.len() > 1 { BASE64[(((b[1] & 0b1111) << 2) | (b[2] >> 6)) as usize] as char } else { '=' });
        out.push(if chunk.len() > 2 { BASE64[(b[2] & 0b111111) as usize] as char } else { '=' });
    }
    out
}

/// Add a string as `JSON.stringify` of JavaScript writes it.
pub fn json_string(text: &str, out: &mut String) {
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{08}' => out.push_str("\\b"),
            '\u{0c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}

#[cfg(test)]
mod tests {
    use super::*;

    // `should emit source maps` of `source-maps.test.ts`.
    #[test]
    fn should_emit_source_maps() {
        let map = raw_source_map("{\"version\":3,\"sources\":[],\"names\":[],\"mappings\":\"\"}");
        assert_eq!(map.comment("app.css.map"), "/*# sourceMappingURL=app.css.map */\n");
    }

    #[test]
    fn an_empty_map() {
        let map = to_source_map(&DecodedSourceMap::default());
        assert_eq!(map.raw, "{\"version\":3,\"sources\":[],\"names\":[],\"mappings\":\"\"}");
        assert_eq!(
            map.inline(),
            "/*# sourceMappingURL=data:application/json;base64,eyJ2ZXJzaW9uIjozLCJzb3VyY2VzIjpbXSwibmFtZXMiOltdLCJtYXBwaW5ncyI6IiJ9 */\n"
        );
    }

    #[test]
    fn vlq_values() {
        let text = |value| {
            let mut out = String::new();
            vlq(value, &mut out);
            out
        };
        assert_eq!(text(0), "A");
        assert_eq!(text(1), "C");
        assert_eq!(text(-1), "D");
        assert_eq!(text(15), "e");
        assert_eq!(text(16), "gB");
        assert_eq!(text(-16), "hB");
        assert_eq!(text(1000), "w+B");
    }

    #[test]
    fn base64_padding() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
    }

    #[test]
    fn json_strings() {
        let mut out = String::new();
        json_string("a\"b\\c\n\t\u{1}\u{7f}\u{2028}é", &mut out);
        assert_eq!(out, "\"a\\\"b\\\\c\\n\\t\\u0001\u{7f}\u{2028}é\"");
    }
}
