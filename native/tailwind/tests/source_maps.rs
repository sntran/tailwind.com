//! The source maps of this compiler against the records of Tailwind CSS
//! in `tests/upstream/source-maps.jsonl.gz`
//! (`tests/upstream/tools/gen-source-maps.mjs`).
//!
//! For each record, the test compiles the input with the `from` option,
//! builds each list of candidates, and compares the CSS, the decoded map
//! (`Compiler::build_source_map`) and the JSON text of the map
//! (`cli::source_maps::to_source_map`).
//!
//! Environment:
//! - `MAPS_SHOW`: the number of failures to print (default 5).
//! - `MAPS_FILTER`: use only the records whose name contains this text.
//! - `MAPS_FILE`: a file of records to use, not the file of the repository
//!   (for a larger file from the recorder with `SOURCE_MAPS_STEP=1`).
//! - `MAPS_DUMP`: print the output and all the mappings of a record that is
//!   different (expected, then actual).

use flate2::read::GzDecoder;
use serde::Deserialize;
use std::cell::Cell;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use tailwind::ast::AstNode;
use tailwind::cli::source_maps::{json_string, to_source_map};
use tailwind::css_parser::{parse, parse_with_source};
use tailwind::source_maps::DecodedSourceMap;
use tailwind::{compile, CompileOptions, Error, Stylesheet, POLYFILLS_ALL};

const CSS_NAMES: [&str; 4] = ["index.css", "theme.css", "preflight.css", "utilities.css"];

/// A content: the text, a stylesheet of Tailwind CSS, or the output.
#[derive(Deserialize, Clone)]
#[serde(untagged)]
enum Content {
    Text(String),
    Css { css: String },
    Generated {
        #[allow(dead_code)]
        generated: bool,
    },
}

#[derive(Deserialize)]
struct Loaded {
    path: String,
    base: String,
    content: Content,
}

#[derive(Deserialize)]
struct StylesheetCall {
    id: String,
    base: String,
    result: Option<Loaded>,
    error: Option<String>,
}

#[derive(Deserialize)]
struct RecordedSource {
    url: Option<String>,
    content: Option<Content>,
}

#[derive(Deserialize)]
struct RecordedMap {
    file: Option<String>,
    sources: Vec<RecordedSource>,
    /// `[generated line, generated column, source, line, column]`
    mappings: Vec<[i64; 5]>,
}

#[derive(Deserialize)]
struct Build {
    candidates: Vec<String>,
    css: String,
    map: RecordedMap,
    raw: String,
}

#[derive(Deserialize)]
struct Record {
    name: String,
    input: String,
    from: Option<String>,
    base: String,
    polyfills: Option<u8>,
    stylesheets: Vec<StylesheetCall>,
    builds: Vec<Build>,
    /// The `[start, end]` of the `src` of each node of the parsed input.
    locations: Option<Vec<Option<[usize; 2]>>>,
    /// The message of the error of `compile`.
    error: Option<String>,
}

fn upstream(path: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../tests/upstream").join(path)
}

fn css_file(name: &str) -> String {
    std::fs::read_to_string(upstream("css").join(name)).unwrap_or_else(|e| panic!("cannot read {name}: {e}"))
}

fn text(content: &Content, generated: &str) -> String {
    match content {
        Content::Text(text) => text.clone(),
        Content::Css { css } => css_file(css),
        Content::Generated { .. } => generated.to_string(),
    }
}

/// The decoded map in the form of the records.
fn mappings(map: &DecodedSourceMap) -> Vec<[i64; 5]> {
    map.mappings
        .iter()
        .map(|mapping| {
            let generated = mapping.generated_position;
            match mapping.original_position {
                Some(original) => [
                    generated.line as i64,
                    generated.column as i64,
                    original.source as i64,
                    original.line as i64,
                    original.column as i64,
                ],
                None => [generated.line as i64, generated.column as i64, -1, 0, 0],
            }
        })
        .collect()
}

