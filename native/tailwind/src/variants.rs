//! The variants: `variants.ts` of Tailwind CSS.

use crate::switches;
use crate::ast::{at_root, at_rule, context, decl, rule, style_rule, AstNode, AtRule, ContextMap, StyleRule};
use crate::candidate::{ValueKind, Variant};
use crate::compile::apply_variant;
use crate::design_system::DesignSystem;
use crate::theme::Theme;
use crate::utils::compare_breakpoints::{compare_breakpoints, Direction};
use crate::utils::infer_data_type::is_positive_integer;
use crate::utils::js::{is_js_whitespace, js_trim};
use crate::utils::segment::segment;
use crate::value_parser::{self, ValueAstNode};
use crate::walk::{walk, WalkAction};
use crate::{Error, Features, FEATURES_NONE, FEATURES_VARIANTS};
use indexmap::IndexMap;
use std::cell::RefCell;
use std::cmp::Ordering;
use std::collections::HashMap;
use std::rc::Rc;

/// The test of a variant name: `/^@?[a-z0-9][a-zA-Z0-9_-]*(?<![_-])$/`.
/// Before 4.1.13: `/^@?[a-zA-Z0-9_-]*$/`.
pub fn is_valid_variant_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    let bytes = bytes.strip_prefix(b"@").unwrap_or(bytes);
    if !crate::switches::VARIANT_NAME_START_AND_END.on() {
        return bytes.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-');
    }
    let Some((first, rest)) = bytes.split_first() else { return false };
    if !(first.is_ascii_lowercase() || first.is_ascii_digit()) {
        return false;
    }
    if !rest.iter().all(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-') {
        return false;
    }
    !matches!(bytes.last(), Some(b'_' | b'-'))
}

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
    /// The error of a variant function. Upstream throws it.
    error: RefCell<Option<Error>>,
    /// True: `from_ast` does not replace the `@variant` rules of a body.
    /// Only for a version before 4.1.14 and custom variants that use each
    /// other in a circle (`switches::CUSTOM_VARIANT_CYCLE_ERROR`).
    pub keep_at_variant: bool,
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

    /// Start a group: the variants until `group_end` get the same order.
    /// This is `group` in two steps, for the plugins of JavaScript.
    pub fn group_start(&mut self, compare_fn: Option<CompareFn>) {
        let order = self.next_order();
        self.group_order = Some(order);
        if let Some(compare_fn) = compare_fn {
            self.compare_fns.insert(order, compare_fn);
        }
    }

    /// End the group of `group_start`.
    pub fn group_end(&mut self) {
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

    pub fn entries(&self) -> impl Iterator<Item = (&String, &VariantInfo)> {
        self.variants.iter()
    }

    /// Register a static variant from the body of an `@custom-variant`
    /// rule. Each `@slot` of the body gets the nodes of the rule.
    ///
    /// Upstream throws when an `@variant` of the body is not valid. Here
    /// the function of the variant returns `None`, and `take_error` gives
    /// the error.
    pub fn from_ast(&mut self, name: impl Into<String>, ast: Vec<AstNode>) {
        let mut selectors: Vec<String> = Vec::new();
        let mut uses_at_variant = false;
        let mut uses_at_scope = false;
        collect_selectors(&ast, &mut selectors, &mut uses_at_variant, &mut uses_at_scope);
        let uses_at_scope = uses_at_scope && switches::SCOPE_HOISTING.on();
        let selector_refs: Vec<&str> = selectors.iter().map(String::as_str).collect();
        // Before 4.1.14, an `@variant` rule in the body was an at-rule as
        // each other: a compound variant (`group-*`) did not take the
        // variant.
        let compounds = if uses_at_variant && !switches::CUSTOM_VARIANT_AT_VARIANT_COMPOUNDS.on() {
            COMPOUNDS_NEVER
        } else {
            compounds_for_selectors(&selector_refs)
        };
        // See `keep_at_variant`.
        let uses_at_variant = uses_at_variant && !self.keep_at_variant;

        self.r#static(
            name,
            move |r, _, ds| {
                let mut body = ast.clone();
                if uses_at_variant {
                    if let Err(error) = substitute_at_variant(&mut body, ds) {
                        ds.variants.set_error(error);
                        return None;
                    }
                }

                let nodes = r.children_mut()?;
                // Upstream puts the same nodes at each `@slot`.
                if ds.track_sources.get() {
                    crate::ast::share_identity(nodes);
                }
                if uses_at_scope {
                    let user = context(source_context("user"), std::mem::take(nodes));
                    substitute_at_slot(&mut body, std::slice::from_ref(&user));
                    *nodes = vec![context(source_context("variant"), body)];
                } else {
                    substitute_at_slot(&mut body, nodes);
                    *nodes = body;
                }
                Some(())
            },
            VariantOptions { compounds: Some(compounds), order: None },
        );
    }

    /// Keep the error of a variant function. The first error stays.
    pub fn set_error(&self, error: Error) {
        let mut slot = self.error.borrow_mut();
        if slot.is_none() {
            *slot = Some(error);
        }
    }

    /// Take the error of a variant function, if there is one.
    pub fn take_error(&self) -> Option<Error> {
        self.error.borrow_mut().take()
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

/// A context with one key: `source`.
fn source_context(source: &str) -> ContextMap {
    let mut map = ContextMap::new();
    map.insert("source".into(), source.into());
    map
}

/// Collect the selectors of the body of a custom variant.
fn collect_selectors(ast: &[AstNode], selectors: &mut Vec<String>, uses_at_variant: &mut bool, uses_at_scope: &mut bool) {
    for node in ast {
        match node {
            AstNode::Rule(r) => selectors.push(r.selector.clone()),
            AstNode::AtRule(r) if r.name == "@variant" => *uses_at_variant = true,
            AstNode::AtRule(r) if r.name != "@slot" => {
                if r.name == "@scope" {
                    *uses_at_scope = true;
                }
                selectors.push(format!("{} {}", r.name, r.params));
            }
            _ => {}
        }
        if let Some(children) = node.children() {
            collect_selectors(children, selectors, uses_at_variant, uses_at_scope);
        }
    }
}

/// Put `nodes` around the children of the rule, one time for each selector.
fn wrap_children(r: &mut AstNode, selectors: &[&str], track_sources: bool) -> Option<()> {
    let nodes = r.children_mut()?;
    let mut body = std::mem::take(nodes);
    // Upstream puts the same nodes in each rule.
    if track_sources && selectors.len() > 1 {
        crate::ast::share_identity(&mut body);
    }
    let last = selectors.len().saturating_sub(1);
    let mut body = Some(body);
    *nodes = selectors
        .iter()
        .enumerate()
        .map(|(idx, selector)| {
            let children = if idx == last { body.take().unwrap_or_default() } else { body.clone().unwrap_or_default() };
            rule(*selector, children)
        })
        .collect();
    Some(())
}

/// Register a static variant such as `hover`.
fn static_variant(variants: &mut Variants, name: &str, selectors: &'static [&'static str], compounds: Option<Compounds>) {
    let compounds = compounds.unwrap_or_else(|| compounds_for_selectors(selectors));
    variants.r#static(name, move |r, _, ds| wrap_children(r, selectors, ds.track_sources.get()), VariantOptions { compounds: Some(compounds), order: None });
}

