//! The `@import` and `@reference` rules: `at-import.ts` of Tailwind CSS.
//!
//! Upstream loads the stylesheets with promises. Here the loader is a
//! synchronous callback. The sequence of the loader calls is the sequence of
//! upstream with a loader that resolves immediately: level by level, and in
//! document order in each level.

use crate::ast::{at_rule, context, AstNode, ContextMap, ContextValue};
use crate::css_parser;
use crate::source_maps::SourceLocation;
use crate::utils::js::js_to_lower_case;
use crate::value_parser::{self, ValueAstNode};
use crate::{Error, Features, Result, FEATURES_AT_IMPORT, FEATURES_NONE};
use std::collections::VecDeque;

/// A stylesheet that the loader gives for an `@import`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Stylesheet {
    /// The path of the file.
    pub path: String,
    /// The directory for the imports of this file.
    pub base: String,
    /// The CSS text.
    pub content: String,
}

/// The loader of stylesheets: `(id, base)` gives the stylesheet.
pub type LoadStylesheet<'a> = &'a dyn Fn(&str, &str) -> Result<Stylesheet>;

/// The parts of the parameters of an `@import` rule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportParams {
    pub uri: String,
    /// `Some("")` is `layer` without a name.
    pub layer: Option<String>,
    pub media: Option<String>,
    pub supports: Option<String>,
}

/// The key that marks the place of an import until its file is ready.
const PENDING_KEY: &str = "__import";

/// The largest depth of imports. Upstream cannot find circular imports, so
/// it has this limit.
const MAX_DEPTH: usize = 100;

/// An import whose file is loaded, and not yet parsed.
struct Pending {
    id: usize,
    loaded: Stylesheet,
    params: ImportParams,
    depth: usize,
    /// The location of the `@import` rule.
    src: Option<SourceLocation>,
}

/// Replace each `@import` and `@reference` rule with the content of its
/// file. The result has `FEATURES_AT_IMPORT` when the tree has an import.
pub fn substitute_at_imports(ast: &mut [AstNode], base: &str, load_stylesheet: LoadStylesheet) -> Result<Features> {
    substitute_at_imports_tracked(ast, base, load_stylesheet, false)
}

/// `substitute_at_imports`. With `track`, the nodes of each imported file
/// have their source locations: the file of a location is the path that
/// the loader gives.
pub fn substitute_at_imports_tracked(
    ast: &mut [AstNode],
    base: &str,
    load_stylesheet: LoadStylesheet,
    track: bool,
) -> Result<Features> {
    let mut queue: VecDeque<Pending> = VecDeque::new();
    let mut ready: Vec<Option<Vec<AstNode>>> = Vec::new();

    let features = collect(ast, base, load_stylesheet, 0, &mut queue, &mut ready)?;

    while let Some(pending) = queue.pop_front() {
        let from = if track { Some(pending.loaded.path.as_str()) } else { None };
        let mut imported = css_parser::parse_with_source(&pending.loaded.content, from)?;
        collect(&mut imported, &pending.loaded.base, load_stylesheet, pending.depth + 1, &mut queue, &mut ready)?;

        let mut import_context = ContextMap::new();
        import_context.insert("base".into(), ContextValue::Str(pending.loaded.base));
        ready[pending.id] =
            Some(build_import_nodes(vec![context(import_context, imported)], &pending.params, pending.src.as_ref()));
    }

    if !ready.is_empty() {
        assemble(ast, &mut ready);
    }

    Ok(features)
}