/// The `src` of each node of a tree, in document order.
fn locations(nodes: &[AstNode], out: &mut Vec<Option<[usize; 2]>>) -> Result<(), String> {
    for node in nodes {
        out.push(node.src().map(|src| [src.start, src.end]));
        if node.src() != node.dst() {
            return Err("the parser gave a dst that is not the src".to_string());
        }
        if let Some(children) = node.children() {
            locations(children, out)?;
        }
    }
    Ok(())
}

/// Compare one record. The result is the description of the first difference.
fn check(record: &Record) -> Result<(), String> {
    if let Some(expected) = &record.locations {
        let ast = parse_with_source(&record.input, record.from.as_deref()).map_err(|e| format!("parse failed: {e}"))?;
        let mut actual = Vec::new();
        locations(&ast, &mut actual)?;
        if &actual != expected {
            let at = actual.iter().zip(expected).position(|(a, b)| a != b).unwrap_or(0);
            return Err(format!(
                "the locations of the parser are different at node {at}: {:?}, expected {:?}",
                actual.get(at),
                expected.get(at)
            ));
        }
        // The locations have no effect on the tree.
        if Ok(ast) != parse(&record.input) {
            return Err("the tree with locations is not equal to the tree without them".to_string());
        }
    }

    let calls = Cell::new(0);
    let loader = |id: &str, base: &str| -> Result<Stylesheet, Error> {
        calls.set(calls.get() + 1);
        let call = record
            .stylesheets
            .iter()
            .find(|call| call.id == id && call.base == base)
            .ok_or_else(|| Error::new(format!("no stylesheet {id} in {base}")))?;
        match &call.result {
            Some(loaded) => Ok(Stylesheet {
                path: loaded.path.clone(),
                base: loaded.base.clone(),
                content: text(&loaded.content, ""),
            }),
            None => Err(Error::new(call.error.clone().unwrap_or_default())),
        }
    };
    let options = CompileOptions {
        base: record.base.clone(),
        polyfills: record.polyfills.unwrap_or(POLYFILLS_ALL),
        load_stylesheet: Some(&loader),
        test_mode: true,
        from: record.from.clone(),
    };
    let mut compiler = match (compile(&record.input, options), &record.error) {
        (Ok(compiler), None) => compiler,
        (Err(e), None) => return Err(format!("compile failed: {e}")),
        (Ok(_), Some(expected)) => return Err(format!("no error, expected: {expected}")),
        (Err(e), Some(expected)) if e.message == *expected => return Ok(()),
        (Err(e), Some(expected)) => return Err(format!("the error is {:?}, expected {expected:?}", e.message)),
    };

    for (index, build) in record.builds.iter().enumerate() {
        let css = compiler.build(&build.candidates);
        if css != build.css {
            return Err(format!("build {index}: the CSS is different"));
        }

        let map = compiler.build_source_map();
        if map.file != build.map.file {
            return Err(format!("build {index}: file {:?}, expected {:?}", map.file, build.map.file));
        }
        let urls: Vec<&Option<String>> = map.sources.iter().map(|source| &source.url).collect();
        let expected_urls: Vec<&Option<String>> = build.map.sources.iter().map(|source| &source.url).collect();
        if urls != expected_urls {
            return Err(format!("build {index}: sources {urls:?}, expected {expected_urls:?}"));
        }
        for (source, expected) in map.sources.iter().zip(&build.map.sources) {
            let expected_content = expected.content.as_ref().map(|content| text(content, &build.css));
            if source.content != expected_content {
                return Err(format!("build {index}: the content of the source {:?} is different", source.url));
            }
        }
        let actual = mappings(&map);
        if actual != build.map.mappings {
            let first = actual.iter().zip(&build.map.mappings).position(|(a, b)| a != b);
            let at = first.unwrap_or(actual.len().min(build.map.mappings.len()));
            let line = build.map.mappings.get(at).map(|m| m[0] as usize).unwrap_or(0);
            if std::env::var_os("MAPS_DUMP").is_some() {
                println!("--- {}\n{}", record.name, css);
                for i in 0..actual.len().max(build.map.mappings.len()) {
                    let (a, e) = (actual.get(i), build.map.mappings.get(i));
                    println!("{} {:?} {:?}", if a == e { ' ' } else { '!' }, e, a);
                }
            }
            return Err(format!(
                "build {index}: {} mappings, expected {}; the first different mapping is number {at}: {:?}, expected {:?}\n    output line: {:?}",
                actual.len(),
                build.map.mappings.len(),
                actual.get(at),
                build.map.mappings.get(at),
                css.split('\n').nth(line.saturating_sub(1)).unwrap_or(""),
            ));
        }

        let mut expected_raw = build.raw.clone();
        for name in CSS_NAMES {
            let marker = format!("\"@@css:{name}@@\"");
            if expected_raw.contains(&marker) {
                let mut content = String::new();
                json_string(&css_file(name), &mut content);
                expected_raw = expected_raw.replace(&marker, &content);
            }
        }
        let raw = to_source_map(&map).raw;
        if raw != expected_raw {
            let at = raw.bytes().zip(expected_raw.bytes()).position(|(a, b)| a != b).unwrap_or(0);
            let start = at.saturating_sub(60);
            let side = |text: &str| String::from_utf8_lossy(&text.as_bytes()[start..(at + 60).min(text.len())]).into_owned();
            return Err(format!(
                "build {index}: the JSON text is different at byte {at}\n    - {}\n    + {}",
                side(&expected_raw),
                side(&raw)
            ));
        }
    }
    Ok(())
}

