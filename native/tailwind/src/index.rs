//! The compiler of a stylesheet: `index.ts` of Tailwind CSS.
//!
//! Upstream keeps references to nodes of the tree and changes the nodes
//! later. Here:
//!
//! - `@tailwind utilities` becomes a context node with the key
//!   `__utilities`. At the end of `compile`, the compiler records the path
//!   of this node and removes the key. `build` puts the rules of the
//!   candidates into the node at that path.
//! - The first `@theme` becomes the rule `:root, :host` with one empty
//!   context node `{ theme: true }`. No other step makes this shape, so the
//!   compiler finds the rule again when the theme is complete.
//! - Each `@utility` rule stays in the tree until the end, as upstream. The
//!   utility reads a shared copy of the rule. The compiler copies the rule
//!   to the shared copy after each step that can change it.
//!
//! With the `from` option, the nodes have source locations, and
//! `build_source_map` gives the source map of the last output.

use crate::apply::substitute_at_apply;
use crate::ast::{
    at_root, at_rule, comment, context, css_context, css_parent, decl, rule, share_identity, style_rule, AstNode,
    AtRule, ContextMap, ContextValue,
};
use crate::at_import::{substitute_at_imports_tracked, LoadStylesheet};
use crate::compat::module_loader::ModuleLoader;
use crate::compat::{apply_compatibility_hooks, CompatOptions};
use crate::compile::compile_candidates;
use crate::css_functions::substitute_functions;
use crate::css_parser;
use crate::design_system::{build_design_system, DesignSystem};
use crate::optimize::optimize_ast;
use crate::source_maps::{create_source_map, DecodedSourceMap, SourceLocation};
use crate::theme::{self, Theme, ThemeOptions};
use crate::to_css::{to_css, to_css_tracked};
use crate::utilities::css_utility::{create_css_utility_shared, RegisterUtility, SharedAtRule};
use crate::utils::brace_expansion::expand;
use crate::utils::escape::{escape, unescape};
use crate::utils::js::js_trim;
use crate::utils::segment::segment;
use crate::utils::topological_sort::topological_sort;
use crate::variants::{compounds_for_selectors, is_valid_variant_name, substitute_at_variant, VariantOptions};
use crate::version::{self, Version};
use crate::walk::{walk, WalkAction};
use crate::{
    Error, Features, Polyfills, Result, FEATURES_AT_THEME, FEATURES_NONE, FEATURES_UTILITIES, POLYFILLS_ALL,
};
use indexmap::IndexMap;
use std::cell::RefCell;
use std::collections::HashSet;
use std::rc::Rc;

pub use crate::at_import::Stylesheet;

/// The version of Tailwind CSS that this compiler follows. The license
/// comment of the output has it.
pub const VERSION: &str = "4.3.5";

/// The key of the context node that is at the place of `@tailwind utilities`.
const UTILITIES_KEY: &str = "__utilities";
// `compat/js/json.rs` uses the same key: JavaScript gets the at-rule.

/// The options of `compile`.
pub struct CompileOptions<'a> {
    /// The directory of the stylesheet, for its imports and sources.
    pub base: String,
    /// The fallbacks that the output has.
    pub polyfills: Polyfills,
    /// The loader of the stylesheets of `@import`: `(id, base)`.
    pub load_stylesheet: Option<LoadStylesheet<'a>>,
    /// False: the output has the license comment at the top, as the CLI of
    /// Tailwind CSS prints it (upstream: `process.env.NODE_ENV !== 'test'`).
    pub test_mode: bool,
    /// The path of the stylesheet. With a path, the compiler keeps the
    /// source locations, and `Compiler::build_source_map` gives a map.
    pub from: Option<String>,
    /// The file system for the JavaScript modules of `@plugin` and
    /// `@config`. `None`: these rules are errors, as upstream with no
    /// `loadModule`.
    pub module_loader: Option<Rc<dyn ModuleLoader>>,
    /// The version of Tailwind CSS to compile as (`version.rs`). `None`:
    /// the version of the port.
    pub version: Option<Version>,
}

impl Default for CompileOptions<'_> {
    fn default() -> Self {
        Self {
            base: String::new(),
            polyfills: POLYFILLS_ALL,
            load_stylesheet: None,
            test_mode: false,
            from: None,
            module_loader: None,
            version: None,
        }
    }
}

/// A source of candidates, from `@source`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Source {
    pub base: String,
    pub pattern: String,
    pub negated: bool,
}

/// The root of the automatic source detection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Root {
    /// The stylesheet gives no root (`null` upstream).
    Default,
    /// `source(none)`: no automatic source detection.
    None,
    /// `source("…")`, relative to `base`.
    Pattern { base: String, pattern: String },
}

/// A compiled stylesheet. `build` gives the CSS for a set of candidates.
pub struct Compiler {
    pub sources: Vec<Source>,
    pub root: Root,
    pub features: Features,

    design_system: DesignSystem,
    ast: Vec<AstNode>,
    /// The text of the stylesheet: the output when it uses no feature.
    input: String,
    /// The path of the node that gets the rules of the candidates.
    utilities_path: Option<Vec<usize>>,
    polyfills: Polyfills,
    /// All the candidates that are possibly valid, in the sequence of their
    /// first use.
    all_valid_candidates: Vec<String>,
    all_valid_candidates_set: HashSet<String>,
    /// The version of Tailwind CSS that this compiler follows.
    version: Version,
    /// The output of the last compilation.
    compiled: Option<String>,
    previous_ast_node_count: usize,
    default_did_change: bool,
    /// True when `from` is a path: the compiler keeps the source locations.
    track: bool,
    /// The location of `@tailwind utilities`.
    utilities_src: Option<SourceLocation>,
    /// The printed tree of the last compilation, for the source map.
    compiled_ast: Option<Vec<AstNode>>,
}

