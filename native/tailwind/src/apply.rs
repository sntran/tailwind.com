//! The `@apply` rule: `apply.ts` of Tailwind CSS.
//!
//! Upstream uses the identity of the nodes for the set of parents and for
//! the sort. Here a node is a path of indices from the root. A path stays
//! valid until the substitution changes an ancestor of the node, and then
//! the node needs no more work.
//!
//! Upstream reads the nodes of an `@utility` rule at the time of each
//! compilation, so a change by `@apply` in the rule is visible to the
//! utility. Here the registered utility cannot read the tree. Thus the
//! caller gives a function that gets each changed `@utility` rule, and
//! that gives the new nodes to the utility.

use crate::ast::{rule, AstNode, AtRule};
use crate::compile::compile_candidates;
use crate::source_maps::{utf16_len, SourceLocation};
use crate::design_system::DesignSystem;
use crate::to_css::to_css;
use crate::utils::js::is_js_whitespace;
use crate::utils::segment::segment;
use crate::{Error, Features, Result, FEATURES_AT_APPLY};
use indexmap::{IndexMap, IndexSet};
use std::collections::{HashMap, HashSet};

/// The path of a node: the index at each level, from the root.
type Path = Vec<usize>;

/// A function that the substitution calls after it changes the nodes of an
/// `@utility` rule. The number is the position of the rule in all the
/// `@utility` rules of the tree, in document order, from 0.
pub type OnUtilityChange<'a> = &'a mut dyn FnMut(usize, &AtRule);

/// Replace each `@apply` rule with the declarations of its utilities.
///
/// `on_utility_change` gets each `@utility` rule that changed: see the
/// module text. `None` is correct when the tree has no `@utility` rule with
/// an `@apply` in it.
pub fn substitute_at_apply(
    ast: &mut Vec<AstNode>,
    design_system: &DesignSystem,
    mut on_utility_change: Option<OnUtilityChange>,
) -> Result<Features> {
    // A root rule makes sure that each `@apply` has a parent.
    let mut tree = vec![rule("&", std::mem::take(ast))];
    let result = substitute(&mut tree, design_system, &mut on_utility_change);
    if let Some(AstNode::Rule(root)) = tree.pop() {
        *ast = root.nodes;
    }
    result
}

/// What the first walk collects.
#[derive(Default)]
struct Collected {
    features: Features,
    /// The nodes that contain an `@apply`.
    parents: IndexSet<Path>,
    /// The roots of the utilities that a node applies.
    dependencies: HashMap<Path, IndexSet<String>>,
    /// The `@utility` rules of each root.
    definitions: IndexMap<String, IndexSet<Path>>,
    /// The number of `@utility` rules before each node, for the parents and
    /// the definitions.
    utilities_before: HashMap<Path, usize>,
    utility_count: usize,
}

fn substitute(
    tree: &mut Vec<AstNode>,
    design_system: &DesignSystem,
    on_utility_change: &mut Option<OnUtilityChange>,
) -> Result<Features> {
    let mut collected = Collected::default();
    collect(tree, &mut Vec::new(), design_system, &mut collected)?;

    // Sort: a node comes after the definitions of the utilities it applies.
    let mut sorter = Sorter {
        tree,
        design_system,
        collected: &collected,
        seen: HashSet::new(),
        wip: HashSet::new(),
        sorted: Vec::new(),
    };
    for parent in &collected.parents {
        sorter.visit(parent, &mut Vec::new())?;
    }
    let sorted = sorter.sorted;

    if !crate::switches::APPLY_NESTED_WALK.on() {
        substitute_direct_children(tree, sorted, &collected, design_system, on_utility_change)?;
        return Ok(collected.features);
    }

    // Substitute in the sorted sequence. The substitution of a node also
    // does all its descendants.
    let mut done: Vec<Path> = Vec::new();
    for path in sorted {
        if done.iter().any(|ancestor| path.len() > ancestor.len() && path.starts_with(ancestor)) {
            continue;
        }

        let Some(node) = node_at_mut(tree, &path) else { continue };
        let mut state = State {
            design_system,
            on_utility_change,
            next_utility: collected.utilities_before.get(&path).copied().unwrap_or(0),
        };
        match node {
            AstNode::AtRule(utility) if utility.name == "@utility" => {
                state.next_utility += 1;
                let ordinal = state.next_utility - 1;
                if substitute_in(&mut utility.nodes, &mut state)? {
                    if let Some(callback) = state.on_utility_change.as_mut() {
                        callback(ordinal, utility);
                    }
                }
            }
            node => {
                if let Some(nodes) = node.children_mut() {
                    substitute_in(nodes, &mut state)?;
                }
            }
        }
        done.push(path);
    }

    Ok(collected.features)
}