/// Replace the children of the rule with one node that has them.
fn wrap_in(r: &mut AstNode, make: impl FnOnce(Vec<AstNode>) -> AstNode) -> Option<()> {
    let nodes = r.children_mut()?;
    let body = std::mem::take(nodes);
    *nodes = vec![make(body)];
    Some(())
}

fn negate_conditions(rule_name: &str, conditions: &[&str]) -> Vec<String> {
    conditions
        .iter()
        .map(|condition| {
            if !crate::switches::NOT_CONTAINER_VALUE_PARSER.on() {
                // The condition in parts, at each space.
                let condition = js_trim(condition);
                let parts = segment(condition, b' ');
                if parts[0] == "not" {
                    parts[1..].join(" ")
                } else if rule_name != "@container" || parts[0].starts_with('(') {
                    format!("not {condition}")
                } else if parts.get(1) == Some(&"not") {
                    // `{name} not {query}`
                    format!("{} {}", parts[0], parts[2..].join(" "))
                } else {
                    // `{name} {query}`
                    format!("{} not {}", parts[0], parts[1..].join(" "))
                }
            } else if rule_name == "@container" {
                let mut ast = value_parser::parse(js_trim(condition));
                let is_word = |node: &ValueAstNode| node.is_word();
                let is_not = |node: &ValueAstNode| node.is_word() && node.value() == "not";

                if !ast.is_empty() && ast[0].is_function() {
                    // `{query}`
                    format!("not {condition}")
                } else if ast.len() >= 3 && is_not(&ast[0]) && ast[2].is_function() {
                    // `not {query}`
                    ast.drain(0..2);
                    value_parser::to_css(&ast)
                } else if ast.len() >= 5 && is_word(&ast[0]) && is_not(&ast[2]) && ast[4].is_function() {
                    // `{name} not {query}`
                    ast.drain(2..4);
                    value_parser::to_css(&ast)
                } else if ast.len() >= 3 && is_word(&ast[0]) && !is_not(&ast[0]) && ast[2].is_function() {
                    // `{name} {query}`
                    ast.splice(1..1, [value_parser::separator(" "), value_parser::word("not")]);
                    value_parser::to_css(&ast)
                } else {
                    format!("not {condition}")
                }
            } else {
                let condition = js_trim(condition);
                let parts = segment(condition, b' ');
                if parts[0] == "not" {
                    // `@media not {query}`, `@supports not {query}`
                    parts[1..].join(" ")
                } else {
                    format!("not {condition}")
                }
            }
        })
        .collect()
}