impl Compiler {
    /// The warnings of the compiler since the last call: each text that
    /// Tailwind CSS gives to `console.warn` in its own code (a data type
    /// that `--value(…)` of `@utility` cannot have). The JavaScript of
    /// the plugin layer and of a plugin gives its warnings to the
    /// `ModuleLoader` (`console`).
    pub fn take_warnings(&self) -> Vec<String> {
        std::mem::take(&mut *self.design_system.warnings.borrow_mut())
    }

    /// The CSS for all the candidates of this call and of the calls before.
    ///
    /// When a function of a JavaScript plugin throws, upstream throws from
    /// `build`. Upstream also throws when a candidate uses a custom variant
    /// whose `@variant` rule is not valid. Here `try_build` gives that
    /// error, and `build` gives the CSS without the candidate of the
    /// failure.
    pub fn build(&mut self, candidates: &[String]) -> String {
        let _version = version::enter(self.version);
        let output = self.build_css(candidates);
        self.design_system.take_js_error();
        self.design_system.variants.take_error();
        output
    }

    /// `build`, with the error of a JavaScript plugin function or of a
    /// custom variant.
    pub fn try_build(&mut self, candidates: &[String]) -> Result<String> {
        let _version = version::enter(self.version);
        self.design_system.variants.take_error();
        let output = self.build_css(candidates);
        if let Some(error) = self.design_system.take_js_error() {
            return Err(error);
        }
        match self.design_system.variants.take_error() {
            Some(error) => Err(error),
            None => Ok(output),
        }
    }

    fn build_css(&mut self, candidates: &[String]) -> String {
        if self.features == FEATURES_NONE {
            return self.input.clone();
        }

        let Some(utilities_path) = self.utilities_path.clone() else {
            return self.cached();
        };

        let mut did_change = self.default_did_change;
        let mut did_add_external_variable = false;
        self.default_did_change = false;

        // Add each new candidate, unless it is known as invalid.
        for candidate in candidates {
            if self.design_system.invalid_candidates.borrow().contains(candidate) {
                continue;
            }
            if candidate.starts_with("--") {
                let did_mark_variable_as_used = self.design_system.theme.mark_used_variable(candidate);
                did_change |= did_mark_variable_as_used;
                did_add_external_variable |= did_mark_variable_as_used;
            } else if self.all_valid_candidates_set.insert(candidate.clone()) {
                self.all_valid_candidates.push(candidate.clone());
                did_change = true;
            }
        }

        // No new candidate: the output is the same.
        if !did_change {
            return self.cached();
        }

        let mut invalid: Vec<String> = Vec::new();
        let mut new_nodes: Vec<AstNode> = compile_candidates(
            &self.all_valid_candidates,
            &self.design_system,
            &mut |candidate| invalid.push(candidate.to_string()),
            true,
        )
        .into_iter()
        .map(|sorted| sorted.node)
        .collect();
        self.design_system.invalid_candidates.borrow_mut().extend(invalid);

        // A rule of a candidate comes from `@tailwind utilities`. The nodes
        // from `@utility` and `@custom-variant` keep their locations.
        if self.track {
            if let Some(src) = &self.utilities_src {
                set_missing_src(&mut new_nodes, src);
            }
        }

        // No new rule: the output is the same.
        if !did_add_external_variable && self.previous_ast_node_count == new_nodes.len() {
            return self.cached();
        }

        self.previous_ast_node_count = new_nodes.len();

        if let Some(AstNode::Context(utilities)) = node_at_mut(&mut self.ast, &utilities_path) {
            utilities.nodes = new_nodes;
        }

        self.compiled = None;
        self.cached()
    }

    /// The output of the last compilation. Compile when there is none.
    fn cached(&mut self) -> String {
        if self.compiled.is_none() {
            let mut optimized = optimize_ast(&self.ast, &self.design_system, self.polyfills);
            if self.track {
                self.compiled = Some(to_css_tracked(&mut optimized));
                self.compiled_ast = Some(optimized);
            } else {
                self.compiled = Some(to_css(&optimized));
            }
        }
        self.compiled.clone().unwrap_or_default()
    }

    /// The source map of the last output of `build` (`buildSourceMap`).
    /// It has mappings only when `compile` got the `from` option.
    pub fn build_source_map(&self) -> DecodedSourceMap {
        let _version = version::enter(self.version);
        match &self.compiled_ast {
            Some(ast) => create_source_map(ast),
            // No compilation yet, or the stylesheet uses no feature: the
            // tree is the tree of the input.
            None => match self.ast.iter().find_map(|node| match node {
                AstNode::Context(root) => Some(&root.nodes),
                _ => None,
            }) {
                Some(nodes) => create_source_map(nodes),
                None => DecodedSourceMap::default(),
            },
        }
    }

    /// The design system of the stylesheet.
    pub fn design_system(&self) -> &DesignSystem {
        &self.design_system
    }
}

/// Set the `src` of each node that has none (`node.src ??= src`).
fn set_missing_src(nodes: &mut [AstNode], src: &SourceLocation) {
    for node in nodes {
        if let Some(loc) = node.loc_mut() {
            if loc.src().is_none() {
                loc.set_src(Some(src.clone()));
            }
        }
        if let Some(children) = node.children_mut() {
            set_missing_src(children, src);
        }
    }
}

fn node_at_mut<'a>(tree: &'a mut [AstNode], path: &[usize]) -> Option<&'a mut AstNode> {
    let (first, rest) = path.split_first()?;
    let mut node = tree.get_mut(*first)?;
    for index in rest {
        node = node.children_mut()?.get_mut(*index)?;
    }
    Some(node)
}

