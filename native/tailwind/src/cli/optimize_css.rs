//! The optimizer of the output: `optimize.ts` of `@tailwindcss/node`.
//!
//! Tailwind CSS uses Lightning CSS for `--minify` and `--optimize`. This
//! file uses the same version of Lightning CSS with the same options, so
//! the output is the same.

use lightningcss::stylesheet::{MinifyOptions, ParserFlags, ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::{Browsers, Features, Targets};
use parcel_sourcemap::SourceMap;

use std::sync::{Arc, RwLock};

use super::remapping;

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
        browsers: Some(Browsers {
            safari: Some((16 << 16) | (4 << 8)),
            ios_saf: Some((16 << 16) | (4 << 8)),
            firefox: Some(128 << 16),
            chrome: Some(111 << 16),
            ..Browsers::default()
        }),
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

/// Optimize the CSS. `file` is the name of the input, for messages. `map`
/// is the source map of the input as JSON text.
pub fn optimize(input: &str, file: &str, minify: bool, map: Option<&str>) -> Result<Optimized, String> {
    // Two passes: the second one merges the rules that the first one made
    // adjacent when it removed the nesting.
    // The warnings are the warnings of the first pass, as upstream.
    let mut warnings = Vec::new();
    let (once, map) = pass(input, file, minify, map, Some(&mut warnings))?;
    let warnings = format_warnings(input, &warnings);
    let (twice, map) = pass(&once, file, minify, map.as_deref(), None)?;
    // The range syntax of media queries becomes `@media not (…)`, which is
    // not valid in level 3 of media queries. With a source map, the
    // replacement makes a new map.
    let replaced = remapping::replace_all(&twice, "@media not (", "@media not all and (", map.as_deref());
    Ok(Optimized { code: replaced.code, map: replaced.map.or(map), warnings })
}