#[test]
fn source_maps() {
    let path = std::env::var_os("MAPS_FILE").map(PathBuf::from).unwrap_or_else(|| upstream("source-maps.jsonl.gz"));
    let file = std::fs::File::open(path).expect("the records");
    let filter = std::env::var("MAPS_FILTER").ok();
    let show: usize = std::env::var("MAPS_SHOW").ok().and_then(|v| v.parse().ok()).unwrap_or(5);

    let mut count = 0;
    let mut builds = 0;
    let mut failures = Vec::new();
    for line in BufReader::new(GzDecoder::new(file)).lines() {
        let record: Record = serde_json::from_str(&line.expect("a line")).expect("a record");
        if filter.as_ref().is_some_and(|filter| !record.name.contains(filter.as_str())) {
            continue;
        }
        count += 1;
        builds += record.builds.len();
        if let Err(message) = check(&record) {
            failures.push(format!("{}\n    {message}", record.name));
        }
    }

    println!("source_maps: {count} records, {builds} builds, {} failed", failures.len());
    for failure in failures.iter().take(show) {
        println!("  {failure}");
    }
    assert!(failures.is_empty(), "{} of {count} records are different", failures.len());
    assert!(filter.is_some() || count >= 150, "the fixture has only {count} records");
}

// ---------------------------------------------------------------------------
// `source-maps/translation-map.test.ts`. Upstream shows the ranges as a
// picture. The tests here have the numbers of the pictures:
// `A @ 1:0-2:7` is from line 1, column 0 to line 2, column 7.

mod translation_map {
    use tailwind::ast::AstNode;
    use tailwind::css_parser::parse_with_source;
    use tailwind::source_maps::{create_translation_map, Position};
    use tailwind::to_css::to_css_tracked;

    type Range = ((usize, usize), (usize, usize));