/// Compile a stylesheet.
pub fn compile(css: &str, options: CompileOptions) -> Result<Compiler> {
    let version = options.version.unwrap_or(Version::NATIVE);
    let _version = version::enter(version);
    let ast = css_parser::parse_with_source(css, options.from.as_deref())?;
    let mut parsed = parse_css(ast, &options)?;

    if !options.test_mode {
        parsed.ast.insert(0, comment(format!("! tailwindcss v{version} | MIT License | https://tailwindcss.com ")));
    }

    // The node of `@tailwind utilities` becomes a plain context node. It
    // prints its children directly.
    let utilities_path = find_utilities(&mut parsed.ast, &mut Vec::new());

    let mut all_valid_candidates = Vec::new();
    let mut all_valid_candidates_set = HashSet::new();
    let mut default_did_change = false;
    for candidate in parsed.inline_candidates {
        if !parsed.design_system.invalid_candidates.borrow().contains(&candidate) {
            if all_valid_candidates_set.insert(candidate.clone()) {
                all_valid_candidates.push(candidate);
            }
            default_did_change = true;
        }
    }

    Ok(Compiler {
        sources: parsed.sources,
        root: parsed.root,
        features: parsed.features,
        design_system: parsed.design_system,
        ast: parsed.ast,
        input: css.to_string(),
        utilities_path,
        polyfills: options.polyfills,
        version,
        all_valid_candidates,
        all_valid_candidates_set,
        compiled: None,
        previous_ast_node_count: 0,
        default_did_change,
        track: options.from.as_deref().is_some_and(|from| !from.is_empty()),
        utilities_src: parsed.utilities_src,
        compiled_ast: None,
    })
}

/// Find the marked node of `@tailwind utilities`, remove its mark, and give
/// its path.
fn find_utilities(nodes: &mut [AstNode], path: &mut Vec<usize>) -> Option<Vec<usize>> {
    for (index, node) in nodes.iter_mut().enumerate() {
        path.push(index);
        if let AstNode::Context(node) = node {
            if node.context.remove(UTILITIES_KEY).is_some() {
                return Some(path.clone());
            }
        }
        if let Some(children) = node.children_mut() {
            if let Some(found) = find_utilities(children, path) {
                return Some(found);
            }
        }
        path.pop();
    }
    None
}

/// The design system of a stylesheet (`__unstable__loadDesignSystem`).
pub fn load_design_system(css: &str, options: CompileOptions) -> Result<DesignSystem> {
    Ok(parse_css(css_parser::parse_with_source(css, options.from.as_deref())?, &options)?.design_system)
}

/// `parseThemeOptions`
fn parse_theme_options(params: &str) -> (ThemeOptions, Option<String>) {
    let mut options = theme::NONE;
    let mut prefix = None;

    for option in segment(params, b' ') {
        match option {
            "reference" => options |= theme::REFERENCE,
            "inline" => options |= theme::INLINE,
            "default" => options |= theme::DEFAULT,
            "static" => options |= theme::STATIC,
            _ if option.starts_with("prefix(") && option.ends_with(')') => {
                prefix = Some(slice(option, 7).to_string());
            }
            _ => {}
        }
    }

    (options, prefix)
}

/// `text.slice(start, -1)`, where the first `start` characters are ASCII.
fn slice(text: &str, start: usize) -> &str {
    let Some(rest) = text.get(start..) else { return "" };
    let mut chars = rest.chars();
    chars.next_back();
    chars.as_str()
}

/// False when the path has no quotes or its quotes do not agree.
fn is_quoted(path: &str) -> bool {
    let first = path.chars().next();
    let last = path.chars().next_back();
    match first {
        Some('"') => last == Some('"'),
        Some('\'') => last == Some('\''),
        _ => false,
    }
}

/// `/^[a-z]+$/`
fn is_valid_prefix(prefix: &str) -> bool {
    !prefix.is_empty() && prefix.bytes().all(|b| b.is_ascii_lowercase())
}

fn context_with(key: &str, value: ContextValue) -> ContextMap {
    let mut map = ContextMap::new();
    map.insert(key.to_string(), value);
    map
}

/// A function that registers one custom variant.
type RegisterVariant = Box<dyn Fn(&mut DesignSystem)>;

/// What `parse_css` gives.
struct Parsed {
    design_system: DesignSystem,
    ast: Vec<AstNode>,
    sources: Vec<Source>,
    root: Root,
    features: Features,
    inline_candidates: Vec<String>,
    utilities_src: Option<SourceLocation>,
}

/// What the walk of the at-rules collects.
struct Collector {
    features: Features,
    important: bool,
    theme: Theme,
    custom_variants: IndexMap<String, RegisterVariant>,
    custom_variant_dependencies: IndexMap<String, Vec<String>>,
    custom_utilities: Vec<(SharedAtRule, RegisterUtility)>,
    has_first_theme_rule: bool,
    has_utilities_node: bool,
    utilities_src: Option<SourceLocation>,
    sources: Vec<Source>,
    inline_candidates: Vec<String>,
    ignored_candidates: Vec<String>,
    root: Root,
}

fn no_stylesheet_loader(_: &str, _: &str) -> Result<Stylesheet> {
    Err(Error::new("No `loadStylesheet` function provided to `compile`"))
}

