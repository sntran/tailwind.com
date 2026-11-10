//! The optimizer of the output: `optimize.ts` of `@tailwindcss/node`.
//!
//! Tailwind CSS uses Lightning CSS for `--minify` and `--optimize`. This
//! file uses the same version of Lightning CSS with the same options, so
//! the output is the same.

use lightningcss::stylesheet::{MinifyOptions, ParserFlags, ParserOptions, PrinterOptions, StyleSheet};
use lightningcss::targets::{Browsers, Features, Targets};

/// One pass of Lightning CSS.
fn pass(code: &str, file: &str, minify: bool) -> Result<String, String> {
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
        .to_css(PrinterOptions { minify, targets, ..PrinterOptions::default() })
        .map_err(|e| e.to_string())?;
    Ok(out.code)
}

/// Optimize the CSS. `file` is the name of the input, for messages.
pub fn optimize(input: &str, file: &str, minify: bool) -> Result<String, String> {
    // Two passes: the second one merges the rules that the first one made
    // adjacent when it removed the nesting.
    let once = pass(input, file, minify)?;
    let twice = pass(&once, file, minify)?;
    // The range syntax of media queries becomes `@media not (…)`, which is
    // not valid in level 3 of media queries.
    Ok(twice.replace("@media not (", "@media not all and ("))
}
