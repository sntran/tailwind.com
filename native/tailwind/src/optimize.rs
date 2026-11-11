//! The last pass on the tree before the print: `optimizeAst` and
//! `handleNesting` of `ast.ts` of Tailwind CSS.
//!
//! Upstream uses the identity of JavaScript objects. `optimize_ast` builds
//! the new tree in an arena: each node and each list of children has an id.
//! `handle_nesting` uses paths of child indices into its result.

use crate::switches;
use crate::ast::{self, at_rule, style_rule, AstNode, AtRule, ContextMap, ContextValue, Declaration, Loc};
use crate::source_maps::SourceLocation;
use crate::design_system::DesignSystem;
use crate::selector_parser::{self, SelectorAstNode};
use crate::theme::{self, Theme};
use crate::utils::js::{is_js_whitespace, js_to_lower_case, js_trim, js_trim_start};
use crate::utils::segment::segment;
use crate::utils::variables::extract_used_variables;
use crate::value_parser::{self, ValueAstNode};
use crate::walk::{walk, WalkAction};
use indexmap::{IndexMap, IndexSet};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// At-rules that can go to the top of the tree.
const HOISTABLE_AT_RULES: [&str; 8] =
    ["@container", "@layer", "@media", "@page", "@scope", "@starting-style", "@supports", "@view-transition"];

/// True for an at-rule that goes out of a rule. Before 4.3.4, `@scope`
/// was not one of them.
fn is_hoistable(name: &str) -> bool {
    HOISTABLE_AT_RULES.contains(&name) && (name != "@scope" || switches::SCOPE_HOISTING.on())
}

/// At-rules that have no effect when they have no nodes.
const DROPPABLE_IF_EMPTY_AT_RULES: [&str; 7] =
    ["@container", "@media", "@page", "@scope", "@starting-style", "@supports", "@view-transition"];

/// The standard pseudo-elements. A functional pseudo-element has no arguments here.
const STANDARD_PSEUDO_ELEMENTS: [&str; 32] = [
    "::after",
    "::backdrop",
    "::before",
    "::checkmark",
    "::column",
    "::cue",
    "::cue-region",
    "::details-content",
    "::file-selector-button",
    "::first-letter",
    "::first-line",
    "::grammar-error",
    "::highlight",
    "::marker",
    "::part",
    "::picker",
    "::picker-icon",
    "::placeholder",
    "::scroll-button",
    "::scroll-marker",
    "::scroll-marker-group",
    "::search-text",
    "::selection",
    "::slotted",
    "::spelling-error",
    "::target-text",
    "::view-transition",
    "::view-transition-group",
    "::view-transition-group-children",
    "::view-transition-image-pair",
    "::view-transition-new",
    "::view-transition-old",
];

/// The params of the `@supports` rule that holds the `@property` fallbacks.
fn at_property_supports() -> &'static str {
    if switches::AT_PROPERTY_SUPPORTS_BLOCK.on() {
        "((-webkit-hyphens: none) and (not (margin-trim: block))) or ((-moz-orient: inline) and (not (color:rgb(from red r g b))))"
    } else {
        "((-webkit-hyphens: none) and (not (margin-trim: inline))) or ((-moz-orient: inline) and (not (color:rgb(from red r g b))))"
    }
}

// ---------------------------------------------------------------------------
// optimize_ast
// ---------------------------------------------------------------------------

/// The id of a node of the arena.
type Id = usize;

/// A node of the arena. A node with children holds the ids of the children.
enum Item {
    /// A finished subtree: a declaration, a comment, or a made rule.
    Node(AstNode),
    Rule { selector: String, nodes: Vec<Id>, loc: Loc },
    AtRule { name: String, params: String, nodes: Vec<Id>, loc: Loc },
    Context { context: ContextMap, nodes: Vec<Id> },
    /// A list that is not a node: the root, the at-roots, a temporary list.
    List(Vec<Id>),
}

struct Entry {
    item: Item,
    /// The node whose list holds this node, while the node is in that list.
    parent: Option<Id>,
}

#[derive(Default)]
struct Arena {
    entries: Vec<Entry>,
}

impl Arena {
    fn add(&mut self, item: Item) -> Id {
        self.entries.push(Entry { item, parent: None });
        self.entries.len() - 1
    }

    fn children(&self, id: Id) -> &Vec<Id> {
        match &self.entries[id].item {
            Item::Rule { nodes, .. } | Item::AtRule { nodes, .. } | Item::Context { nodes, .. } | Item::List(nodes) => nodes,
            Item::Node(_) => unreachable!("a finished subtree has no list of ids"),
        }
    }

    fn children_mut(&mut self, id: Id) -> &mut Vec<Id> {
        match &mut self.entries[id].item {
            Item::Rule { nodes, .. } | Item::AtRule { nodes, .. } | Item::Context { nodes, .. } | Item::List(nodes) => nodes,
            Item::Node(_) => unreachable!("a finished subtree has no list of ids"),
        }
    }

    /// Put a node at the end of the list of `parent`.
    fn attach(&mut self, parent: Id, id: Id) {
        self.children_mut(parent).push(id);
        self.entries[id].parent = Some(parent);
    }

    /// Make the final node. The arena does not keep the node.
    fn build(&mut self, id: Id) -> AstNode {
        let item = std::mem::replace(&mut self.entries[id].item, Item::List(Vec::new()));
        match item {
            Item::Node(node) => node,
            Item::Rule { selector, nodes, loc } => {
                AstNode::Rule(ast::StyleRule { selector, nodes: self.build_all(nodes), loc })
            }
            Item::AtRule { name, params, nodes, loc } => {
                AstNode::AtRule(AtRule { name, params, nodes: self.build_all(nodes), loc })
            }
            Item::Context { context, nodes } => ast::context(context, self.build_all(nodes)),
            Item::List(_) => unreachable!("a list is not a node"),
        }
    }

    fn build_all(&mut self, ids: Vec<Id>) -> Vec<AstNode> {
        ids.into_iter().map(|id| self.build(id)).collect()
    }
}

/// The context of a node that `transform` uses.
#[derive(Clone, Copy, Default)]
struct Ctx {
    theme: bool,
    keyframes: bool,
    supports_color_mix: bool,
}

impl Ctx {
    fn merge(mut self, map: &ContextMap) -> Self {
        if let Some(value) = map.get("theme") {
            self.theme = value.truthy();
        }
        if let Some(value) = map.get("keyframes") {
            self.keyframes = value.truthy();
        }
        if let Some(value) = map.get("supportsColorMix") {
            self.supports_color_mix = value.truthy();
        }
        self
    }
}