fn parse_css(ast: Vec<AstNode>, options: &CompileOptions) -> Result<Parsed> {
    let mut features = FEATURES_NONE;
    let mut ast = vec![context(context_with("base", ContextValue::Str(options.base.clone())), ast)];

    features |= substitute_at_imports_tracked(
        &mut ast,
        &options.base,
        options.load_stylesheet.unwrap_or(&no_stylesheet_loader),
        options.from.is_some(),
    )?;

    let mut collector = Collector {
        features: FEATURES_NONE,
        important: false,
        theme: Theme::new(),
        custom_variants: IndexMap::new(),
        custom_variant_dependencies: IndexMap::new(),
        custom_utilities: Vec::new(),
        has_first_theme_rule: false,
        has_utilities_node: false,
        utilities_src: None,
        sources: Vec::new(),
        inline_candidates: Vec::new(),
        ignored_candidates: Vec::new(),
        root: Root::Default,
    };

    // Handle the at-rules.
    let mut error: Option<Error> = None;
    walk(&mut ast, |node, ctx| match collector.visit(node, ctx.path) {
        Ok(action) => action,
        Err(e) => {
            error = Some(e);
            WalkAction::Stop
        }
    });
    if let Some(error) = error {
        return Err(error);
    }

    let Collector {
        features: collected_features,
        important,
        theme,
        custom_variants,
        custom_variant_dependencies,
        custom_utilities,
        has_first_theme_rule,
        utilities_src,
        mut sources,
        inline_candidates,
        ignored_candidates,
        root,
        ..
    } = collector;
    features |= collected_features;

    let mut design_system = build_design_system(theme);
    design_system.track_sources.set(options.from.as_deref().is_some_and(|from| !from.is_empty()));

    if important {
        design_system.important = true;
    }

    design_system.invalid_candidates.borrow_mut().extend(ignored_candidates);

    // The compatibility layer.
    features |= apply_compatibility_hooks(
        &mut design_system,
        &mut ast,
        &mut sources,
        CompatOptions { base: &options.base, module_loader: options.module_loader.clone() },
    )?;

    // Register each custom variant now: its position in the list of the
    // variants is its position in the CSS.
    for name in custom_variants.keys() {
        design_system.variants.r#static(name.clone(), |_, _, _| Some(()), VariantOptions::default());
    }

    // Register the custom variants: a variant comes after the variants that
    // it uses.
    let graph: Vec<(String, Vec<String>)> = custom_variant_dependencies.into_iter().collect();
    let sorted = topological_sort(&graph);
    // Before 4.1.14, custom variants that use each other in a circle were
    // no error: an `@variant` rule in `@custom-variant` stayed in the
    // output. For such a stylesheet, these rules stay, as they did then.
    let sorted = match sorted {
        Err(_) if !crate::switches::CUSTOM_VARIANT_CYCLE_ERROR.on() => {
            design_system.variants.keep_at_variant = true;
            Ok(custom_variants.keys().cloned().collect())
        }
        sorted => sorted,
    };
    let sorted = sorted.map_err(|circular| {
        let nodes: Vec<AstNode> = circular
            .path
            .iter()
            .enumerate()
            .map(|(index, name)| {
                let next = circular.path.get(index + 1).unwrap_or(&circular.start);
                at_rule("@custom-variant", name.clone(), vec![at_rule("@variant", next.clone(), Vec::new())])
            })
            .collect();
        let start = &circular.start;
        let output = to_css(&nodes).replace(';', " { … }").replacen(
            &format!("@custom-variant {start} {{"),
            &format!("@custom-variant {start} {{ /* ← */"),
            1,
        );
        Error::new(format!("Circular dependency detected in custom variants:\n\n{output}"))
    })?;
    for variant in sorted {
        if let Some(register) = custom_variants.get(&variant) {
            register(&mut design_system);
        }
    }

    // Register the custom utilities. The walk can change a rule after the
    // collection, and the registration of a functional utility changes the
    // shared copy. Thus copy the rule to the shared copy, register, and
    // copy the result back to the tree.
    let sync = |ordinal: usize, node: &AtRule| {
        if let Some((shared, _)) = custom_utilities.get(ordinal) {
            *shared.borrow_mut() = node.clone();
        }
    };
    for_each_utility(&mut ast, &mut 0, &mut |ordinal, node| sync(ordinal, node));
    for (_, register) in &custom_utilities {
        register(&mut design_system);
    }
    for_each_utility(&mut ast, &mut 0, &mut |ordinal, node| {
        if let Some((shared, _)) = custom_utilities.get(ordinal) {
            *node = shared.borrow().clone();
        }
    });

    // Put the variables of the theme at the place of the first `@theme`.
    if has_first_theme_rule {
        let mut nodes = Vec::new();
        for (key, value) in design_system.theme.entries() {
            if value.options & theme::REFERENCE != 0 {
                continue;
            }
            nodes.push(decl(escape(&key).into_owned(), value.value).with_src(value.src.as_ref()));
        }

        // An `AtRoot` moves the `@keyframes` out of `:root`. They are at
        // the top level of the tree, so that a `@reference` cannot remove
        // them.
        for keyframes in design_system.theme.get_keyframes() {
            ast.push(context(context_with("theme", true.into()), vec![at_root(vec![AstNode::AtRule(keyframes)])]));
        }

        fill_first_theme_rule(&mut ast, &mut Some(nodes));
    }

    features |= substitute_at_variant(&mut ast, &design_system)?;
    features |= substitute_functions(&mut ast, &design_system)?;

    // The utilities read the shared copies of the `@utility` rules.
    for_each_utility(&mut ast, &mut 0, &mut |ordinal, node| sync(ordinal, node));
    features |= substitute_at_apply(&mut ast, &design_system, Some(&mut |ordinal, node| sync(ordinal, node)))?;

    // Remove each `@utility`. This was not possible before the `@apply`
    // rules in them were done.
    walk(&mut ast, |node, _| match node {
        AstNode::AtRule(rule) if rule.name == "@utility" => WalkAction::Replace(Vec::new()),
        // An `@utility` is at the top level, so there is no walk into the
        // other at-rules.
        AstNode::AtRule(_) => WalkAction::Skip,
        _ => WalkAction::Continue,
    });

    Ok(Parsed { design_system, ast, sources, root, features, inline_candidates, utilities_src })
}

/// Call the function for each `@utility` rule of the tree, in document
/// order, with the position of the rule in that sequence.
fn for_each_utility(nodes: &mut [AstNode], ordinal: &mut usize, f: &mut dyn FnMut(usize, &mut AtRule)) {
    for node in nodes.iter_mut() {
        if let AstNode::AtRule(rule) = node {
            if rule.name == "@utility" {
                f(*ordinal, rule);
                *ordinal += 1;
            }
        }
        if let Some(children) = node.children_mut() {
            for_each_utility(children, ordinal, f);
        }
    }
}

/// True for the rule that is at the place of the first `@theme`.
fn is_first_theme_rule(node: &AstNode) -> bool {
    let AstNode::Rule(rule) = node else { return false };
    if rule.selector != ":root, :host" || rule.nodes.len() != 1 {
        return false;
    }
    matches!(&rule.nodes[0], AstNode::Context(inner)
        if inner.nodes.is_empty() && inner.context.len() == 1 && inner.context.get("theme") == Some(&ContextValue::Bool(true)))
}