/// Find the imports of one file, load their files, and put a marked context
/// node at the place of each import.
fn collect(
    ast: &mut [AstNode],
    base: &str,
    load_stylesheet: LoadStylesheet,
    depth: usize,
    queue: &mut VecDeque<Pending>,
    ready: &mut Vec<Option<Vec<AstNode>>>,
) -> Result<Features> {
    let mut features = FEATURES_NONE;

    for node in ast.iter_mut() {
        let is_import = matches!(node, AstNode::AtRule(rule) if rule.name == "@import" || rule.name == "@reference");
        if !is_import {
            if let Some(children) = node.children_mut() {
                features |= collect(children, base, load_stylesheet, depth, queue, ready)?;
            }
            continue;
        }

        let AstNode::AtRule(rule) = &*node else { continue };
        let Some(mut params) = parse_import_params(&value_parser::parse(&rule.params))? else {
            // Not an import that the compiler can inline. The walk of
            // upstream continues into the children.
            if let Some(children) = node.children_mut() {
                features |= collect(children, base, load_stylesheet, depth, queue, ready)?;
            }
            continue;
        };
        if rule.name == "@reference" {
            params.media = Some("reference".to_string());
        }

        features |= FEATURES_AT_IMPORT;

        // A data URI or a remote URI stays in the output.
        if params.uri.starts_with("data:") || params.uri.starts_with("http://") || params.uri.starts_with("https://") {
            if let Some(children) = node.children_mut() {
                features |= collect(children, base, load_stylesheet, depth, queue, ready)?;
            }
            continue;
        }

        if depth > MAX_DEPTH {
            return Err(Error::new(format!(
                "Exceeded maximum recursion depth while resolving `{}` in `{}`)",
                params.uri, base
            )));
        }

        let loaded = load_stylesheet(&params.uri, base)?;

        let id = ready.len();
        ready.push(None);
        let src = rule.loc.src().cloned();
        queue.push_back(Pending { id, loaded, params, depth, src });

        let mut marker = ContextMap::new();
        marker.insert(PENDING_KEY.into(), ContextValue::Str(id.to_string()));
        *node = context(marker, Vec::new());
    }

    Ok(features)
}

/// Put the content of each file at the place of its marked context node.
fn assemble(ast: &mut [AstNode], ready: &mut Vec<Option<Vec<AstNode>>>) {
    for node in ast.iter_mut() {
        if let AstNode::Context(node) = node {
            let id = node.context.get(PENDING_KEY).and_then(|v| v.as_str()).and_then(|v| v.parse::<usize>().ok());
            if let Some(id) = id {
                node.context.remove(PENDING_KEY);
                node.nodes = ready[id].take().unwrap_or_default();
            }
        }
        if let Some(children) = node.children_mut() {
            assemble(children, ready);
        }
    }
}

/// Read the parameters of an `@import` rule. `None` is "do not inline this
/// import". This is the changed `parse-statements` of `postcss-import`.
pub fn parse_import_params(params: &[ValueAstNode]) -> Result<Option<ImportParams>> {
    let mut uri: Option<String> = None;
    let mut layer: Option<String> = None;
    let mut media: Option<String> = None;
    let mut supports: Option<String> = None;

    // An empty text is "no value", as in JavaScript.
    fn has(value: &Option<String>) -> bool {
        value.as_deref().is_some_and(|v| !v.is_empty())
    }

    for (i, node) in params.iter().enumerate() {
        if node.is_separator() {
            continue;
        }

        if let (ValueAstNode::Word(word), false) = (node, has(&uri)) {
            let Some(first) = word.value.chars().next() else { return Ok(None) };
            if first != '"' && first != '\'' {
                return Ok(None);
            }
            // `slice(1, -1)`.
            let mut inner = word.value[1..].chars();
            inner.next_back();
            uri = Some(inner.as_str().to_string());
            continue;
        }

        let name = js_to_lower_case(node.value());

        if node.is_function() && name == "url" {
            // An `@import url(…)` stays in the output.
            return Ok(None);
        }

        if !has(&uri) {
            return Ok(None);
        }

        if name == "layer" {
            if has(&layer) {
                return Ok(None);
            }
            if has(&supports) {
                return Err(Error::new(
                    "`layer(…)` in an `@import` should come before any other functions or conditions",
                ));
            }
            layer = Some(match node {
                ValueAstNode::Function(function) => value_parser::to_css(&function.nodes),
                _ => String::new(),
            });
            continue;
        }

        if let ValueAstNode::Function(function) = node {
            if name == "supports" {
                if has(&supports) {
                    return Ok(None);
                }
                supports = Some(value_parser::to_css(&function.nodes));
                continue;
            }
        }

        media = Some(value_parser::to_css(&params[i..]));
        break;
    }

    match uri {
        Some(uri) if !uri.is_empty() => Ok(Some(ImportParams { uri, layer, media, supports })),
        _ => Ok(None),
    }
}

