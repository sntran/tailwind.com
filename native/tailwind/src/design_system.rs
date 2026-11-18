//! The design system: `design-system.ts` of Tailwind CSS.
//!
//! The design system holds the theme, the utilities and the variants, and
//! the caches of the parser and of the compiler.

use crate::ast::AstNode;
use crate::candidate::{parse_candidate, parse_variant, Candidate, Variant};
use crate::compile::{compile_ast_nodes, CompiledNode};
use crate::theme::{self, Theme};
use crate::utilities::{create_utilities, with_alpha, Utilities};
use crate::utils::variables::extract_used_variables;
use crate::variants::{create_variants, Variants};
use std::cell::RefCell;
use crate::hash::{HashMap, HashSet};
use std::rc::Rc;

/// The entries that `transient_compiles` removes at its end: the keys of
/// the compiled cache, and the texts of the parse cache. `kept` has the
/// candidates of the compiled entries that stay: their parse entries
/// stay too, because the key of a compiled entry is the address of its
/// candidate.
#[derive(Default)]
struct Transient {
    compiled: Vec<(CompileAstFlags, *const Candidate)>,
    parsed: Vec<String>,
    kept: HashSet<*const Candidate>,
}

/// The flags of `compile_ast_nodes`, as bits.
pub type CompileAstFlags = u8;
pub const COMPILE_NONE: CompileAstFlags = 0;
pub const COMPILE_RESPECT_IMPORTANT: CompileAstFlags = 1 << 0;

/// A replacement of `DesignSystem::resolve_theme_value`. The compatibility
/// layer sets it (`compat.rs`), as upstream replaces `resolveThemeValue`.
pub type ResolveThemeValueFn = Rc<dyn Fn(&DesignSystem, &str, bool) -> Option<String>>;

/// The rules of each candidate, by the flags and the address of the candidate.
/// Each entry also holds its candidate: while the entry lives, no other
/// candidate can get the same address.
type CompiledCache = HashMap<(CompileAstFlags, *const Candidate), (Rc<Candidate>, Rc<Vec<CompiledNode>>)>;

/// The order of each parsed variant, by the address of the variant.
pub type VariantOrder = HashMap<*const Variant, u32>;

pub struct DesignSystem {
    pub theme: Theme,
    pub utilities: Utilities,
    pub variants: Variants,
    pub invalid_candidates: RefCell<HashSet<String>>,
    /// The warnings of the compiler: each text that Tailwind CSS gives to
    /// `console.warn`, when the compile has no module loader (`console`).
    /// `Compiler::take_warnings` gives them to the caller.
    pub warnings: RefCell<Vec<String>>,
    /// The module loader of the compile. Its `console` gets each warning
    /// at the time that upstream gives it to `console.warn`, thus also
    /// before an error of the compile.
    pub console: Option<std::rc::Rc<dyn crate::ModuleLoader>>,
    /// True: each declaration of a utility is `!important`.
    pub important: bool,
    /// When set, `resolve_theme_value` calls this function.
    pub resolve_theme_value_hook: Option<ResolveThemeValueFn>,
    /// True when the compiler keeps the source locations (the `from`
    /// option), for source maps.
    pub track_sources: std::cell::Cell<bool>,
    /// The JavaScript engine with the plugins of `@plugin` and `@config`.
    /// The utilities and variants of the plugins call it.
    #[cfg(tw_js)]
    pub js: Option<Rc<crate::compat::js::JsRuntime>>,

    /// Only for Tailwind CSS 4.1.14 (`compile::share_declarations`): the
    /// declarations of the fixed values of the utilities, by identity. A
    /// value is true when a candidate with `!important` used the
    /// declaration.
    pub shared_declarations: RefCell<HashMap<*const (), bool>>,

    parsed_variants: RefCell<crate::hash::IndexMap<String, Option<Rc<Variant>>>>,
    parsed_candidates: RefCell<HashMap<String, Rc<Vec<Rc<Candidate>>>>>,
    compiled: RefCell<CompiledCache>,
    /// False: `compiled_ast_nodes_owned` takes the nodes out of the cache
    /// (`set_keep_compiled`).
    keep_compiled: std::cell::Cell<bool>,
    tracked: RefCell<HashSet<String>>,
    variant_order: RefCell<Option<(usize, Rc<VariantOrder>)>>,
    /// The caches of the canonical form of candidates.
    #[cfg(feature = "canonicalize")]
    pub(crate) canonical: crate::canonicalize::Storage,
    /// The entries of `transient_compiles`. `None` out of it.
    transient: RefCell<Option<Transient>>,
    /// The version of Tailwind CSS that the design system follows: the
    /// version of the thread at its start (`version::enter`). The
    /// functions for the editor tools and the canonical form of
    /// candidates follow it also after the compile.
    pub version: crate::Version,
}

