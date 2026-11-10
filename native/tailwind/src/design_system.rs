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
use std::collections::{HashMap, HashSet};
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
    /// True: each declaration of a utility is `!important`.
    pub important: bool,
    /// When set, `resolve_theme_value` calls this function.
    pub resolve_theme_value_hook: Option<ResolveThemeValueFn>,
    /// True when the compiler keeps the source locations (the `from`
    /// option), for source maps.
    pub track_sources: std::cell::Cell<bool>,

    parsed_variants: RefCell<indexmap::IndexMap<String, Option<Rc<Variant>>>>,
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
        invalid_candidates: RefCell::new(HashSet::new()),
        important: false,
        resolve_theme_value_hook: None,
        track_sources: std::cell::Cell::new(false),
        parsed_variants: RefCell::new(indexmap::IndexMap::new()),
        parsed_candidates: RefCell::new(HashMap::new()),
        compiled: RefCell::new(HashMap::new()),
        tracked: RefCell::new(HashSet::new()),
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
        // substitution fails, the candidate is not valid.
        let mut failed = false;
        for item in compiled.iter_mut() {
            let mut nodes = vec![std::mem::replace(&mut item.node, crate::ast::comment(""))];
            if crate::css_functions::substitute_functions(&mut nodes, self).is_err()
                || crate::variants::substitute_at_variant(&mut nodes, self).is_err()
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

    /// The order of each variant that the parser saw, for the sort of the
    /// output. Variants that compare as equal have the same order.
    pub fn get_variant_order(&self) -> Rc<VariantOrder> {
        let size = self.parsed_variants.borrow().len();
        if let Some((cached_size, order)) = self.variant_order.borrow().as_ref() {
            if *cached_size == size {
                return order.clone();
            }
        }

        let variants: Vec<Rc<Variant>> = self.parsed_variants.borrow().values().flatten().cloned().collect();
        // The compare function is not a total order for some invalid
        // variants, so the sort must accept any comparator.
        let variants = merge_sort(variants, &|a, z| self.variants.compare(Some(z), Some(a), self) >= 0);

        let mut order = VariantOrder::new();
        let mut prev: Option<&Rc<Variant>> = None;
        let mut index = 0u32;
        for variant in &variants {
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

/// A stable merge sort that accepts a comparator that is not a total
/// order. `in_order(a, z)` is true when `a` stays before `z`: the sort of
/// V8 moves `z` before `a` only when `compare(z, a)` is negative.
fn merge_sort<T: Clone>(items: Vec<T>, in_order: &dyn Fn(&T, &T) -> bool) -> Vec<T> {
    if items.len() <= 1 {
        return items;
    }
    let right = items[items.len() / 2..].to_vec();
    let mut left = items;
    left.truncate(left.len() - right.len());
    let (left, right) = (merge_sort(left, in_order), merge_sort(right, in_order));
    let mut merged = Vec::with_capacity(left.len() + right.len());
    let (mut i, mut j) = (0, 0);
    while i < left.len() && j < right.len() {
        if in_order(&left[i], &right[j]) {
            merged.push(left[i].clone());
            i += 1;
        } else {
            merged.push(right[j].clone());
            j += 1;
        }
    }
    merged.extend_from_slice(&left[i..]);
    merged.extend_from_slice(&right[j..]);
    merged
}