/// Put the declarations of the theme into the rule of the first `@theme`.
fn fill_first_theme_rule(nodes: &mut [AstNode], declarations: &mut Option<Vec<AstNode>>) {
    for node in nodes.iter_mut() {
        if declarations.is_none() {
            return;
        }
        if is_first_theme_rule(node) {
            if let (AstNode::Rule(rule), Some(declarations)) = (&mut *node, declarations.take()) {
                if let AstNode::Context(inner) = &mut rule.nodes[0] {
                    inner.nodes = declarations;
                }
            }
            return;
        }
        if let Some(children) = node.children_mut() {
            fill_first_theme_rule(children, declarations);
        }
    }
}

impl Collector {
    /// The visit of one node of the walk of the at-rules. `ancestors` is
    /// the path of the walk.
    fn visit(&mut self, node: &mut AstNode, ancestors: &[AstNode]) -> Result<WalkAction<AstNode>> {
        let AstNode::AtRule(at) = node else { return Ok(WalkAction::Continue) };
        let has_parent = css_parent(ancestors).is_some();

        // Find `@tailwind utilities`. `build` replaces it with the rules of
        // the candidates.
        if at.name == "@tailwind" && at.params.starts_with("utilities") {
            // Only the first `@tailwind utilities` stays.
            if self.has_utilities_node {
                return Ok(WalkAction::Replace(Vec::new()));
            }

            // In a `@reference`, `@tailwind utilities` is not there.
            let node_context = css_context(ancestors);
            if node_context.get("reference").is_some_and(|v| v.truthy()) {
                return Ok(WalkAction::Replace(Vec::new()));
            }

            for param in segment(&at.params, b' ') {
                if !param.starts_with("source(") {
                    continue;
                }
                let path = slice(param, 7);

                // The keyword `source(none)`.
                if path == "none" {
                    self.root = Root::None;
                    continue;
                }

                // A path: `source('…')`.
                if !is_quoted(path) {
                    return Err(Error::new("`source(…)` paths must be quoted."));
                }

                let base = node_context.get("sourceBase").or_else(|| node_context.get("base"));
                self.root = Root::Pattern {
                    base: base.and_then(|v| v.as_str()).unwrap_or("").to_string(),
                    pattern: slice(path, 1).to_string(),
                };
            }

            self.has_utilities_node = true;
            self.utilities_src = at.loc.src().cloned();
            self.features |= FEATURES_UTILITIES;
            let params = at.params.clone();
            *node = context(context_with(UTILITIES_KEY, params.into()), Vec::new());
            return Ok(WalkAction::Continue);
        }

        match at.name.as_str() {
            "@utility" => self.visit_utility(at, has_parent),
            "@source" => self.visit_source(at, has_parent, ancestors),
            "@variant" | "@custom-variant" => self.visit_variant(at, has_parent),
            "@media" => self.visit_media(node, ancestors),
            "@theme" => self.visit_theme(node, ancestors),
            _ => Ok(WalkAction::Continue),
        }
    }

    /// Collect an `@utility` rule. The rule stays in the tree.
    fn visit_utility(&mut self, at: &mut AtRule, has_parent: bool) -> Result<WalkAction<AstNode>> {
        if has_parent {
            return Err(Error::new("`@utility` cannot be nested."));
        }

        if at.nodes.is_empty() {
            return Err(Error::new(format!(
                "`@utility {}` is empty. Utilities should include at least one property.",
                at.params
            )));
        }

        let shared: SharedAtRule = Rc::new(RefCell::new(at.clone()));
        let Some(utility) = create_css_utility_shared(shared.clone()) else {
            let params = &at.params;
            if !params.ends_with("-*") {
                if params.ends_with('*') {
                    return Err(Error::new(format!(
                        "`@utility {params}` defines an invalid utility name. A functional utility must end in `-*`."
                    )));
                } else if params.contains('*') {
                    return Err(Error::new(format!(
                        "`@utility {params}` defines an invalid utility name. The dynamic portion marked by `-*` must appear once at the end."
                    )));
                }
            }

            return Err(Error::new(format!(
                "`@utility {params}` defines an invalid utility name. Utilities should be alphanumeric and start with a lowercase letter."
            )));
        };

        self.custom_utilities.push((shared, utility));
        Ok(WalkAction::Continue)
    }

    /// Collect the path or the candidates of an `@source` rule.
    fn visit_source(&mut self, at: &mut AtRule, has_parent: bool, ancestors: &[AstNode]) -> Result<WalkAction<AstNode>> {
        if !at.nodes.is_empty() {
            return Err(Error::new("`@source` cannot have a body."));
        }

        if has_parent {
            return Err(Error::new("`@source` cannot be nested."));
        }

        let mut not = false;
        let mut inline = false;
        let mut path: &str = &at.params;

        if path.starts_with("not ") {
            not = true;
            path = &path[4..];
        }

        if path.starts_with("inline(") {
            inline = true;
            path = slice(path, 7);
            // Before 4.2.0, white space around the argument was an error.
            if crate::switches::SOURCE_INLINE_WHITESPACE.on() {
                path = js_trim(path);
            }
        }

        if !is_quoted(path) {
            return Err(Error::new("`@source` paths must be quoted."));
        }

        let source = slice(path, 1);

        if inline {
            let destination = if not { &mut self.ignored_candidates } else { &mut self.inline_candidates };
            for source in segment(source, b' ') {
                destination.extend(expand(source).map_err(|e| Error::new(e.to_string()))?);
            }
        } else {
            let base = css_context(ancestors).get("base").and_then(|v| v.as_str()).unwrap_or("").to_string();
            self.sources.push(Source { base, pattern: source.to_string(), negated: not });
        }

        Ok(WalkAction::ReplaceSkip(Vec::new()))
    }

