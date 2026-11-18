//! The canonical form of a variant: `VARIANT_CANONICALIZATIONS` of
//! `canonicalize-candidates.ts`. One variant can become more variants:
//! `[&>[data-selected]]` is `*:data-selected`.

use super::signatures::{pre_computed_variants, variant_signature};
use super::utilities::{convert, CONVERT_MIGRATE_THEME_ONLY};
use super::{InternalOptions, Signature};
use crate::candidate::{ValueKind, Variant, VariantValue};
use crate::design_system::DesignSystem;
use crate::selector_parser::{self as sp, SelectorAstNode};
use crate::utils::infer_data_type::is_positive_integer;
use crate::utils::js::js_trim;
use crate::utils::segment::segment;
use crate::value_parser::{self as vp, ValueAstNode};
use crate::walk::{walk, walk_enter_exit, WalkAction};
use std::rc::Rc;

/// `CANONICALIZE_VARIANT_KEY`: the canonical form of a variant of the
/// parser (the key is the variant of the cache of the parser).
pub fn canonicalize_variant(ds: &DesignSystem, options: InternalOptions, variant: &Rc<Variant>) -> Rc<Vec<Variant>> {
    let key = (options, Rc::as_ptr(variant));
    if let Some(found) = ds.canonical.variants.borrow().get(&key) {
        return found.clone();
    }
    let mut replacement = vec![(**variant).clone()];
    type Step = fn(&DesignSystem, InternalOptions, Variant) -> Vec<Variant>;
    let steps: [Step; 4] = [theme_to_var_variant, arbitrary_value_to_bare_value_variant, modernize_arbitrary_values_variant, arbitrary_variants];
    for step in steps {
        replacement = std::mem::take(&mut replacement).into_iter().flat_map(|current| step(ds, options, current)).collect();
    }
    let result = Rc::new(replacement);
    ds.canonical.variants.borrow_mut().insert(key, result.clone());
    result
}

/// `walkVariants`: visit a variant, then the variant of a compound
/// variant (after the visit, which can replace the variant). The visitor
/// gets the variant and true when it has a parent. `Some` from the
/// visitor stops the walk with this result.
fn walk_variants<T>(variant: &mut Variant, has_parent: bool, visit: &mut dyn FnMut(&mut Variant, bool) -> Option<T>) -> Option<T> {
    if let Some(result) = visit(variant, has_parent) {
        return Some(result);
    }
    if let Variant::Compound { variant: child, .. } = variant {
        return walk_variants(Rc::make_mut(child), true, visit);
    }
    None
}

// ---- themeToVarVariant

fn theme_to_var_variant(ds: &DesignSystem, _: InternalOptions, mut variant: Variant) -> Vec<Variant> {
    walk_variants::<()>(&mut variant, false, &mut |variant, _| {
        match variant {
            Variant::Arbitrary { selector, .. } => {
                let (value, _) = convert(ds, selector, CONVERT_MIGRATE_THEME_ONLY);
                if value != *selector {
                    *selector = value;
                }
            }
            Variant::Functional { value: Some(value), .. } if value.kind == ValueKind::Arbitrary => {
                let (new_value, _) = convert(ds, &value.value, CONVERT_MIGRATE_THEME_ONLY);
                if new_value != value.value {
                    value.value = new_value;
                }
            }
            _ => {}
        }
        None
    });
    vec![variant]
}

// ---- arbitraryValueToBareValueVariant

fn named(value: impl Into<String>) -> Option<VariantValue> {
    Some(VariantValue { kind: ValueKind::Named, value: value.into() })
}

fn arbitrary_value_to_bare_value_variant(_: &DesignSystem, _: InternalOptions, mut variant: Variant) -> Vec<Variant> {
    walk_variants::<()>(&mut variant, false, &mut |variant, _| {
        let Variant::Functional { root, value: slot @ Some(_), .. } = variant else { return None };
        let value = slot.as_ref().unwrap();
        if value.kind != ValueKind::Arbitrary {
            return None;
        }
        let text = value.value.clone();
        match root.as_str() {
            // `data-[selected]` is `data-selected`.
            "data" if !text.contains('=') => *slot = named(text),
            // `aria-[selected="true"]` is `aria-selected`.
            "aria" if text.ends_with("=true") || text.ends_with("=\"true\"") || text.ends_with("='true'") => {
                let key = segment(&text, b'=')[0];
                // Not `aria-[foo~="true"]`, `aria-[foo|="true"]`, …
                if matches!(key.chars().last(), Some('~' | '|' | '^' | '$' | '*')) {
                    return None;
                }
                *slot = named(&text[..text.find('=').unwrap()]);
            }
            // `supports-[gap]` is `supports-gap`.
            "supports" if is_simple_name(&text) => *slot = named(text),
            _ => {}
        }
        None
    });
    vec![variant]
}