/// The substitution before 4.1.6: for each parent in the sorted sequence,
/// only the `@apply` rules that are children of the parent.
///
/// A utility that an `@apply` compiles before the substitution of its
/// nested rules keeps the nested `@apply` rules, and the cache of the
/// compiler keeps that result. An `@apply` that stays is not in the
/// output: the last pass of the compiler drops an at-rule with no
/// children.
fn substitute_direct_children(
    tree: &mut [AstNode],
    sorted: Vec<Path>,
    collected: &Collected,
    design_system: &DesignSystem,
    on_utility_change: &mut Option<OnUtilityChange>,
) -> Result<()> {
    // The `@utility` rule that contains each parent: the length of its
    // path, and its position in all the `@utility` rules. A substitution
    // moves the nodes after it, so the paths change, but not the lengths.
    let utility_paths: HashSet<&Path> = collected.definitions.values().flatten().collect();
    let utilities: Vec<Option<(usize, usize)>> = sorted
        .iter()
        .map(|path| {
            (1..=path.len()).find(|length| utility_paths.contains(&path[..*length].to_vec())).and_then(|length| {
                collected.utilities_before.get(&path[..length]).map(|ordinal| (length, *ordinal))
            })
        })
        .collect();

    // A path is `None` when a substitution removed its node.
    let mut sorted: Vec<Option<Path>> = sorted.into_iter().map(Some).collect();
    for position in 0..sorted.len() {
        let Some(path) = sorted[position].clone() else { continue };
        // Upstream has the rule above a context node as the parent, and
        // the `@apply` is not a child of that rule: no substitution. An
        // `@apply` at the top level of a stylesheet is such a rule.
        let Some(nodes) = node_at_mut(tree, &path)
            .filter(|node| !matches!(node, AstNode::Context(_)))
            .and_then(AstNode::children_mut)
        else {
            continue;
        };

        let mut changed = false;
        let mut index = 0;
        while index < nodes.len() {
            let AstNode::AtRule(child) = &nodes[index] else {
                index += 1;
                continue;
            };
            if child.name != "@apply" {
                index += 1;
                continue;
            }
            let Some(new_nodes) = apply(child, design_system)? else {
                index += 1;
                continue;
            };
            let count = new_nodes.len();
            nodes.splice(index..=index, new_nodes);
            changed = true;

            // The nodes after the `@apply` rule have a new index.
            for later in sorted[position + 1..].iter_mut() {
                let Some(other) = later else { continue };
                if other.len() <= path.len() || !other.starts_with(&path) {
                    continue;
                }
                let at = &mut other[path.len()];
                if *at == index {
                    *later = None;
                } else if *at > index {
                    *at = *at + count - 1;
                }
            }

            // As the loop of upstream: the next index, also when the new
            // nodes start there.
            index += 1;
        }

        if !changed {
            continue;
        }
        if let (Some((length, ordinal)), Some(callback)) = (utilities[position], on_utility_change.as_mut()) {
            if let Some(AstNode::AtRule(utility)) = node_at(tree, &path[..length]) {
                callback(ordinal, utility);
            }
        }
    }
    Ok(())
}

fn node_at<'a>(tree: &'a [AstNode], path: &[usize]) -> Option<&'a AstNode> {
    let (first, rest) = path.split_first()?;
    let mut node = tree.get(*first)?;
    for index in rest {
        node = node.children()?.get(*index)?;
    }
    Some(node)
}

fn node_at_mut<'a>(tree: &'a mut [AstNode], path: &[usize]) -> Option<&'a mut AstNode> {
    let (first, rest) = path.split_first()?;
    let mut node = tree.get_mut(*first)?;
    for index in rest {
        node = node.children_mut()?.get_mut(*index)?;
    }
    Some(node)
}