    /// Handle an `@variant` or an `@custom-variant` rule.
    fn visit_variant(&mut self, at: &mut AtRule, has_parent: bool) -> Result<WalkAction<AstNode>> {
        if at.name == "@variant" {
            // An `@variant` in a rule stays: `substitute_at_variant`
            // replaces it when all the variants are registered.
            if has_parent {
                return Ok(WalkAction::Continue);
            }

            // An `@variant` at the top level with no body or with an
            // `@slot` is an `@custom-variant` of older versions.
            if !at.nodes.is_empty() && !has_slot(&at.nodes) {
                return Ok(WalkAction::Continue);
            }
            at.name = "@custom-variant".to_string();
        }

        // Register the custom variant.
        if has_parent {
            return Err(Error::new("`@custom-variant` cannot be nested."));
        }

        let parts = segment(&at.params, b' ');
        let name = parts[0].to_string();
        let selector = parts.get(1).copied().filter(|selector| !selector.is_empty());

        if !is_valid_variant_name(&name) {
            return Err(Error::new(format!(
                "`@custom-variant {name}` defines an invalid variant name. Variants should only contain alphanumeric, dashes, or underscore characters and start with a lowercase letter or number."
            )));
        }

        if !at.nodes.is_empty() && selector.is_some() {
            return Err(Error::new(format!("`@custom-variant {name}` cannot have both a selector and a body.")));
        }

        if at.nodes.is_empty() {
            // A selector and no body: `@custom-variant hocus (&:hover, &:focus);`
            let Some(selector) = selector else {
                return Err(Error::new(format!("`@custom-variant {name}` has no selector or body.")));
            };

            let selectors = segment(slice(selector, 1), b',');
            if selectors.iter().any(|selector| js_trim(selector).is_empty()) {
                return Err(Error::new(format!(
                    "`@custom-variant {name} ({})` selector is invalid.",
                    selectors.join(",")
                )));
            }

            let mut at_rule_params: Vec<String> = Vec::new();
            let mut style_rule_selectors: Vec<String> = Vec::new();
            for selector in selectors {
                let selector = js_trim(selector);
                if selector.starts_with('@') {
                    at_rule_params.push(selector.to_string());
                } else {
                    style_rule_selectors.push(selector.to_string());
                }
            }

            let uses_at_scope =
                crate::switches::SCOPE_HOISTING.on() && at_rule_params.iter().any(|param| param.starts_with("@scope"));
            let all: Vec<&str> = style_rule_selectors.iter().chain(&at_rule_params).map(String::as_str).collect();
            let compounds = compounds_for_selectors(&all);

            let variant_name = name.clone();
            self.custom_variants.insert(
                name.clone(),
                Box::new(move |design_system: &mut DesignSystem| {
                    let style_rule_selectors = style_rule_selectors.clone();
                    let at_rule_params = at_rule_params.clone();
                    design_system.variants.r#static(
                        variant_name.clone(),
                        move |r, _, ds| {
                            let children = r.children_mut()?;
                            let mut body = std::mem::take(children);
                            // Upstream puts the same nodes in each rule.
                            if ds.track_sources.get() && style_rule_selectors.len().min(1) + at_rule_params.len() > 1 {
                                share_identity(&mut body);
                            }
                            let slot = if uses_at_scope {
                                vec![context(context_with("source", "user".into()), body)]
                            } else {
                                body
                            };

                            let mut nodes: Vec<AstNode> = Vec::new();
                            if !style_rule_selectors.is_empty() {
                                nodes.push(style_rule(style_rule_selectors.join(", "), slot.clone()));
                            }
                            for selector in &at_rule_params {
                                nodes.push(rule(selector.clone(), slot.clone()));
                            }

                            *children = if uses_at_scope {
                                vec![context(context_with("source", "variant".into()), nodes)]
                            } else {
                                nodes
                            };
                            Some(())
                        },
                        VariantOptions { compounds: Some(compounds), order: None },
                    );
                }),
            );
            self.custom_variant_dependencies.insert(name, Vec::new());
        } else {
            // A body and no selector: the body has `@slot` rules.
            let mut dependencies: Vec<String> = Vec::new();
            collect_variant_dependencies(&at.nodes, &mut dependencies);

            let variant_name = name.clone();
            let body = at.nodes.clone();
            self.custom_variants.insert(
                name.clone(),
                Box::new(move |design_system: &mut DesignSystem| {
                    design_system.variants.from_ast(variant_name.clone(), body.clone());
                }),
            );
            self.custom_variant_dependencies.insert(name, dependencies);
        }