fn negate_at_rule(rule: &AtRule) -> Option<AstNode> {
    if !matches!(rule.name.as_str(), "@media" | "@supports" | "@container") {
        return None;
    }
    let conditions = segment(&rule.params, b',');
    // The negation of a list of conditions is not supported.
    if conditions.len() > 1 {
        return None;
    }
    let conditions = negate_conditions(&rule.name, &conditions);
    Some(at_rule(rule.name.clone(), conditions.join(", "), Vec::new()))
}

fn negate_selector(selector: &str) -> Option<String> {
    if selector.contains("::") {
        return None;
    }
    // `&:hover` becomes `&:not(*:hover)`.
    let selectors: Vec<String> = segment(selector, b',').into_iter().map(|sel| sel.replace('&', "*")).collect();
    Some(format!("&:not({})", selectors.join(", ")))
}

/// The state of the walk of the `not` variant.
#[derive(Default)]
struct NotWalk {
    /// The new children of the rule, after the first rule that applies.
    rules: Option<Vec<AstNode>>,
    did_apply: bool,
}

/// One step of the walk of the `not` variant. The result is `false` when
/// the walk stops.
///
/// Upstream replaces the root of the walk when a rule applies, and then
/// continues in the old tree. From then on, the root in the path is the
/// style rule `&`.
fn not_visit<'a>(node: &'a AstNode, path: &mut Vec<&'a AstNode>, state: &mut NotWalk) -> bool {
    let children = match node {
        AstNode::Rule(StyleRule { nodes, .. }) | AstNode::AtRule(AtRule { nodes, .. }) if nodes.is_empty() => None,
        other => other.children(),
    };

    if let Some(children) = children {
        if children.is_empty() {
            return true;
        }
        path.push(node);
        for child in children {
            if !not_visit(child, path, state) {
                path.pop();
                return false;
            }
        }
        path.pop();
        return true;
    }
    if !matches!(node, AstNode::Rule(_) | AstNode::AtRule(_)) {
        return true;
    }

    // A rule with no children. Reject nested style rules and nested at-rules.
    let mut at_rules: Vec<&AtRule> = Vec::new();
    let mut style_rules: Vec<&str> = Vec::new();
    for (idx, ancestor) in path.iter().copied().chain(std::iter::once(node)).enumerate() {
        if idx == 0 && state.rules.is_some() {
            style_rules.push("&");
            continue;
        }
        match ancestor {
            AstNode::AtRule(r) => at_rules.push(r),
            AstNode::Rule(r) => style_rules.push(&r.selector),
            _ => {}
        }
    }
    if at_rules.len() > 1 || style_rules.len() > 1 {
        return false;
    }

    let mut rules: Vec<AstNode> = Vec::new();
    for selector in style_rules {
        let Some(selector) = negate_selector(selector) else {
            state.did_apply = false;
            return false;
        };
        rules.push(style_rule(selector, Vec::new()));
    }
    for r in at_rules {
        let Some(negated) = negate_at_rule(r) else {
            state.did_apply = false;
            return false;
        };
        rules.push(negated);
    }

    state.rules = Some(rules);
    state.did_apply = true;
    true
}

/// Change the selector of each style rule of the tree. The result is
/// `false` when a style rule is in another style rule.
fn rewrite_selectors(node: &mut AstNode, in_style_rule: bool, rewrite: &dyn Fn(&str) -> String) -> bool {
    let is_style_rule = if let AstNode::Rule(r) = node {
        if in_style_rule {
            return false;
        }
        r.selector = rewrite(&r.selector);
        true
    } else {
        false
    };
    if let Some(children) = node.children_mut() {
        for child in children {
            if !rewrite_selectors(child, in_style_rule || is_style_rule, rewrite) {
                return false;
            }
        }
    }
    true
}

/// The function of a compound variant that changes each selector.
fn selector_variant(rule_node: &mut AstNode, rewrite: &dyn Fn(&str) -> String) -> Option<()> {
    // No style rule in the tree: the variant does not apply.
    fn has_style_rule(node: &AstNode) -> bool {
        matches!(node, AstNode::Rule(_)) || node.children().is_some_and(|c| c.iter().any(has_style_rule))
    }
    if !rewrite_selectors(rule_node, false, rewrite) {
        return None;
    }
    has_style_rule(rule_node).then_some(())
}

fn is_relative_arbitrary(variant: &Variant) -> bool {
    matches!(variant, Variant::Compound { variant, .. } if matches!(**variant, Variant::Arbitrary { relative: true, .. }))
}