/// `params.split(/\s+/g)`.
fn split_whitespace(params: &str) -> Vec<&str> {
    split_with_separators(params).into_iter().step_by(2).collect()
}

/// `params.split(/(\s+)/g)`: the parts at the even positions, and the white
/// space between them at the odd positions.
fn split_with_separators(params: &str) -> Vec<&str> {
    let mut parts = Vec::new();
    let mut start = 0;
    let mut chars = params.char_indices().peekable();
    while let Some((index, c)) = chars.next() {
        if !is_js_whitespace(c) {
            continue;
        }
        let mut end = index + c.len_utf8();
        while let Some(&(next_index, next)) = chars.peek() {
            if !is_js_whitespace(next) {
                break;
            }
            end = next_index + next.len_utf8();
            chars.next();
        }
        parts.push(&params[start..index]);
        parts.push(&params[index..end]);
        start = end;
    }
    parts.push(&params[start..]);
    parts
}

/// `params.replace(/-\*$/, '')`: the root of the utility of an `@utility`.
fn utility_root(params: &str) -> &str {
    params.strip_suffix("-*").unwrap_or(params)
}

/// The roots of the utilities that an `@apply` rule applies.
fn resolve_apply_dependencies(node: &AtRule, design_system: &DesignSystem) -> Vec<String> {
    let mut roots = Vec::new();
    for candidate in split_whitespace(&node.params) {
        for parsed in design_system.parse_candidate(candidate).iter() {
            // An arbitrary property needs no definition.
            if let Some(root) = parsed.root() {
                roots.push(root.to_string());
            }
        }
    }
    roots
}

/// Call the function for each `@apply` rule of the subtree.
fn for_each_apply(nodes: &[AstNode], f: &mut dyn FnMut(&AtRule) -> Result<()>) -> Result<()> {
    for node in nodes {
        if let AstNode::AtRule(rule) = node {
            if rule.name == "@apply" {
                f(rule)?;
            }
        }
        if let Some(children) = node.children() {
            for_each_apply(children, f)?;
        }
    }
    Ok(())
}

/// The first walk: find the `@utility` definitions and the `@apply` rules.
fn collect(nodes: &[AstNode], path: &mut Path, design_system: &DesignSystem, out: &mut Collected) -> Result<()> {
    for (index, node) in nodes.iter().enumerate() {
        path.push(index);

        if let AstNode::AtRule(at) = node {
            // No `@apply` in `@keyframes`.
            if at.name == "@keyframes" {
                for_each_apply(&at.nodes, &mut |_| {
                    Err(Error::new("You cannot use `@apply` inside `@keyframes`."))
                })?;
                path.pop();
                continue;
            }

            // The sort needs the definition of each utility.
            if at.name == "@utility" {
                out.utilities_before.insert(path.clone(), out.utility_count);
                out.utility_count += 1;

                out.definitions.entry(utility_root(&at.params).to_string()).or_default().insert(path.clone());

                for_each_apply(&at.nodes, &mut |child| {
                    out.parents.insert(path.clone());
                    let dependencies = out.dependencies.entry(path.clone()).or_default();
                    for dependency in resolve_apply_dependencies(child, design_system) {
                        dependencies.insert(dependency);
                    }
                    Ok(())
                })?;
            } else if at.name == "@apply" {
                out.features |= FEATURES_AT_APPLY;

                let parent = path[..path.len() - 1].to_vec();
                if !out.utilities_before.contains_key(&parent) {
                    out.utilities_before.insert(parent.clone(), out.utility_count);
                }
                out.parents.insert(parent);

                for dependency in resolve_apply_dependencies(at, design_system) {
                    // Each parent in the path depends on that utility.
                    // Before 4.1.6: only the parent of the `@apply` rule.
                    let first = if crate::switches::APPLY_NESTED_WALK.on() { 1 } else { path.len() - 1 };
                    for length in first..path.len() {
                        let ancestor = &path[..length];
                        if !out.parents.contains(ancestor) {
                            continue;
                        }
                        out.dependencies.entry(ancestor.to_vec()).or_default().insert(dependency.clone());
                    }
                }
            }
        }

        if let Some(children) = node.children() {
            collect(children, path, design_system, out)?;
        }
        path.pop();
    }
    Ok(())
}