struct Optimizer<'a> {
    arena: Arena,
    design_system: &'a DesignSystem,
    polyfills: crate::Polyfills,
    /// The list of the nodes that go to the end of the tree.
    at_roots: Id,
    seen_at_properties: HashSet<String>,
    /// The theme variables of each list, in the sequence of the tree.
    css_theme_variables: IndexMap<Id, Vec<Id>>,
    /// The declarations with `color-mix(…)` of each list.
    color_mix_declarations: IndexMap<Id, Vec<Id>>,
    /// The `@keyframes` rules of the theme.
    keyframes: Vec<(Id, String)>,
    used_keyframe_names: HashSet<String>,
    property_fallbacks_root: Vec<AstNode>,
    property_fallbacks_universal: Vec<AstNode>,
    variable_dependencies: IndexMap<String, IndexSet<String>>,
}

impl Optimizer<'_> {
    fn transform(&mut self, node: &AstNode, parent: Id, mut context: Ctx, depth: usize) {
        match node {
            AstNode::Declaration(decl) => {
                if decl.property == "--tw-sort" {
                    return;
                }
                let Some(value) = &decl.value else { return };
                let is_theme_variable = context.theme && decl.property.starts_with("--");

                // A theme variable with the value `initial` gives no output.
                if is_theme_variable && value == "initial" {
                    return;
                }

                if value.contains("var(") {
                    // A variable that uses a variable is a dependency, not a use.
                    if is_theme_variable {
                        for variable in extract_used_variables(value) {
                            self.variable_dependencies.entry(variable).or_default().insert(decl.property.clone());
                        }
                    } else {
                        self.design_system.track_used_variables(value);
                    }
                }

                if decl.property == "animation" {
                    for name in extract_keyframe_names(value) {
                        self.used_keyframe_names.insert(name.to_string());
                    }
                }

                let needs_color_mix = self.polyfills & crate::POLYFILLS_COLOR_MIX != 0
                    && !context.supports_color_mix
                    && !context.keyframes
                    && value.contains("color-mix(");

                let id = self.arena.add(Item::Node(node.clone()));
                self.arena.attach(parent, id);
                if is_theme_variable && !context.keyframes {
                    self.css_theme_variables.entry(parent).or_default().push(id);
                }
                if needs_color_mix {
                    self.color_mix_declarations.entry(parent).or_default().push(id);
                }
            }

            AstNode::Rule(rule) => {
                let copy = self.arena.add(Item::Rule { selector: rule.selector.clone(), nodes: Vec::new(), loc: rule.loc.clone() });
                for child in &rule.nodes {
                    self.transform(child, copy, context, depth + 1);
                }
                if switches::NESTING_PASS.on() {
                    if !self.arena.children(copy).is_empty() {
                        self.arena.attach(parent, copy);
                    }
                } else {
                    self.finish_rule_without_nesting_pass(parent, copy);
                }
            }

            AstNode::AtRule(rule) if rule.name == "@property" && depth == 0 => {
                if self.seen_at_properties.contains(&rule.params) {
                    return;
                }

                if self.polyfills & crate::POLYFILLS_AT_PROPERTY != 0 {
                    let mut initial_value: Option<String> = None;
                    let mut inherits = false;
                    for prop in &rule.nodes {
                        let AstNode::Declaration(prop) = prop else { continue };
                        if prop.property == "initial-value" {
                            initial_value = prop.value.clone();
                        } else if prop.property == "inherits" {
                            inherits = prop.value.as_deref() == Some("true");
                        }
                    }
                    let fallback = ast::decl(rule.params.clone(), initial_value.unwrap_or_else(|| "initial".to_string()))
                        .with_src(rule.loc.src());
                    if inherits {
                        self.property_fallbacks_root.push(fallback);
                    } else {
                        self.property_fallbacks_universal.push(fallback);
                    }
                }

                self.seen_at_properties.insert(rule.params.clone());

                let copy = self.copy_at_rule(rule);
                for child in &rule.nodes {
                    self.transform(child, copy, context, depth + 1);
                }
                self.arena.attach(parent, copy);
            }

            AstNode::AtRule(rule) => {
                if rule.name == "@keyframes" {
                    context.keyframes = true;
                } else if rule.name == "@supports"
                    && rule.params.contains("color-mix(")
                    && crate::switches::SUPPORTS_COLOR_MIX_NO_FALLBACK.on()
                {
                    // Before 4.2.0, a declaration in this rule got a
                    // fallback as each other.
                    context.supports_color_mix = true;
                }

                let copy = self.copy_at_rule(rule);
                for child in &rule.nodes {
                    self.transform(child, copy, context, depth + 1);
                }

                if rule.name == "@keyframes" && context.theme {
                    self.keyframes.push((copy, rule.params.clone()));
                }

                if !self.arena.children(copy).is_empty()
                    || matches!(rule.name.as_str(), "@layer" | "@charset" | "@custom-media" | "@namespace" | "@import")
                    || (rule.name == "@apply" && switches::APPLY_CSS_MIXINS.on())
                {
                    self.arena.attach(parent, copy);
                }
            }

            AstNode::AtRoot(root) => {
                for child in &root.nodes {
                    // Upstream makes a new list for each child and copies it
                    // to the at-roots. A change of this list later has no
                    // effect on the tree.
                    let new_parent = self.arena.add(Item::List(Vec::new()));
                    self.transform(child, new_parent, context, 0);
                    let made = self.arena.children(new_parent).clone();
                    self.arena.children_mut(self.at_roots).extend(made);
                }
            }

            AstNode::Context(node) => {
                if node.context.get("reference").is_some_and(ContextValue::truthy) {
                    return;
                }
                let merged = context.merge(&node.context);
                // Before 4.3.4, no pass used the source of a subtree.
                if node.context.get("source").is_some_and(ContextValue::truthy) && switches::SCOPE_HOISTING.on() {
                    let copy = self.arena.add(Item::Context { context: node.context.clone(), nodes: Vec::new() });
                    for child in &node.nodes {
                        self.transform(child, copy, merged, depth);
                    }
                    if !self.arena.children(copy).is_empty() {
                        self.arena.attach(parent, copy);
                    }
                } else {
                    for child in &node.nodes {
                        self.transform(child, parent, merged, depth);
                    }
                }
            }

            AstNode::Comment(_) => {
                let id = self.arena.add(Item::Node(node.clone()));
                self.arena.attach(parent, id);
            }
        }
    }

    /// The end of a style rule as before the nesting pass
    /// (`switches::NESTING_PASS`): remove the declarations that are equal
    /// to a later declaration of the rule, and replace a rule with the
    /// selector `&` by its children.
    ///
    /// Upstream made a new list for these two changes, but the lists of
    /// the theme variables and of the `color-mix(…)` declarations kept the
    /// first list. So the later steps (the removal of unused theme
    /// variables, the fallbacks of `color-mix(…)`) changed a list that was
    /// not in the tree. Here `copy` stays as that first list, with no
    /// parent, and the tree gets a new rule.
    fn finish_rule_without_nesting_pass(&mut self, parent: Id, copy: Id) {
        let children = self.arena.children(copy).clone();
        if children.is_empty() {
            return;
        }

        // Keep the last declaration of each group of equal declarations.
        // The key is one text, as upstream.
        let key = |decl: &Declaration| format!("{}:{}:{}", decl.property, decl.value.as_deref().unwrap_or("undefined"), decl.important);
        let mut last: HashMap<String, Id> = HashMap::new();
        let mut declarations = 0;
        for id in &children {
            if let Item::Node(AstNode::Declaration(decl)) = &self.arena.entries[*id].item {
                declarations += 1;
                last.insert(key(decl), *id);
            }
        }
        let has_duplicates = last.len() < declarations;
        let is_nesting_selector =
            matches!(&self.arena.entries[copy].item, Item::Rule { selector, .. } if selector == "&");

        if !has_duplicates && !is_nesting_selector {
            self.arena.attach(parent, copy);
            return;
        }

        let kept: Vec<Id> = if has_duplicates {
            children
                .into_iter()
                .filter(|id| match &self.arena.entries[*id].item {
                    Item::Node(AstNode::Declaration(decl)) => last.get(&key(decl)) == Some(id),
                    _ => true,
                })
                .collect()
        } else {
            children
        };

        if is_nesting_selector {
            for id in kept {
                self.arena.attach(parent, id);
            }
        } else {
            let Item::Rule { selector, loc, .. } = &self.arena.entries[copy].item else { unreachable!("the copy is a rule") };
            let rule = Item::Rule { selector: selector.clone(), nodes: Vec::new(), loc: loc.clone() };
            let rule = self.arena.add(rule);
            for id in kept {
                self.arena.attach(rule, id);
            }
            self.arena.attach(parent, rule);
        }
    }

    fn copy_at_rule(&mut self, rule: &AtRule) -> Id {
        self.arena.add(Item::AtRule {
            name: rule.name.clone(),
            params: rule.params.clone(),
            nodes: Vec::new(),
            loc: rule.loc.clone(),
        })
    }

    fn declaration(&self, id: Id) -> &Declaration {
        match &self.arena.entries[id].item {
            Item::Node(AstNode::Declaration(decl)) => decl,
            _ => unreachable!("the id is the id of a declaration"),
        }
    }

    /// Remove the theme variables that have no use.
    fn remove_unused_theme_variables(&mut self, root: Id) {
        let animate_prefix = self.design_system.theme.prefix_key("--animate-");
        let lists = std::mem::take(&mut self.css_theme_variables);

        for (parent, declarations) in lists {
            for declaration in declarations {
                let decl = self.declaration(declaration);
                let used = is_variable_used(
                    &decl.property,
                    &self.design_system.theme,
                    &self.variable_dependencies,
                    &mut HashSet::new(),
                );
                if used {
                    if decl.property.starts_with(&animate_prefix) {
                        let names: Vec<String> =
                            extract_keyframe_names(decl.value.as_deref().unwrap_or("")).into_iter().map(String::from).collect();
                        self.used_keyframe_names.extend(names);
                    }
                    continue;
                }

                // JavaScript `splice(-1, 1)` removes the last node.
                let list = self.arena.children_mut(parent);
                match list.iter().position(|id| *id == declaration) {
                    Some(idx) => {
                        list.remove(idx);
                    }
                    None => {
                        list.pop();
                    }
                }

                if !self.arena.children(parent).is_empty() {
                    continue;
                }

                // The list is empty. When the list is the list of a style
                // rule in the tree, remove the rule, and each at-rule above
                // it. Upstream does not examine the other children of these
                // at-rules.
                if matches!(self.arena.entries[parent].item, Item::Rule { .. }) && self.is_in_tree(parent, root) {
                    let mut node = parent;
                    while let Some(remove_from) = self.arena.entries[node].parent {
                        let is_root_or_at_rule =
                            remove_from == root || matches!(self.arena.entries[remove_from].item, Item::AtRule { .. });
                        if !is_root_or_at_rule {
                            break;
                        }
                        let list = self.arena.children_mut(remove_from);
                        let Some(idx) = list.iter().position(|id| *id == node) else { break };
                        list.remove(idx);
                        self.arena.entries[node].parent = None;
                        node = remove_from;
                    }
                }
                break;
            }
        }
    }

    /// True when the chain of parents of the node ends at the root list.
    fn is_in_tree(&self, mut node: Id, root: Id) -> bool {
        while let Some(parent) = self.arena.entries[node].parent {
            if parent == root {
                return true;
            }
            node = parent;
        }
        false
    }

    /// Add a fallback before each declaration that uses `color-mix(…)`.
    fn add_color_mix_fallbacks(&mut self) {
        let lists = std::mem::take(&mut self.color_mix_declarations);
        for (parent, declarations) in lists {
            for declaration in declarations {
                // A removed declaration needs no fallback.
                let Some(idx) = self.arena.children(parent).iter().position(|id| *id == declaration) else { continue };
                let decl = self.declaration(declaration);
                let Some(value) = &decl.value else { continue };
                let Some(fallback_value) = color_mix_fallback(value, &self.design_system.theme) else { continue };

                let fallback = AstNode::Declaration(Declaration {
                    property: decl.property.clone(),
                    value: Some(fallback_value),
                    important: decl.important,
                    loc: decl.loc.clone(),
                });
                let query_loc = Loc::from_src(decl.loc.src().cloned());
                // The fallback is a new object of upstream.
                let mut fallback = fallback;
                if let Some(loc) = fallback.loc_mut() {
                    loc.clear_identity();
                }
                let fallback = self.arena.add(Item::Node(fallback));
                let query = self.arena.add(Item::AtRule {
                    name: "@supports".to_string(),
                    params: "(color: color-mix(in lab, red, red))".to_string(),
                    nodes: vec![declaration],
                    loc: query_loc,
                });
                self.arena.entries[declaration].parent = Some(query);
                self.arena.entries[fallback].parent = Some(parent);
                self.arena.entries[query].parent = Some(parent);
                self.arena.children_mut(parent).splice(idx..=idx, [fallback, query]);
            }
        }
    }
}