/// The function of `group` and `peer`. `combinator` is the text between
/// the selector of the group and `&`.
fn group_or_peer(name: &'static str, combinator: &'static str) -> impl Fn(&mut AstNode, &Variant, &DesignSystem) -> Option<()> {
    move |rule_node, variant, ds| {
        if is_relative_arbitrary(variant) {
            return None;
        }

        let prefix = match &ds.theme.prefix {
            Some(prefix) if !prefix.is_empty() => format!("{prefix}\\:"),
            _ => String::new(),
        };
        // The modifier gives a name to the group.
        let variant_selector = match variant.modifier() {
            Some(modifier) => format!(":where(.{prefix}{name}\\/{})", modifier.value),
            None => format!(":where(.{prefix}{name})"),
        };

        selector_variant(rule_node, &|selector| {
            let mut selector = selector.replace('&', &variant_selector);
            // A list of selectors must be in `:is(…)`.
            if segment(&selector, b',').len() > 1 {
                selector = format!(":is({selector})");
            }
            if switches::GROUP_PEER_TARGET_IN_IS.on() {
                format!(":is({selector}{combinator}&)")
            } else {
                format!("&:is({selector}{combinator}*)")
            }
        })
    }
}

/// `[\w-]*\s*\(` at the start of the value.
fn starts_with_function(value: &str) -> bool {
    let rest = value.trim_start_matches(|c: char| c.is_ascii_alphanumeric() || c == '_' || c == '-');
    let rest = rest.trim_start_matches(is_js_whitespace);
    rest.starts_with('(')
}

/// Before 4.3.4: `value.replace(/\b(and|or|not)\b/g, ' $1 ')`. Each of
/// the words gets spaces around it, also in `selector(…)` and between
/// hyphens.
fn space_supports_words(value: &str) -> String {
    let bytes = value.as_bytes();
    let is_word = |byte: u8| byte.is_ascii_alphanumeric() || byte == b'_';
    let mut out = String::with_capacity(value.len() + 8);
    let mut i = 0;
    while i < bytes.len() {
        let before = i == 0 || !is_word(bytes[i - 1]);
        let found = ["and", "or", "not"].into_iter().find(|word| {
            before && bytes[i..].starts_with(word.as_bytes()) && bytes.get(i + word.len()).is_none_or(|&next| !is_word(next))
        });
        match found {
            Some(word) => {
                out.push(' ');
                out.push_str(word);
                out.push(' ');
                i += word.len();
            }
            None => {
                let length = value[i..].chars().next().map_or(1, char::len_utf8);
                out.push_str(&value[i..i + length]);
                i += length;
            }
        }
    }
    out
}

/// Put spaces around `and`, `or` and `not` when they are functions.
fn space_supports_keywords(ast: &mut [ValueAstNode]) -> bool {
    let mut changed = false;
    for node in ast {
        let ValueAstNode::Function(f) = node else { continue };
        // A selector can have `a:not(b)`: do not change it.
        if f.value == "selector" {
            continue;
        }
        if matches!(f.value.as_str(), "and" | "or" | "not") {
            changed = true;
            f.value = format!(" {} ", f.value);
        }
        changed |= space_supports_keywords(&mut f.nodes);
    }
    changed
}

/// The function of the `nth-*` variants.
fn nth_variant(pseudo: &'static str) -> impl Fn(&mut AstNode, &Variant, &DesignSystem) -> Option<()> {
    move |rule_node, variant, _| {
        let value = variant.value()?;
        if variant.modifier().is_some() {
            return None;
        }
        // A bare value must be a number.
        if value.kind == ValueKind::Named && !is_positive_integer(&value.value) {
            return None;
        }
        let selector = format!("&:{pseudo}({})", value.value);
        wrap_in(rule_node, |nodes| style_rule(selector, nodes))
    }
}

/// The value of a breakpoint variant (`sm`, `min-*`, `max-*`).
fn resolved_breakpoint(variant: &Variant, theme: &Theme) -> Option<String> {
    match variant {
        Variant::Static { root } => theme.resolve_value(Some(root), &["--breakpoint"]),
        Variant::Functional { value, modifier, .. } => {
            let value = value.as_ref()?;
            if modifier.is_some() {
                return None;
            }
            let value = match value.kind {
                ValueKind::Arbitrary => Some(value.value.clone()),
                ValueKind::Named => theme.resolve_value(Some(&value.value), &["--breakpoint"]),
            }?;
            if value.is_empty() || value.contains("var(") {
                return None;
            }
            Some(value)
        }
        Variant::Arbitrary { .. } | Variant::Compound { .. } => None,
    }
}

/// The value of a container query variant (`@*`, `@min-*`, `@max-*`).
fn resolved_width(variant: &Variant, theme: &Theme) -> Option<String> {
    match variant {
        Variant::Functional { value, .. } => {
            let value = value.as_ref()?;
            let value = match value.kind {
                ValueKind::Arbitrary => Some(value.value.clone()),
                ValueKind::Named => theme.resolve_value(Some(&value.value), &["--container"]),
            }?;
            if value.is_empty() || value.contains("var(") {
                return None;
            }
            Some(value)
        }
        _ => None,
    }
}