/// The topological sort.
struct Sorter<'a> {
    tree: &'a [AstNode],
    design_system: &'a DesignSystem,
    collected: &'a Collected,
    seen: HashSet<Path>,
    wip: HashSet<Path>,
    sorted: Vec<Path>,
}

impl Sorter<'_> {
    fn visit(&mut self, node: &Path, path: &mut Vec<Path>) -> Result<()> {
        if self.seen.contains(node) {
            return Ok(());
        }

        if self.wip.contains(node) {
            return Err(self.circular_dependency(node, path));
        }

        self.wip.insert(node.clone());

        let collected = self.collected;
        if let Some(dependencies) = collected.dependencies.get(node) {
            for dependency_id in dependencies {
                let Some(definitions) = collected.definitions.get(dependency_id) else { continue };
                for dependency in definitions {
                    path.push(node.clone());
                    self.visit(dependency, path)?;
                    path.pop();
                }
            }
        }

        self.seen.insert(node.clone());
        self.wip.remove(node);

        self.sorted.push(node.clone());
        Ok(())
    }

    fn circular_dependency(&self, node_path: &Path, path: &[Path]) -> Error {
        // The next node in the path is the cause of the circular dependency.
        let position = path.iter().position(|p| p == node_path).unwrap_or(0);
        let next_path = &path[(position + 1) % path.len()];

        let (Some(node), Some(next)) = (node_at(self.tree, node_path), node_at(self.tree, next_path)) else {
            return Error::new("Circular dependency detected");
        };

        if let (AstNode::AtRule(node), AstNode::AtRule(next)) = (node, next) {
            if node.name == "@utility" && next.name == "@utility" {
                let next_root = utility_root(&next.params);
                let found = for_each_apply(&node.nodes, &mut |child| {
                    for candidate in split_whitespace(&child.params) {
                        for parsed in self.design_system.parse_candidate(candidate).iter() {
                            if parsed.root() == Some(next_root) {
                                return Err(Error::new(format!(
                                    "You cannot `@apply` the `{candidate}` utility here because it creates a circular dependency."
                                )));
                            }
                        }
                    }
                    Ok(())
                });
                if let Err(error) = found {
                    return error;
                }
            }
        }

        // The origin of the circular dependency is not clear.
        Error::new(format!(
            "Circular dependency detected:\n\n{}\nRelies on:\n\n{}",
            to_css(std::slice::from_ref(node)),
            to_css(std::slice::from_ref(next))
        ))
    }
}

/// The state of the substitution in one parent.
struct State<'a, 'c> {
    design_system: &'a DesignSystem,
    on_utility_change: &'a mut Option<OnUtilityChange<'c>>,
    /// The position of the next `@utility` rule that the walk finds.
    next_utility: usize,
}

/// Substitute each `@apply` rule in the nodes and in their descendants. The
/// result is true when a node of this list or of a descendant changed. A
/// change in a nested `@utility` rule is reported to the callback, and also
/// to the caller.
fn substitute_in(nodes: &mut Vec<AstNode>, state: &mut State) -> Result<bool> {
    let mut changed = false;
    let mut index = 0;
    while index < nodes.len() {
        match &mut nodes[index] {
            AstNode::AtRule(child) if child.name == "@apply" => {
                match apply(child, state.design_system)? {
                    // A CSS mixin stays as it is, and the walk skips its body.
                    None => index += 1,
                    // The walk visits the new nodes.
                    Some(new_nodes) => {
                        nodes.splice(index..=index, new_nodes);
                        changed = true;
                    }
                }
            }
            AstNode::AtRule(utility) if utility.name == "@utility" => {
                let ordinal = state.next_utility;
                state.next_utility += 1;
                if substitute_in(&mut utility.nodes, state)? {
                    changed = true;
                    if let Some(callback) = state.on_utility_change.as_mut() {
                        callback(ordinal, utility);
                    }
                }
                index += 1;
            }
            node => {
                if let Some(children) = node.children_mut() {
                    changed |= substitute_in(children, state)?;
                }
                index += 1;
            }
        }
    }
    Ok(changed)
}