/// Make the tree ready for the printer.
pub fn optimize_ast(ast: &[AstNode], design_system: &DesignSystem, polyfills: crate::Polyfills) -> Vec<AstNode> {
    let mut arena = Arena::default();
    let root = arena.add(Item::List(Vec::new()));
    let at_roots = arena.add(Item::List(Vec::new()));
    let mut optimizer = Optimizer {
        arena,
        design_system,
        polyfills,
        at_roots,
        seen_at_properties: HashSet::new(),
        css_theme_variables: IndexMap::new(),
        color_mix_declarations: IndexMap::new(),
        keyframes: Vec::new(),
        used_keyframe_names: HashSet::new(),
        property_fallbacks_root: Vec::new(),
        property_fallbacks_universal: Vec::new(),
        variable_dependencies: IndexMap::new(),
    };

    for node in ast {
        optimizer.transform(node, root, Ctx::default(), 0);
    }

    optimizer.remove_unused_theme_variables(root);

    // Remove the keyframes that have no use. A rule that is not in the
    // at-roots has the index -1 upstream: `splice(-1, 1)` removes the last
    // node of the at-roots.
    for (keyframe, params) in std::mem::take(&mut optimizer.keyframes) {
        if !optimizer.used_keyframe_names.contains(&params) {
            let list = optimizer.arena.children_mut(at_roots);
            match list.iter().position(|id| *id == keyframe) {
                Some(idx) => {
                    list.remove(idx);
                }
                None => {
                    list.pop();
                }
            }
        }
    }

    // Upstream makes a new array here (`concat`). A later change of the old
    // root list has no effect on the result.
    let mut new_ast: Vec<Id> = optimizer.arena.children(root).clone();
    new_ast.extend(optimizer.arena.children(at_roots).iter().copied());

    if polyfills & crate::POLYFILLS_COLOR_MIX != 0 {
        optimizer.add_color_mix_fallbacks();
    }

    let mut arena = optimizer.arena;
    let mut new_ast = arena.build_all(new_ast);

    if polyfills & crate::POLYFILLS_AT_PROPERTY != 0 {
        let mut fallback_ast = Vec::new();
        if !optimizer.property_fallbacks_root.is_empty() {
            let src = optimizer.property_fallbacks_root[0].src().cloned();
            fallback_ast.push(style_rule(":root, :host", optimizer.property_fallbacks_root).with_src(src.as_ref()));
        }
        if !optimizer.property_fallbacks_universal.is_empty() {
            let src = optimizer.property_fallbacks_universal[0].src().cloned();
            fallback_ast.push(
                style_rule("*, ::before, ::after, ::backdrop", optimizer.property_fallbacks_universal)
                    .with_src(src.as_ref()),
            );
        }

        if !fallback_ast.is_empty() {
            // The empty `@layer properties;` goes after the comments, the
            // `@charset` and the `@import` rules at the start.
            let first_valid_node_index = new_ast.iter().position(|node| match node {
                AstNode::Comment(_) => false,
                AstNode::AtRule(rule) => rule.name != "@charset" && rule.name != "@import",
                _ => true,
            });
            let src: Option<SourceLocation> = fallback_ast[0].src().cloned();
            let src = src.as_ref();
            new_ast.insert(
                first_valid_node_index.unwrap_or(new_ast.len()),
                at_rule("@layer", "properties", Vec::new()).with_src(src),
            );
            new_ast.push(
                at_rule(
                    "@layer",
                    "properties",
                    vec![at_rule("@supports", at_property_supports(), fallback_ast).with_src(src)],
                )
                .with_src(src),
            );
        }
    }

    if switches::NESTING_PASS.on() {
        handle_nesting(new_ast)
    } else {
        new_ast
    }
}

