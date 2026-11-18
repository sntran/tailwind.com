//! The constant fold of a value: `constant-fold-declaration.ts` of Tailwind
//! CSS.
//!
//! The fold expects valid `calc()` expressions: `calc(1 + 1)`, not
//! `calc(1+1)`.

use crate::utils::dimensions::dimensions;
use crate::utils::infer_data_type::is_length;
use crate::utils::js::js_number_to_string as num;
use crate::value_parser::{self as vp, ValueAstNode, ValueFunctionNode};
use crate::walk::{walk_enter_exit, VisitContext, WalkAction};

/// `constantFoldDeclaration`: the folded text, or the input when there is
/// nothing to fold.
pub fn constant_fold_declaration(input: &str, rem: Option<f64>) -> String {
    constant_fold_declaration_with(input, rem, true)
}

/// `constantFoldDeclaration` with the argument `normalizeUnit`.
pub fn constant_fold_declaration_with(input: &str, rem: Option<f64>, normalize_unit: bool) -> String {
    let (folded, ast) = constant_fold_declaration_ast(vp::parse(input), rem, normalize_unit);
    if folded {
        vp::to_css(&ast)
    } else {
        input.to_string()
    }
}

/// A dimension: the number and the unit.
type Dimension<'a> = Option<(f64, Option<&'a str>)>;

fn word_dimension(node: &ValueAstNode) -> Dimension<'_> {
    match node {
        ValueAstNode::Word(word) => dimensions(&word.value),
        _ => None,
    }
}

fn parent_is_function(ctx: &VisitContext<ValueAstNode>) -> bool {
    matches!(ctx.parent(), Some(ValueAstNode::Function(_)))
}

/// `constantFoldDeclarationAst`: true when a part of the value changed,
/// and the value.
pub fn constant_fold_declaration_ast(
    mut ast: Vec<ValueAstNode>,
    rem: Option<f64>,
    normalize_unit: bool,
) -> (bool, Vec<ValueAstNode>) {
    let mut folded = false;

    walk_enter_exit(
        &mut ast,
        |_, _| WalkAction::Continue,
        |value_node, ctx| {
            match value_node {
                // Each dimension in its simplest form: `-0`, `+0`, `0.0` are
                // `0`, `-0px` is `0<unit>`, and some units become a base
                // unit.
                ValueAstNode::Word(word) if word.value != "0" => {
                    let Some(canonical) = canonicalize_dimension(&word.value, rem, normalize_unit) else {
                        return WalkAction::Continue;
                    };
                    if canonical == word.value {
                        return WalkAction::Continue;
                    }

                    // `0<unit>` can be `0` only for a `<length>`. In a
                    // function such as `calc(…)`, it is not always safe.
                    if canonical == "0" && parent_is_function(ctx) {
                        let Some(with_unit) = canonicalize_dimension(&word.value, rem, false) else {
                            return WalkAction::Continue;
                        };
                        folded = true;
                        return WalkAction::ReplaceSkip(vec![vp::word(with_unit)]);
                    }

                    folded = true;
                    WalkAction::ReplaceSkip(vec![vp::word(canonical)])
                }

                // A `calc()` expression with two operands and one operator.
                ValueAstNode::Function(function) if function.value == "calc" || function.value.is_empty() => {
                    match fold_calc(function, parent_is_function(ctx)) {
                        Some(replacement) => {
                            folded = true;
                            WalkAction::ReplaceSkip(vec![replacement])
                        }
                        None => WalkAction::Continue,
                    }
                }

                _ => WalkAction::Continue,
            }
        },
    );

    (folded, ast)
}

