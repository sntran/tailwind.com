//! The compiler of candidates to rules: `compile.ts` of Tailwind CSS.

use crate::ast::{at_rule, context, decl, rule, AstNode, ContextMap, StyleRule};
use crate::candidate::{Candidate, Variant};
use crate::design_system::{CompileAstFlags, DesignSystem, COMPILE_NONE, COMPILE_RESPECT_IMPORTANT};
use crate::property_order::property_index;
use crate::utilities::{as_color, CompileFn, Compiled, Utility};
use crate::utils::compare::compare;
use crate::utils::escape::escape;
use crate::variants::Variants;
use crate::walk::{walk, WalkAction};
use std::cmp::Ordering;
use std::collections::VecDeque;
use std::rc::Rc;

/// The data for the sort of one rule by its properties.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct PropertySort {
    pub order: Vec<usize>,
    pub count: usize,
}

/// One rule of a candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompiledNode {
    pub node: AstNode,
    pub property_sort: PropertySort,
}

/// One rule of the sorted output.
#[derive(Debug, Clone)]
pub struct SortedNode {
    pub node: AstNode,
    pub properties: PropertySort,
    /// The orders of the variants of the candidate, as a set of bits.
    pub variants: VariantBits,
    pub candidate: String,
}

/// A set of variant orders. Its order is the order of the number that has
/// these bits (a BigInt upstream).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct VariantBits(Vec<u64>);

impl VariantBits {
    pub fn set(&mut self, bit: u32) {
        let word = (bit / 64) as usize;
        if self.0.len() <= word {
            self.0.resize(word + 1, 0);
        }
        self.0[word] |= 1 << (bit % 64);
    }
    pub fn is_zero(&self) -> bool {
        self.0.iter().all(|w| *w == 0)
    }
}

