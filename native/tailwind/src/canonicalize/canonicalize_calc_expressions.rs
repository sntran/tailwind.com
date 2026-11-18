//! The order of the operands of a `calc()` expression:
//! `canonicalize-calc-expressions.ts` of Tailwind CSS.
//!
//! Two operands of `*` or `+` that give the same result in the two orders
//! get one order: `calc(var(--foo) * -1)` and `calc(-1 * var(--foo))` are
//! the same text.

use crate::utils::dimensions::dimensions;
use crate::utils::locale_compare::locale_compare;
use crate::value_parser::{self as vp, ValueAstNode};
use crate::walk::{walk_enter_exit, WalkAction};
use std::cmp::Ordering;

/// `canonicalizeCalcExpressions`.
pub fn canonicalize_calc_expressions(input: &str) -> String {
    let (changed, ast) = canonicalize_calc_expressions_ast(vp::parse(input));
    if changed {
        vp::to_css(&ast)
    } else {
        input.to_string()
    }
}

/// `canonicalizeCalcExpressionsAst`: true when the order changed, and the
/// value.
pub fn canonicalize_calc_expressions_ast(mut ast: Vec<ValueAstNode>) -> (bool, Vec<ValueAstNode>) {
    let mut changed = false;

    walk_enter_exit(
        &mut ast,
        |_, _| WalkAction::Continue,
        |value_node, _| {
            // Only binary expressions with `*` or `+` in `calc(…)` and `(…)`.
            let ValueAstNode::Function(function) = value_node else { return WalkAction::Continue };
            if function.value != "calc" && !function.value.is_empty() {
                return WalkAction::Continue;
            }
            if function.nodes.len() != 5 {
                return WalkAction::Continue;
            }
            if !matches!(&function.nodes[2], ValueAstNode::Word(w) if w.value == "*" || w.value == "+") {
                return WalkAction::Continue;
            }

            if should_swap(&function.nodes[0], &function.nodes[4]) {
                changed = true;
                let mut nodes = function.nodes.clone();
                nodes.swap(0, 4);
                return WalkAction::ReplaceSkip(vec![vp::fun(function.value.clone(), nodes)]);
            }
            WalkAction::Continue
        },
    );

    (changed, ast)
}

fn should_swap(lhs: &ValueAstNode, rhs: &ValueAstNode) -> bool {
    let dimension = |node: &ValueAstNode| match node {
        ValueAstNode::Word(word) => dimensions(&word.value).map(|(v, u)| (v, u.map(str::to_string))),
        _ => None,
    };
    let lhs_dimension = dimension(lhs);
    let rhs_dimension = dimension(rhs);

    match (&lhs_dimension, &rhs_dimension) {
        (Some(_), None) => return true,
        (None, Some(_)) => return false,
        (Some((lhs_value, lhs_unit)), Some((rhs_value, rhs_unit))) => {
            // A value with a unit comes before a number: `1rem` before `1`.
            if lhs_unit.is_none() && rhs_unit.is_some() {
                return true;
            }
            if lhs_unit.is_some() && rhs_unit.is_none() {
                return false;
            }
            // Then the order of the numbers, then the order of the units.
            if lhs_value != rhs_value {
                return lhs_value - rhs_value > 0.0;
            }
            if lhs_unit != rhs_unit {
                return locale_compare(lhs_unit.as_deref().unwrap_or(""), rhs_unit.as_deref().unwrap_or(""))
                    == Ordering::Greater;
            }
        }
        (None, None) => {}
    }

    // Values that are not dimensions: the order of the text.
    locale_compare(&vp::to_css(std::slice::from_ref(lhs)), &vp::to_css(std::slice::from_ref(rhs))) == Ordering::Greater
}