/// A design system with the utilities and variants of Tailwind CSS.
pub fn build_design_system(theme: Theme) -> DesignSystem {
    let utilities = create_utilities(&theme);
    let variants = create_variants(&theme);
    DesignSystem {
        theme,
        utilities,
        variants,
        invalid_candidates: RefCell::new(HashSet::default()),
        warnings: RefCell::new(Vec::new()),
        console: None,
        important: false,
        resolve_theme_value_hook: None,
        track_sources: std::cell::Cell::new(false),
        #[cfg(tw_js)]
        js: None,
        shared_declarations: RefCell::new(HashMap::default()),
        parsed_variants: RefCell::new(crate::hash::IndexMap::default()),
        parsed_candidates: RefCell::new(HashMap::default()),
        compiled: RefCell::new(HashMap::default()),
        keep_compiled: std::cell::Cell::new(true),
        tracked: RefCell::new(HashSet::default()),
        variant_order: RefCell::new(None),
        #[cfg(feature = "canonicalize")]
        canonical: Default::default(),
        transient: RefCell::new(None),
        version: crate::version::current(),
    }
}

impl DesignSystem {
    /// `console.warn` of Tailwind CSS: the text goes to the `console` of
    /// the module loader, or else to `warnings`.
    pub fn warn(&self, text: String) {
        match &self.console {
            Some(loader) => loader.console(&text, true),
            None => self.warnings.borrow_mut().push(text),
        }
    }

    /// All the readings of a candidate, from the cache.
    pub fn parse_candidate(&self, candidate: &str) -> Rc<Vec<Rc<Candidate>>> {
        if let Some(found) = self.parsed_candidates.borrow().get(candidate) {
            return found.clone();
        }
        let parsed: Rc<Vec<Rc<Candidate>>> = Rc::new(parse_candidate(candidate, self).into_iter().map(Rc::new).collect());
        self.parsed_candidates.borrow_mut().insert(candidate.to_string(), parsed.clone());
        if let Some(transient) = self.transient.borrow_mut().as_mut() {
            transient.parsed.push(candidate.to_string());
        }
        parsed
    }

    /// A variant, from the cache.
    pub fn parse_variant(&self, variant: &str) -> Option<Rc<Variant>> {
        if let Some(found) = self.parsed_variants.borrow().get(variant) {
            return found.clone();
        }
        let parsed = parse_variant(variant, self).map(Rc::new);
        self.parsed_variants.borrow_mut().insert(variant.to_string(), parsed.clone());
        parsed
    }

    /// The rules of a candidate, from the cache. The candidate must come
    /// from `parse_candidate` of this design system.
    pub fn compile_ast_nodes(&self, candidate: &Rc<Candidate>, flags: CompileAstFlags) -> Rc<Vec<CompiledNode>> {
        let key = (flags, Rc::as_ptr(candidate));
        if let Some((_, found)) = self.compiled.borrow().get(&key) {
            return found.clone();
        }
        let errors_before = self.js_error_count();
        let inline_reads_before = self.theme.inline_reads();
        let mut compiled = compile_ast_nodes(candidate, self, flags);

        // An arbitrary value or property can have a function such as
        // `theme(…)`, and a utility can have an `@variant`. When the
        // substitution fails, the candidate is not valid. (For
        // `keep_at_variant`, an `@variant` rule stays as it is.)
        let mut failed = false;
        for item in compiled.iter_mut() {
            let mut nodes = vec![std::mem::replace(&mut item.node, crate::ast::comment(""))];
            if crate::css_functions::substitute_functions(&mut nodes, self).is_err()
                || (!self.variants.keep_at_variant && crate::variants::substitute_at_variant(&mut nodes, self).is_err())
                || nodes.len() != 1
            {
                failed = true;
                break;
            }
            item.node = nodes.pop().unwrap();
        }
        if failed {
            compiled.clear();
        }

        let compiled = Rc::new(compiled);
        // Upstream keeps no result when a plugin throws: the next compile
        // of the candidate throws again.
        if self.js_error_count() == errors_before {
            self.compiled.borrow_mut().insert(key, (candidate.clone(), compiled.clone()));
            // In `transient_compiles`: an entry that read no `inline`
            // option of the theme, in a design system with no plugins,
            // goes out of the cache at the end. A second compile gives the
            // same nodes, so no result changes.
            if let Some(transient) = self.transient.borrow_mut().as_mut() {
                if self.theme.inline_reads() == inline_reads_before && !self.has_js() {
                    transient.compiled.push(key);
                } else {
                    transient.kept.insert(key.1);
                }
            }
        }
        compiled
    }

