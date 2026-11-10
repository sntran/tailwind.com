//! The variants: `variants.ts` of Tailwind CSS.

use crate::ast::AstNode;
use crate::candidate::{ValueKind, Variant};
use crate::design_system::DesignSystem;
use crate::theme::Theme;
use indexmap::IndexMap;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;

/// The kind of a variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VariantKind {
    Arbitrary,
    Static,
    Functional,
    Compound,
}

/// What a variant can compound with, as bits.
pub type Compounds = u8;
pub const COMPOUNDS_NEVER: Compounds = 0;
pub const COMPOUNDS_AT_RULES: Compounds = 1 << 0;
pub const COMPOUNDS_STYLE_RULES: Compounds = 1 << 1;

/// The function of a variant. It changes the rule (a style rule or an
/// at-rule) in place. `None` is "this variant does not apply" (`null`
/// upstream).
pub type VariantFn = Rc<dyn Fn(&mut AstNode, &Variant, &DesignSystem) -> Option<()>>;

/// A function that compares two variants of one group.
pub type CompareFn = Rc<dyn Fn(&Variant, &Variant, &DesignSystem) -> i32>;

/// One registered variant.
#[derive(Clone)]
pub struct VariantInfo {
    pub kind: VariantKind,
    pub order: i64,
    pub apply_fn: VariantFn,
    pub compounds_with: Compounds,
    pub compounds: Compounds,
}

/// The options of a registration.
#[derive(Debug, Clone, Copy, Default)]
pub struct VariantOptions {
    pub compounds: Option<Compounds>,
    pub order: Option<i64>,
}

/// The registry of the variants, in the sequence of their registration.
#[derive(Default)]
pub struct Variants {
    pub compare_fns: HashMap<i64, CompareFn>,
    pub variants: IndexMap<String, VariantInfo>,
    /// The variants of one group have the same order.
    group_order: Option<i64>,
    last_order: i64,
}