/// True for a key that a JavaScript object puts before its other keys: a
/// canonical array index.
fn array_index(key: &str) -> Option<u32> {
    if key.is_empty() || key.len() > 10 || !key.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    if key.len() > 1 && key.starts_with('0') {
        return None;
    }
    key.parse::<u64>().ok().filter(|n| *n < u64::from(u32::MAX)).map(|n| n as u32)
}

/// The nodes for one `@apply` rule. `None` is a CSS mixin.
fn apply(child: &AtRule, design_system: &DesignSystem) -> Result<Option<Vec<AstNode>>> {
    let parts = split_with_separators(&child.params);
    let mut candidates: IndexSet<&str> = IndexSet::new();
    let mut normal_idents: Vec<&str> = Vec::new();
    let mut dashed_idents: Vec<&str> = Vec::new();

    // The offset of each candidate in the params, in UTF-16 code units.
    // The last use of a candidate gives its offset, as upstream.
    let track = design_system.track_sources.get() && child.loc.src().is_some();
    let mut candidate_offsets: Vec<(&str, usize)> = Vec::new();
    let mut offset = 0;
    for (index, part) in parts.iter().enumerate() {
        if index % 2 == 0 {
            if part.starts_with("--") {
                dashed_idents.push(part);
            } else {
                normal_idents.push(part);
            }
            candidates.insert(part);
            if track {
                candidate_offsets.push((part, offset));
            }
        }
        if track {
            offset += utf16_len(part);
        }
    }

    if !dashed_idents.is_empty() && crate::switches::APPLY_CSS_MIXINS.on() {
        // Only dashed identifiers: this is a CSS mixin, and it stays.
        // https://drafts.csswg.org/css-mixins-1/#apply-rule
        if normal_idents.is_empty() {
            return Ok(None);
        }

        // Mixins and utilities together are not valid.
        let list = dashed_idents.join(" ");
        return Err(Error::new(format!(
            "You cannot use `@apply` with both mixins and utilities. Please move `@apply {list}` into a separate rule."
        )));
    }

    if !child.nodes.is_empty() && !normal_idents.is_empty() && crate::switches::APPLY_BODY_ERROR.on() {
        let list = normal_idents.join(" ");
        return Err(Error::new(format!("The rule `@apply {list}` must not have a body.")));
    }

    // The keys of a JavaScript object: the array indices first, in the
    // sequence of the numbers.
    // Before 4.1.6 the candidates were a list, in the sequence of the text.
    let as_keys = crate::switches::APPLY_CANDIDATES_AS_KEYS.on();
    let array_index = |key: &str| if as_keys { array_index(key) } else { None };
    let mut numeric: Vec<(u32, &str)> = candidates.iter().filter_map(|c| array_index(c).map(|n| (n, *c))).collect();
    numeric.sort();
    let candidates: Vec<String> = numeric
        .into_iter()
        .map(|(_, c)| c.to_string())
        .chain(candidates.iter().filter(|c| array_index(c).is_none()).map(|c| c.to_string()))
        .collect();

    let mut error: Option<Error> = None;
    design_system.variants.take_error();
    let compiled = compile_candidates(
        &candidates,
        design_system,
        &mut |candidate| {
            if error.is_none() {
                error = Some(invalid_candidate_error(candidate, design_system));
            }
        },
        false,
    );
    if let Some(error) = error {
        return Err(error);
    }
    // Upstream throws when a custom variant has an invalid `@variant`.
    if let Some(error) = design_system.variants.take_error() {
        return Err(error);
    }

    // For a style rule, use its children: the selector is not necessary.
    let mut new_nodes = Vec::new();
    for mut sorted in compiled {
        // The `@apply` rule made the node, also when the node comes from
        // an `@utility`: the location is the candidate in the params.
        if design_system.track_sources.get() {
            let candidate_src = child.loc.src().map(|src| {
                let candidate_offset = candidate_offsets
                    .iter()
                    .rev()
                    .find(|(part, _)| *part == sorted.candidate)
                    .map_or(0, |(_, offset)| *offset);
                let start = src.start + 7 + candidate_offset;
                SourceLocation {
                    source: std::rc::Rc::clone(&src.source),
                    start,
                    end: start + utf16_len(&sorted.candidate),
                }
            });
            set_src(std::slice::from_mut(&mut sorted.node), candidate_src.as_ref());
        }
        match sorted.node {
            AstNode::Rule(rule) => new_nodes.extend(rule.nodes),
            node => new_nodes.push(node),
        }
    }

    Ok(Some(new_nodes))
}