    /// Parse and print with locations, and give the ranges of one node:
    /// in the output, and in the input.
    fn analyze(input: &str, path: &[usize], css: &str) -> (Range, Range) {
        let mut ast = parse_with_source(input, Some("input.css")).unwrap();
        let printed = to_css_tracked(&mut ast);
        assert_eq!(printed, css);
        let translate = create_translation_map(input, &printed);

        let mut node: &AstNode = &ast[path[0]];
        for index in &path[1..] {
            node = &node.children().unwrap()[*index];
        }
        let translations = translate.translate(node);
        assert_eq!(translations.len(), 1);
        let (original_start, original_end, generated_start, generated_end) = translations[0];
        let pair = |position: Position| (position.line, position.column);
        (
            (pair(generated_start.unwrap()), pair(generated_end.unwrap())),
            (pair(original_start), pair(original_end)),
        )
    }

    #[test]
    fn comment_single_line() {
        let (output, input) = analyze("/*! foo */", &[0], "/*! foo */\n");
        assert_eq!(output, ((1, 0), (1, 10)));
        assert_eq!(input, ((1, 0), (1, 10)));
    }

    #[test]
    fn comment_multi_line() {
        let (output, input) = analyze("/*! foo \n bar */", &[0], "/*! foo \n bar */\n");
        assert_eq!(output, ((1, 0), (2, 7)));
        assert_eq!(input, ((1, 0), (2, 7)));
    }

    #[test]
    fn declaration_normal_property_single_line() {
        let (output, input) = analyze(".foo { color: red; }", &[0, 0], ".foo {\n  color: red;\n}\n");
        assert_eq!(output, ((2, 2), (2, 12)));
        assert_eq!(input, ((1, 7), (1, 17)));
    }

    #[test]
    fn declaration_normal_property_multi_line() {
        let (output, input) = analyze(
            ".foo {\n  grid-template-areas:\n    \"a b c\"\n    \"d e f\"\n    \"g h i\";\n}",
            &[0, 0],
            ".foo {\n  grid-template-areas: \"a b c\" \"d e f\" \"g h i\";\n}\n",
        );
        assert_eq!(output, ((2, 2), (2, 46)));
        assert_eq!(input, ((2, 2), (5, 11)));
    }

    #[test]
    fn declaration_custom_property_single_line() {
        let (output, input) = analyze(".foo { --foo: bar; }", &[0, 0], ".foo {\n  --foo: bar;\n}\n");
        assert_eq!(output, ((2, 2), (2, 12)));
        assert_eq!(input, ((1, 7), (1, 17)));
    }

    #[test]
    fn declaration_custom_property_multi_line() {
        let (output, input) = analyze(".foo {\n  --foo: bar\nbaz;\n}", &[0, 0], ".foo {\n  --foo: bar\nbaz;\n}\n");
        assert_eq!(output, ((2, 2), (3, 3)));
        assert_eq!(input, ((2, 2), (3, 3)));
    }

    #[test]
    fn at_rules_bodyless_single_line() {
        // The input has more spaces than the output.
        let (output, input) = analyze("@layer foo,     bar;", &[0], "@layer foo, bar;\n");
        assert_eq!(output, ((1, 0), (1, 15)));
        assert_eq!(input, ((1, 0), (1, 19)));
    }

    #[test]
    fn at_rules_bodyless_multi_line() {
        let (output, input) = analyze("@layer\n  foo,\n  bar\n;", &[0], "@layer foo, bar;\n");
        assert_eq!(output, ((1, 0), (1, 15)));
        assert_eq!(input, ((1, 0), (4, 0)));
    }

    #[test]
    fn at_rules_body_single_line() {
        let (output, input) = analyze("@layer foo { color: red; }", &[0], "@layer foo {\n  color: red;\n}\n");
        assert_eq!(output, ((1, 0), (1, 11)));
        assert_eq!(input, ((1, 0), (1, 11)));
    }

    #[test]
    fn at_rules_body_multi_line() {
        let (output, input) = analyze("@layer\n  foo\n{\n  color: baz;\n}", &[0], "@layer foo {\n  color: baz;\n}\n");
        assert_eq!(output, ((1, 0), (1, 11)));
        assert_eq!(input, ((1, 0), (3, 0)));
    }