    /// Run `f`, and then remove from the cache of the compiled candidates
    /// the entries of `f` that do not change a result (see
    /// `compile_ast_nodes`). The canonical form of candidates compiles
    /// many candidates one time: this keeps its memory small. A call in
    /// a call of this function adds its entries to the outer call.
    pub fn transient_compiles<T>(&self, f: impl FnOnce() -> T) -> T {
        let outer = self.transient.borrow().is_some();
        if !outer {
            *self.transient.borrow_mut() = Some(Transient::default());
        }
        let result = f();
        if !outer {
            let transient = self.transient.borrow_mut().take().unwrap_or_default();
            let mut compiled = self.compiled.borrow_mut();
            for key in &transient.compiled {
                compiled.remove(key);
            }
            let mut parsed = self.parsed_candidates.borrow_mut();
            for text in &transient.parsed {
                let keep = parsed.get(text).is_some_and(|list| list.iter().any(|c| transient.kept.contains(&Rc::as_ptr(c))));
                if !keep {
                    parsed.remove(text);
                }
            }
        }
        result
    }

    /// True when the design system has a JavaScript engine of plugins.
    fn has_js(&self) -> bool {
        #[cfg(tw_js)]
        if self.js.is_some() {
            return true;
        }
        false
    }

    /// The number of exceptions of plugins since the start.
    pub fn js_error_count(&self) -> u64 {
        #[cfg(tw_js)]
        if let Some(js) = &self.js {
            return js.error_count();
        }
        0
    }

    /// True when an exception of a plugin waits for `take_js_error`.
    pub fn has_js_error(&self) -> bool {
        #[cfg(tw_js)]
        if let Some(js) = &self.js {
            return js.has_error();
        }
        false
    }

    /// True (the default): a build copies the compiled nodes of a candidate
    /// from the cache, and the next build uses the cache again (a watch).
    /// False: a build takes the nodes out of the cache, with no copy, and
    /// a next build compiles each candidate again. The program sets false
    /// for a build with no `--watch`.
    pub fn set_keep_compiled(&self, keep: bool) {
        self.keep_compiled.set(keep);
    }

    /// The compiled nodes of a candidate (`compile_ast_nodes`), as values
    /// of the caller: a copy of the cache, or, with `set_keep_compiled`
    /// false, the nodes of the cache.
    pub fn compiled_ast_nodes_owned(&self, candidate: &Rc<Candidate>, flags: CompileAstFlags) -> Vec<CompiledNode> {
        let rules = self.compile_ast_nodes(candidate, flags);
        if self.keep_compiled.get() {
            return (*rules).clone();
        }
        self.compiled.borrow_mut().remove(&(flags, Rc::as_ptr(candidate)));
        Rc::try_unwrap(rules).unwrap_or_else(|rules| (*rules).clone())
    }

    /// Replace a node in the cached rules of a candidate: the rule with the
    /// index `rule`, or the child `child` of that rule. Only `@apply` of
    /// before 4.0.10 uses this (`apply.rs`): there, the nodes of an
    /// `@apply` were the nodes of the cache, and a change in them was a
    /// change of the cache.
    pub fn replace_compiled_node(
        &self,
        flags: CompileAstFlags,
        (candidate, rule): (*const Candidate, usize),
        child: Option<usize>,
        node: AstNode,
    ) {
        let mut compiled = self.compiled.borrow_mut();
        let Some(item) = compiled.get_mut(&(flags, candidate)).and_then(|(_, rules)| Rc::make_mut(rules).get_mut(rule)) else {
            return;
        };
        match child {
            None => item.node = node,
            Some(index) => {
                if let Some(slot) = item.node.children_mut().and_then(|children| children.get_mut(index)) {
                    *slot = node;
                }
            }
        }
    }