/// Compare two variants by their values. The responsive variants and the
/// container queries use this function.
fn compare_breakpoint_variants(
    a: &Variant,
    z: &Variant,
    direction: Direction,
    lookup: fn(&Variant, &Theme) -> Option<String>,
    theme: &Theme,
) -> i32 {
    if std::ptr::eq(a, z) {
        return 0;
    }
    let asc = direction == Direction::Asc;
    let Some(a_value) = lookup(a, theme) else { return if asc { -1 } else { 1 } };
    let Some(z_value) = lookup(z, theme) else { return if asc { 1 } else { -1 } };
    match compare_breakpoints(&a_value, &z_value, direction) {
        Ordering::Less => -1,
        Ordering::Equal => 0,
        Ordering::Greater => 1,
    }
}

fn compare_by(direction: Direction, lookup: fn(&Variant, &Theme) -> Option<String>) -> Option<CompareFn> {
    Some(Rc::new(move |a, z, ds| compare_breakpoint_variants(a, z, direction, lookup, &ds.theme)))
}

/// The function of a container query variant. `operator` is `<` or `>=`.
fn container_variant(operator: &'static str) -> impl Fn(&mut AstNode, &Variant, &DesignSystem) -> Option<()> {
    move |rule_node, variant, ds| {
        let value = resolved_width(variant, &ds.theme)?;
        let params = match variant.modifier() {
            Some(modifier) => format!("{} (width {operator} {value})", modifier.value),
            None => format!("(width {operator} {value})"),
        };
        wrap_in(rule_node, |nodes| at_rule("@container", params, nodes))
    }
}

/// The function of `min` and `max`. `operator` is `<` or `>=`.
fn media_width_variant(operator: &'static str) -> impl Fn(&mut AstNode, &Variant, &DesignSystem) -> Option<()> {
    move |rule_node, variant, ds| {
        if variant.modifier().is_some() {
            return None;
        }
        let value = resolved_breakpoint(variant, &ds.theme)?;
        wrap_in(rule_node, |nodes| at_rule("@media", format!("(width {operator} {value})"), nodes))
    }
}