/// True when the variable is in use, or a variable that uses it is in use.
fn is_variable_used(
    variable: &str,
    theme: &Theme,
    variable_dependencies: &IndexMap<String, IndexSet<String>>,
    already_seen_variables: &mut HashSet<String>,
) -> bool {
    // A second visit of a variable stops the recursion.
    if !already_seen_variables.insert(variable.to_string()) {
        return true;
    }

    if theme.get_options(variable) & (theme::STATIC | theme::USED) != 0 {
        return true;
    }
    if let Some(dependencies) = variable_dependencies.get(variable) {
        for dependency in dependencies {
            if is_variable_used(dependency, theme, variable_dependencies, already_seen_variables) {
                return true;
            }
        }
    }
    false
}

/// JavaScript `value.split(/[\s,]+/)`.
fn extract_keyframe_names(value: &str) -> Vec<&str> {
    let is_separator = |c: char| c == ',' || is_js_whitespace(c);
    let mut names = Vec::new();
    let mut start = 0;
    let mut chars = value.char_indices().peekable();
    while let Some((idx, c)) = chars.next() {
        if !is_separator(c) {
            continue;
        }
        names.push(&value[start..idx]);
        let mut end = idx + c.len_utf8();
        while let Some(&(next_idx, next)) = chars.peek() {
            if !is_separator(next) {
                break;
            }
            end = next_idx + next.len_utf8();
            chars.next();
        }
        start = end;
    }
    names.push(&value[start..]);
    names
}

/// JavaScript `args.replace(/\/\*[\s\S]*?\*\//g, ' ')`.
fn replace_comments(args: &str) -> String {
    let mut out = String::with_capacity(args.len());
    let mut rest = args;
    while let Some(start) = rest.find("/*") {
        let Some(end) = rest[start + 2..].find("*/") else { break };
        out.push_str(&rest[..start]);
        out.push(' ');
        rest = &rest[start + 2 + end + 2..];
    }
    out.push_str(rest);
    out
}

/// JavaScript `/^in\s/i.test(text)`.
fn starts_with_in(text: &str) -> bool {
    let mut chars = text.chars();
    matches!(chars.next(), Some('i' | 'I')) && matches!(chars.next(), Some('n' | 'N')) && chars.next().is_some_and(is_js_whitespace)
}