/// Set the `src` of each node of a tree. The nodes are new objects of
/// upstream (`cloneAstNode`): they have no shared identity.
fn set_src(nodes: &mut [AstNode], src: Option<&SourceLocation>) {
    for node in nodes {
        node.set_src(src.cloned());
        if let Some(loc) = node.loc_mut() {
            loc.clear_identity();
        }
        if let Some(children) = node.children_mut() {
            set_src(children, src);
        }
    }
}

/// The error for a candidate of `@apply` that gives no rule.
fn invalid_candidate_error(candidate: &str, design_system: &DesignSystem) -> Error {
    if !crate::switches::APPLY_ERROR_REASONS.on() {
        return Error::new(format!("Cannot apply unknown utility class: {candidate}"));
    }

    // With a prefix, the candidate must have the prefix.
    if let Some(prefix) = design_system.theme.prefix.as_deref().filter(|p| !p.is_empty()) {
        if !candidate.starts_with(prefix) {
            return Error::new(format!(
                "Cannot apply unprefixed utility class `{candidate}`. Did you mean `{prefix}:{candidate}`?"
            ));
        }
    }

    // The candidate is in `@source not inline(…)`.
    if design_system.invalid_candidates.borrow().contains(candidate) {
        return Error::new(format!(
            "Cannot apply utility class `{candidate}` because it has been explicitly disabled: https://tailwindcss.com/docs/detecting-classes-in-source-files#explicitly-excluding-classes"
        ));
    }

    // Find the variants that do not exist.
    let mut parts = segment(candidate, b':');
    if parts.len() > 1 {
        let utility = parts.pop().unwrap_or("");

        // The utility alone must compile. If not, a later error applies.
        if design_system.candidates_to_css(&[utility])[0].as_deref().is_some_and(|css| !css.is_empty()) {
            let checks: Vec<String> = parts.iter().map(|variant| format!("{variant}:[--tw-variant-check:1]")).collect();
            let check_refs: Vec<&str> = checks.iter().map(|c| c.as_str()).collect();
            let compiled_variants = design_system.candidates_to_css(&check_refs);
            let unknown_variants: Vec<String> = parts
                .iter()
                .zip(&compiled_variants)
                .filter(|(_, css)| css.is_none())
                .map(|(variant, _)| format!("`{variant}`"))
                .collect();
            if unknown_variants.len() == 1 {
                return Error::new(format!(
                    "Cannot apply utility class `{candidate}` because the {} variant does not exist.",
                    unknown_variants[0]
                ));
            } else if unknown_variants.len() > 1 {
                return Error::new(format!(
                    "Cannot apply utility class `{candidate}` because the {} variants do not exist.",
                    format_list(&unknown_variants)
                ));
            }
        }
    }

    // An empty theme: the import of the theme is probably not there.
    if design_system.theme.size() == 0 {
        return Error::new(format!(
            "Cannot apply unknown utility class `{candidate}`. Are you using CSS modules or similar and missing `@reference`? https://tailwindcss.com/docs/functions-and-directives#reference-directive"
        ));
    }

    Error::new(format!("Cannot apply unknown utility class `{candidate}`"))
}