    /// The order of each variant that the parser saw, for the sort of the
    /// output. Variants that compare as equal have the same order.
    pub fn get_variant_order(&self) -> Rc<VariantOrder> {
        let size = self.parsed_variants.borrow().len();
        if let Some((cached_size, order)) = self.variant_order.borrow().as_ref() {
            if *cached_size == size {
                return order.clone();
            }
        }

        // The list has an empty entry for each text that is not a
        // variant, as upstream (`null`): the sort compares them too.
        let mut variants: Vec<Option<Rc<Variant>>> = self.parsed_variants.borrow().values().cloned().collect();
        // The compare function is not consistent for some variants (two
        // breakpoints with no value, for example). Then the result
        // depends on the sort algorithm: this is the algorithm of V8.
        crate::utils::v8_sort::sort(&mut variants, |a, z| self.variants.compare(a.as_deref(), z.as_deref(), self));

        let mut order = VariantOrder::default();
        let mut prev: Option<&Rc<Variant>> = None;
        let mut index = 0u32;
        for variant in variants.iter().flatten() {
            if let Some(prev) = prev {
                if self.variants.compare(Some(prev), Some(variant), self) != 0 {
                    index += 1;
                }
            }
            order.insert(Rc::as_ptr(variant), index);
            prev = Some(variant);
        }

        let order = Rc::new(order);
        *self.variant_order.borrow_mut() = Some((size, order.clone()));
        order
    }

    /// The value of a theme key, for example `--color-red-500 / 50%`. The
    /// compatibility layer can replace this function.
    pub fn resolve_theme_value(&self, path: &str, force_inline: bool) -> Option<String> {
        match self.resolve_theme_value_hook.clone() {
            Some(hook) => hook(self, path, force_inline),
            None => self.resolve_theme_variable_value(path, force_inline),
        }
    }

    /// The value of a theme variable, for example `--color-red-500 / 50%`:
    /// `resolveThemeValue` of `design-system.ts`.
    pub fn resolve_theme_variable_value(&self, path: &str, force_inline: bool) -> Option<String> {
        // A modifier is after the last slash.
        let (path, modifier) = match path.rfind('/') {
            Some(idx) => (crate::utils::js::js_trim(&path[..idx]), Some(crate::utils::js::js_trim(&path[idx + 1..]))),
            None => (path, None),
        };

        let theme_value =
            self.theme.resolve_with_options(None, &[path], if force_inline { theme::INLINE } else { theme::NONE });

        match (modifier, theme_value) {
            (Some(modifier), Some(value)) if !modifier.is_empty() && !value.is_empty() => Some(with_alpha(&value, modifier)),
            (_, value) => value,
        }
    }

    /// Take the first exception of a JavaScript plugin function in a
    /// build. Upstream throws it from `build`.
    pub fn take_js_error(&self) -> Option<crate::Error> {
        #[cfg(tw_js)]
        if let Some(js) = &self.js {
            return js.take_error();
        }
        None
    }

    /// Mark the theme variables of a value as used.
    pub fn track_used_variables(&self, raw: &str) {
        if self.tracked.borrow().contains(raw) {
            return;
        }
        self.tracked.borrow_mut().insert(raw.to_string());
        for variable in extract_used_variables(raw) {
            self.theme.mark_used_variable(&variable);
        }
    }

    /// The CSS of each class, or `None` for an invalid class.
    pub fn candidates_to_css(&self, classes: &[&str]) -> Vec<Option<String>> {
        classes
            .iter()
            .map(|class_name| {
                let mut was_valid = true;
                let result = crate::compile::compile_candidates(&[class_name.to_string()], self, &mut |_| was_valid = false, true);
                let nodes: Vec<AstNode> = result.into_iter().map(|c| c.node).collect();
                let nodes = crate::optimize::optimize_ast(&nodes, self, crate::POLYFILLS_NONE);
                if was_valid && !nodes.is_empty() {
                    Some(crate::to_css::to_css(&nodes))
                } else {
                    None
                }
            })
            .collect()
    }
}