/// The value with the theme variables of each `color-mix(…)` replaced by
/// their values. The result is `None` when the value needs no fallback.
fn color_mix_fallback(value: &str, theme: &Theme) -> Option<String> {
    let mut ast = value_parser::parse(value);
    let mut requires_polyfill = false;

    walk(&mut ast, |node, _| {
        let ValueAstNode::Function(function) = node else { return WalkAction::Continue };
        if function.value != "color-mix" {
            return WalkAction::Continue;
        }

        // The interpolation method is optional. Without it, there is no
        // fallback. A comment can be before `in`.
        if switches::COLOR_MIX_NEEDS_METHOD.on() {
            let mut args = value_parser::to_css(&function.nodes);
            if args.contains("/*") {
                args = replace_comments(&args);
                function.nodes = value_parser::parse(&args);
            }
            if !starts_with_in(js_trim_start(&args)) {
                return WalkAction::Continue;
            }
        }

        let mut contains_unresolvable_vars = false;
        let mut contains_currentcolor = false;
        walk(&mut function.nodes, |node, _| {
            if let ValueAstNode::Word(word) = node {
                if js_to_lower_case(&word.value) == "currentcolor" {
                    contains_currentcolor = true;
                    requires_polyfill = true;
                    return WalkAction::Continue;
                }
            }

            let mut resolved: Option<ValueAstNode> = None;
            let mut inlined_color;
            let mut seen_variables: HashSet<String> = HashSet::new();
            loop {
                let var_node = resolved.as_ref().unwrap_or(node);
                let ValueAstNode::Function(var) = var_node else { return WalkAction::Continue };
                if var.value != "var" {
                    return WalkAction::Continue;
                }
                let Some(ValueAstNode::Word(first_child)) = var.nodes.first() else { return WalkAction::Continue };
                let variable_name = first_child.value.clone();

                if !seen_variables.insert(variable_name.clone()) {
                    contains_unresolvable_vars = true;
                    return WalkAction::Continue;
                }

                requires_polyfill = true;

                inlined_color = match theme.resolve_value(None, &[&variable_name]) {
                    Some(color) if !color.is_empty() => color,
                    _ => {
                        contains_unresolvable_vars = true;
                        return WalkAction::Continue;
                    }
                };
                if js_to_lower_case(&inlined_color) == "currentcolor" {
                    contains_currentcolor = true;
                    return WalkAction::Continue;
                }

                if !inlined_color.starts_with("var(") {
                    break;
                }
                match value_parser::parse(&inlined_color).into_iter().next() {
                    Some(next) => resolved = Some(next),
                    None => break,
                }
            }

            WalkAction::Replace(vec![value_parser::word(inlined_color)])
        });

        if contains_unresolvable_vars || contains_currentcolor {
            let separator_index = function
                .nodes
                .iter()
                .position(|node| matches!(node, ValueAstNode::Separator(s) if js_trim(&s.value).contains(',')));
            let Some(separator_index) = separator_index else { return WalkAction::Continue };
            let Some(first_color_value) = function.nodes.get(separator_index + 1) else { return WalkAction::Continue };
            return WalkAction::Replace(vec![first_color_value.clone()]);
        } else if requires_polyfill {
            // The fallback uses `srgb`: old browsers do not know `oklab(…)`.
            // The color space is the second node after the first node that
            // is not a separator. With no such node, upstream reads index 1.
            // Before 4.3.5: always the third node.
            let index = if switches::COLOR_MIX_NEEDS_METHOD.on() {
                function.nodes.iter().position(|node| !node.is_separator()).map_or(1, |idx| idx + 2)
            } else {
                2
            };
            if let Some(ValueAstNode::Word(colorspace)) = function.nodes.get_mut(index) {
                if matches!(colorspace.value.as_str(), "oklab" | "oklch" | "lab" | "lch") {
                    colorspace.value = "srgb".to_string();
                }
            }
        }
        WalkAction::Continue
    });

    requires_polyfill.then(|| value_parser::to_css(&ast))
}

// ---------------------------------------------------------------------------
// handle_nesting
// ---------------------------------------------------------------------------

/// A selector of the stack. `dead`: the selector can never match.
struct SelectorEntry {
    selector: String,
    dead: bool,
    /// The locations of the rule, for the rule that `emit` makes.
    loc: Loc,
}

/// The list that `emit` adds to.
enum Current {
    /// The next `emit` starts a new list.
    None,
    /// The children of the node at this path of child indices in the result.
    Path(Vec<usize>),
    /// A list that is not in the result. Upstream loses the nodes that go
    /// there.
    Detached,
}

/// The facts about a parent selector that the substitution of `&` uses.
struct ParentFacts {
    /// The parent is a selector list.
    is_list: bool,
    first_is_complex: bool,
    first_is_universal_or_type: bool,
}

/// The kind of the parent of a selector node.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ParentKind {
    Complex,
    Compound,
    Function,
    List,
}

/// The parent of a level of the selector tree.
#[derive(Clone, Copy)]
struct ParentInfo {
    kind: ParentKind,
    /// The grandparent is a complex selector and the parent is not its first node.
    not_first_in_complex: bool,
}

struct Nesting {
    selector_stack: Vec<SelectorEntry>,
    at_rule_stack: Vec<(String, String, Loc)>,
    current: Current,
    seen_declaration_properties: HashSet<String>,
    /// The paths of the lists that can have the same declaration two times.
    dedupe_declarations_in_nodes: HashSet<Vec<usize>>,
    result: Vec<AstNode>,
    source_stack: Vec<ContextValue>,
    parse_selector_cache: HashMap<String, Rc<Vec<SelectorAstNode>>>,
}

/// The list of children at a path of child indices.
fn nodes_at<'a>(mut nodes: &'a mut Vec<AstNode>, path: &[usize]) -> &'a mut Vec<AstNode> {
    for idx in path {
        nodes = nodes[*idx].children_mut().expect("a node of a path has children");
    }
    nodes
}

/// True when the subtree has an `@scope` rule.
fn contains_at_scope(nodes: &[AstNode]) -> bool {
    if !switches::SCOPE_HOISTING.on() {
        return false;
    }
    contains_at_scope_rule(nodes)
}

fn contains_at_scope_rule(nodes: &[AstNode]) -> bool {
    nodes.iter().any(|node| match node.children() {
        None => false,
        Some(children) => node.is_at_rule("@scope") || contains_at_scope_rule(children),
    })
}

/// Put each group of declarations and comments of an `@scope` rule in an
/// `&` rule. This also applies to the context nodes that are direct children.
fn wrap_bare_declarations(nodes: Vec<AstNode>) -> Vec<AstNode> {
    let mut new_nodes: Vec<AstNode> = Vec::with_capacity(nodes.len());
    let mut in_group = false;
    for mut child in nodes {
        match &mut child {
            AstNode::Declaration(_) | AstNode::Comment(_) => {
                if !in_group {
                    in_group = true;
                    new_nodes.push(style_rule("&", Vec::new()));
                }
                new_nodes.last_mut().and_then(AstNode::children_mut).expect("the group is a rule").push(child);
            }
            other => {
                if let AstNode::Context(context) = other {
                    context.nodes = wrap_bare_declarations(std::mem::take(&mut context.nodes));
                }
                in_group = false;
                new_nodes.push(child);
            }
        }
    }
    new_nodes
}

/// Replace each `&` of a selector tree.
fn replace_nesting_selectors(nodes: &mut [SelectorAstNode], replacement: &str, changed: &mut bool) {
    for node in nodes {
        match node {
            SelectorAstNode::Selector(selector) if selector.value == "&" => {
                *changed = true;
                selector.value = replacement.to_string();
            }
            SelectorAstNode::Complex(n) => replace_nesting_selectors(&mut n.nodes, replacement, changed),
            SelectorAstNode::Compound(n) => replace_nesting_selectors(&mut n.nodes, replacement, changed),
            SelectorAstNode::Function(n) => replace_nesting_selectors(&mut n.nodes, replacement, changed),
            SelectorAstNode::List(n) => replace_nesting_selectors(&mut n.nodes, replacement, changed),
            _ => {}
        }
    }
}