/// The fold of one `calc()` node, or `None` when it stays as it is.
fn fold_calc(node: &ValueFunctionNode, in_function: bool) -> Option<ValueAstNode> {
    // [0.25rem] [ ] [*] [ ] [256]
    if node.nodes.len() != 5 {
        return None;
    }
    let ValueAstNode::Word(operator) = &node.nodes[2] else { return None };
    let operator = operator.value.as_str();

    let lhs_node = &node.nodes[0];
    let rhs_node = &node.nodes[4];
    let lhs = word_dimension(lhs_node);
    let rhs = word_dimension(rhs_node);

    let is = |d: Dimension, value: f64| matches!(d, Some((v, None)) if v == value);
    let zero_with_unit = |d: Dimension| matches!(d, Some((v, Some(_))) if v == 0.0);

    // `calc(0 * 100vw)` is `0`.
    if operator == "*" && (is(lhs, 0.0) || is(rhs, 0.0)) {
        return Some(vp::word("0"));
    }

    // `0<unit> * something-with-no-unit` is `0<unit>` in a function.
    if operator == "*" && zero_with_unit(lhs) && rhs.is_some_and(|(_, unit)| unit.is_none()) {
        return Some(vp::word(if in_function { format!("0{}", lhs.unwrap().1.unwrap()) } else { "0".into() }));
    }

    // `something-with-no-unit * 0<unit>` is `0<unit>` in a function.
    if operator == "*" && zero_with_unit(rhs) && lhs.is_some_and(|(_, unit)| unit.is_none()) {
        return Some(vp::word(if in_function { format!("0{}", rhs.unwrap().1.unwrap()) } else { "0".into() }));
    }

    if operator == "*" {
        // A multiplication by `1` is the other side, also when that side
        // is an expression such as `var(--foo)`.
        if is(lhs, 1.0) {
            return Some(rhs_node.clone());
        }
        if is(rhs, 1.0) {
            return Some(lhs_node.clone());
        }
    }

    if operator == "*" || operator == "+" {
        // When one side is known, and the other side is an expression with
        // the same operator, combine the two known parts:
        //
        // - `calc(2rem * calc(3 * var(--foo)))` is `calc(6rem * var(--foo))`
        // - `calc(2rem + calc(3rem + var(--foo)))` is `calc(5rem + var(--foo))`
        let constant = lhs.or(rhs);
        let nested_node = if lhs.is_none() {
            Some(lhs_node)
        } else if rhs.is_none() {
            Some(rhs_node)
        } else {
            None
        };

        if let (Some(constant), Some(ValueAstNode::Function(nested))) = (constant, nested_node) {
            let same_operator = matches!(nested.nodes.get(2), Some(ValueAstNode::Word(w)) if w.value == operator);
            if (nested.value == "calc" || nested.value.is_empty()) && nested.nodes.len() == 5 && same_operator {
                let nested_lhs_node = &nested.nodes[0];
                let nested_rhs_node = &nested.nodes[4];
                let nested_lhs = word_dimension(nested_lhs_node);
                let nested_rhs = word_dimension(nested_rhs_node);

                let known = nested_lhs.or(nested_rhs);
                let unknown = if nested_lhs.is_none() {
                    Some(nested_lhs_node)
                } else if nested_rhs.is_none() {
                    Some(nested_rhs_node)
                } else {
                    None
                };

                if let (Some(known), Some(unknown)) = (known, unknown) {
                    // `*`: one value at most has a unit.
                    if operator == "*" && constant.1.is_some() && known.1.is_some() {
                        return None;
                    }
                    // `+`: the same unit, or no units.
                    if operator == "+" && constant.1 != known.1 {
                        return None;
                    }

                    let unit = constant.1.or(known.1).unwrap_or("");
                    let combined = match operator {
                        "*" => format!("{}{unit}", num(constant.0 * known.0)),
                        _ => format!("{}{unit}", num(constant.0 + known.0)),
                    };

                    if operator == "*" && combined == "1" {
                        return Some(unknown.clone());
                    }

                    return Some(vp::fun(
                        node.value.clone(),
                        vec![vp::word(combined), node.nodes[1].clone(), node.nodes[2].clone(), node.nodes[3].clone(), unknown.clone()],
                    ));
                }
            }
        }
    }

    // Not dimensions: no fold.
    let ((lhs_value, lhs_unit), (rhs_value, rhs_unit)) = (lhs?, rhs?);

    match operator {
        "*" => {
            if lhs_unit == rhs_unit || lhs_unit.is_none() || rhs_unit.is_none() {
                return Some(vp::word(format!("{}{}", num(lhs_value * rhs_value), lhs_unit.or(rhs_unit).unwrap_or(""))));
            }
        }
        "+" => {
            if lhs_unit == rhs_unit {
                return Some(vp::word(format!("{}{}", num(lhs_value + rhs_value), lhs_unit.unwrap_or(""))));
            }
        }
        "-" => {
            if lhs_unit == rhs_unit {
                return Some(vp::word(format!("{}{}", num(lhs_value - rhs_value), lhs_unit.unwrap_or(""))));
            }
        }
        "/" => {
            // No division by zero. Unitless / unitless, or unit / unitless.
            if rhs_value != 0.0 && rhs_unit.is_none() {
                let computed = lhs_value / rhs_value;
                // Only a fold with a precision of .xx: `100% / 3.5` stays.
                if js_round(computed * 100.0) / 100.0 != computed {
                    return None;
                }
                return Some(vp::word(format!("{}{}", num(computed), lhs_unit.unwrap_or(""))));
            }
        }
        _ => {}
    }
    None
}

/// `Math.round` of JavaScript: a half goes up.
fn js_round(x: f64) -> f64 {
    if !x.is_finite() {
        return x;
    }
    let floor = x.floor();
    if x - floor >= 0.5 {
        floor + 1.0
    } else {
        floor
    }
}

/// `canonicalizeDimension`.
fn canonicalize_dimension(input: &str, rem: Option<f64>, normalize_unit: bool) -> Option<String> {
    let (value, unit) = dimensions(input)?;
    let Some(unit) = unit else {
        // Already unitless.
        return Some(num(value));
    };

    // `0<length>` is `0`.
    if value == 0.0 && is_length(input) {
        return Some(if normalize_unit { "0".into() } else { format!("0{unit}") });
    }

    // A base unit only when necessary.
    if !normalize_unit {
        return Some(input.to_string());
    }

    use std::f64::consts::PI;
    Some(match unit.to_ascii_lowercase().as_str() {
        // <length> to px
        "in" => format!("{}px", num(value * 96.0)),
        "cm" => format!("{}px", num(value * 96.0 / 2.54)),
        "mm" => format!("{}px", num(value * 96.0 / 2.54 / 10.0)),
        "q" => format!("{}px", num(value * 96.0 / 2.54 / 10.0 / 4.0)),
        "pc" => format!("{}px", num(value * 96.0 / 6.0)),
        "pt" => format!("{}px", num(value * 96.0 / 72.0)),
        "rem" => return rem.map(|rem| format!("{}px", num(value * rem))),
        // <angle> to deg
        "grad" => format!("{}deg", num(value * 0.9)),
        "rad" => format!("{}deg", num(value * 180.0 / PI)),
        "turn" => format!("{}deg", num(value * 360.0)),
        // <time> to s
        "ms" => format!("{}s", num(value / 1000.0)),
        // <frequency> to hz
        "khz" => format!("{}hz", num(value * 1000.0)),
        _ => format!("{}{unit}", num(value)),
    })
}