impl Ord for VariantBits {
    fn cmp(&self, other: &Self) -> Ordering {
        let len = self.0.len().max(other.0.len());
        for i in (0..len).rev() {
            let a = self.0.get(i).copied().unwrap_or(0);
            let z = other.0.get(i).copied().unwrap_or(0);
            if a != z {
                return a.cmp(&z);
            }
        }
        Ordering::Equal
    }
}
impl PartialOrd for VariantBits {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// Compile the candidates, and sort their rules. `on_invalid_candidate`
/// gets each candidate that makes no rule.
pub fn compile_candidates(
    raw_candidates: &[String],
    design_system: &DesignSystem,
    on_invalid_candidate: &mut dyn FnMut(&str),
    respect_important: bool,
) -> Vec<SortedNode> {
    let mut sorted: Vec<SortedNode> = Vec::new();
    let mut matches: indexmap::IndexMap<&str, Rc<Vec<Rc<Candidate>>>> = indexmap::IndexMap::new();

    // Parse the candidates and the variants.
    for raw in raw_candidates {
        if design_system.invalid_candidates.borrow().contains(raw.as_str()) {
            on_invalid_candidate(raw);
            continue;
        }
        let candidates = design_system.parse_candidate(raw);
        if candidates.is_empty() {
            on_invalid_candidate(raw);
            continue;
        }
        matches.insert(raw.as_str(), candidates);
    }

    let flags = if respect_important { COMPILE_RESPECT_IMPORTANT } else { COMPILE_NONE };

    // With one candidate, the sort needs no variant order.
    let variant_order_map = if matches.len() > 1 || !crate::switches::ONE_CANDIDATE_NO_VARIANT_ORDER.on() {
        Some(design_system.get_variant_order())
    } else {
        None
    };

    // Make the rules.
    for (raw, candidates) in &matches {
        let mut found = false;
        for candidate in candidates.iter() {
            let rules = design_system.compile_ast_nodes(candidate, flags);
            if rules.is_empty() {
                continue;
            }
            found = true;

            for compiled in rules.iter() {
                // The more important variants have the higher bits.
                let mut variant_order = VariantBits::default();
                if let Some(map) = &variant_order_map {
                    for variant in candidate.variants() {
                        if let Some(bit) = map.get(&Rc::as_ptr(variant)) {
                            variant_order.set(*bit);
                        }
                    }
                }
                sorted.push(SortedNode {
                    node: compiled.node.clone(),
                    properties: compiled.property_sort.clone(),
                    variants: variant_order,
                    candidate: raw.to_string(),
                });
            }
        }
        if !found {
            on_invalid_candidate(raw);
        }
    }

    sorted.sort_by(|a, z| {
        // First the variants.
        let by_variants = a.variants.cmp(&z.variants);
        if by_variants != Ordering::Equal {
            return by_variants;
        }

        // Then the first property order that is different.
        let (a_order, z_order) = (&a.properties.order, &z.properties.order);
        let mut offset = 0;
        while offset < a_order.len() && offset < z_order.len() && a_order[offset] == z_order[offset] {
            offset += 1;
        }
        match (a_order.get(offset), z_order.get(offset)) {
            (Some(a), Some(z)) => return a.cmp(z),
            (Some(_), None) => return Ordering::Less,
            (None, Some(_)) => return Ordering::Greater,
            (None, None) => {}
        }

        // Then the count of properties (more comes first), then the name.
        z.properties.count.cmp(&a.properties.count).then_with(|| compare(&a.candidate, &z.candidate))
    });

    sorted
}

/// The rules of one candidate, with its variants applied.
pub fn compile_ast_nodes(candidate: &Candidate, design_system: &DesignSystem, flags: CompileAstFlags) -> Vec<CompiledNode> {
    let asts = compile_base_utility(candidate, design_system);
    if asts.is_empty() {
        return Vec::new();
    }

    let respect_important = design_system.important && (flags & COMPILE_RESPECT_IMPORTANT != 0);

    let mut rules = Vec::new();
    let selector = format!(".{}", escape(candidate.raw()));

    for mut nodes in asts {
        let property_sort = get_property_sort(&nodes);

        // The `!important` of the candidate (`!flex`) is always applied. The
        // one of the design system is applied only when the flag asks.
        if candidate.important() || respect_important {
            apply_important(&mut nodes);
        }

        let mut node = AstNode::Rule(StyleRule { selector: selector.clone(), nodes, loc: Default::default() });

        for variant in candidate.variants() {
            if apply_variant(&mut node, variant, design_system, 0).is_none() {
                return Vec::new();
            }
        }

        rules.push(CompiledNode { node, property_sort });
    }

    rules
}

/// Apply a variant to a rule (a style rule or an at-rule). `None` is "the
/// variant does not apply".
pub fn apply_variant(node: &mut AstNode, variant: &Variant, design_system: &DesignSystem, depth: usize) -> Option<()> {
    let variants: &Variants = &design_system.variants;

    if let Variant::Arbitrary { selector, relative } = variant {
        // A relative arbitrary variant (`[>img]`) is valid only inside a
        // compound variant.
        if *relative && depth == 0 {
            return None;
        }

        let nodes = std::mem::take(node.children_mut()?);
        let mut child = rule(selector.clone(), Vec::new());
        if child.is_at_rule("@scope") && crate::switches::SCOPE_HOISTING.on() {
            let mut user = ContextMap::new();
            user.insert("source".into(), "user".into());
            *child.children_mut().unwrap() = vec![context(user, nodes)];
            let mut variant_context = ContextMap::new();
            variant_context.insert("source".into(), "variant".into());
            child = context(variant_context, vec![child]);
        } else {
            *child.children_mut().unwrap() = nodes;
        }
        *node.children_mut()? = vec![child];
        return Some(());
    }

    let root = variant.root()?;
    let apply_fn = variants.get(root)?.apply_fn.clone();

    if let Variant::Compound { variant: inner, .. } = variant {
        // Apply the inner variant to an isolated rule. The compound variant
        // then changes the selectors of the result.
        let mut isolated = at_rule("@slot", "", Vec::new());
        apply_variant(&mut isolated, inner, design_system, depth + 1)?;

        let isolated_nodes = isolated.children_mut().unwrap();

        // `not-*` with more than one rule needs a list of at-rule
        // conditions, which CSS cannot nest.
        if root == "not" && isolated_nodes.len() > 1 {
            return None;
        }

        for child in isolated_nodes.iter_mut() {
            // Only rules and at-rules can be at the top of a compound variant.
            if !matches!(child, AstNode::Rule(_) | AstNode::AtRule(_)) {
                return None;
            }
            apply_fn(child, variant, design_system)?;
        }

        // Put the nodes of the rule at the place of the slot: each rule
        // with no children.
        let mut isolated_nodes = std::mem::take(isolated_nodes);
        let mut body = std::mem::take(node.children_mut()?);
        // Upstream puts the same nodes in each empty rule.
        if design_system.track_sources.get() {
            crate::ast::share_identity(&mut body);
        }
        walk(&mut isolated_nodes, |child, _| match child {
            AstNode::Rule(StyleRule { nodes, .. }) | AstNode::AtRule(crate::ast::AtRule { nodes, .. })
                if nodes.is_empty() =>
            {
                *nodes = body.clone();
                WalkAction::Skip
            }
            _ => WalkAction::Continue,
        });
        *node.children_mut()? = isolated_nodes;
        return Some(());
    }

    // All the other variants.
    apply_fn(node, variant, design_system)
}

fn is_fallback_utility(utility: &Utility) -> bool {
    match &utility.options {
        Some(options) => options.types.len() > 1 && options.types.iter().any(|t| t == "any"),
        None => false,
    }
}

fn run_utility(utility: &Utility, candidate: &Candidate, design_system: &DesignSystem) -> Option<Compiled> {
    match (&utility.compile_fn, candidate) {
        (CompileFn::Static(f), Candidate::Static(c)) => Some(f(c, design_system)),
        (CompileFn::Functional(f), Candidate::Functional(c)) => Some(f(c, design_system)),
        _ => None,
    }
}

fn compile_base_utility(candidate: &Candidate, design_system: &DesignSystem) -> Vec<Vec<AstNode>> {
    if let Candidate::Arbitrary(c) = candidate {
        // An arbitrary property with a modifier is a color with an opacity.
        let value = match &c.modifier {
            Some(_) => as_color(&c.value, c.modifier.as_ref(), &design_system.theme),
            None => Some(c.value.clone()),
        };
        return match value {
            Some(value) => vec![vec![decl(c.property.clone(), value)]],
            None => Vec::new(),
        };
    }

    let utilities = design_system.utilities.get(candidate.root().unwrap_or(""));
    let mut asts: Vec<Vec<AstNode>> = Vec::new();

    for fallback_pass in [false, true] {
        for utility in utilities.iter().filter(|u| is_fallback_utility(u) == fallback_pass) {
            let Some(compiled) = run_utility(utility, candidate, design_system) else { continue };
            match compiled {
                Compiled::Undefined => continue,
                Compiled::Null => {
                    if utility.options.as_ref().is_some_and(|o| !o.types.is_empty()) {
                        return asts;
                    }
                    continue;
                }
                Compiled::Nodes(nodes) => asts.push(nodes),
            }
        }
        if !asts.is_empty() {
            return asts;
        }
    }

    asts
}

fn apply_important(ast: &mut [AstNode]) {
    for node in ast {
        match node {
            // An at-root has `@property` rules and such: no `!important`.
            AstNode::AtRoot(_) => continue,
            AstNode::Declaration(d) => d.important = true,
            AstNode::Rule(StyleRule { nodes, .. }) | AstNode::AtRule(crate::ast::AtRule { nodes, .. }) => {
                apply_important(nodes)
            }
            _ => {}
        }
    }
}

fn get_property_sort(nodes: &[AstNode]) -> PropertySort {
    // The unique property orders of the rule, and the count of its
    // declarations.
    let mut order: Vec<usize> = Vec::new();
    let mut count = 0;
    let mut q: VecDeque<&AstNode> = nodes.iter().collect();
    let mut seen_tw_sort = false;

    while let Some(node) = q.pop_front() {
        match node {
            AstNode::Declaration(d) => {
                let Some(value) = &d.value else { continue };
                count += 1;
                if seen_tw_sort {
                    continue;
                }
                if d.property == "--tw-sort" {
                    if let Some(idx) = property_index(value) {
                        if !order.contains(&idx) {
                            order.push(idx);
                        }
                        seen_tw_sort = true;
                        continue;
                    }
                }
                if let Some(idx) = property_index(&d.property) {
                    if !order.contains(&idx) {
                        order.push(idx);
                    }
                }
            }
            AstNode::Rule(StyleRule { nodes, .. }) | AstNode::AtRule(crate::ast::AtRule { nodes, .. }) => {
                q.extend(nodes.iter());
            }
            _ => {}
        }
    }

    order.sort_unstable();
    PropertySort { order, count }
}