        // The `@custom-variant` rule is not in the output.
        Ok(WalkAction::ReplaceSkip(Vec::new()))
    }

    /// Handle the options of an import, which are the parameters of an
    /// `@media` rule: `source(…)`, `theme(…)`, `prefix(…)`, `important` and
    /// `reference`.
    fn visit_media(&mut self, node: &mut AstNode, ancestors: &[AstNode]) -> Result<WalkAction<AstNode>> {
        let AstNode::AtRule(at) = node else { return Ok(WalkAction::Continue) };
        let params: Vec<String> = segment(&at.params, b' ').into_iter().map(str::to_string).collect();
        let mut unknown_params: Vec<&str> = Vec::new();

        for param in &params {
            if param.starts_with("source(") {
                // `@media source(…)`
                let path = slice(param, 7);
                let base = css_context(ancestors).get("base").cloned();

                walk(&mut at.nodes, |child, _| {
                    let AstNode::AtRule(child_rule) = child else { return WalkAction::Continue };
                    if child_rule.name == "@tailwind" && child_rule.params == "utilities" {
                        child_rule.params.push_str(&format!(" source({path})"));
                        let mut source_context = ContextMap::new();
                        if let Some(base) = &base {
                            source_context.insert("sourceBase".to_string(), base.clone());
                        }
                        let child = std::mem::replace(child, comment(""));
                        return WalkAction::ReplaceStop(vec![context(source_context, vec![child])]);
                    }
                    WalkAction::Continue
                });
            } else if param.starts_with("theme(") {
                // `@media theme(…)`: `@import "…" theme(reference)` imports
                // a theme file as a reference.
                let theme_params = slice(param, 6);
                let has_reference = theme_params.contains("reference");
                let mut invalid = false;

                walk(&mut at.nodes, |child, _| match child {
                    AstNode::Context(_) => WalkAction::Continue,
                    AstNode::AtRule(child_rule) => {
                        if child_rule.name == "@theme" {
                            child_rule.params.push(' ');
                            child_rule.params.push_str(theme_params);
                            return WalkAction::Skip;
                        }
                        WalkAction::Continue
                    }
                    _ => {
                        if has_reference {
                            invalid = true;
                            return WalkAction::Stop;
                        }
                        WalkAction::Continue
                    }
                });

                if invalid {
                    return Err(Error::new(
                        "Files imported with `@import \"…\" theme(reference)` must only contain `@theme` blocks.\nUse `@reference \"…\";` instead.",
                    ));
                }
            } else if param.starts_with("prefix(") {
                // `@media prefix(…)`: `@import "…" prefix(ident)` sets the
                // prefix of the variables and of the utilities.
                let prefix = slice(param, 7);

                walk(&mut at.nodes, |child, _| match child {
                    AstNode::AtRule(child_rule) if child_rule.name == "@theme" => {
                        child_rule.params.push_str(&format!(" prefix({prefix})"));
                        WalkAction::Skip
                    }
                    _ => WalkAction::Continue,
                });
            } else if param == "important" {
                self.important = true;
            } else if param == "reference" {
                // `@import "…" reference`
                let nodes = std::mem::take(&mut at.nodes);
                at.nodes = vec![context(context_with("reference", true.into()), nodes)];
            } else {
                unknown_params.push(param);
            }
        }

        if !unknown_params.is_empty() {
            at.params = unknown_params.join(" ");
        } else if !params.is_empty() {
            return Ok(WalkAction::Replace(std::mem::take(&mut at.nodes)));
        }

        Ok(WalkAction::Continue)
    }

    /// Record the variables of an `@theme` rule.
    fn visit_theme(&mut self, node: &mut AstNode, ancestors: &[AstNode]) -> Result<WalkAction<AstNode>> {
        let AstNode::AtRule(at) = node else { return Ok(WalkAction::Continue) };
        let (mut theme_options, theme_prefix) = parse_theme_options(&at.params);

        // Before 4.1.14, `@theme` was not a feature: with no other feature,
        // the output was the input.
        if crate::switches::AT_THEME_IS_A_FEATURE.on() {
            self.features |= FEATURES_AT_THEME;
        }

        if css_context(ancestors).get("reference").is_some_and(|v| v.truthy()) {
            theme_options |= theme::REFERENCE;
        }

        if let Some(theme_prefix) = theme_prefix.filter(|prefix| !prefix.is_empty()) {
            if !is_valid_prefix(&theme_prefix) {
                return Err(Error::new(format!(
                    "The prefix \"{theme_prefix}\" is invalid. Prefixes must be lowercase ASCII letters (a-z) only."
                )));
            }

            self.theme.prefix = Some(theme_prefix);
        }

        // Record each custom property of the `@theme` rule.
        let mut error: Option<Error> = None;
        let (name, params) = (at.name.clone(), at.params.clone());
        let theme = &self.theme;
        walk(&mut at.nodes, |child, _| {
            match child {
                // The `@theme` rule itself goes away, so keep the
                // `@keyframes` rules for the output.
                AstNode::AtRule(child_rule) if child_rule.name == "@keyframes" => {
                    theme.add_keyframes(child_rule.clone());
                    return WalkAction::Skip;
                }
                AstNode::Comment(_) => return WalkAction::Continue,
                AstNode::Declaration(declaration) if declaration.property.starts_with("--") => {
                    let value = declaration.value.as_deref().unwrap_or("");
                    let src = declaration.loc.src().cloned();
                    if let Err(e) = theme.add_with_src(&unescape(&declaration.property), value, theme_options, src) {
                        error = Some(e);
                        return WalkAction::Stop;
                    }
                    return WalkAction::Continue;
                }
                _ => {}
            }

            let css = to_css(&[at_rule(name.clone(), params.clone(), vec![child.clone()])]);
            let lines: Vec<&str> = css.split('\n').collect();
            let snippet: Vec<String> = lines
                .iter()
                .enumerate()
                .map(|(index, line)| {
                    let mark = if index == 0 || index + 2 >= lines.len() { ' ' } else { '>' };
                    format!("{mark} {line}")
                })
                .collect();

            error = Some(Error::new(format!(
                "`@theme` blocks must only contain custom properties or `@keyframes`.\n\n{}",
                snippet.join("\n")
            )));
            WalkAction::Stop
        });
        if let Some(error) = error {
            return Err(error);
        }

        // The first `@theme` rule becomes the rule that gets the complete
        // theme later. The other `@theme` rules go away.
        if !self.has_first_theme_rule {
            self.has_first_theme_rule = true;
            let marker = context(context_with("theme", true.into()), Vec::new());
            Ok(WalkAction::ReplaceSkip(vec![style_rule(":root, :host", vec![marker]).with_src(at.loc.src())]))
        } else {
            Ok(WalkAction::ReplaceSkip(Vec::new()))
        }
    }
}

/// True when the subtree has an `@slot` rule.
fn has_slot(nodes: &[AstNode]) -> bool {
    nodes.iter().any(|node| node.is_at_rule("@slot") || node.children().is_some_and(|children| has_slot(children)))
}

