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

/// The flags of `compile_ast_nodes`, as bits.
pub type CompileAstFlags = u8;
pub const COMPILE_NONE: CompileAstFlags = 0;
pub const COMPILE_RESPECT_IMPORTANT: CompileAstFlags = 1 << 0;

/// A replacement of `DesignSystem::resolve_theme_value`. The compatibility
/// layer sets it (`compat.rs`), as upstream replaces `resolveThemeValue`.
pub type ResolveThemeValueFn = Rc<dyn Fn(&DesignSystem, &str, bool) -> Option<String>>;

/// The rules of each candidate, by the flags and the address of the candidate.
type CompiledCache = HashMap<(CompileAstFlags, *const Candidate), Rc<Vec<CompiledNode>>>;

/// The order of each parsed variant, by the address of the variant.
pub type VariantOrder = HashMap<*const Variant, u32>;

pub struct DesignSystem {
    pub theme: Theme,
    pub utilities: Utilities,
    pub variants: Variants,
    pub invalid_candidates: RefCell<HashSet<String>>,
    /// The warnings of the compiler: each text that Tailwind CSS gives to
    /// `console.warn`. `Compiler::take_warnings` gives them to the caller.
    pub warnings: RefCell<Vec<String>>,
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
    tracked: RefCell<HashSet<String>>,
    variant_order: RefCell<Option<(usize, Rc<VariantOrder>)>>,
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
        important: false,
        resolve_theme_value_hook: None,
        track_sources: std::cell::Cell::new(false),
        #[cfg(tw_js)]
        js: None,
        shared_declarations: RefCell::new(HashMap::default()),
        parsed_variants: RefCell::new(crate::hash::IndexMap::default()),
        parsed_candidates: RefCell::new(HashMap::default()),
        compiled: RefCell::new(HashMap::default()),
        tracked: RefCell::new(HashSet::default()),
        variant_order: RefCell::new(None),
    }
}

impl DesignSystem {
    /// All the readings of a candidate, from the cache.
    pub fn parse_candidate(&self, candidate: &str) -> Rc<Vec<Rc<Candidate>>> {
        if let Some(found) = self.parsed_candidates.borrow().get(candidate) {
            return found.clone();
        }
        let parsed: Rc<Vec<Rc<Candidate>>> = Rc::new(parse_candidate(candidate, self).into_iter().map(Rc::new).collect());
        self.parsed_candidates.borrow_mut().insert(candidate.to_string(), parsed.clone());
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
        if let Some(found) = self.compiled.borrow().get(&key) {
            return found.clone();
        }
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
        self.compiled.borrow_mut().insert(key, compiled.clone());
        compiled
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
        let Some(item) = compiled.get_mut(&(flags, candidate)).and_then(|rules| Rc::make_mut(rules).get_mut(rule)) else {
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