/// Put the wrappers of the import conditions around the imported nodes.
fn build_import_nodes(imported: Vec<AstNode>, params: &ImportParams, src: Option<&SourceLocation>) -> Vec<AstNode> {
    let mut root = imported;

    if let Some(layer) = &params.layer {
        root = vec![at_rule("@layer", layer.clone(), root).with_src(src)];
    }

    if let Some(media) = &params.media {
        root = vec![at_rule("@media", media.clone(), root).with_src(src)];
    }

    if let Some(supports) = &params.supports {
        let condition = if supports.starts_with('(') { supports.clone() } else { format!("({supports})") };
        root = vec![at_rule("@supports", condition, root).with_src(src)];
    }

    root
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::to_css::to_css;
    use std::cell::RefCell;

    fn params(input: &str) -> Result<Option<ImportParams>> {
        parse_import_params(&value_parser::parse(input))
    }

    fn parts(input: &str) -> (String, Option<String>, Option<String>, Option<String>) {
        let p = params(input).unwrap().unwrap();
        (p.uri, p.layer, p.media, p.supports)
    }

    fn s(v: &str) -> Option<String> {
        Some(v.to_string())
    }

    #[test]
    fn reads_the_uri() {
        assert_eq!(parts("\"./a.css\""), ("./a.css".into(), None, None, None));
        assert_eq!(parts("'./a.css'"), ("./a.css".into(), None, None, None));
    }

    #[test]
    fn does_not_inline_some_imports() {
        assert_eq!(params("url(./a.css)").unwrap(), None);
        assert_eq!(params("url(\"./a.css\") print").unwrap(), None);
        assert_eq!(params("URL(./a.css)").unwrap(), None);
        assert_eq!(params("a.css").unwrap(), None);
        assert_eq!(params("").unwrap(), None);
        assert_eq!(params("\"\"").unwrap(), None);
        assert_eq!(params("layer(a) \"./a.css\"").unwrap(), None);
    }

    #[test]
    fn reads_the_conditions() {
        assert_eq!(parts("\"a.css\" layer"), ("a.css".into(), s(""), None, None));
        assert_eq!(parts("\"a.css\" layer(theme)"), ("a.css".into(), s("theme"), None, None));
        assert_eq!(parts("\"a.css\" LAYER(a.b)"), ("a.css".into(), s("a.b"), None, None));
        assert_eq!(parts("\"a.css\" print"), ("a.css".into(), None, s("print"), None));
        assert_eq!(
            parts("\"a.css\" screen and (min-width: 100px)"),
            ("a.css".into(), None, s("screen and (min-width: 100px)"), None)
        );
        assert_eq!(parts("\"a.css\" supports(display: grid)"), ("a.css".into(), None, None, s("display: grid")));
        assert_eq!(
            parts("\"a.css\" layer(x) supports(not (display: grid)) screen"),
            ("a.css".into(), s("x"), s("screen"), s("not (display: grid)"))
        );
        assert_eq!(
            parts("\"a.css\" theme(reference) source(none)"),
            ("a.css".into(), None, s("theme(reference) source(none)"), None)
        );
        // A second named layer is not valid. A second `layer` after a layer
        // without a name is a part of the media query.
        assert_eq!(params("\"a.css\" layer(a) layer(b)").unwrap(), None);
        assert_eq!(parts("\"a.css\" layer layer(b)"), ("a.css".into(), s("b"), None, None));
        assert_eq!(params("\"a.css\" supports(a) supports(b)").unwrap(), None);
    }

    #[test]
    fn layer_after_supports_is_an_error() {
        assert_eq!(
            params("\"a.css\" supports(display: grid) layer(x)").unwrap_err().message,
            "`layer(…)` in an `@import` should come before any other functions or conditions"
        );
    }

    /// The tree with the children of each context node at the place of the
    /// context node: the printer prints nothing for a context node.
    fn flatten(nodes: Vec<AstNode>) -> Vec<AstNode> {
        let mut result = Vec::new();
        for mut node in nodes {
            if let AstNode::Context(context) = node {
                result.extend(flatten(context.nodes));
                continue;
            }
            if let Some(children) = node.children_mut() {
                *children = flatten(std::mem::take(children));
            }
            result.push(node);
        }
        result
    }

    /// The CSS, the features, and the calls of the loader.
    type Run = (String, Features, Vec<(String, String)>);

    fn run(css: &str, files: &[(&str, &str)]) -> Result<Run> {
        let calls = RefCell::new(Vec::new());
        let loader = |id: &str, base: &str| -> Result<Stylesheet> {
            calls.borrow_mut().push((id.to_string(), base.to_string()));
            match files.iter().find(|(name, _)| *name == id) {
                Some((name, content)) => Ok(Stylesheet {
                    path: format!("/root/{name}"),
                    base: format!("{base}/{}", name.trim_end_matches(".css")),
                    content: content.to_string(),
                }),
                None => Err(Error::new(format!("no file {id}"))),
            }
        };
        let mut ast = css_parser::parse(css).unwrap();
        let features = substitute_at_imports(&mut ast, "/root", &loader)?;
        let css = to_css(&flatten(ast));
        Ok((css, features, calls.into_inner()))
    }

    #[test]
    fn inlines_a_file_with_wrappers() {
        let (css, features, calls) = run(
            "@import \"a.css\" layer(x) supports(display: grid) print;\n.b { color: blue }",
            &[("a.css", ".a { color: red }")],
        )
        .unwrap();
        assert_eq!(features, FEATURES_AT_IMPORT);
        assert_eq!(calls, vec![("a.css".to_string(), "/root".to_string())]);
        assert_eq!(
            css,
            "@supports (display: grid) {\n  @media print {\n    @layer x {\n      .a {\n        color: red;\n      }\n    }\n  }\n}\n.b {\n  color: blue;\n}\n"
        );
    }

    #[test]
    fn reference_is_a_media_wrapper() {
        let (css, features, _) = run("@reference \"a.css\" print;", &[("a.css", ".a { color: red }")]).unwrap();
        assert_eq!(features, FEATURES_AT_IMPORT);
        assert_eq!(css, "@media reference {\n  .a {\n    color: red;\n  }\n}\n");
    }

    #[test]
    fn keeps_remote_imports_and_url_imports() {
        let css = "@import \"https://example.com/a.css\";\n@import url(a.css);\n@import \"data:text/css,a\";\n";
        let (out, features, calls) = run(css, &[]).unwrap();
        assert_eq!(out, css);
        assert_eq!(features, FEATURES_AT_IMPORT);
        assert!(calls.is_empty());
    }

    #[test]
    fn no_import_gives_no_feature() {
        let (_, features, _) = run("@import url(a.css);\n.a { color: red }", &[]).unwrap();
        assert_eq!(features, FEATURES_NONE);
    }

    #[test]
    fn loads_level_by_level_with_the_base_of_each_file() {
        let (css, _, calls) = run(
            "@import \"a.css\";\n@import \"b.css\";",
            &[
                ("a.css", "@import \"c.css\";\n.a { color: red }"),
                ("b.css", ".b { color: red }"),
                ("c.css", ".c { color: red }"),
            ],
        )
        .unwrap();
        assert_eq!(
            calls,
            vec![
                ("a.css".to_string(), "/root".to_string()),
                ("b.css".to_string(), "/root".to_string()),
                ("c.css".to_string(), "/root/a".to_string()),
            ]
        );
        assert_eq!(css, ".c {\n  color: red;\n}\n.a {\n  color: red;\n}\n.b {\n  color: red;\n}\n");
    }

    #[test]
    fn the_context_has_the_base_of_the_file() {
        let loader = |_: &str, _: &str| -> Result<Stylesheet> {
            Ok(Stylesheet { path: "/x/a.css".into(), base: "/x".into(), content: ".a { color: red }".into() })
        };
        let mut ast = css_parser::parse("@import \"a.css\";").unwrap();
        substitute_at_imports(&mut ast, "/root", &loader).unwrap();
        let AstNode::Context(outer) = &ast[0] else { panic!("no context node") };
        assert!(outer.context.is_empty());
        let AstNode::Context(inner) = &outer.nodes[0] else { panic!("no context node") };
        assert_eq!(inner.context.get("base").and_then(|v| v.as_str()), Some("/x"));
    }

    #[test]
    fn circular_imports_have_a_limit() {
        let error = run("@import \"a.css\";", &[("a.css", "@import \"a.css\";")]).unwrap_err();
        assert!(error.message.starts_with("Exceeded maximum recursion depth while resolving `a.css` in `/root/a/a"));
        assert!(error.message.ends_with("`)"));
    }

    #[test]
    fn a_loader_error_is_the_error() {
        assert_eq!(run("@import \"x.css\";", &[]).unwrap_err().message, "no file x.css");
    }
}