/// `/^[a-z-][a-z0-9-]*$/i`
fn is_simple_name(value: &str) -> bool {
    let mut bytes = value.bytes();
    match bytes.next() {
        Some(b) if b.is_ascii_alphabetic() || b == b'-' => {}
        _ => return false,
    }
    bytes.all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

// ---- modernizeArbitraryValuesVariant

fn node_value(node: &SelectorAstNode) -> &str {
    match node {
        SelectorAstNode::Combinator(n) => &n.value,
        SelectorAstNode::Function(n) => &n.value,
        SelectorAstNode::Selector(n) => &n.value,
        SelectorAstNode::Value(n) => &n.value,
        _ => "",
    }
}

fn is_selector(node: &SelectorAstNode, value: &str) -> bool {
    matches!(node, SelectorAstNode::Selector(n) if n.value == value)
}

fn is_combinator(node: &SelectorAstNode, value: &str) -> bool {
    matches!(node, SelectorAstNode::Combinator(n) if n.value == value)
}

/// `isSingleSelector`.
fn is_single_selector(ast: &[SelectorAstNode]) -> bool {
    !matches!(ast, [SelectorAstNode::List(_)])
}

/// `isAttributeSelector` of `canonicalize-candidates.ts`.
fn is_attribute_selector(node: &SelectorAstNode) -> bool {
    let value = node_value(node);
    value.starts_with('[') && value.ends_with(']')
}

/// The `replaceObject` of a variant with a variant of the parser. A
/// variant that the parser refuses stays as it is.
fn replace_with(ds: &DesignSystem, variant: &mut Variant, text: &str) -> bool {
    match ds.parse_variant(text) {
        Some(parsed) => {
            *variant = (*parsed).clone();
            true
        }
        None => false,
    }
}

/// The result of one visit of `modernizeArbitraryValuesVariant`.
enum Modern {
    /// The prefixed variant before the changed top variant.
    Prefixed(Rc<Variant>),
}

fn modernize_arbitrary_values_variant(ds: &DesignSystem, options: InternalOptions, mut variant: Variant) -> Vec<Variant> {
    let rem = options.signature_options.rem;
    let signature = |text: &str| variant_signature(ds, rem, text);

    let result = walk_variants::<Modern>(&mut variant, false, &mut |variant, has_parent| {
        // The modifier of `has`, `not` and `in` goes to their variant.
        if let Variant::Compound { root, modifier, variant: child } = variant {
            if (root == "has" || root == "not" || root == "in") && modifier.is_some() {
                let child = Rc::make_mut(child);
                match child {
                    Variant::Functional { modifier: inner, .. } | Variant::Compound { modifier: inner, .. } => {
                        *inner = modifier.take();
                    }
                    _ => {}
                }
            }
        }

        // An arbitrary variant that is not relative.
        let Variant::Arbitrary { selector, relative } = variant else { return None };
        if *relative {
            return None;
        }
        let selector = selector.clone();
        let top = !has_parent;

        let mut ast = sp::parse(&selector);
        if !is_single_selector(&ast) {
            return None;
        }
        // A complex selector is its nodes, for the forms below.
        if let [SelectorAstNode::Complex(complex)] = ast.as_slice() {
            ast = complex.nodes.clone();
        }

        // `[&>*]` is `*`.
        if top && ast.len() == 3 && is_selector(&ast[0], "&") && is_combinator(&ast[1], ">") && is_selector(&ast[2], "*") {
            replace_with(ds, variant, "*");
            return None;
        }
        // `[&_*]` is `**`.
        if top && ast.len() == 3 && is_selector(&ast[0], "&") && is_combinator(&ast[1], " ") && is_selector(&ast[2], "*") {
            replace_with(ds, variant, "**");
            return None;
        }
        // `[&:has(…)]` is `has-[…]`.
        if top {
            if let [SelectorAstNode::Compound(compound)] = ast.as_slice() {
                if let [amp, SelectorAstNode::Function(has)] = compound.nodes.as_slice() {
                    if is_selector(amp, "&") && has.value == ":has" && matches!(has.nodes.as_slice(), [SelectorAstNode::Selector(_)]) {
                        replace_with(ds, variant, &format!("has-[{}]", sp::to_css(&has.nodes, true)));
                        return None;
                    }
                }
            }
        }
        // `[[data-visible]_&]` is `in-[[data-visible]]`, and later
        // `in-data-visible`.
        if top
            && ast.len() == 3
            && matches!(ast[0], SelectorAstNode::Selector(_))
            && is_combinator(&ast[1], " ")
            && is_selector(&ast[2], "&")
        {
            ast.truncate(1);
            replace_with(ds, variant, &format!("in-[{}]", sp::to_css(&ast, true)));
            return None;
        }

        // The `not` of a `@media` or `@supports` variant goes out:
        // `[@media_not_(scripting:none)]` is `not-[@media_(scripting:none)]`.
        if top && matches!(ast.first(), Some(SelectorAstNode::Selector(s)) if s.value == "@media" || s.value == "@supports") {
            let target = signature(&ds.print_variant(variant));
            let mut parsed = vp::parse(&sp::to_css(&ast, true));
            let mut contains_not = false;
            walk(&mut parsed, |node, _| {
                if matches!(node, ValueAstNode::Word(w) if w.value == "not") {
                    contains_not = true;
                    return WalkAction::Replace(vec![]);
                }
                WalkAction::Continue
            });
            // No whitespace that is not necessary.
            let mut parsed = vp::parse(&vp::to_css(&parsed));
            walk(&mut parsed, |node, _| {
                if let ValueAstNode::Separator(s) = node {
                    if s.value != " " && js_trim(&s.value).is_empty() {
                        s.value = " ".into();
                    }
                }
                WalkAction::Continue
            });
            if contains_not {
                let Some(hoisted) = ds.parse_variant(&format!("not-[{}]", vp::to_css(&parsed))) else { return None };
                if target == signature(&ds.print_variant(&hoisted)) {
                    *variant = (*hoisted).clone();
                    return None;
                }
            }
        }

        let mut prefixed: Option<Rc<Variant>> = None;

        // A child: `[&>[data-visible]]` is `*:data-visible`.
        if top
            && ast.len() == 3
            && is_selector(&ast[0], "&")
            && is_combinator(&ast[1], ">")
            && matches!(&ast[2], SelectorAstNode::Selector(s) if s.value.starts_with(':') || is_attribute_selector(&ast[2]))
        {
            ast = vec![ast[2].clone()];
            prefixed = ds.parse_variant("*");
        }
        // A descendant: `[&_[data-visible]]` is `**:data-visible`.
        if top
            && ast.len() == 3
            && is_selector(&ast[0], "&")
            && is_combinator(&ast[1], " ")
            && matches!(&ast[2], SelectorAstNode::Selector(s) if s.value.starts_with(':') || is_attribute_selector(&ast[2]))
        {
            ast = vec![ast[2].clone()];
            prefixed = ds.parse_variant("**");
        }

        // No `&`: `&[data-foo]` is `[data-foo]`.
        walk_enter_exit(
            &mut ast,
            |node, _| match node {
                SelectorAstNode::Selector(s) if s.value == "&" => WalkAction::Replace(vec![]),
                SelectorAstNode::Function(_) => WalkAction::Skip,
                _ => WalkAction::Continue,
            },
            |node, _| match node {
                SelectorAstNode::Compound(compound) if compound.nodes.len() == 1 => WalkAction::ReplaceSkip(compound.nodes.clone()),
                _ => WalkAction::Continue,
            },
        );

        // One selector (a selector or an attribute selector).
        if ast.len() != 1 {
            return None;
        }

        let mut target = ast[0].clone();
        if let SelectorAstNode::Function(is) = &target {
            if is.value == ":is" {
                // One selector node. (`[foo][bar]` is one selector of more
                // nodes.)
                if !is_single_selector(&is.nodes) || is.nodes.len() != 1 {
                    return None;
                }
                // One attribute selector.
                if matches!(is.nodes[0], SelectorAstNode::Selector(_)) && !is_attribute_selector(&is.nodes[0]) {
                    return None;
                }
                target = is.nodes[0].clone();
            }
        }

        let is_pseudo = matches!(&target, SelectorAstNode::Function(f) if f.value.starts_with(':'))
            || matches!(&target, SelectorAstNode::Selector(s) if s.value.starts_with(':'));

        if is_pseudo {
            let mut target_node = target.clone();
            let mut compound_not = false;
            if let SelectorAstNode::Function(not) = &target_node {
                if not.value == ":not" {
                    compound_not = true;
                    if not.nodes.len() != 1 {
                        return None;
                    }
                    if !matches!(not.nodes[0], SelectorAstNode::Selector(_) | SelectorAstNode::Function(_)) {
                        return None;
                    }
                    if !node_value(&not.nodes[0]).starts_with(':') {
                        return None;
                    }
                    target_node = not.nodes[0].clone();
                }
            }

            let value = node_value(&target_node).to_string();
            let only_value = |name: &str| match &target_node {
                SelectorAstNode::Function(f) if f.nodes.len() == 1 => match &f.nodes[0] {
                    SelectorAstNode::Value(v) => v.value == name,
                    _ => false,
                },
                _ => false,
            };
            let new_variant: Option<String> = (|| {
                if value == ":nth-child" && only_value("odd") {
                    if compound_not {
                        compound_not = false;
                        return Some("even".to_string());
                    }
                    return Some("odd".to_string());
                }
                if value == ":nth-child" && only_value("even") {
                    if compound_not {
                        compound_not = false;
                        return Some("odd".to_string());
                    }
                    return Some("even".to_string());
                }
                for (selector, name) in
                    [(":nth-child", "nth"), (":nth-last-child", "nth-last"), (":nth-of-type", "nth-of-type"), (":nth-last-of-type", "nth-of-last-type")]
                {
                    if value == selector {
                        if let SelectorAstNode::Function(f) = &target_node {
                            if f.nodes.len() == 1 {
                                if let SelectorAstNode::Value(v) = &f.nodes[0] {
                                    if is_positive_integer(&v.value) {
                                        return Some(format!("{name}-{}", v.value));
                                    }
                                }
                                return Some(format!("{name}-[{}]", sp::to_css(&f.nodes, true)));
                            }
                        }
                    }
                }
                // The `not` goes out.
                if compound_not {
                    let target = signature(&ds.print_variant(variant));
                    let replacement = signature(&format!("not-[{value}]"));
                    if target == replacement {
                        return Some(format!("[&{value}]"));
                    }
                }
                None
            })();

            let Some(mut new_variant) = new_variant else {
                if let Some(prefixed) = prefixed {
                    // A pseudo-element cannot go in `&:is(…)`: it keeps the `&`.
                    let selector = if sp::is_pseudo_element(&target) {
                        format!("&{}", node_value(&target))
                    } else {
                        node_value(&target).to_string()
                    };
                    *variant = Variant::Arbitrary { selector, relative: false };
                    return Some(Modern::Prefixed(prefixed));
                }
                return None;
            };
            if compound_not {
                new_variant = format!("not-{new_variant}");
            }
            if !replace_with(ds, variant, &new_variant) {
                return None;
            }
        } else if matches!(target, SelectorAstNode::Selector(_)) && is_attribute_selector(&target) {
            let text = node_value(&target).to_string();
            let Some(attribute) = crate::attribute_selector_parser::parse(&text) else { return None };
            let rest = |name: &str| {
                format!(
                    "{name}{}{q}{}{q}{}",
                    attribute.operator.unwrap_or(""),
                    attribute.value.unwrap_or(""),
                    attribute.sensitivity.map(|s| format!(" {s}")).unwrap_or_default(),
                    q = attribute.quote.map(String::from).unwrap_or_default(),
                )
            };
            if let Some(name) = attribute.attribute.strip_prefix("data-") {
                let value = match attribute.value {
                    None => VariantValue { kind: ValueKind::Named, value: name.to_string() },
                    Some(_) => VariantValue { kind: ValueKind::Arbitrary, value: rest(name) },
                };
                *variant = Variant::Functional { root: "data".into(), value: Some(value), modifier: None };
            } else if let Some(name) = attribute.attribute.strip_prefix("aria-") {
                let value = match attribute.value {
                    None => VariantValue { kind: ValueKind::Arbitrary, value: name.to_string() },
                    Some("true") if attribute.operator == Some("=") && attribute.sensitivity.is_none() => {
                        VariantValue { kind: ValueKind::Named, value: name.to_string() }
                    }
                    Some(_) => VariantValue { kind: ValueKind::Arbitrary, value: rest(attribute.attribute) },
                };
                *variant = Variant::Functional { root: "aria".into(), value: Some(value), modifier: None };
            } else {
                *variant = Variant::Arbitrary { selector: text, relative: false };
            }
        }

        prefixed.map(Modern::Prefixed)
    });

    match result {
        Some(Modern::Prefixed(prefixed)) => vec![(*prefixed).clone(), variant],
        None => vec![variant],
    }
}

// ---- arbitraryVariants

fn arbitrary_variants(ds: &DesignSystem, options: InternalOptions, mut variant: Variant) -> Vec<Variant> {
    let rem = options.signature_options.rem;
    let variants = pre_computed_variants(ds, rem);
    walk_variants::<()>(&mut variant, false, &mut |variant, _| {
        if matches!(variant, Variant::Compound { .. }) {
            return None;
        }
        let target = ds.print_variant(variant);
        let Signature::Css(signature) = variant_signature(ds, rem, &target) else { return None };
        let Some(found) = variants.get(&signature).filter(|found| !found.is_empty()) else { return None };
        // A variant in a canonical form stays: `min-lg` and `lg` give the
        // same CSS, and both are valid.
        if found.contains(&target) {
            return None;
        }
        replace_with(ds, variant, &found[0]);
        None
    });
    vec![variant]
}
