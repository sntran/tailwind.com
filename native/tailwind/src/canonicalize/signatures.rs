//! The signatures of utilities and variants: the CSS of a utility or of a
//! variant in a normal form (`canonicalize-candidates.ts`). Two utilities
//! with the same signature give the same CSS. The caches of this file are
//! the `storage` keys of upstream.

use super::constant_fold_declaration::{constant_fold_declaration, constant_fold_declaration_ast};
use super::canonicalize_calc_expressions::canonicalize_calc_expressions_ast;
use super::expand_declaration::expand_declaration;
use super::{RemKey, Signature, SignatureOptions, SIGNATURE_EXPAND_PROPERTIES};
use crate::ast::{at_rule, style_rule, AstNode, Loc};
use crate::candidate::print_arbitrary_value;
use crate::design_system::{DesignSystem, COMPILE_NONE, COMPILE_RESPECT_IMPORTANT};
use crate::hash::{HashMap, IndexMap, IndexSet};
use crate::selector_parser::{self as sp, SelectorAstNode};
use crate::utils::infer_data_type::is_valid_spacing_multiplier;
use crate::utils::js::js_to_lower_case;
use crate::utils::locale_compare::locale_compare;
use crate::value_parser::{self as vp, ValueAstNode};
use crate::walk::{walk, walk_siblings, WalkAction};
use std::rc::Rc;

/// The properties of a utility and their values: `property` to the set of
/// values, in the sequence of the declarations.
pub type PropertyValues = IndexMap<String, IndexSet<String>>;

/// A list of names by signature.
pub type Lookup = HashMap<Rc<str>, Vec<String>>;

/// The theme variables that are inlined in each declaration, by the
/// identity of the declaration (`Loc::identity`). Each entry keeps the
/// locations of its declaration, so no other declaration gets the same
/// identity.
#[derive(Default)]
pub struct ThemeVariables(Vec<(Loc, IndexSet<String>)>);

impl ThemeVariables {
    /// The set of a declaration. The declaration gets an identity.
    fn get(&mut self, loc: &mut Loc) -> &mut IndexSet<String> {
        let found = loc.identity().and_then(|id| self.0.iter().position(|(l, _)| l.identity() == Some(id)));
        let index = match found {
            Some(index) => index,
            None => {
                loc.clear_identity();
                loc.set_identity();
                self.0.push((loc.clone(), IndexSet::default()));
                self.0.len() - 1
            }
        };
        &mut self.0[index].1
    }

    /// The variables of a declaration, as `Array.from(set).sort().join(' ')`.
    fn text(&self, loc: &Loc) -> String {
        let Some(id) = loc.identity() else { return String::new() };
        let Some((_, set)) = self.0.iter().find(|(l, _)| l.identity() == Some(id)) else { return String::new() };
        let mut list: Vec<&String> = set.iter().collect();
        list.sort_by(|a, b| crate::utils::js::js_compare_strings(a, b));
        list.into_iter().map(String::as_str).collect::<Vec<_>>().join(" ")
    }
}

/// `UTILITY_SIGNATURE_KEY`: the signature of a utility. A utility that is
/// not valid has a signature of its own, equal to no other one.
pub fn utility_signature(ds: &DesignSystem, options: SignatureOptions, utility: &str) -> Signature {
    let key = (options, utility.to_string());
    if let Some(found) = ds.canonical.utility_signatures.borrow().get(&key) {
        return found.clone();
    }
    let signature = match compile_utility_signature_ast(ds, utility, options, None) {
        Ok(ast) => Signature::Css(crate::to_css::to_css(&ast).into()),
        Err(()) => ds.canonical.new_symbol(),
    };
    ds.canonical.utility_signatures.borrow_mut().insert(key, signature.clone());
    signature
}

