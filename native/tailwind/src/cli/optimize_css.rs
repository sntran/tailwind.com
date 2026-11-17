//! The optimizer of the output: `optimize.ts` of `@tailwindcss/node`.
//!
//! Tailwind CSS uses Lightning CSS for `--minify` and `--optimize`. This
//! file uses the same version of Lightning CSS with the same options, so
//! the output is the same.

use lightningcss::stylesheet::{MinifyOptions, ParserFlags, ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::{Features, Targets};
use parcel_sourcemap::SourceMap;

use std::sync::{Arc, RwLock};

use super::css_mixins::{nesting_depth, string_end};
use super::remapping;
use crate::Error;

/// The browsers of the optimizer: the targets of `optimize.ts` of
/// `@tailwindcss/node` (`css_mixins.rs`).
pub use super::css_mixins::TARGETS;

/// A warning of Lightning CSS, as its Node.js package gives it: the
/// message, the line (from 1) and the column (from 1, in UTF-16 code
/// units).
struct Warning {
    message: String,
    line: u32,
    column: u32,
}

/// One pass of Lightning CSS. `map` is the source map of the input as
/// JSON text: with a map, the result has the source map of the output.
fn pass(code: &str, file: &str, minify: bool, map: Option<&str>, warnings: Option<&mut Vec<Warning>>) -> Result<(String, Option<String>), String> {
    // As the Node.js package of Lightning CSS: a map from the output to
    // the input, then the input map makes it a map to the first sources.
    let mut source_map = match map {
        Some(_) => {
            let mut source_map = SourceMap::new("/");
            source_map.add_source(file);
            source_map.set_source_content(0, code).map_err(|e| e.to_string())?;
            Some(source_map)
        }
        None => None,
    };

    let targets = Targets {
        browsers: Some(TARGETS),
        include: Features::Nesting | Features::MediaQueries,
        exclude: Features::LogicalProperties | Features::DirSelector | Features::LightDark,
    };
    // With `error_recovery`, the parser drops a rule that it cannot read,
    // and puts a warning in this list.
    let parser_warnings = Arc::new(RwLock::new(Vec::new()));
    let mut sheet = StyleSheet::parse(
        code,
        ParserOptions {
            filename: file.to_string(),
            flags: ParserFlags::CUSTOM_MEDIA | ParserFlags::DEEP_SELECTOR_COMBINATOR,
            error_recovery: true,
            warnings: Some(parser_warnings.clone()),
            ..ParserOptions::default()
        },
    )
    .map_err(|e| e.to_string())?;
    if let Some(warnings) = warnings {
        if let Ok(list) = parser_warnings.read() {
            for warning in list.iter() {
                let (line, column) = warning.loc.as_ref().map_or((1, 1), |loc| (loc.line + 1, loc.column));
                warnings.push(Warning { message: warning.kind.to_string(), line, column });
            }
        }
    }
    sheet.minify(MinifyOptions { targets, ..MinifyOptions::default() }).map_err(|e| e.to_string())?;
    let out = sheet
        .to_css(PrinterOptions { minify, targets, source_map: source_map.as_mut(), ..PrinterOptions::default() })
        .map_err(|e| e.to_string())?;

    let map = match (source_map, map) {
        (Some(mut source_map), Some(input)) => {
            if let Ok(mut input) = SourceMap::from_json("/", input) {
                let _ = source_map.extends(&mut input);
            }
            source_map.to_json(None).ok()
        }
        _ => None,
    };
    Ok((out.code, map))
}

/// The map from the result of `a` to the sources of `b`: `a` maps to the
/// input of `b`.
fn compose(a: &str, b: &str) -> Result<String, String> {
    let mut a = SourceMap::from_json("/", a).map_err(|e| e.to_string())?;
    let mut b = SourceMap::from_json("/", b).map_err(|e| e.to_string())?;
    a.extends(&mut b).map_err(|e| e.to_string())?;
    a.to_json(None).map_err(|e| e.to_string())
}

/// The result of the optimizer.
pub struct Optimized {
    pub code: String,
    /// The source map as JSON text, when the optimizer got a map.
    pub map: Option<String>,
    /// The text of the warnings of the optimizer, as the official program
    /// gives it to `console.warn`.
    pub warnings: Option<String>,
}

fn dim(text: &str) -> String {
    format!("\x1B[2m{text}\x1B[22m")
}

fn yellow(text: &str) -> String {
    format!("\x1B[33m{text}\x1B[39m")
}

/// The pseudo-classes of Vue and Angular: their own compilers handle them,
/// and the official program gives no warning for them.
fn is_framework_pseudo(message: &str) -> bool {
    ["deep", "slotted", "global", "ng-deep", "host-context"]
        .iter()
        .any(|name| message.contains(&format!("'{name}' is not recognized as a valid pseudo-")))
}

/// The text of the warnings, with two lines of the CSS before and after
/// each one: `optimize.ts` of `@tailwindcss/node`. The colors are always
/// there, as upstream.
fn format_warnings(input: &str, warnings: &[Warning]) -> Option<String> {
    let warnings: Vec<&Warning> = warnings.iter().filter(|warning| !is_framework_pseudo(&warning.message)).collect();
    if warnings.is_empty() {
        return None;
    }
    let lines: Vec<&str> = input.split('\n').collect();
    let mut output =
        vec![format!("Found {} {} while optimizing generated CSS:", warnings.len(), if warnings.len() == 1 { "warning" } else { "warnings" })];
    for (index, warning) in warnings.iter().enumerate() {
        output.push(String::new());
        if warnings.len() > 1 {
            output.push(format!("Issue #{}:", index + 1));
        }
        let context = 2;
        let line = warning.line as usize;
        let start = line.saturating_sub(context + 1);
        let end = lines.len().min(line + context);
        let mut snippet: Vec<String> = lines[start.min(end)..end]
            .iter()
            .enumerate()
            .map(|(offset, text)| {
                if start + offset + 1 == line {
                    format!("{} {text}", dim("\u{2502}"))
                } else {
                    dim(&format!("\u{2502} {text}"))
                }
            })
            .collect();
        let at = (line - start.min(line)).min(snippet.len());
        snippet.insert(at, dim("\u{2506}"));
        snippet.insert(
            at,
            format!(
                "{}{} {}",
                dim("\u{2506}"),
                " ".repeat((warning.column as usize).saturating_sub(1)),
                yellow(&format!("{} {}", dim("^--"), warning.message))
            ),
        );
        output.extend(snippet);
    }
    output.push(String::new());
    Some(output.join("\n"))
}

/// The largest number of nested blocks (`(`, `[` and `{`) in the input of
/// Lightning CSS. Its parsers are recursive, and some have no limit: the
/// official program stops with a stack overflow for a deeper input
/// (`calc()` in `calc()` 4,419 times, or `:is()` in `:is()` 2,219 times).
/// The stack of a run (`nesting::STACK_SIZE`) holds this depth.
const MAX_NESTING: usize = 10_000;

/// Make the input safe for Lightning CSS, when it nests deeper than
/// `MAX_NESTING`. `None`: the input does not change.
///
/// Lightning CSS reads the value of a custom property, or of a property
/// that it does not know, as a list of tokens. At the depth 501, it gives
/// the warning "Overflowed the maximum nesting depth", and removes the
/// declaration, with no deeper recursion. The official program gives this
/// warning for each deep value that does not overflow its stack. Thus a
/// deeper declaration gets a name that Lightning CSS does not know, with
/// the same length: the warning is the same, at the same place. A
/// selector or an at-rule that is too deep is an error.
fn limit_nesting(input: &str) -> Result<Option<String>, Error> {
    if nesting_depth(input) <= MAX_NESTING {
        return Ok(None);
    }
    let bytes = input.as_bytes();
    let mut out = input.to_string();
    // The first character of the statement, when it is not white space.
    let mut first: Option<usize> = None;
    // The first `:` of the statement, out of the brackets.
    let mut colon: Option<usize> = None;
    // The open brackets of the statement, and the most of them.
    let (mut depth, mut max) = (0usize, 0usize);
    let mut index = 0;
    while index < bytes.len() {
        let byte = bytes[index];
        if first.is_none() && !byte.is_ascii_whitespace() {
            first = Some(index);
        }
        // A custom property can have `{` and `}` in its value.
        let custom = first.is_some_and(|first| input[first..].starts_with("--"));
        match byte {
            b'"' | b'\'' => {
                index = string_end(input, index);
                continue;
            }
            b'\\' => index += 1,
            b'/' if bytes.get(index + 1) == Some(&b'*') => {
                index = input[index + 2..].find("*/").map_or(bytes.len(), |end| index + 2 + end + 2);
                continue;
            }
            b':' if depth == 0 && colon.is_none() => colon = Some(index),
            // The start of a block, or the end of a statement.
            b'{' | b';' | b'}' if depth == 0 && !(byte == b'{' && custom) => {
                if max > MAX_NESTING {
                    let name = first.zip(colon).map(|(first, colon)| input[first..colon].trim_end());
                    match name {
                        // A custom property has no deeper recursion.
                        Some(name) if byte != b'{' && name.starts_with("--") => {}
                        Some(name) if byte != b'{' && !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'-') => {
                            let start = first.unwrap_or(0);
                            out.replace_range(start..start + name.len(), &format!("x{}", "-".repeat(name.len() - 1)));
                        }
                        _ => return Err(crate::nesting::error()),
                    }
                }
                first = None;
                colon = None;
                max = 0;
            }
            b'(' | b'[' | b'{' => {
                depth += 1;
                max = max.max(depth);
            }
            b')' | b']' | b'}' => depth = depth.saturating_sub(1),
            _ => {}
        }
        index += 1;
    }
    // A statement with no end.
    if max > MAX_NESTING {
        return Err(crate::nesting::error());
    }
    Ok(Some(out))
}