/// The selector with `:is(…)` or without, for one `&`.
fn nesting_replacement(
    siblings: &[SelectorAstNode],
    index: usize,
    parent: Option<ParentInfo>,
    parent_selector: &str,
    facts: &ParentFacts,
) -> String {
    let wrapped = || format!(":is({parent_selector})");

    let Some(parent) = parent else { return wrapped() };

    // A selector list always needs `:is(…)`.
    if facts.is_list {
        return wrapped();
    }

    match parent.kind {
        // `& [after]`, `[before] &` and `[before] & [after]`
        ParentKind::Complex => {
            if index == 0 || !facts.first_is_complex {
                parent_selector.to_string()
            } else {
                wrapped()
            }
        }
        // `&[after]`, `[before]&` and `[before]&[after]`
        ParentKind::Compound => {
            if facts.first_is_complex && parent.not_first_in_complex {
                return wrapped();
            }
            // `&*` and `&div` are not valid CSS and stay not valid.
            let type_follows = siblings[index + 1..]
                .iter()
                .any(|sibling| selector_parser::is_universal_selector(sibling) || selector_parser::is_type_selector(sibling));
            if type_follows {
                return wrapped();
            }
            if index == 0 || !(facts.first_is_complex || facts.first_is_universal_or_type) {
                parent_selector.to_string()
            } else {
                wrapped()
            }
        }
        // `:not(&)`
        ParentKind::Function => parent_selector.to_string(),
        ParentKind::List => wrapped(),
    }
}

/// Replace each `&` with the parent selector, and remove a `*` at the start
/// of a compound selector.
fn substitute_level(
    nodes: &mut Vec<SelectorAstNode>,
    parent: Option<ParentInfo>,
    parent_selector: &str,
    facts: &ParentFacts,
    changed: &mut bool,
) {
    let mut index = 0;
    while index < nodes.len() {
        if matches!(&nodes[index], SelectorAstNode::Selector(s) if s.value == "&") {
            *changed = true;
            let replacement = nesting_replacement(nodes, index, parent, parent_selector, facts);
            if let SelectorAstNode::Selector(selector) = &mut nodes[index] {
                selector.value = replacement;
            }
        } else {
            let kind = match &nodes[index] {
                SelectorAstNode::Complex(_) => Some(ParentKind::Complex),
                SelectorAstNode::Compound(_) => Some(ParentKind::Compound),
                SelectorAstNode::Function(_) => Some(ParentKind::Function),
                SelectorAstNode::List(_) => Some(ParentKind::List),
                _ => None,
            };
            if let Some(kind) = kind {
                let info = ParentInfo {
                    kind,
                    not_first_in_complex: parent.is_some_and(|p| p.kind == ParentKind::Complex) && index != 0,
                };
                let children = match &mut nodes[index] {
                    SelectorAstNode::Complex(n) => &mut n.nodes,
                    SelectorAstNode::Compound(n) => &mut n.nodes,
                    SelectorAstNode::Function(n) => &mut n.nodes,
                    SelectorAstNode::List(n) => &mut n.nodes,
                    _ => unreachable!(),
                };
                substitute_level(children, Some(info), parent_selector, facts, changed);
            }
        }

        // `*:hover` is `:hover`. `*|div` keeps the `*`: it is a namespace.
        if index == 0
            && nodes.len() > 1
            && parent.is_some_and(|p| p.kind == ParentKind::Compound)
            && selector_parser::is_universal_selector(&nodes[0])
        {
            let is_namespace = matches!(&nodes[1], SelectorAstNode::Selector(s) if s.value.as_bytes().first() == Some(&b'|'));
            if !is_namespace {
                nodes.remove(0);
                continue;
            }
        }
        index += 1;
    }
}

/// True when no selector of the tree can match an element.
fn can_never_match(ast: &[SelectorAstNode]) -> bool {
    // Before 4.3.4, the pass had no such check.
    if !switches::DEAD_PSEUDO_ELEMENT_RULES.on() {
        return false;
    }
    can_never_match_selector(ast)
}

fn can_never_match_selector(ast: &[SelectorAstNode]) -> bool {
    !ast.is_empty() && ast.iter().all(is_dead_selector)
}

fn is_standard_pseudo_element(node: &SelectorAstNode) -> bool {
    let value = match node {
        SelectorAstNode::Selector(n) => &n.value,
        SelectorAstNode::Function(n) => &n.value,
        _ => return false,
    };
    if !selector_parser::is_pseudo_element(node) {
        return false;
    }
    let value = js_to_lower_case(value);
    // An old pseudo-element with one colon, or a pseudo-element with a
    // vendor prefix.
    if !value.starts_with("::") || value.starts_with("::-") {
        return true;
    }
    STANDARD_PSEUDO_ELEMENTS.contains(&value.as_ref())
}

/// A combinator after a pseudo-element makes a selector that never matches.
fn is_dead_selector(node: &SelectorAstNode) -> bool {
    match node {
        SelectorAstNode::List(list) => list.nodes.iter().all(is_dead_selector),
        SelectorAstNode::Complex(complex) => {
            for (i, child) in complex.nodes.iter().enumerate() {
                if matches!(child, SelectorAstNode::Combinator(_)) {
                    if i == 0 {
                        continue;
                    }
                    let previous = &complex.nodes[i - 1];
                    if is_standard_pseudo_element(previous)
                        || matches!(previous, SelectorAstNode::Compound(c) if c.nodes.iter().any(is_standard_pseudo_element))
                    {
                        return true;
                    }
                } else if is_dead_selector(child) {
                    return true;
                }
            }
            false
        }
        SelectorAstNode::Compound(compound) => compound.nodes.iter().any(is_dead_selector),
        SelectorAstNode::Function(function) => {
            (function.value == ":is" || function.value == ":where") && can_never_match_selector(&function.nodes)
        }
        _ => false,
    }
}

impl Nesting {
    fn parse_selector(&mut self, selector: &str) -> Rc<Vec<SelectorAstNode>> {
        if let Some(found) = self.parse_selector_cache.get(selector) {
            return found.clone();
        }
        let parsed = Rc::new(selector_parser::parse(selector));
        self.parse_selector_cache.insert(selector.to_string(), parsed.clone());
        parsed
    }