/// All the variants of Tailwind CSS.
pub fn create_variants(theme: &Theme) -> Variants {
    let mut variants = Variants::new();
    let v = &mut variants;
    let at_rules = VariantOptions { compounds: Some(COMPOUNDS_AT_RULES), order: None };
    let never = VariantOptions { compounds: Some(COMPOUNDS_NEVER), order: None };

    static_variant(v, "*", &[":is(& > *)"], Some(COMPOUNDS_NEVER));
    static_variant(v, "**", &[":is(& *)"], Some(COMPOUNDS_NEVER));

    v.compound(
        "not",
        COMPOUNDS_STYLE_RULES | COMPOUNDS_AT_RULES,
        |rule_node, variant, _| {
            if is_relative_arbitrary(variant) {
                return None;
            }
            if variant.modifier().is_some() {
                return None;
            }

            let mut state = NotWalk::default();
            not_visit(rule_node, &mut Vec::new(), &mut state);
            // Upstream changes the rule in place (`Object.assign`): the
            // rule keeps its source locations when the new node has none.
            let keep_loc = |rule_node: &mut AstNode, new: AstNode| {
                let old = rule_node.loc().cloned();
                *rule_node = new;
                if let (Some(old), Some(loc)) = (old, rule_node.loc_mut()) {
                    if !loc.is_some() {
                        *loc = old;
                    }
                }
            };
            if let Some(rules) = state.rules.take() {
                keep_loc(rule_node, style_rule("&", rules));
            }

            if let AstNode::Rule(r) = rule_node {
                if r.selector == "&" && r.nodes.len() == 1 {
                    let only = r.nodes.pop()?;
                    keep_loc(rule_node, only);
                }
            }

            // The variant does not apply when no rule was changed.
            state.did_apply.then_some(())
        },
        VariantOptions::default(),
    );

    v.compound("group", COMPOUNDS_STYLE_RULES, group_or_peer("group", " "), VariantOptions::default());
    v.compound("peer", COMPOUNDS_STYLE_RULES, group_or_peer("peer", " ~ "), VariantOptions::default());

    static_variant(v, "first-letter", &["&::first-letter"], None);
    static_variant(v, "first-line", &["&::first-line"], None);

    if crate::switches::MARKER_DETAILS_MARKER.on() {
        static_variant(v, "marker", &["& *::marker", "&::marker", "& *::-webkit-details-marker", "&::-webkit-details-marker"], None);
    } else {
        // Before 4.0.16, `marker` had no `::-webkit-details-marker`.
        static_variant(v, "marker", &["& *::marker", "&::marker"], None);
    }

    static_variant(v, "selection", &["& *::selection", "&::selection"], None);
    static_variant(v, "file", &["&::file-selector-button"], None);
    static_variant(v, "placeholder", &["&::placeholder"], None);
    static_variant(v, "backdrop", &["&::backdrop"], None);
    // The variants of 4.1.0 (`switches::STATIC_VARIANTS_OF_4_1`).
    let variants_of_4_1 = switches::STATIC_VARIANTS_OF_4_1.on();
    if variants_of_4_1 {
        static_variant(v, "details-content", &["&::details-content"], None);
    }

    for (name, selector) in [("before", "&::before"), ("after", "&::after")] {
        v.r#static(
            name,
            move |r, _, _| {
                wrap_in(r, |nodes| {
                    let content_properties = at_root(vec![at_rule(
                        "@property",
                        "--tw-content",
                        vec![decl("syntax", "\"*\""), decl("initial-value", "\"\""), decl("inherits", "false")],
                    )]);
                    let mut children = vec![content_properties, decl("content", "var(--tw-content)")];
                    children.extend(nodes);
                    style_rule(selector, children)
                })
            },
            never,
        );
    }

    // Positional
    static_variant(v, "first", &["&:first-child"], None);
    static_variant(v, "last", &["&:last-child"], None);
    static_variant(v, "only", &["&:only-child"], None);
    static_variant(v, "odd", &["&:nth-child(odd)"], None);
    static_variant(v, "even", &["&:nth-child(even)"], None);
    static_variant(v, "first-of-type", &["&:first-of-type"], None);
    static_variant(v, "last-of-type", &["&:last-of-type"], None);
    static_variant(v, "only-of-type", &["&:only-of-type"], None);

    // State
    static_variant(v, "visited", &["&:visited"], None);
    static_variant(v, "target", &["&:target"], None);
    static_variant(v, "open", &["&:is([open], :popover-open, :open)"], None);

    // Forms
    static_variant(v, "default", &["&:default"], None);
    static_variant(v, "checked", &["&:checked"], None);
    static_variant(v, "indeterminate", &["&:indeterminate"], None);
    static_variant(v, "placeholder-shown", &["&:placeholder-shown"], None);
    static_variant(v, "autofill", &["&:autofill"], None);
    static_variant(v, "optional", &["&:optional"], None);
    static_variant(v, "required", &["&:required"], None);
    static_variant(v, "valid", &["&:valid"], None);
    static_variant(v, "invalid", &["&:invalid"], None);
    if variants_of_4_1 {
        static_variant(v, "user-valid", &["&:user-valid"], None);
        static_variant(v, "user-invalid", &["&:user-invalid"], None);
    }
    static_variant(v, "in-range", &["&:in-range"], None);
    static_variant(v, "out-of-range", &["&:out-of-range"], None);
    static_variant(v, "read-only", &["&:read-only"], None);

    // Content
    static_variant(v, "empty", &["&:empty"], None);

    // Interactive
    static_variant(v, "focus-within", &["&:focus-within"], None);
    v.r#static(
        "hover",
        |r, _, _| wrap_in(r, |nodes| style_rule("&:hover", vec![at_rule("@media", "(hover: hover)", nodes)])),
        VariantOptions::default(),
    );
    static_variant(v, "focus", &["&:focus"], None);
    static_variant(v, "focus-visible", &["&:focus-visible"], None);
    static_variant(v, "active", &["&:active"], None);
    static_variant(v, "enabled", &["&:enabled"], None);
    static_variant(v, "disabled", &["&:disabled"], None);

    static_variant(v, "inert", &["&:is([inert], [inert] *)"], None);

    v.compound(
        "in",
        COMPOUNDS_STYLE_RULES,
        |rule_node, variant, _| {
            if variant.modifier().is_some() {
                return None;
            }
            // `&:hover` becomes `:where(*:hover) &`.
            selector_variant(rule_node, &|selector| format!(":where({}) &", selector.replace('&', "*")))
        },
        VariantOptions::default(),
    );

    v.compound(
        "has",
        COMPOUNDS_STYLE_RULES,
        |rule_node, variant, _| {
            if variant.modifier().is_some() {
                return None;
            }
            // `&:hover` becomes `&:has(*:hover)`.
            selector_variant(rule_node, &|selector| format!("&:has({})", selector.replace('&', "*")))
        },
        VariantOptions::default(),
    );

    v.functional(
        "aria",
        |rule_node, variant, _| {
            let value = variant.value()?;
            if variant.modifier().is_some() {
                return None;
            }
            let selector = match value.kind {
                ValueKind::Arbitrary => format!("[aria-{}]", quote_attribute_value(&value.value)),
                ValueKind::Named => format!("[aria-{}=\"true\"]", value.value),
            };
            if crate::switches::ARIA_DATA_ATTRIBUTE_CHECK.on() {
                crate::attribute_selector_parser::parse(&selector)?;
            }
            wrap_in(rule_node, |nodes| style_rule(format!("&{selector}"), nodes))
        },
        VariantOptions::default(),
    );

    v.functional(
        "data",
        |rule_node, variant, _| {
            let value = variant.value()?;
            if variant.modifier().is_some() {
                return None;
            }
            let selector = format!("[data-{}]", quote_attribute_value(&value.value));
            if crate::switches::ARIA_DATA_ATTRIBUTE_CHECK.on() {
                crate::attribute_selector_parser::parse(&selector)?;
            }
            wrap_in(rule_node, |nodes| style_rule(format!("&{selector}"), nodes))
        },
        VariantOptions::default(),
    );

    v.functional("nth", nth_variant("nth-child"), VariantOptions::default());
    v.functional("nth-last", nth_variant("nth-last-child"), VariantOptions::default());
    v.functional("nth-of-type", nth_variant("nth-of-type"), VariantOptions::default());
    v.functional("nth-last-of-type", nth_variant("nth-last-of-type"), VariantOptions::default());

    v.functional(
        "supports",
        |rule_node, variant, _| {
            let value = variant.value()?;
            if variant.modifier().is_some() {
                return None;
            }
            let mut value = value.value.clone();

            // A value that starts with a function, such as `not(…)` or
            // `selector(…)`, is used as it is.
            if starts_with_function(&value) {
                // `(a)or(b)` is not valid in Chrome, `(a) or (b)` is.
                let query = if switches::SUPPORTS_KEYWORDS_AS_FUNCTIONS.on() {
                    let mut ast = value_parser::parse(&value);
                    if space_supports_keywords(&mut ast) {
                        value_parser::to_css(&ast)
                    } else {
                        value
                    }
                } else {
                    space_supports_words(&value)
                };
                return wrap_in(rule_node, |nodes| at_rule("@supports", query, nodes));
            }

            // `supports-[display]` becomes `(display: var(--tw))`.
            if !value.contains(':') {
                value = format!("{value}: var(--tw)");
            }

            // `supports-[display:flex]` becomes `(display:flex)`.
            if !value.starts_with('(') || !value.ends_with(')') {
                value = format!("({value})");
            }

            wrap_in(rule_node, |nodes| at_rule("@supports", value, nodes))
        },
        at_rules,
    );

    static_variant(v, "motion-safe", &["@media (prefers-reduced-motion: no-preference)"], None);
    static_variant(v, "motion-reduce", &["@media (prefers-reduced-motion: reduce)"], None);

    static_variant(v, "contrast-more", &["@media (prefers-contrast: more)"], None);
    static_variant(v, "contrast-less", &["@media (prefers-contrast: less)"], None);

    // Breakpoints. Upstream reads the theme for `max-*` and `min-*` at the
    // time of the call. It reads the values of `sm`, `md`, … here.
    v.group(|v| v.functional("max", media_width_variant("<"), at_rules), compare_by(Direction::Desc, resolved_breakpoint));

    v.group(
        |v| {
            for (key, value) in theme.namespace("--breakpoint") {
                let Some(key) = key else { continue };
                v.r#static(
                    key,
                    move |r, _, _| wrap_in(r, |nodes| at_rule("@media", format!("(width >= {value})"), nodes)),
                    at_rules,
                );
            }
            v.functional("min", media_width_variant(">="), at_rules);
        },
        compare_by(Direction::Asc, resolved_breakpoint),
    );

    // Container queries
    v.group(|v| v.functional("@max", container_variant("<"), at_rules), compare_by(Direction::Desc, resolved_width));

    v.group(
        |v| {
            v.functional("@", container_variant(">="), at_rules);
            v.functional("@min", container_variant(">="), at_rules);
        },
        compare_by(Direction::Asc, resolved_width),
    );

    static_variant(v, "portrait", &["@media (orientation: portrait)"], None);
    static_variant(v, "landscape", &["@media (orientation: landscape)"], None);

    static_variant(v, "ltr", &["&:where(:dir(ltr), [dir=\"ltr\"], [dir=\"ltr\"] *)"], None);
    static_variant(v, "rtl", &["&:where(:dir(rtl), [dir=\"rtl\"], [dir=\"rtl\"] *)"], None);

    static_variant(v, "dark", &["@media (prefers-color-scheme: dark)"], None);

    static_variant(v, "starting", &["@starting-style"], None);

    static_variant(v, "print", &["@media print"], None);

    static_variant(v, "forced-colors", &["@media (forced-colors: active)"], None);

    if variants_of_4_1 {
        static_variant(v, "inverted-colors", &["@media (inverted-colors: inverted)"], None);

        static_variant(v, "pointer-none", &["@media (pointer: none)"], None);
        static_variant(v, "pointer-coarse", &["@media (pointer: coarse)"], None);
        static_variant(v, "pointer-fine", &["@media (pointer: fine)"], None);
        static_variant(v, "any-pointer-none", &["@media (any-pointer: none)"], None);
        static_variant(v, "any-pointer-coarse", &["@media (any-pointer: coarse)"], None);
        static_variant(v, "any-pointer-fine", &["@media (any-pointer: fine)"], None);

        static_variant(v, "noscript", &["@media (scripting: none)"], None);
    }

    variants
}