/// `Intl.ListFormat('en', { style: 'long', type: 'conjunction' })`.
fn format_list(items: &[String]) -> String {
    match items {
        [] => String::new(),
        [one] => one.clone(),
        [first, second] => format!("{first} and {second}"),
        [rest @ .., last] => format!("{}, and {last}", rest.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast::decl;
    use crate::css_parser;
    use crate::design_system::build_design_system;
    use crate::theme::Theme;

    /// A design system with the static utilities `one` and `two`.
    fn design_system() -> DesignSystem {
        let mut design_system = build_design_system(Theme::new());
        design_system.utilities.r#static("one", |_, _| Some(vec![decl("--one", "1")]));
        design_system.utilities.r#static("two", |_, _| Some(vec![decl("--two", "2"), decl("--more", "3")]));
        design_system
    }

    fn run(css: &str) -> Result<(String, Features)> {
        let mut ast = css_parser::parse(css).unwrap();
        let features = substitute_at_apply(&mut ast, &design_system(), None)?;
        Ok((to_css(&ast), features))
    }

    #[test]
    fn replaces_the_rule_with_the_declarations() {
        // The sort of the compiler puts the utility with more declarations first.
        let (css, features) = run(".a { color: red; @apply two one; top: 0; .b & { @apply one; } }").unwrap();
        assert_eq!(features, FEATURES_AT_APPLY);
        assert_eq!(css, ".a {\n  color: red;\n  --two: 2;\n  --more: 3;\n  --one: 1;\n  top: 0;\n  .b & {\n    --one: 1;\n  }\n}\n");
    }

    #[test]
    fn no_apply_gives_no_feature() {
        let (css, features) = run(".a { color: red; }").unwrap();
        assert_eq!(features, crate::FEATURES_NONE);
        assert_eq!(css, ".a {\n  color: red;\n}\n");
    }

    #[test]
    fn works_at_the_top_level() {
        assert_eq!(run("@apply one;").unwrap().0, "--one: 1;\n");
    }

    #[test]
    fn a_mixin_stays() {
        assert_eq!(run(".a { @apply --mixin; }").unwrap().0, ".a {\n  @apply --mixin;\n}\n");
        assert_eq!(
            run(".a { @apply --mixin one; }").unwrap_err().message,
            "You cannot use `@apply` with both mixins and utilities. Please move `@apply --mixin` into a separate rule."
        );
    }

    #[test]
    fn errors() {
        assert_eq!(
            run(".a { @apply nothing; }").unwrap_err().message,
            "Cannot apply unknown utility class `nothing`. Are you using CSS modules or similar and missing `@reference`? https://tailwindcss.com/docs/functions-and-directives#reference-directive"
        );
        assert_eq!(run(".a { @apply one { top: 0; } }").unwrap_err().message, "The rule `@apply one` must not have a body.");
        assert_eq!(
            run("@keyframes a { from { @apply one; } }").unwrap_err().message,
            "You cannot use `@apply` inside `@keyframes`."
        );
    }

    #[test]
    fn reports_each_changed_utility_with_its_position() {
        let mut ast = css_parser::parse(
            "@utility plain { top: 0; }\n@utility first { @apply one; }\n.a { @apply two; }\n@utility second { &:hover { @apply two; } }",
        )
        .unwrap();
        let mut changed: Vec<(usize, String)> = Vec::new();
        substitute_at_apply(&mut ast, &design_system(), Some(&mut |ordinal, node| {
            changed.push((ordinal, to_css(&node.nodes)));
        }))
        .unwrap();
        assert_eq!(
            changed,
            vec![(1, "--one: 1;\n".to_string()), (2, "&:hover {\n  --two: 2;\n  --more: 3;\n}\n".to_string())]
        );
    }

    #[test]
    fn splits_as_javascript() {
        assert_eq!(split_whitespace("a  b\n c"), vec!["a", "b", "c"]);
        assert_eq!(split_whitespace(" a "), vec!["", "a", ""]);
        assert_eq!(split_whitespace(""), vec![""]);
        assert_eq!(split_with_separators("a \t b"), vec!["a", " \t ", "b"]);
    }

    #[test]
    fn formats_a_list_as_javascript() {
        let items = |n: usize| (0..n).map(|i| format!("`{i}`")).collect::<Vec<_>>();
        assert_eq!(format_list(&items(2)), "`0` and `1`");
        assert_eq!(format_list(&items(3)), "`0`, `1`, and `2`");
    }

    #[test]
    fn array_index_keys() {
        assert_eq!(array_index("0"), Some(0));
        assert_eq!(array_index("12"), Some(12));
        assert_eq!(array_index("4294967294"), Some(4294967294));
        assert_eq!(array_index("4294967295"), None);
        assert_eq!(array_index("01"), None);
        assert_eq!(array_index("1.5"), None);
        assert_eq!(array_index(""), None);
    }
}