/// `UTILITY_THEME_VARIABLES_KEY`: for each declaration of the signature of
/// a utility, the theme variables that are inlined in its value. `None`
/// for a utility that is not valid.
pub fn utility_theme_variables(ds: &DesignSystem, options: SignatureOptions, utility: &str) -> Option<Rc<Vec<String>>> {
    let key = (options, utility.to_string());
    if let Some(found) = ds.canonical.utility_theme_variables.borrow().get(&key) {
        return found.clone();
    }
    let mut variables = ThemeVariables::default();
    let result = compile_utility_signature_ast(ds, utility, options, Some(&mut variables)).ok().map(|mut ast| {
        let mut result = Vec::new();
        walk(&mut ast, |node, _| {
            if let AstNode::Declaration(decl) = node {
                result.push(variables.text(&decl.loc));
            }
            WalkAction::Continue
        });
        Rc::new(result)
    });
    ds.canonical.utility_theme_variables.borrow_mut().insert(key, result.clone());
    result
}

/// `compileUtilitySignatureAst`: the utility in an `@apply`, in a normal
/// form. `Err` is an exception of upstream.
fn compile_utility_signature_ast(
    ds: &DesignSystem,
    utility: &str,
    options: SignatureOptions,
    theme_variables: Option<&mut ThemeVariables>,
) -> Result<Vec<AstNode>, ()> {
    // The prefix, when the utility does not have it.
    let utility = match ds.theme.prefix.as_deref().filter(|p| !p.is_empty()) {
        Some(prefix) if !utility.starts_with(prefix) => format!("{prefix}:{utility}"),
        _ => utility.to_string(),
    };

    // `@apply` gives the selector `.x`.
    let mut ast = vec![style_rule(".x", vec![at_rule("@apply", utility.clone(), vec![])])];

    let result = ds.theme.without_inline(|| {
        // Each kind of `important` has its own cache: compile with no
        // `@theme inline` for the cache of `@apply`.
        for candidate in ds.parse_candidate(&utility).iter() {
            ds.compile_ast_nodes(candidate, COMPILE_RESPECT_IMPORTANT);
        }
        crate::apply::substitute_at_apply(&mut ast, ds, None)
    });
    if result.is_err() || ds.take_js_error().is_some() {
        return Err(());
    }

    // No nodes of the compiler, no declarations with no value, and no
    // `--tw-sort`.
    canonicalize_ast(ds, &mut ast, options, theme_variables);
    Ok(ast)
}

/// `TW_VARIABLE_PROPERTIES`: the `--tw-{name}` variables that the core
/// utilities set next to a property with another name.
fn tw_variable_property(property: &str) -> &str {
    match property {
        "--tw-duration" => "transition-duration",
        "--tw-ease" => "transition-timing-function",
        "--tw-leading" => "line-height",
        "--tw-tracking" => "letter-spacing",
        _ => &property[5..],
    }
}

