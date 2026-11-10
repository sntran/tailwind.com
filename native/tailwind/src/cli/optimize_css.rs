//! The optimizer of the output: `optimize.ts` of `@tailwindcss/node`.
//!
//! Tailwind CSS uses Lightning CSS for `--minify` and `--optimize`. This
//! file uses the same version of Lightning CSS with the same options, so
//! the output is the same.

use lightningcss::stylesheet::{MinifyOptions, ParserFlags, ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::{Browsers, Features, Targets};
use parcel_sourcemap::SourceMap;

use super::remapping;

/// One pass of Lightning CSS. `map` is the source map of the input as
/// JSON text: with a map, the result has the source map of the output.
fn pass(code: &str, file: &str, minify: bool, map: Option<&str>) -> Result<(String, Option<String>), String> {
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
    let mut sheet = StyleSheet::parse(
        code,
        ParserOptions {
            filename: file.to_string(),
            flags: ParserFlags::CUSTOM_MEDIA | ParserFlags::DEEP_SELECTOR_COMBINATOR,
            error_recovery: true,
            ..ParserOptions::default()
        },
    )
    .map_err(|e| e.to_string())?;
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
}

/// Optimize the CSS. `file` is the name of the input, for messages. `map`
/// is the source map of the input as JSON text.
pub fn optimize(input: &str, file: &str, minify: bool, map: Option<&str>) -> Result<Optimized, String> {
    // Two passes: the second one merges the rules that the first one made
    // adjacent when it removed the nesting.
    let (once, map) = pass(input, file, minify, map)?;
    let (twice, map) = pass(&once, file, minify, map.as_deref())?;
    // The range syntax of media queries becomes `@media not (…)`, which is
    // not valid in level 3 of media queries. With a source map, the
    // replacement makes a new map.
    let replaced = remapping::replace_all(&twice, "@media not (", "@media not all and (", map.as_deref());
    Ok(Optimized { code: replaced.code, map: replaced.map.or(map) })
}