/// Put quotes around the value of an attribute selector, when it has none.
fn quote_attribute_value(input: &str) -> String {
    if !input.contains('=') {
        return input.to_string();
    }
    let parts = segment(input, b'=');
    let attribute = parts[0];
    let after = parts[1..].join("=");
    let value = js_trim(&after);

    // The value has quotes.
    if value.starts_with('\'') || value.starts_with('"') {
        return input.to_string();
    }

    // A flag for the case of the value: ` i` or ` s`.
    for flag in ['i', 'I', 's', 'S'] {
        if let Some(rest) = value.strip_suffix(flag).and_then(|rest| rest.strip_suffix(' ')) {
            return format!("{attribute}=\"{rest}\" {flag}");
        }
    }

    format!("{attribute}=\"{value}\"")
}

/// Replace each `@slot` with `nodes`. Put each `@keyframes` and `@property`
/// rule in an at-root node.
pub fn substitute_at_slot(ast: &mut Vec<AstNode>, nodes: &[AstNode]) {
    // Before 4.2.0, the walk also went into the nodes that replaced
    // `@slot`. So the `@keyframes` and `@property` rules of these nodes
    // went into at-root nodes too. (An `@slot` in these nodes made an
    // endless loop in the official compiler: here it stays.)
    let wrapped: Vec<AstNode>;
    let nodes = if switches::SLOT_NODES_STAY.on() {
        nodes
    } else {
        let mut copy = nodes.to_vec();
        walk(&mut copy, |node, _| match node {
            AstNode::AtRule(r) if r.name == "@keyframes" || r.name == "@property" => {
                let mut inner = std::mem::take(r);
                inner.loc = Default::default();
                *node = at_root(vec![AstNode::AtRule(inner)]);
                WalkAction::Skip
            }
            _ => WalkAction::Continue,
        });
        wrapped = copy;
        &wrapped
    };
    walk(ast, |node, _| {
        let AstNode::AtRule(r) = node else { return WalkAction::Continue };
        if r.name == "@slot" {
            return WalkAction::ReplaceSkip(nodes.to_vec());
        }
        if r.name == "@keyframes" || r.name == "@property" {
            // Upstream makes a new at-rule: it has no source location.
            let mut inner = std::mem::take(r);
            inner.loc = Default::default();
            let inner = AstNode::AtRule(inner);
            *node = at_root(vec![inner]);
            return WalkAction::Skip;
        }
        WalkAction::Continue
    });
}