/// `canonicalizeAst`: the tree in a form for the comparison of
/// signatures. The declarations are expanded and sorted, with no comments.
pub fn canonicalize_ast(
    ds: &DesignSystem,
    ast: &mut Vec<AstNode>,
    options: SignatureOptions,
    mut theme_variables: Option<&mut ThemeVariables>,
) {
    let rem = options.rem();

    walk_siblings(
        ast,
        |siblings, ctx| {
            let index = ctx.index;
            match &siblings[index] {
                AstNode::Declaration(node) => {
                    if node.value.is_none() || node.property == "--tw-sort" {
                        return WalkAction::Replace(vec![]);
                    }
                    let value = node.value.clone().unwrap();

                    // No `--tw-{property}` when `{property}` has the same value.
                    if node.property.starts_with("--tw-") {
                        let property = tw_variable_property(&node.property);
                        let same = siblings.iter().any(|sibling| {
                            matches!(sibling, AstNode::Declaration(s)
                                if s.property == property && s.value.as_deref() == Some(value.as_str()) && s.important == node.important)
                        });
                        if same {
                            return WalkAction::Replace(vec![]);
                        }
                    }

                    if options.features & SIGNATURE_EXPAND_PROPERTIES != 0 {
                        if let Some(replacement) = expand_declaration(node, options.features) {
                            return WalkAction::Replace(replacement);
                        }
                    }

                    let AstNode::Declaration(node) = &mut siblings[index] else { unreachable!() };
                    let mut value = value;

                    // The inlined values of the theme.
                    if value.contains("var(") {
                        let (resolved, variables) = resolve_variables_cached(ds, &value);
                        if let Some(set) = theme_variables.as_deref_mut().map(|variables| variables.get(&mut node.loc)) {
                            set.extend(variables.iter().cloned());
                        }
                        value = resolved;
                    }

                    node.value = Some(normal_value(ds, &value, rem, options.rem));
                    WalkAction::Continue
                }
                // The special nodes are their children.
                AstNode::Context(node) => WalkAction::Replace(node.nodes.clone()),
                AstNode::AtRoot(node) => WalkAction::Replace(node.nodes.clone()),
                AstNode::Comment(_) => WalkAction::Replace(vec![]),
                AstNode::AtRule(node) if node.name == "@property" => WalkAction::Replace(vec![]),
                _ => WalkAction::Continue,
            }
        },
        |siblings, ctx| {
            match &mut siblings[ctx.index] {
                AstNode::Rule(_) | AstNode::AtRule(_) => {
                    let nodes = siblings[ctx.index].children_mut().unwrap();
                    // A later declaration of a property wins.
                    if nodes.len() > 1 {
                        let mut seen = crate::hash::HashSet::default();
                        let mut i = nodes.len();
                        while i > 0 {
                            i -= 1;
                            let AstNode::Declaration(child) = &nodes[i] else { continue };
                            if child.value.is_none() {
                                continue;
                            }
                            let property = child.property.clone();
                            if seen.contains(&property) {
                                nodes.remove(i);
                            }
                            seen.insert(property);
                        }
                    }

                    // The declarations in the order of their properties.
                    crate::utils::v8_sort::sort(nodes, |a, b| match (a, b) {
                        (AstNode::Declaration(a), AstNode::Declaration(b)) => locale_compare(&a.property, &b.property) as i64,
                        _ => 0,
                    });
                }
                AstNode::Declaration(node) => {
                    if let Some(value) = node.value.as_mut().filter(|v| !v.is_empty()) {
                        // A CSS variable stays as it is. The hex colors are lowercase.
                        if !node.property.starts_with("--") {
                            *value = lowercase_hex_colors(value);
                        }
                    }
                }
                _ => {}
            }
            WalkAction::Continue
        },
    );
}

/// `value.replace(HEX_REGEX, (color) => color.toLowerCase())`, with
/// `/#(?:[a-f0-9]{8}|[a-f0-9]{6}|[a-f0-9]{4}|[a-f0-9]{3})/gi`.
fn lowercase_hex_colors(value: &str) -> String {
    if !value.contains('#') {
        return value.to_string();
    }
    let bytes = value.as_bytes();
    let mut result = String::with_capacity(value.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'#' {
            let digits = bytes[i + 1..].iter().take_while(|b| b.is_ascii_hexdigit()).count();
            let take = [8, 6, 4, 3].into_iter().find(|n| digits >= *n).unwrap_or(0);
            if take > 0 {
                result.push('#');
                result.push_str(&value[i + 1..i + 1 + take].to_ascii_lowercase());
                i += 1 + take;
                continue;
            }
        }
        let c = value[i..].chars().next().unwrap();
        result.push(c);
        i += c.len_utf8();
    }
    result
}

/// The normal form of a value of a declaration, from a cache: the fold,
/// the order of the operands of `calc()`, and the print of arbitrary
/// values.
fn normal_value(ds: &DesignSystem, value: &str, rem: Option<f64>, rem_key: RemKey) -> String {
    let key = (rem_key, value.to_string());
    if let Some(found) = ds.canonical.normal_values.borrow().get(&key) {
        return found.clone();
    }
    // `--spacing(4)` is `calc(var(--spacing, 0.25rem) * 4)`, with the
    // inlined variable `calc(0.25rem * 4)`, and the fold gives `1rem`.
    let (folded, folded_ast) = constant_fold_declaration_ast(vp::parse(value), rem, true);
    // `calc(var(--foo) * -1)` and `calc(-1 * var(--foo))` are the same text.
    let (normalized, canonical_ast) = canonicalize_calc_expressions_ast(folded_ast);
    let value = if folded || normalized { vp::to_css(&canonical_ast) } else { value.to_string() };
    // No whitespace that is not necessary, as for the print of arbitrary
    // values.
    let result = print_arbitrary_value(&value);
    ds.canonical.normal_values.borrow_mut().insert(key, result.clone());
    result
}

