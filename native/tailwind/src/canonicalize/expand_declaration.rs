//! The expansion of a shorthand declaration into its longhand
//! declarations: `expand-declaration.ts` of Tailwind CSS.

use super::{SignatureFeatures, SIGNATURE_LOGICAL_TO_PHYSICAL};
use crate::ast::{decl_full, AstNode, Declaration};
use crate::utils::segment::segment;

/// The properties of a value with 1 to 4 parts (`createBareQuad`): the
/// property and the index of the part, for each number of parts.
type Expansion = [&'static [(&'static str, usize)]];

macro_rules! quad {
    ($t:expr, $r:expr, $b:expr, $l:expr) => {
        &[
            &[($t, 0), ($r, 0), ($b, 0), ($l, 0)],
            &[($t, 0), ($r, 1), ($b, 0), ($l, 1)],
            &[($t, 0), ($r, 1), ($b, 2), ($l, 1)],
            &[($t, 0), ($r, 1), ($b, 2), ($l, 3)],
        ]
    };
}

macro_rules! pair {
    ($lhs:expr, $rhs:expr) => {
        &[&[($lhs, 0), ($rhs, 0)], &[($lhs, 0), ($rhs, 1)]]
    };
}

/// `VARIADIC_EXPANSION_MAP`: the properties depend on the number of parts.
fn variadic(property: &str) -> Option<&'static Expansion> {
    Some(match property {
        "inset" => quad!("top", "right", "bottom", "left"),
        "margin" => quad!("margin-top", "margin-right", "margin-bottom", "margin-left"),
        "padding" => quad!("padding-top", "padding-right", "padding-bottom", "padding-left"),
        "scroll-margin" => quad!("scroll-margin-top", "scroll-margin-right", "scroll-margin-bottom", "scroll-margin-left"),
        "scroll-padding" => {
            quad!("scroll-padding-top", "scroll-padding-right", "scroll-padding-bottom", "scroll-padding-left")
        }
        "border-width" => quad!("border-top-width", "border-right-width", "border-bottom-width", "border-left-width"),
        "border-style" => quad!("border-top-style", "border-right-style", "border-bottom-style", "border-left-style"),
        "border-color" => quad!("border-top-color", "border-right-color", "border-bottom-color", "border-left-color"),
        "gap" => pair!("row-gap", "column-gap"),
        "overflow" => pair!("overflow-x", "overflow-y"),
        "overscroll-behavior" => pair!("overscroll-behavior-x", "overscroll-behavior-y"),
        _ => return None,
    })
}

/// `VARIADIC_LOGICAL_EXPANSION_MAP`.
fn variadic_logical(property: &str) -> Option<&'static Expansion> {
    Some(match property {
        "inset-block" => pair!("top", "bottom"),
        "inset-inline" => pair!("left", "right"),
        "margin-block" => pair!("margin-top", "margin-bottom"),
        "margin-inline" => pair!("margin-left", "margin-right"),
        "padding-block" => pair!("padding-top", "padding-bottom"),
        "padding-inline" => pair!("padding-left", "padding-right"),
        "scroll-margin-block" => pair!("scroll-margin-top", "scroll-margin-bottom"),
        "scroll-margin-inline" => pair!("scroll-margin-left", "scroll-margin-right"),
        "scroll-padding-block" => pair!("scroll-padding-top", "scroll-padding-bottom"),
        "scroll-padding-inline" => pair!("scroll-padding-left", "scroll-padding-right"),
        _ => return None,
    })
}

/// `LOGICAL_EXPANSION_MAP`: the full value goes to each property.
fn logical(property: &str) -> Option<&'static [&'static str]> {
    Some(match property {
        "border-block" => &["border-bottom", "border-top"],
        "border-block-color" => &["border-bottom-color", "border-top-color"],
        "border-block-style" => &["border-bottom-style", "border-top-style"],
        "border-block-width" => &["border-bottom-width", "border-top-width"],
        "border-inline" => &["border-left", "border-right"],
        "border-inline-color" => &["border-left-color", "border-right-color"],
        "border-inline-style" => &["border-left-style", "border-right-style"],
        "border-inline-width" => &["border-left-width", "border-right-width"],
        _ => return None,
    })
}

fn expand(node: &Declaration, expansion: &Expansion) -> Option<Vec<AstNode>> {
    let value = node.value.as_deref().unwrap_or("");
    let args = segment(value, b' ');
    let properties = expansion.get(args.len().wrapping_sub(1))?;
    Some(
        properties
            .iter()
            .map(|(property, index)| decl_full(*property, Some(args[*index].to_string()), node.important))
            .collect(),
    )
}

/// `expandDeclaration`: the longhand declarations of a declaration, or
/// `None` when it stays as it is.
pub fn expand_declaration(node: &Declaration, features: SignatureFeatures) -> Option<Vec<AstNode>> {
    if features & SIGNATURE_LOGICAL_TO_PHYSICAL != 0 {
        if let Some(expansion) = variadic_logical(&node.property) {
            return expand(node, expansion);
        }
        if let Some(properties) = logical(&node.property) {
            return Some(
                properties
                    .iter()
                    .map(|property| decl_full(*property, node.value.clone(), node.important))
                    .collect(),
            );
        }
    }

    if let Some(expansion) = variadic(&node.property) {
        return expand(node, expansion);
    }

    None
}