    /// Replace `&` with the parent selector. With no `&`, the parent
    /// selector goes before the selector.
    fn substitute_nesting_selector(&mut self, selector: &str, parent_selector: &str) -> String {
        let parent_ast = self.parse_selector(parent_selector);
        // Upstream throws a `TypeError` for a parent selector with no node
        // in some of these conditions. Here such a parent is a simple one.
        let first = parent_ast.first();
        let facts = ParentFacts {
            is_list: parent_ast.len() == 1 && matches!(first, Some(SelectorAstNode::List(_))),
            first_is_complex: matches!(first, Some(SelectorAstNode::Complex(_))),
            first_is_universal_or_type: first
                .is_some_and(|node| selector_parser::is_universal_selector(node) || selector_parser::is_type_selector(node)),
        };

        // An `&` can be escaped or in an attribute selector, thus the
        // parser finds the `&`.
        if selector.contains('&') {
            let mut ast = selector_parser::parse(selector);
            let mut changed = false;
            substitute_level(&mut ast, None, parent_selector, &facts, &mut changed);
            if changed {
                return selector_parser::to_css(&ast, false);
            }
        }

        if facts.is_list {
            format!(":is({parent_selector}) {selector}")
        } else {
            format!("{parent_selector} {selector}")
        }
    }

    fn is_dead(&self) -> bool {
        self.selector_stack.last().is_some_and(|entry| entry.dead)
    }

    fn visit_all(&mut self, nodes: Vec<AstNode>) {
        for node in nodes {
            self.visit(node);
        }
    }

    fn visit(&mut self, node: AstNode) {
        match node {
            AstNode::Rule(rule) => {
                self.current = Current::None;

                if self.selector_stack.is_empty() {
                    // A rule with `&` and no parent rule: `&` is `:scope`.
                    // In an `@scope` rule, `&` is `:where(:scope)`.
                    let mut selector = None;
                    if rule.selector.contains('&') {
                        let replacement = if self.at_rule_stack.iter().any(|(name, _, _)| name == "@scope") {
                            ":where(:scope)"
                        } else {
                            ":scope"
                        };
                        let mut ast = selector_parser::parse(&rule.selector);
                        let mut changed = false;
                        replace_nesting_selectors(&mut ast, replacement, &mut changed);
                        if changed {
                            selector = Some(selector_parser::to_css(&ast, false));
                        }
                    }
                    self.selector_stack
                        .push(SelectorEntry {
                        selector: selector.unwrap_or_else(|| rule.selector.clone()),
                        dead: false,
                        loc: rule.loc.clone(),
                    });
                } else {
                    // A rule with only `&` is the same as its children.
                    if rule.selector == "&" {
                        self.visit_all(rule.nodes);
                        return;
                    }

                    let last_selector = self.selector_stack[self.selector_stack.len() - 1].selector.clone();
                    let selectors: Vec<String> = segment(&rule.selector, b',')
                        .into_iter()
                        .map(|selector| self.substitute_nesting_selector(selector, &last_selector))
                        .collect();
                    let mut live_selectors = Vec::with_capacity(selectors.len());
                    for selector in &selectors {
                        if !can_never_match(&self.parse_selector(selector)) {
                            live_selectors.push(selector.as_str());
                        }
                    }

                    // No selector can match. The declarations of the rule
                    // have no effect, but a nested rule can match.
                    if live_selectors.is_empty() {
                        self.selector_stack.push(SelectorEntry { selector: selectors.join(", "), dead: true, loc: rule.loc.clone() });
                        self.visit_all(rule.nodes);
                        self.current = Current::None;
                        self.selector_stack.pop();
                        return;
                    }

                    let selector = live_selectors.join(", ");
                    self.selector_stack.push(SelectorEntry { selector, dead: false, loc: rule.loc.clone() });
                }

                // A rule with a declaration keeps its nested rules as they
                // are, but not when the rule contains an `@scope` rule.
                if rule.nodes.iter().any(|child| matches!(child, AstNode::Declaration(_))) && !contains_at_scope(&rule.nodes) {
                    for child in rule.nodes {
                        self.emit(child);
                    }
                } else {
                    self.visit_all(rule.nodes);
                }

                self.current = Current::None;
                self.selector_stack.pop();
            }

            AstNode::AtRule(mut rule) => {
                self.current = Current::None;

                // The current rule can never match. Only an at-rule that can
                // go to the top can contain a rule that matches.
                if self.is_dead() {
                    // An empty `@layer` sets the sequence of the layers.
                    if rule.name == "@layer" && rule.nodes.is_empty() {
                        let dead = std::mem::take(&mut self.selector_stack);
                        self.emit(AstNode::AtRule(rule));
                        self.selector_stack = dead;
                        return;
                    }
                    if rule.nodes.is_empty() || !is_hoistable(&rule.name) {
                        return;
                    }
                }

                if rule.nodes.is_empty()
                    && !(DROPPABLE_IF_EMPTY_AT_RULES.contains(&rule.name.as_str()) && (rule.name != "@scope" || switches::SCOPE_HOISTING.on()))
                {
                    self.emit(AstNode::AtRule(rule));
                    return;
                }

                if !is_hoistable(&rule.name) {
                    self.emit(AstNode::AtRule(rule));
                    return;
                }

                let mut params = std::mem::take(&mut rule.params);
                let mut masked_selector_stack = None;

                if rule.name == "@scope" {
                    let is_variant = matches!(self.source_stack.last(), Some(ContextValue::Str(source)) if source == "variant");
                    let has_parent = !self.selector_stack.is_empty();

                    rule.nodes = wrap_bare_declarations(std::mem::take(&mut rule.nodes));

                    if params.contains('&') || (!is_variant && has_parent) {
                        let parent_selector = self.selector_stack.last().map(|entry| entry.selector.clone());
                        let mut params_ast = value_parser::parse(&params);

                        // `@scope (start) to (end)`
                        let mut is_scope_end = false;
                        let mut consumed_parent = false;
                        let mut scope_start: Option<String> = None;

                        for param_node in params_ast.iter_mut() {
                            if matches!(param_node, ValueAstNode::Word(word) if word.value == "to") {
                                is_scope_end = true;
                                continue;
                            }
                            // A `(…)` group is a function with no name.
                            let ValueAstNode::Function(group) = param_node else { continue };
                            if !group.value.is_empty() {
                                continue;
                            }

                            let selector = value_parser::to_css(&group.nodes);

                            match &parent_selector {
                                // The start selector of a user rule is
                                // relative to the parent rule.
                                Some(parent_selector) if !is_scope_end && !is_variant => {
                                    let resolved: Vec<String> = segment(&selector, b',')
                                        .into_iter()
                                        .map(|part| self.substitute_nesting_selector(js_trim(part), parent_selector))
                                        .collect();
                                    group.nodes = vec![value_parser::word(resolved.join(", "))];
                                    consumed_parent = true;
                                }
                                // If not, only `&` changes.
                                _ if selector.contains('&') => {
                                    let replacement = if is_scope_end {
                                        ":where(:scope)"
                                    } else {
                                        parent_selector.as_deref().unwrap_or(":scope")
                                    };
                                    group.nodes =
                                        vec![value_parser::word(self.substitute_nesting_selector(&selector, replacement))];
                                    if !is_scope_end && has_parent {
                                        consumed_parent = true;
                                    }
                                }
                                _ => {}
                            }

                            if !is_scope_end {
                                scope_start = Some(value_parser::to_css(&group.nodes));
                            }
                        }

                        params = value_parser::to_css(&params_ast);

                        // The start selector has the parent, and it can
                        // never match: nothing is in scope.
                        if consumed_parent && self.is_dead() {
                            if let Some(scope_start) = &scope_start {
                                if can_never_match(&self.parse_selector(scope_start)) {
                                    return;
                                }
                            }
                        }

                        // The rules in the scope are relative to the root of
                        // the scope, not to the parent rule.
                        if consumed_parent {
                            masked_selector_stack = Some(std::mem::take(&mut self.selector_stack));
                        }
                    }
                }

                self.at_rule_stack.push((rule.name, params, rule.loc));
                self.visit_all(rule.nodes);
                self.current = Current::None;
                self.at_rule_stack.pop();
                if let Some(stack) = masked_selector_stack {
                    self.selector_stack = stack;
                }
            }

            AstNode::Declaration(_) | AstNode::Comment(_) => {
                if !self.is_dead() {
                    self.emit(node);
                }
            }

            AstNode::Context(context) => {
                let source = context.context.get("source").filter(|value| value.truthy()).cloned();
                let has_source = source.is_some();
                if let Some(source) = source {
                    self.source_stack.push(source);
                }
                self.visit_all(context.nodes);
                if has_source {
                    self.source_stack.pop();
                }
            }

            AstNode::AtRoot(root) => self.visit_all(root.nodes),
        }
    }