impl Variants {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn r#static(
        &mut self,
        name: impl Into<String>,
        apply_fn: impl Fn(&mut AstNode, &Variant, &DesignSystem) -> Option<()> + 'static,
        options: VariantOptions,
    ) {
        self.set(name.into(), VariantKind::Static, Rc::new(apply_fn), COMPOUNDS_NEVER, options);
    }

    pub fn functional(
        &mut self,
        name: impl Into<String>,
        apply_fn: impl Fn(&mut AstNode, &Variant, &DesignSystem) -> Option<()> + 'static,
        options: VariantOptions,
    ) {
        self.set(name.into(), VariantKind::Functional, Rc::new(apply_fn), COMPOUNDS_NEVER, options);
    }

    pub fn compound(
        &mut self,
        name: impl Into<String>,
        compounds_with: Compounds,
        apply_fn: impl Fn(&mut AstNode, &Variant, &DesignSystem) -> Option<()> + 'static,
        options: VariantOptions,
    ) {
        self.set(name.into(), VariantKind::Compound, Rc::new(apply_fn), compounds_with, options);
    }

    /// Register a group of variants: they all get the same order.
    pub fn group(&mut self, f: impl FnOnce(&mut Variants), compare_fn: Option<CompareFn>) {
        let order = self.next_order();
        self.group_order = Some(order);
        if let Some(compare_fn) = compare_fn {
            self.compare_fns.insert(order, compare_fn);
        }
        f(self);
        self.group_order = None;
    }

    pub fn has(&self, name: &str) -> bool {
        self.variants.contains_key(name)
    }

    pub fn get(&self, name: &str) -> Option<&VariantInfo> {
        self.variants.get(name)
    }

    pub fn kind(&self, name: &str) -> Option<VariantKind> {
        self.variants.get(name).map(|v| v.kind)
    }

    /// True when the compound variant `parent` accepts `child`.
    pub fn compounds_with(&self, parent: &str, child: &Variant) -> bool {
        let Some(parent_info) = self.variants.get(parent) else { return false };
        let child_compounds = match child {
            Variant::Arbitrary { selector, .. } => compounds_for_selectors(&[selector.as_str()]),
            other => match self.variants.get(other.root().unwrap()) {
                Some(info) => info.compounds,
                None => return false,
            },
        };
        self.compounds_check(parent_info, child_compounds)
    }

    /// As `compounds_with`, for the name of a child variant.
    pub fn compounds_with_name(&self, parent: &str, child: &str) -> bool {
        let (Some(parent_info), Some(child_info)) = (self.variants.get(parent), self.variants.get(child)) else {
            return false;
        };
        self.compounds_check(parent_info, child_info.compounds)
    }

    fn compounds_check(&self, parent_info: &VariantInfo, child_compounds: Compounds) -> bool {
        if parent_info.kind != VariantKind::Compound {
            return false;
        }
        if child_compounds == COMPOUNDS_NEVER {
            return false;
        }
        if parent_info.compounds_with == COMPOUNDS_NEVER {
            return false;
        }
        parent_info.compounds_with & child_compounds != 0
    }

    /// Compare two variants for the sort of the output. The result is
    /// negative, zero or positive, as a comparator of JavaScript.
    pub fn compare(&self, a: Option<&Variant>, z: Option<&Variant>, ds: &DesignSystem) -> i64 {
        let (a, z) = match (a, z) {
            (None, None) => return 0,
            (None, _) => return -1,
            (_, None) => return 1,
            (Some(a), Some(z)) => (a, z),
        };
        if std::ptr::eq(a, z) {
            return 0;
        }

        match (a, z) {
            (Variant::Arbitrary { selector: a, .. }, Variant::Arbitrary { selector: z, .. }) => {
                return if js_less(a, z) { -1 } else { 1 };
            }
            (Variant::Arbitrary { .. }, _) => return 1,
            (_, Variant::Arbitrary { .. }) => return -1,
            _ => {}
        }

        let a_root = a.root().unwrap();
        let z_root = z.root().unwrap();
        let a_order = self.variants.get(a_root).map_or(0, |v| v.order);
        let z_order = self.variants.get(z_root).map_or(0, |v| v.order);

        let ordered_by_variant = a_order - z_order;
        if ordered_by_variant != 0 {
            return ordered_by_variant;
        }

        if let (
            Variant::Compound { variant: a_variant, modifier: a_modifier, .. },
            Variant::Compound { variant: z_variant, modifier: z_modifier, .. },
        ) = (a, z)
        {
            let order = self.compare(Some(a_variant), Some(z_variant), ds);
            if order != 0 {
                return order;
            }
            return match (a_modifier, z_modifier) {
                (Some(a), Some(z)) => {
                    if js_less(&a.value, &z.value) {
                        -1
                    } else {
                        1
                    }
                }
                (Some(_), None) => 1,
                (None, Some(_)) => -1,
                (None, None) => 0,
            };
        }

        if let Some(compare_fn) = self.compare_fns.get(&a_order) {
            return compare_fn(a, z, ds) as i64;
        }

        if a_root != z_root {
            return if js_less(a_root, z_root) { -1 } else { 1 };
        }

        // The two variants have the same root, so they are functional.
        let (a_value, z_value) = (a.value(), z.value());
        let Some(a_value) = a_value else { return -1 };
        let Some(z_value) = z_value else { return 1 };

        if a_value.kind == ValueKind::Arbitrary && z_value.kind != ValueKind::Arbitrary {
            return 1;
        }
        if a_value.kind != ValueKind::Arbitrary && z_value.kind == ValueKind::Arbitrary {
            return -1;
        }
        if js_less(&a_value.value, &z_value.value) {
            -1
        } else {
            1
        }
    }

    pub fn keys(&self) -> impl Iterator<Item = &String> {
        self.variants.keys()
    }

    fn set(&mut self, name: String, kind: VariantKind, apply_fn: VariantFn, compounds_with: Compounds, options: VariantOptions) {
        let compounds = options.compounds.unwrap_or(COMPOUNDS_STYLE_RULES);
        if let Some(existing) = self.variants.get_mut(&name) {
            existing.kind = kind;
            existing.apply_fn = apply_fn;
            existing.compounds = compounds;
        } else {
            let order = match options.order {
                Some(order) => order,
                None => {
                    self.last_order = self.next_order();
                    self.last_order
                }
            };
            self.variants.insert(name, VariantInfo { kind, order, apply_fn, compounds_with, compounds });
        }
    }

    fn next_order(&self) -> i64 {
        self.group_order.unwrap_or(self.last_order + 1)
    }
}

/// `a < z` of JavaScript for strings: the comparison of UTF-16 code units.
pub fn js_less(a: &str, z: &str) -> bool {
    js_string_cmp(a, z) == Ordering::Less
}

/// The comparison of two strings by UTF-16 code units, as JavaScript.
pub fn js_string_cmp(a: &str, z: &str) -> Ordering {
    if a.is_ascii() && z.is_ascii() {
        return a.as_bytes().cmp(z.as_bytes());
    }
    a.encode_utf16().cmp(z.encode_utf16())
}

/// What the selectors of a variant let it compound with.
pub fn compounds_for_selectors(selectors: &[&str]) -> Compounds {
    let mut compounds = COMPOUNDS_NEVER;
    for sel in selectors {
        if sel.starts_with('@') {
            // Only some at-rules are safe to compound.
            if !sel.starts_with("@media") && !sel.starts_with("@supports") && !sel.starts_with("@container") {
                return COMPOUNDS_NEVER;
            }
            compounds |= COMPOUNDS_AT_RULES;
            continue;
        }
        // A pseudo-element is not valid in a compound selector.
        if sel.contains("::") {
            return COMPOUNDS_NEVER;
        }
        compounds |= COMPOUNDS_STYLE_RULES;
    }
    compounds
}

/// All the variants of Tailwind CSS.
pub fn create_variants(_theme: &Theme) -> Variants {
    Variants::new()
}

/// Replace each `@variant` rule with the rules of its variant.
pub fn substitute_at_variant(_ast: &mut Vec<AstNode>, _design_system: &DesignSystem) -> Result<crate::Features, crate::Error> {
    Ok(crate::FEATURES_NONE)
}