/// Replace each `@variant` rule with the rules of its variant.
pub fn substitute_at_variant(ast: &mut Vec<AstNode>, design_system: &DesignSystem) -> Result<Features, Error> {
    let mut features = FEATURES_NONE;
    let mut error: Option<Error> = None;

    walk(ast, |variant_node, _| {
        let AstNode::AtRule(variant_rule) = variant_node else { return WalkAction::Continue };
        if variant_rule.name != "@variant" {
            return WalkAction::Continue;
        }

        let mut body = Some(std::mem::take(&mut variant_rule.nodes));
        let mut nodes: Vec<AstNode> = Vec::new();
        // Before 4.3.0, the text was one variant.
        let list = crate::switches::AT_VARIANT_LIST.on();
        let compound_variants = if list { segment(&variant_rule.params, b',') } else { vec![variant_rule.params.as_str()] };
        let last = compound_variants.len() - 1;

        for (idx, compound_variant) in compound_variants.iter().enumerate() {
            // Start with the `&` rule. The last one uses the nodes of the
            // `@variant` rule, the others use copies.
            let children = if idx == last { body.take().unwrap_or_default() } else { body.clone().unwrap_or_default() };
            let mut node = style_rule("&", children);

            let stacked_variants = if list { segment(compound_variant, b':') } else { vec![*compound_variant] };
            for variant in stacked_variants.into_iter().rev() {
                let variant = if list { js_trim(variant) } else { variant };

                if variant.is_empty() && list {
                    error = Some(Error::new("Cannot use `@variant` with empty variant"));
                    return WalkAction::Stop;
                }

                let Some(variant_ast) = design_system.parse_variant(variant) else {
                    error = Some(Error::new(format!("Cannot use `@variant` with unknown variant: {variant}")));
                    return WalkAction::Stop;
                };

                design_system.variants.take_error();
                if apply_variant(&mut node, &variant_ast, design_system, 0).is_none() {
                    // An error of the function of the variant comes first.
                    error = Some(
                        design_system
                            .variants
                            .take_error()
                            .unwrap_or_else(|| Error::new(format!("Cannot use `@variant` with variant: {variant}"))),
                    );
                    return WalkAction::Stop;
                }
            }

            match node {
                // Before the nesting pass, the `&` rule stayed until the
                // last step of the compiler.
                AstNode::Rule(StyleRule { selector, nodes: children, .. })
                    if selector == "&" && crate::switches::NESTING_PASS.on() =>
                {
                    nodes.extend(children)
                }
                other => nodes.push(other),
            }
        }

        // The walk visits the new nodes: they can have an `@variant` rule.
        features |= FEATURES_VARIANTS;
        WalkAction::Replace(nodes)
    });

    match error {
        Some(error) => Err(error),
        None => Ok(features),
    }
}