    fn emit(&mut self, node: AstNode) {
        match &self.current {
            Current::Path(path) => {
                if let AstNode::Declaration(decl) = &node {
                    if self.seen_declaration_properties.contains(&decl.property) {
                        self.dedupe_declarations_in_nodes.insert(path.clone());
                    } else {
                        self.seen_declaration_properties.insert(decl.property.clone());
                    }
                }
                nodes_at(&mut self.result, path).push(node);
                return;
            }
            Current::Detached => return,
            Current::None => {}
        }

        // No parent rule and no parent at-rule: the node goes to the result.
        if self.selector_stack.is_empty() && self.at_rule_stack.is_empty() {
            // Two equal at-rules with no body, one after the other: one is sufficient.
            if let (Some(AstNode::AtRule(last)), AstNode::AtRule(new)) = (self.result.last(), &node) {
                if last.nodes.is_empty() && new.nodes.is_empty() && last.name == new.name && last.params == new.params {
                    return;
                }
            }
            self.result.push(node);
            return;
        }

        self.seen_declaration_properties.clear();
        if let AstNode::Declaration(decl) = &node {
            self.seen_declaration_properties.insert(decl.property.clone());
        }

        // Find the at-rules at the end of the result that are equal to the
        // start of the stack: the new node can go into them.
        let mut target_path: Vec<usize> = Vec::new();
        let mut at_rule_offset = 0;
        {
            let mut target = &self.result;
            for (name, params, _) in &self.at_rule_stack {
                match target.last() {
                    Some(AstNode::AtRule(last)) if last.name == *name && last.params == *params => {
                        target_path.push(target.len() - 1);
                        at_rule_offset += 1;
                        target = &last.nodes;
                    }
                    _ => break,
                }
            }
        }

        let remaining_at_rules = self.at_rule_stack.len() - at_rule_offset;
        let target = nodes_at(&mut self.result, &target_path);
        let mut root: Option<AstNode> = None;
        let mut pending = Some(node);

        if let Some(entry) = self.selector_stack.last() {
            // The last rule has the same selector: add to that rule.
            if remaining_at_rules == 0 {
                if let Some(AstNode::Rule(last)) = target.last_mut() {
                    if last.selector == entry.selector {
                        last.nodes.extend(pending.take());
                        target_path.push(target.len() - 1);
                        self.dedupe_declarations_in_nodes.insert(target_path.clone());
                        self.current = Current::Path(target_path);
                        return;
                    }
                }
            }
            let mut made = ast::rule(entry.selector.clone(), pending.take().into_iter().collect());
            if entry.loc.is_some() {
                if let Some(loc) = made.loc_mut() {
                    *loc = entry.loc.clone();
                }
            }
            root = Some(made);
        }

        for (name, params, loc) in self.at_rule_stack[at_rule_offset..].iter().rev() {
            let nodes: Vec<AstNode> = root.take().or_else(|| pending.take()).into_iter().collect();
            root = Some(AstNode::AtRule(AtRule { name: name.clone(), params: params.clone(), nodes, loc: loc.clone() }));
        }

        match root {
            Some(root) => {
                // The new list is under the new node: one level for each
                // wrapper.
                let levels = remaining_at_rules + usize::from(!self.selector_stack.is_empty());
                target_path.push(target.len());
                target_path.extend(std::iter::repeat_n(0, levels - 1));
                target.push(root);
                self.current = Current::Path(target_path);
            }
            // All the at-rules are there already and there is no rule.
            // Upstream copies the node, and loses the nodes that come after
            // it at this level.
            None => {
                target.extend(pending);
                self.current = Current::Detached;
            }
        }
    }
}

/// Flatten the nested rules, and move the at-rules to the top.
pub fn handle_nesting(ast: Vec<AstNode>) -> Vec<AstNode> {
    let mut nesting = Nesting {
        selector_stack: Vec::new(),
        at_rule_stack: Vec::new(),
        current: Current::None,
        seen_declaration_properties: HashSet::new(),
        dedupe_declarations_in_nodes: HashSet::new(),
        result: Vec::new(),
        source_stack: Vec::new(),
        parse_selector_cache: HashMap::new(),
    };
    nesting.visit_all(ast);

    // Remove a declaration when the same declaration comes later in the
    // list. A longer path goes first: a removal moves the nodes after it.
    let mut paths: Vec<Vec<usize>> = nesting.dedupe_declarations_in_nodes.into_iter().collect();
    paths.sort_by(|a, z| z.len().cmp(&a.len()).then_with(|| z.cmp(a)));
    let mut result = nesting.result;
    for path in paths {
        let nodes = nodes_at(&mut result, &path);
        let mut seen = HashSet::new();
        let mut keep = vec![true; nodes.len()];
        for (i, node) in nodes.iter().enumerate().rev() {
            let AstNode::Declaration(decl) = node else { continue };
            let id = format!("{}\0{}\0{}", decl.property, decl.value.as_deref().unwrap_or("undefined"), decl.important);
            if !seen.insert(id) {
                keep[i] = false;
            }
        }
        let mut keep = keep.into_iter();
        nodes.retain(|_| keep.next().unwrap_or(true));
    }

    result
}