    #[test]
    fn style_rules_body_single_line() {
        let (output, input) =
            analyze(".foo:is(.bar) { color: red; }", &[0], ".foo:is(.bar) {\n  color: red;\n}\n");
        assert_eq!(output, ((1, 0), (1, 14)));
        assert_eq!(input, ((1, 0), (1, 14)));
    }

    #[test]
    fn style_rules_body_multi_line() {
        let (output, input) =
            analyze(".foo:is(\n  .bar\n) {\n  color: red;\n}", &[0], ".foo:is( .bar ) {\n  color: red;\n}\n");
        assert_eq!(output, ((1, 0), (1, 16)));
        assert_eq!(input, ((1, 0), (3, 2)));
    }

    #[test]
    fn a_node_with_no_source_has_no_translation() {
        let ast = tailwind::css_parser::parse(".a { color: red }").unwrap();
        assert!(create_translation_map("", "").translate(&ast[0]).is_empty());
    }
}

// ---------------------------------------------------------------------------
// Property tests.

mod properties {
    use proptest::prelude::*;
    use tailwind::cli::source_maps::to_source_map;
    use tailwind::css_parser::{parse, parse_with_source};
    use tailwind::source_maps::DecodedSourceMap;
    use tailwind::to_css::{to_css, to_css_tracked};
    use tailwind::{compile, CompileOptions, Stylesheet};

    fn load(id: &str, base: &str) -> tailwind::Result<Stylesheet> {
        let content = match id {
            "./a.css" => ".a {\n  color: red;\n  @apply underline;\n}\n@theme {\n  --color-a: #abc;\n}\n",
            "./é.css" => "/*! é 🙂 */\n.é { content: \"🙂\" }\n@utility from-é {\n  color: blue;\n}\n",
            _ => return Err(tailwind::Error::new(format!("no stylesheet {id}"))),
        };
        Ok(Stylesheet { path: format!("{base}/{}", &id[2..]), base: base.to_string(), content: content.to_string() })
    }