/// Optimize the CSS. `file` is the name of the input, for messages. `map`
/// is the source map of the input as JSON text.
pub fn optimize(input: &str, file: &str, minify: bool, map: Option<&str>) -> Result<Optimized, Error> {
    // The functions and the mixins first: Lightning CSS does not know them.
    let lowered = super::css_mixins::lower(input, file, &TARGETS, map.is_some());
    let composed;
    let (input, map) = match &lowered {
        Some(lowered) => {
            composed = match (lowered.map.as_deref(), map) {
                (Some(ours), Some(given)) => Some(compose(ours, given).map_err(Error::new)?),
                _ => None,
            };
            (lowered.code.as_str(), composed.as_deref())
        }
        None => (input, map),
    };

    // Two passes: the second one merges the rules that the first one made
    // adjacent when it removed the nesting.
    // The warnings are the warnings of the first pass, as upstream.
    // The warnings show the lines of the input with its own names.
    let limited = limit_nesting(input)?;
    let mut warnings = Vec::new();
    let (once, map) = pass(limited.as_deref().unwrap_or(input), file, minify, map, Some(&mut warnings)).map_err(Error::new)?;
    let warnings = format_warnings(input, &warnings);
    let (twice, map) = pass(&once, file, minify, map.as_deref(), None).map_err(Error::new)?;
    // The range syntax of media queries becomes `@media not (…)`, which is
    // not valid in level 3 of media queries. With a source map, the
    // replacement makes a new map.
    let replaced = remapping::replace_all(&twice, "@media not (", "@media not all and (", map.as_deref());
    Ok(Optimized { code: replaced.code, map: replaced.map.or(map), warnings })
}