/// `resolveVariablesInValue` from a cache: the value, and the inlined
/// variables.
fn resolve_variables_cached(ds: &DesignSystem, value: &str) -> (String, Rc<Vec<String>>) {
    if let Some(found) = ds.canonical.resolved_values.borrow().get(value) {
        return found.clone();
    }
    let mut inlined = IndexSet::default();
    let resolved = resolve_variables_in_value(value, ds, Some(&mut inlined));
    let result = (resolved, Rc::new(inlined.into_iter().collect::<Vec<_>>()));
    ds.canonical.resolved_values.borrow_mut().insert(value.to_string(), result.clone());
    result
}

/// `CSS_WIDE_KEYWORDS`: a theme value with one of them is never inlined.
const CSS_WIDE_KEYWORDS: &[&str] = &["initial", "inherit", "revert", "revert-layer", "revert-rule", "unset"];

/// `resolveVariablesInValue`: each theme variable of the value is its
/// value. `inlined` gets the inlined variables.
pub fn resolve_variables_in_value(value: &str, ds: &DesignSystem, mut inlined: Option<&mut IndexSet<String>>) -> String {
    let mut changed = false;
    let mut ast = vp::parse(value);
    let mut seen: crate::hash::HashSet<String> = Default::default();
    let prefix = ds.theme.prefix.as_deref().filter(|p| !p.is_empty()).map(|p| format!("--{p}-"));

    walk(&mut ast, |value_node, _| {
        let ValueAstNode::Function(function) = value_node else { return WalkAction::Continue };
        if function.value != "var" {
            return WalkAction::Continue;
        }
        if function.nodes.len() != 1 && function.nodes.len() < 3 {
            return WalkAction::Continue;
        }

        let mut variable = function.nodes[0].value().to_string();
        // The variable of the theme has no prefix.
        if let Some(prefix) = &prefix {
            if variable.starts_with(prefix.as_str()) {
                variable = variable[prefix.len()..].to_string();
            }
        }
        let variable_value = ds.resolve_theme_value(&variable, true);
        // No loop when the value has the variable itself.
        if !seen.insert(variable.clone()) {
            return WalkAction::Continue;
        }
        let Some(variable_value) = variable_value else { return WalkAction::Continue };

        if CSS_WIDE_KEYWORDS.contains(&js_to_lower_case(&variable_value).as_ref()) {
            return WalkAction::Continue;
        }

        // The fallback, when there is none.
        if function.nodes.len() == 1 {
            changed = true;
            function.nodes.extend(vp::parse(&format!(",{variable_value}")));
        }

        // A known variable with its inlined fallback is the value itself.
        if function.nodes.len() >= 3 {
            let node_as_string = vp::to_css(&function.nodes);
            let constructed = format!("{},{variable_value}", function.nodes[0].value());
            if node_as_string == constructed {
                changed = true;
                if let Some(inlined) = inlined.as_deref_mut() {
                    inlined.insert(variable);
                }
                return WalkAction::Replace(vp::parse(&variable_value));
            }
        }
        WalkAction::Continue
    });

    if changed {
        vp::to_css(&ast)
    } else {
        value.to_string()
    }
}