/// The names of the variants that the `@variant` rules of the subtree use.
fn collect_variant_dependencies(nodes: &[AstNode], dependencies: &mut Vec<String>) {
    for node in nodes {
        if let AstNode::AtRule(rule) = node {
            if rule.name == "@variant" && !dependencies.contains(&rule.params) {
                dependencies.push(rule.params.clone());
            }
        }
        if let Some(children) = node.children() {
            collect_variant_dependencies(children, dependencies);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options<'a>() -> CompileOptions<'a> {
        CompileOptions { test_mode: true, ..Default::default() }
    }

    fn error(css: &str) -> String {
        match compile(css, options()) {
            Ok(_) => panic!("compile gave no error"),
            Err(error) => error.message,
        }
    }

    #[test]
    fn a_stylesheet_with_no_feature_gives_its_text() {
        let css = ".a  { color : red }\n";
        let mut compiler = compile(css, CompileOptions::default()).unwrap();
        assert_eq!(compiler.features, FEATURES_NONE);
        assert_eq!(compiler.build(&["flex".to_string()]), css);
    }

    #[test]
    fn the_output_has_the_license_comment() {
        let css = "@theme { --color-red: red; }\n.a { color: var(--color-red); }\n";
        let mut compiler = compile(css, CompileOptions::default()).unwrap();
        assert!(compiler.build(&[]).starts_with("/*! tailwindcss v4.3.5 | MIT License | https://tailwindcss.com */\n"));

        let mut compiler = compile(css, options()).unwrap();
        assert!(!compiler.build(&[]).contains("MIT License"));
    }

    #[test]
    fn sources_and_root() {
        let css = "@source \"./a\";\n@source not \"./b/*.html\";\n@tailwind utilities source(\"./src\");\n";
        let compiler = compile(css, CompileOptions { base: "/root".into(), ..options() }).unwrap();
        assert_eq!(
            compiler.sources,
            vec![
                Source { base: "/root".into(), pattern: "./a".into(), negated: false },
                Source { base: "/root".into(), pattern: "./b/*.html".into(), negated: true },
            ]
        );
        assert_eq!(compiler.root, Root::Pattern { base: "/root".into(), pattern: "./src".into() });
        assert_eq!(compiler.features, FEATURES_UTILITIES);

        assert_eq!(compile("@tailwind utilities source(none);", options()).unwrap().root, Root::None);
        assert_eq!(compile("@tailwind utilities;", options()).unwrap().root, Root::Default);
    }

    #[test]
    fn the_candidates_are_cumulative() {
        let mut compiler = compile("@tailwind utilities;", options()).unwrap();
        let first = compiler.build(&["flex".to_string()]);
        assert!(first.contains(".flex {"));
        let second = compiler.build(&["underline".to_string(), "not-a-utility".to_string()]);
        assert!(second.contains(".flex {") && second.contains(".underline {"));
        assert_eq!(compiler.build(&[]), second);
        assert_eq!(compiler.build(&["not-a-utility".to_string()]), second);
    }

    #[test]
    fn inline_sources() {
        let css = "@source inline(\"{hover:,}flex\");\n@source not inline(\"underline\");\n@tailwind utilities;\n";
        let mut compiler = compile(css, options()).unwrap();
        let output = compiler.build(&["underline".to_string()]);
        assert!(output.contains(".flex {") && output.contains(".hover\\:flex"), "{output}");
        assert!(!output.contains("underline"));
    }

    #[test]
    fn errors() {
        assert_eq!(error("@import \"./a.css\";"), "No `loadStylesheet` function provided to `compile`");
        assert_eq!(error("@source ./a;"), "`@source` paths must be quoted.");
        assert_eq!(error("@tailwind utilities source(./a);"), "`source(…)` paths must be quoted.");
        assert_eq!(error(".a { @utility b { color: red; } }"), "`@utility` cannot be nested.");
        assert_eq!(
            error("@theme prefix(a1) { --color-red: red; }"),
            "The prefix \"a1\" is invalid. Prefixes must be lowercase ASCII letters (a-z) only."
        );
        assert_eq!(
            error("@theme { color: red; }"),
            "`@theme` blocks must only contain custom properties or `@keyframes`.\n\n  @theme {\n>   color: red;\n  }\n  "
        );
        assert_eq!(
            error("@custom-variant a { @variant b { @slot; } }\n@custom-variant b { @variant a { @slot; } }"),
            "Circular dependency detected in custom variants:\n\n@custom-variant a { /* ← */\n  @variant b { … }\n}\n@custom-variant b {\n  @variant a { … }\n}\n"
        );
        // With no module loader: the error of upstream with no `loadModule`,
        // or, in a build with no JavaScript engine, the error of this port.
        let (plugin, config) = if crate::compat::HAS_JAVASCRIPT {
            (crate::compat::NO_LOAD_MODULE, crate::compat::NO_LOAD_MODULE)
        } else {
            (crate::compat::PLUGIN_NEEDS_JAVASCRIPT, crate::compat::CONFIG_NEEDS_JAVASCRIPT)
        };
        assert_eq!(error("@plugin \"./plugin.js\";"), plugin);
        assert_eq!(error("@config \"./config.js\";"), config);
    }

    #[test]
    fn build_gives_the_error_of_a_custom_variant() {
        // Upstream throws from `build` when a candidate uses the variant.
        let css = "@custom-variant a {\n  @variant nope { @slot; }\n}\n@tailwind utilities;";
        let mut compiler = compile(css, options()).unwrap();
        assert_eq!(compiler.try_build(&["flex".to_string()]).unwrap(), ".flex {\n  display: flex;\n}\n");
        let error = compiler.try_build(&["a:flex".to_string()]).unwrap_err();
        assert_eq!(error.message, "Cannot use `@variant` with unknown variant: nope");
    }

    #[test]
    fn slices() {
        assert_eq!(slice("source(none)", 7), "none");
        assert_eq!(slice("\"a\"", 1), "a");
        assert_eq!(slice("\"", 1), "");
        assert_eq!(slice("", 1), "");
        assert!(is_quoted("\"a\"") && is_quoted("'a'") && is_quoted("\""));
        assert!(!is_quoted("\"a'") && !is_quoted("a") && !is_quoted(""));
    }
}
