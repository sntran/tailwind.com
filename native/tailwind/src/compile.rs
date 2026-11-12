//! The compiler of candidates to rules: `compile.ts` of Tailwind CSS.

use crate::ast::{at_rule, context, decl, rule, AstNode, ContextMap, Declaration, StyleRule};
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
    /// The place of the rule in the cache of the compiler: the parsed
    /// candidate, and the index of the rule in the rules of that candidate.
    pub origin: (*const Candidate, usize),
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

            for (rule_index, compiled) in rules.iter().enumerate() {
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
                    origin: (Rc::as_ptr(candidate), rule_index),
                });
            }
        }
        if !found {
            on_invalid_candidate(raw);
        }
    }

    let by_order = |a: &SortedNode, z: &SortedNode| {
        // First the variants.
        let by_variants = a.variants.cmp(&z.variants);
        if by_variants != Ordering::Equal {
            return by_variants;
        }

        // Then the first property order that is different.
        // (Before 4.0.8, only the first property was compared.)
        let (a_order, z_order) = (&a.properties.order, &z.properties.order);
        let mut offset = 0;
        if crate::switches::SORT_BY_EACH_PROPERTY.on() {
            while offset < a_order.len() && offset < z_order.len() && a_order[offset] == z_order[offset] {
                offset += 1;
            }
        }
        match (a_order.get(offset), z_order.get(offset)) {
            (Some(a), Some(z)) if a != z => return a.cmp(z),
            (Some(_), Some(_)) => {}
            (Some(_), None) => return Ordering::Less,
            (None, Some(_)) => return Ordering::Greater,
            (None, None) => {}
        }

        // Then the count of properties (more comes first), then the name.
        z.properties.count.cmp(&a.properties.count).then_with(|| compare(&a.candidate, &z.candidate))
    };
    if crate::switches::COMPARE_WHOLE_NUMBERS.on() {
        sorted.sort_by(by_order);
    } else {
        // Before 4.0.7, the order of the names was not consistent (`p-1`
        // before `p-02`, `p-02` before `p-0a`, `p-0a` before `p-1`). So the
        // result is the result of the algorithm of V8.
        crate::utils::v8_sort::sort(&mut sorted, |a, z| by_order(a, z) as i64);
    }

    // Only in 4.1.14: a declaration of a fixed value is `!important` in
    // each rule when one candidate made it so.
    let shared = design_system.shared_declarations.borrow();
    if shared.values().any(|important| *important) {
        for sorted_node in &mut sorted {
            apply_shared_important(std::slice::from_mut(&mut sorted_node.node), &shared);
        }
    }
    drop(shared);

    sorted
}

/// Only for Tailwind CSS 4.1.14. In that version, the fixed values of a
/// utility (`z-auto`, `leading-none`) gave the same node objects to each
/// candidate. So the `!important` of one candidate (`z-auto!`) changed the
/// declarations of each candidate with that value, also in the rules that
/// the compiler made before. This function records the declarations of
/// such a value. `compile_ast_nodes` records the `!important`, and
/// `compile_candidates` gives it to each copy at its end: upstream reads
/// the objects after it compiles all the candidates.
pub fn share_declarations(nodes: &[AstNode], design_system: &DesignSystem) {
    let mut shared = design_system.shared_declarations.borrow_mut();
    for_each_declaration(nodes, &mut |declaration| {
        if let Some(identity) = declaration.loc.identity() {
            shared.entry(identity).or_insert(false);
        }
    });
}

/// The declarations that `apply_important` changes.
fn for_each_declaration(ast: &[AstNode], visit: &mut dyn FnMut(&Declaration)) {
    for node in ast {
        match node {
            AstNode::AtRoot(_) => continue,
            AstNode::Declaration(d) => visit(d),
            AstNode::Rule(StyleRule { nodes, .. }) | AstNode::AtRule(crate::ast::AtRule { nodes, .. }) => {
                for_each_declaration(nodes, visit)
            }
            _ => {}
        }
    }
}

/// Make each copy of a shared declaration with `!important` important.
fn apply_shared_important(ast: &mut [AstNode], shared: &std::collections::HashMap<*const (), bool>) {
    for node in ast {
        if let AstNode::Declaration(d) = node {
            if d.loc.identity().and_then(|identity| shared.get(&identity).copied()) == Some(true) {
                d.important = true;
            }
        } else if let Some(children) = node.children_mut() {
            apply_shared_important(children, shared);
        }
    }
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

            // Only in 4.1.14: see `share_declarations`.
            let mut shared = design_system.shared_declarations.borrow_mut();
            if !shared.is_empty() {
                for_each_declaration(&nodes, &mut |declaration| {
                    if let Some(important) = declaration.loc.identity().and_then(|identity| shared.get_mut(&identity)) {
                        *important = true;
                    }
                });
            }
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
                    // Before 4.3.0, each utility that was not valid stopped
                    // the other utilities.
                    if utility.options.as_ref().is_some_and(|o| !o.types.is_empty())
                        || !crate::switches::UTILITY_NULL_FALLTHROUGH.on()
                    {
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
            AstNode::Declaration(d) => {
                let mut property = d.property.chars();
                let (first, second) = (property.next(), property.next());
                let important = if crate::switches::IMPORTANT_CUSTOM_PROPERTIES.on()
                    || !crate::switches::IMPORTANT_SKIPS_DASH_PROPERTIES.on()
                {
                    // Before 4.0.8 and from 4.0.10: each declaration.
                    true
                } else if crate::switches::IMPORTANT_DASH_PROPERTIES.on() {
                    // Before 4.0.10: not a custom property.
                    !(first == Some('-') && second == Some('-'))
                } else {
                    // Only in 4.0.8: not a property with `-` as its first
                    // or its second character (`-webkit-…`, `z-index`).
                    first != Some('-') && second != Some('-')
                };
                if important {
                    d.important = true;
                }
            }
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
                // A declaration with no value is not in the output.
                // (Before 4.0.12, its property was in the sort.)
                if d.value.is_some() {
                    count += 1;
                } else if crate::switches::SORT_IGNORES_EMPTY_DECLARATIONS.on() {
                    continue;
                }
                if seen_tw_sort {
                    continue;
                }
                if d.property == "--tw-sort" {
                    if let Some(idx) = property_index(d.value.as_deref().unwrap_or("")) {
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
    // Before 4.0.8, the count was the count of the different properties
    // that have a place in the order.
    if !crate::switches::SORT_BY_EACH_PROPERTY.on() {
        count = order.len();
    }
    PropertySort { order, count }
}