/// `UTILITY_PROPERTIES_KEY`: the properties and the values of a utility.
/// It also adds the utility to the index of the static utilities
/// (`STATIC_UTILITIES_KEY`).
pub fn utility_properties(ds: &DesignSystem, options: SignatureOptions, class_name: &str) -> Rc<PropertyValues> {
    let key = (options, class_name.to_string());
    if let Some(found) = ds.canonical.utility_properties.borrow().get(&key) {
        return found.clone();
    }

    let mut lookup = PropertyValues::default();
    let class_name = match ds.theme.prefix.as_deref().filter(|p| !p.is_empty()) {
        Some(prefix) if !class_name.starts_with(prefix) => format!("{prefix}:{class_name}"),
        _ => class_name.to_string(),
    };
    let parsed = ds.parse_candidate(&class_name);
    if let Some(first) = parsed.first() {
        let mut ast: Vec<AstNode> = ds.compile_ast_nodes(first, COMPILE_NONE).iter().map(|x| x.node.clone()).collect();
        // An exception of a plugin: no properties.
        if ds.take_js_error().is_none() {
            canonicalize_ast(ds, &mut ast, options, None);
            let mut index = ds.canonical.static_utilities.borrow_mut();
            let index = index.entry(options).or_default();
            walk(&mut ast, |node, _| {
                if let AstNode::Declaration(decl) = node {
                    let value = decl.value.clone().unwrap_or_default();
                    lookup.entry(decl.property.clone()).or_default().insert(value.clone());
                    index.entry(decl.property.clone()).or_default().entry(value).or_default().insert(class_name.clone());
                }
                WalkAction::Continue
            });
        }
    }

    let lookup = Rc::new(lookup);
    ds.canonical.utility_properties.borrow_mut().insert(key, lookup.clone());
    lookup
}

/// The utilities of the index of the static utilities that set a
/// property, with each value: `staticUtilities.get(property).values()`.
pub fn static_utilities_with(ds: &DesignSystem, options: SignatureOptions, property: &str) -> Vec<String> {
    let index = ds.canonical.static_utilities.borrow();
    let Some(values) = index.get(&options).and_then(|index| index.get(property)) else { return Vec::new() };
    values.values().flat_map(|set| set.iter().cloned()).collect()
}

/// `PRE_COMPUTED_UTILITIES_KEY`: the classes of the class list by their
/// signature. A functional utility of a plugin is a black box: each
/// suggestion of the class list is a static utility here.
pub fn pre_computed_utilities(ds: &DesignSystem, options: SignatureOptions) -> Rc<Lookup> {
    if let Some(found) = ds.canonical.pre_computed_utilities.borrow().get(&options) {
        return found.clone();
    }

    let mut lookup = Lookup::default();
    for (class_name, modifiers) in ds.get_class_list() {
        let Signature::Css(signature) = utility_signature(ds, options, &class_name) else { continue };

        // No `-{utility}-0` when `{utility}-0` has the same signature: the
        // positive value is better.
        if class_name.starts_with('-') && class_name.ends_with("-0") {
            if let Signature::Css(positive) = utility_signature(ds, options, &class_name[1..]) {
                if positive == signature {
                    continue;
                }
            }
        }

        lookup.entry(signature).or_default().push(class_name.clone());
        utility_properties(ds, options, &class_name);

        for modifier in modifiers {
            // A modifier that is a number needs no pre-computed utility.
            if is_valid_spacing_multiplier(&modifier) {
                continue;
            }
            let with_modifier = format!("{class_name}/{modifier}");
            let Signature::Css(signature) = utility_signature(ds, options, &with_modifier) else { continue };
            lookup.entry(signature).or_default().push(with_modifier.clone());
            utility_properties(ds, options, &with_modifier);
        }
    }

    let lookup = Rc::new(lookup);
    ds.canonical.pre_computed_utilities.borrow_mut().insert(options, lookup.clone());
    lookup
}

/// `VARIANT_SIGNATURE_KEY`: the signature of a variant: the CSS of the
/// variant with `flex`, in a normal form.
pub fn variant_signature(ds: &DesignSystem, rem: RemKey, variant: &str) -> Signature {
    let key = (rem, variant.to_string());
    if let Some(found) = ds.canonical.variant_signatures.borrow().get(&key) {
        return found.clone();
    }
    let signature = match compile_variant_signature(ds, rem, variant) {
        Some(css) => Signature::Css(css.into()),
        None => ds.canonical.new_symbol(),
    };
    ds.canonical.variant_signatures.borrow_mut().insert(key, signature.clone());
    signature
}