    fn options(from: Option<&str>) -> CompileOptions<'static> {
        CompileOptions {
            base: "/root".to_string(),
            polyfills: tailwind::POLYFILLS_ALL,
            load_stylesheet: Some(&load),
            test_mode: true,
            from: from.map(String::from),
        }
    }

    /// A stylesheet from parts that use each feature of the compiler.
    fn stylesheet() -> impl Strategy<Value = String> {
        let part = prop::sample::select(vec![
            "@tailwind utilities;\n",
            "@layer utilities {\n  @tailwind utilities;\n}\n",
            "@theme {\n  --color-red: #f00;\n  --spacing: 0.25rem;\n  --breakpoint-md: 48rem;\n}\n",
            "@theme inline {\n  --color-é:\n    blue;\n  --animate-spin: spin 1s;\n  @keyframes spin { to { rotate: 1turn } }\n}\n",
            "@utility card {\n  padding: 1rem;\n  &:hover { color: red }\n}\n",
            "@utility tab-* {\n  tab-size: --value(integer);\n}\n",
            "@custom-variant a (&.a);\n",
            "@custom-variant b (@media print, &.b);\n",
            "@custom-variant c {\n  &:where(.c) { @slot; }\n  @supports (x: y) { @slot; }\n}\n",
            ".x {\n  @apply flex hover:underline;\n  color: red;\n}\n",
            ".y { @variant hover { color: blue; } }\n",
            "@media (width > 1px) {\n  .z {\n    color: red;\n    .w & { color: color-mix(in oklab, red 50%, blue) }\n  }\n}\n",
            "/*! a license 🙂 */\n",
            "/* a comment */\n.é🙂 { content: \"🙂\"; --x: { a: b } }\n",
            "@import \"./a.css\";\n",
            "@import \"./é.css\" layer(e);\n",
            ".crlf {\r\n  color:\r\n    red;\r\n}\r\n",
            "\n\n   ",
        ]);
        prop::collection::vec(part, 0..7).prop_map(|parts| parts.concat())
    }

    fn candidates() -> impl Strategy<Value = Vec<String>> {
        let variant = prop::sample::select(vec![
            "hover", "md", "dark", "group-hover", "not-first", "has-[a]", "[&>*]", "a", "b", "c", "group-b", "not-c", "",
        ]);
        let root = prop::sample::select(vec![
            "flex", "p-4", "w-1/2", "bg-red/50", "text-é", "[color:red]", "shadow-lg", "translate-x-4", "card", "tab-4",
            "from-é", "animate-spin", "content-['🙂']", "bogus", "underline",
        ]);
        let candidate = (prop::collection::vec(variant, 0..3), root).prop_map(|(variants, root)| {
            let mut parts: Vec<&str> = variants.into_iter().filter(|variant| !variant.is_empty()).collect();
            parts.push(root);
            parts.join(":")
        });
        prop::collection::vec(candidate, 0..10)
    }

    /// The length of each line of a text, in UTF-16 code units.
    fn line_lengths(text: &str) -> Vec<usize> {
        text.split('\n').map(|line| line.encode_utf16().count()).collect()
    }

    /// Read the `mappings` field of a source map:
    /// `[generated line, generated column, source, line, column]`, with the
    /// lines from one.
    fn decode_mappings(raw: &str) -> Vec<[i64; 5]> {
        const BASE64: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let start = raw.find("\"mappings\":\"").expect("the mappings") + 12;
        let text = &raw[start..start + raw[start..].find('"').unwrap()];
        let mut out = Vec::new();
        let (mut source, mut line, mut column) = (0i64, 0i64, 0i64);
        for (generated_line, group) in text.split(';').enumerate() {
            let mut generated_column = 0i64;
            for segment in group.split(',').filter(|segment| !segment.is_empty()) {
                let mut fields = Vec::new();
                let (mut value, mut shift) = (0i64, 0);
                for byte in segment.bytes() {
                    let digit = BASE64.iter().position(|b| *b == byte).unwrap() as i64;
                    value |= (digit & 31) << shift;
                    if digit & 32 != 0 {
                        shift += 5;
                    } else {
                        fields.push(if value & 1 == 1 { -(value >> 1) } else { value >> 1 });
                        value = 0;
                        shift = 0;
                    }
                }
                assert_eq!(fields.len(), 4, "a segment has four fields");
                generated_column += fields[0];
                source += fields[1];
                line += fields[2];
                column += fields[3];
                out.push([generated_line as i64 + 1, generated_column, source, line + 1, column]);
            }
        }
        out
    }

    fn check_map(css: &str, map: &DecodedSourceMap) -> Result<(), TestCaseError> {
        let generated = line_lengths(css);
        let sources: Vec<Vec<usize>> =
            map.sources.iter().map(|source| line_lengths(source.content.as_deref().unwrap_or(""))).collect();

        let mut previous = (0, 0);
        for mapping in &map.mappings {
            let position = mapping.generated_position;
            // Each mapping is in the generated text.
            prop_assert!(position.line >= 1 && position.line <= generated.len(), "generated line {}", position.line);
            prop_assert!(position.column <= generated[position.line - 1], "generated column {position:?}");
            // The generated positions are sorted.
            prop_assert!((position.line, position.column) >= previous, "the mappings are not sorted");
            previous = (position.line, position.column);

            // Each mapping is in its source.
            let original = mapping.original_position.expect("an original position");
            prop_assert!(original.source < sources.len());
            let lines = &sources[original.source];
            prop_assert!(original.line >= 1 && original.line <= lines.len(), "original line {original:?}");
            prop_assert!(original.column <= lines[original.line - 1], "original column {original:?}");
            // The source of a mapping is a stylesheet, not the output.
            prop_assert!(map.sources[original.source].url.is_some());
        }

        // The JSON text has the same mappings. The generator writes equal
        // mappings one time, and gives the sources new indices.
        let raw = to_source_map(map).raw;
        let mut urls: Vec<&str> = Vec::new();
        let mut expected: Vec<[i64; 5]> = Vec::new();
        for mapping in &map.mappings {
            let original = mapping.original_position.unwrap();
            let url = map.sources[original.source].url.as_deref().unwrap();
            let index = urls.iter().position(|known| *known == url).unwrap_or_else(|| {
                urls.push(url);
                urls.len() - 1
            });
            let entry = [
                mapping.generated_position.line as i64,
                mapping.generated_position.column as i64,
                index as i64,
                original.line as i64,
                original.column as i64,
            ];
            if expected.last() != Some(&entry) {
                expected.push(entry);
            }
        }
        prop_assert_eq!(decode_mappings(&raw), expected);
        Ok(())
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 96, failure_persistence: None, ..ProptestConfig::default() })]

        /// The CSS is the same with and without the source locations, and
        /// each mapping of the map is in the output and in its source.
        #[test]
        fn the_map_is_in_the_texts_and_the_css_does_not_change(input in stylesheet(), first in candidates(), second in candidates()) {
            let plain = compile(&input, options(None));
            let tracked = compile(&input, options(Some("/root/input.css")));
            let (mut plain, mut tracked) = match (plain, tracked) {
                (Ok(plain), Ok(tracked)) => (plain, tracked),
                // An error has the position only with `from`.
                (Err(plain), Err(tracked)) => {
                    prop_assert!(tracked.message.ends_with(&plain.message));
                    return Ok(());
                }
                _ => return Err(TestCaseError::fail("only one of the two compilers gave an error")),
            };
            // No build: the map is the map of the input.
            check_map(&input, &tracked.build_source_map()).ok();
            for list in [first, second] {
                let css = tracked.build(&list);
                prop_assert_eq!(&css, &plain.build(&list));
                check_map(&css, &tracked.build_source_map())?;
                // Without `from`, the map is empty.
                prop_assert!(plain.build_source_map().mappings.is_empty());
            }
        }

        /// The comparison of two trees ignores the locations, and the
        /// printer gives the same text with and without them.
        #[test]
        fn the_locations_do_not_change_the_tree(input in stylesheet(), noise in "[a-z{}();:,.&\"' \\n\\-/!é🙂]{0,40}") {
            let input = format!("{input}{noise}");
            let plain = parse(&input);
            let tracked = parse_with_source(&input, Some("input.css"));
            match (plain, tracked) {
                (Ok(plain), Ok(mut tracked)) => {
                    prop_assert_eq!(&plain, &tracked);
                    let units = input.encode_utf16().count();
                    fn check(nodes: &[tailwind::ast::AstNode], units: usize) -> Result<(), TestCaseError> {
                        for node in nodes {
                            if let Some(src) = node.src() {
                                prop_assert!(src.start <= src.end && src.end <= units, "{src:?}");
                            } else {
                                prop_assert!(node.loc().is_none(), "a node of the parser has no location");
                            }
                            if let Some(children) = node.children() {
                                check(children, units)?;
                            }
                        }
                        Ok(())
                    }
                    check(&tracked, units)?;
                    let css = to_css(&plain);
                    prop_assert_eq!(&to_css_tracked(&mut tracked), &css);
                    prop_assert_eq!(&plain, &tracked);
                    // Each printed node has its place in the output.
                    let translate = tailwind::source_maps::create_translation_map(&input, &css);
                    for node in &tracked {
                        for (_, _, start, end) in translate.translate(node) {
                            prop_assert!(start.unwrap() <= end.unwrap());
                        }
                    }
                }
                (Err(plain), Err(tracked)) => {
                    prop_assert!(tracked.loc.is_some());
                    prop_assert!(tracked.message.starts_with("input.css:"));
                    prop_assert!(tracked.message.ends_with(&plain.message));
                }
                _ => return Err(TestCaseError::fail("only one of the two parsers gave an error")),
            }
        }
    }
}