fn compile_variant_signature(ds: &DesignSystem, rem: RemKey, variant: &str) -> Option<String> {
    let variant = match ds.theme.prefix.as_deref().filter(|p| !p.is_empty()) {
        Some(prefix) if !variant.starts_with(prefix) => format!("{prefix}:{variant}"),
        _ => variant.to_string(),
    };

    let mut ast = vec![style_rule(".x", vec![at_rule("@apply", format!("{variant}:flex"), vec![])])];
    if crate::apply::substitute_at_apply(&mut ast, ds, None).is_err() || ds.take_js_error().is_some() {
        return None;
    }

    // The selectors in their smallest form.
    let rem = rem.map(f64::from_bits);
    walk(&mut ast, |node, _| {
        match node {
            // Equal dimensions in other units are the same: with a root
            // font size of 16px, `(width >= 64rem)` is `(width >= 1024px)`.
            AstNode::AtRule(rule) => {
                rule.params = constant_fold_declaration(&rule.params, rem);
                if rule.params.contains(' ') {
                    rule.params = rule.params.replace(' ', "");
                }
            }
            AstNode::Rule(rule) => {
                let mut selector_ast = sp::parse(&rule.selector);
                let mut changed = false;
                walk(&mut selector_ast, |node, _| {
                    match node {
                        // A list or a combinator prints with no spaces.
                        SelectorAstNode::List(_) | SelectorAstNode::Combinator(_) => changed = true,
                        SelectorAstNode::Function(function) if function.value == ":is" => {
                            // `:is(.foo)` is `.foo`.
                            if function.nodes.len() == 1 {
                                changed = true;
                                return WalkAction::Replace(function.nodes.clone());
                            }
                            // `:is(*:hover)` is `:hover`.
                            if function.nodes.len() == 2
                                && matches!(&function.nodes[0], SelectorAstNode::Selector(s) if s.value == "*")
                                && matches!(&function.nodes[1], SelectorAstNode::Selector(s) if s.value.starts_with(':'))
                            {
                                changed = true;
                                return WalkAction::Replace(vec![function.nodes[1].clone()]);
                            }
                        }
                        // A `*` before a pseudo selector in `:not(…)`,
                        // `:where(…)`: `:not(:first-child)` is
                        // `:not(*:first-child)`.
                        SelectorAstNode::Function(function)
                            if function.value.starts_with(':')
                                && matches!(function.nodes.first(), Some(SelectorAstNode::Selector(s)) if s.value.starts_with(':')) =>
                        {
                            changed = true;
                            function.nodes.insert(0, sp::selector("*"));
                        }
                        _ => {}
                    }
                    WalkAction::Continue
                });
                if changed {
                    rule.selector = sp::to_css(&selector_ast, true);
                }
            }
            _ => {}
        }
        WalkAction::Continue
    });

    Some(crate::to_css::to_css(&ast))
}

/// `PRE_COMPUTED_VARIANTS_KEY`: the static variants, and the functional
/// variants with their completions, by their signature.
pub fn pre_computed_variants(ds: &DesignSystem, rem: RemKey) -> Rc<Lookup> {
    if let Some(found) = ds.canonical.pre_computed_variants.borrow().get(&rem) {
        return found.clone();
    }
    let mut lookup = Lookup::default();
    let entries: Vec<(String, crate::variants::VariantKind)> =
        ds.variants.entries().map(|(root, info)| (root.clone(), info.kind)).collect();
    for (root, kind) in entries {
        match kind {
            crate::variants::VariantKind::Static => {
                if let Signature::Css(signature) = variant_signature(ds, rem, &root) {
                    lookup.entry(signature).or_default().push(root);
                }
            }
            crate::variants::VariantKind::Functional => {
                for value in ds.variants.get_completions(&root, ds) {
                    let name = if root == "@" { format!("@{value}") } else { format!("{root}-{value}") };
                    if let Signature::Css(signature) = variant_signature(ds, rem, &name) {
                        lookup.entry(signature).or_default().push(name);
                    }
                }
            }
            _ => {}
        }
    }
    let lookup = Rc::new(lookup);
    ds.canonical.pre_computed_variants.borrow_mut().insert(rem, lookup.clone());
    lookup
}
