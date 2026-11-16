//! The lowering of the math functions of CSS Values 4 that the targets of
//! the optimizer do not have (`docs/MIXINS.md`, "Math functions"): the
//! stepped value functions `round()`, `mod()` and `rem()`, `pow()`, and the
//! sign-related functions `abs()` and `sign()`.
//!
//! Lightning CSS computes a call with fixed values (`round(10.4px, 1px)`).
//! This step lowers the other calls, for example with `var()`:
//!
//! - `pow(A, n)`, with a whole number `n`: `A` times itself. `abs(A)`:
//!   `max(A, -1 * (A))`. `sign(A)`: `sin(atan2(A, A * 0))`, which is 1, -1
//!   or 0. These are exact.
//! - `round()`, `mod()` and `rem()`: a custom property with the type
//!   `<integer>` on the element gets the quotient `A / B`, and the browser
//!   rounds it. The value is then a `calc()` with that property. The
//!   quotient of two dimensions is `tan(atan2(A, B))`, which old browsers
//!   have. The rounding of a quotient in floating point is not exact, so a
//!   rule `@supports` after the declaration gives the native functions to
//!   each browser that has them, and only an old browser uses the
//!   lowering.

use crate::ast::{AstNode, AtRule, Declaration, Loc};
use crate::utils::segment::segment;

use super::css_mixins::{ident_end, is_ident_char, matching_paren, string_end};

/// The kinds of functions to lower.
#[derive(Debug, Default, Clone, Copy)]
pub struct Kinds {
    /// `round()`, `mod()` and `rem()`.
    pub stepped: bool,
    /// `pow()`.
    pub pow: bool,
    /// `abs()` and `sign()`.
    pub sign: bool,
}

impl Kinds {
    pub fn any(self) -> bool {
        self.stepped || self.pow || self.sign
    }
}

/// True when the text can have a function of `kinds`.
pub fn uses(input: &str, kinds: Kinds) -> bool {
    (kinds.stepped && (input.contains("round(") || input.contains("mod(") || input.contains("rem(")))
        || (kinds.pow && input.contains("pow("))
        || (kinds.sign && (input.contains("abs(") || input.contains("sign(")))
}

/// A small number, so that a quotient that is a whole number in exact
/// math, but a little less in floating point, rounds to that number.
const EPSILON: &str = "0.000001";

/// The state of the lowering of one stylesheet.
pub struct MathLowering {
    kinds: Kinds,
    /// The number of the last helper property.
    next: usize,
    /// The `@property` rules of the helper properties.
    pub properties: Vec<AstNode>,
}

/// The result of the lowering of one value.
struct Lowered {
    text: String,
    /// The helper properties: name and value.
    helpers: Vec<(String, String)>,
    /// The native functions of the value, for the `@supports` rule.
    natives: Vec<&'static str>,
}

impl MathLowering {
    pub fn new(kinds: Kinds) -> Self {
        MathLowering { kinds, next: 0, properties: Vec::new() }
    }

    /// Lower the functions in the declarations of the style rules.
    pub fn lower(&mut self, nodes: &mut Vec<AstNode>, in_style: bool, in_keyframes: bool) {
        let mut index = 0;
        while index < nodes.len() {
            let mut before = Vec::new();
            let mut after = Vec::new();
            match &mut nodes[index] {
                // In `@keyframes`, a helper property and a nested rule are
                // not valid: the value stays.
                AstNode::Declaration(declaration) if in_style && !in_keyframes => {
                    if let Some(value) = declaration.value.clone() {
                        if let Some(lowered) = self.lower_value(&value) {
                            let loc = declaration.loc.clone();
                            for (name, helper) in &lowered.helpers {
                                before.push(declaration_node(name, helper, false, &loc));
                            }
                            // The native functions, for each browser that has
                            // them, when the lowering is not exact: the
                            // stepped functions round a quotient in floating
                            // point. The test has each native function of the
                            // value.
                            if lowered.natives.iter().any(|test| STEPPED_TESTS.contains(test)) {
                                let conditions: Vec<String> = lowered.natives.iter().map(|test| format!("({test})")).collect();
                                after.push(AstNode::AtRule(AtRule {
                                    name: "@supports".to_string(),
                                    params: conditions.join(" and "),
                                    nodes: vec![declaration_node(&declaration.property, &value, declaration.important, &loc)],
                                    loc: loc.clone(),
                                }));
                            }
                            declaration.value = Some(lowered.text);
                        }
                    }
                }
                AstNode::Rule(rule) => self.lower(&mut rule.nodes, true, in_keyframes),
                AstNode::AtRule(rule) => {
                    let keyframes = in_keyframes || rule.name.to_ascii_lowercase().ends_with("keyframes");
                    self.lower(&mut rule.nodes, in_style, keyframes);
                }
                _ => {}
            }
            let count = before.len();
            nodes.splice(index..index, before);
            index += count + 1;
            let count = after.len();
            nodes.splice(index..index, after);
            index += count;
        }
    }

    /// The lowered value. `None`: there is nothing to lower.
    fn lower_value(&mut self, value: &str) -> Option<Lowered> {
        let mut lowered = Lowered { text: String::new(), helpers: Vec::new(), natives: Vec::new() };
        lowered.text = self.scan(value, &mut lowered.helpers, &mut lowered.natives);
        (lowered.text != value).then_some(lowered)
    }

    /// Replace each call to lower in a text, from the inside out.
    fn scan(&mut self, text: &str, helpers: &mut Vec<(String, String)>, natives: &mut Vec<&'static str>) -> String {
        let mut out = String::with_capacity(text.len());
        let bytes = text.as_bytes();
        let mut index = 0;
        while index < text.len() {
            let byte = bytes[index];
            if byte == b'"' || byte == b'\'' {
                let end = string_end(text, index);
                out.push_str(&text[index..end]);
                index = end;
                continue;
            }
            let c = text[index..].chars().next().unwrap_or(' ');
            if !is_ident_char(c) {
                out.push(c);
                index += c.len_utf8();
                continue;
            }
            let end = ident_end(text, index);
            let name = text[index..end].to_ascii_lowercase();
            if bytes.get(end) == Some(&b'(') {
                if let Some(close) = matching_paren(text, end) {
                    if name == "url" {
                        out.push_str(&text[index..=close]);
                        index = close + 1;
                        continue;
                    }
                    let inner = self.scan(&text[end + 1..close], helpers, natives);
                    match self.call(&name, &inner, helpers, natives) {
                        Some(replacement) => out.push_str(&replacement),
                        None => {
                            out.push_str(&text[index..end]);
                            out.push('(');
                            out.push_str(&inner);
                            out.push(')');
                        }
                    }
                    index = close + 1;
                    continue;
                }
            }
            out.push_str(&text[index..end]);
            index = end;
        }
        out
    }

    /// The lowering of one call, with its arguments already lowered.
    /// `None`: the call stays.
    fn call(&mut self, name: &str, args: &str, helpers: &mut Vec<(String, String)>, natives: &mut Vec<&'static str>) -> Option<String> {
        let list: Vec<&str> = segment(args, b',').into_iter().map(str::trim).collect();
        if list.iter().any(|arg| arg.is_empty()) {
            return None;
        }
        match (name, list.as_slice()) {
            ("pow", [base, exponent]) if self.kinds.pow && !is_fixed(&list) => {
                let n: i32 = exponent.parse().ok().filter(|n: &i32| n.abs() <= 8)?;
                add(natives, "order: pow(2, 2)");
                let product = vec![format!("({base})"); n.unsigned_abs() as usize].join(" * ");
                Some(match n {
                    0 => "1".to_string(),
                    _ if n > 0 => format!("calc({product})"),
                    _ => format!("calc(1 / ({product}))"),
                })
            }
            ("abs", [a]) if self.kinds.sign && !is_fixed(&list) => {
                add(natives, "order: abs(1)");
                Some(format!("max({a}, -1 * ({a}))"))
            }
            ("sign", [a]) if self.kinds.sign && !is_fixed(&list) => {
                add(natives, "order: sign(1)");
                Some(format!("sin(atan2({a}, calc(({a}) * 0)))"))
            }
            ("mod" | "rem", [a, b]) if self.kinds.stepped && !is_fixed(&list) => {
                let q = quotient(a, b);
                if name == "mod" {
                    add(natives, "order: mod(1, 1)");
                    let k = self.helper(helpers, format!("calc({q} - 0.5 + {EPSILON})"));
                    Some(format!("calc(({a}) - ({b}) * var({k}))"))
                } else {
                    add(natives, "order: rem(1, 1)");
                    let k = self.helper(helpers, format!("calc(max({q}, -1 * ({q})) - 0.5 + {EPSILON})"));
                    Some(format!("calc(({a}) - ({b}) * var({k}) * {})", sign_of(&q)))
                }
            }
            ("round", _) if self.kinds.stepped && !list.is_empty() && list.len() <= 3 => {
                let strategies = ["nearest", "up", "down", "to-zero"];
                let (strategy, values) = match list[0].to_ascii_lowercase().as_str() {
                    first if strategies.contains(&first) => (strategies.iter().position(|s| *s == first)?, &list[1..]),
                    _ => (0, &list[..]),
                };
                let (a, b) = match values {
                    [a] => (*a, "1"),
                    [a, b] => (*a, *b),
                    _ => return None,
                };
                if is_fixed(values) {
                    return None;
                }
                add(natives, "order: round(1, 1)");
                let q = quotient(a, b);
                let scale = |k: String| if b == "1" { format!("var({k})") } else { format!("calc(var({k}) * ({b}))") };
                Some(match strategies[strategy] {
                    // `round()` rounds a half up. A registered `<integer>`
                    // rounds a half away from zero: `floor(q + 0.5)`.
                    "nearest" => scale(self.helper(helpers, format!("calc({q} + {EPSILON})"))),
                    "up" => scale(self.helper(helpers, format!("calc({q} + 0.5 - {EPSILON})"))),
                    "down" => scale(self.helper(helpers, format!("calc({q} - 0.5 + {EPSILON})"))),
                    _ => {
                        let k = self.helper(helpers, format!("calc(max({q}, -1 * ({q})) - 0.5 + {EPSILON})"));
                        format!("calc(var({k}) * {} * ({b}))", sign_of(&q))
                    }
                })
            }
            _ => None,
        }
    }

    /// A new helper property with the value `value`, registered as an
    /// `<integer>`. The result is its name.
    fn helper(&mut self, helpers: &mut Vec<(String, String)>, value: String) -> String {
        self.next += 1;
        let name = format!("--math-{}", self.next);
        let loc = Loc::default();
        self.properties.push(AstNode::AtRule(AtRule {
            name: "@property".to_string(),
            params: name.clone(),
            nodes: vec![
                declaration_node("syntax", "\"<integer>\"", false, &loc),
                // Inherited: in a custom property, the value applies where
                // a descendant uses it.
                declaration_node("inherits", "true", false, &loc),
                declaration_node("initial-value", "0", false, &loc),
            ],
            loc,
        }));
        helpers.push((name.clone(), value));
        name
    }
}

fn declaration_node(property: &str, value: &str, important: bool, loc: &Loc) -> AstNode {
    AstNode::Declaration(Declaration { property: property.to_string(), value: Some(value.to_string()), important, loc: loc.clone() })
}

/// The tests of the stepped value functions.
const STEPPED_TESTS: [&str; 3] = ["order: round(1, 1)", "order: mod(1, 1)", "order: rem(1, 1)"];

fn add(natives: &mut Vec<&'static str>, test: &'static str) {
    if !natives.contains(&test) {
        natives.push(test);
    }
}

/// `A / B` as a number. For a dimension, `tan(atan2(A, B))`: old browsers
/// cannot divide two dimensions.
fn quotient(a: &str, b: &str) -> String {
    if is_plain_number(b) {
        format!("({a}) / {b}")
    } else {
        format!("tan(atan2({a}, {b}))")
    }
}

/// 1, -1 or 0 for the sign of a number.
fn sign_of(q: &str) -> String {
    format!("clamp(-1, ({q}) * 1000000000, 1)")
}

fn is_plain_number(text: &str) -> bool {
    number_and_unit(text).is_some_and(|(_, unit)| unit.is_empty())
}

/// A number with a unit or none: `-1.5`, `10px`, `50%`.
fn number_and_unit(text: &str) -> Option<(f64, &str)> {
    let end = text
        .char_indices()
        .find(|(index, c)| !(c.is_ascii_digit() || *c == '.' || ((*c == '-' || *c == '+') && *index == 0)))
        .map_or(text.len(), |(index, _)| index);
    let number: f64 = text[..end].parse().ok()?;
    let unit = &text[end..];
    (unit.is_empty() || unit == "%" || unit.chars().all(|c| c.is_ascii_alphabetic())).then_some((number, unit))
}

/// True when each argument is a number with the same unit: Lightning CSS
/// computes the call.
fn is_fixed(args: &[&str]) -> bool {
    let mut units = args.iter().map(|arg| number_and_unit(arg).map(|(_, unit)| unit.to_ascii_lowercase()));
    let Some(Some(first)) = units.next() else { return false };
    units.all(|unit| unit.as_deref() == Some(first.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all() -> Kinds {
        Kinds { stepped: true, pow: true, sign: true }
    }

    fn value(text: &str) -> (String, Vec<(String, String)>, Vec<&'static str>) {
        let mut lowering = MathLowering::new(all());
        match lowering.lower_value(text) {
            Some(lowered) => (lowered.text, lowered.helpers, lowered.natives),
            None => (text.to_string(), Vec::new(), Vec::new()),
        }
    }

    #[test]
    fn fixed_values_stay_for_lightning_css() {
        assert_eq!(value("round(10.4px, 1px)").0, "round(10.4px, 1px)");
        assert_eq!(value("pow(2, 3)").0, "pow(2, 3)");
        assert_eq!(value("abs(-2px)").0, "abs(-2px)");
        // Not a fixed value: two units.
        assert_ne!(value("round(1.5em, 1px)").0, "round(1.5em, 1px)");
    }

    #[test]
    fn pow_abs_and_sign() {
        assert_eq!(value("pow(var(--x), 3)").0, "calc((var(--x)) * (var(--x)) * (var(--x)))");
        assert_eq!(value("pow(var(--x), -2)").0, "calc(1 / ((var(--x)) * (var(--x))))");
        assert_eq!(value("pow(var(--x), 0.5)").0, "pow(var(--x), 0.5)");
        assert_eq!(value("abs(var(--x))").0, "max(var(--x), -1 * (var(--x)))");
        assert_eq!(value("sign(var(--x))").0, "sin(atan2(var(--x), calc((var(--x)) * 0)))");
    }

    #[test]
    fn round_mod_and_rem() {
        let (text, helpers, natives) = value("mod(var(--a), 3)");
        assert_eq!(text, "calc((var(--a)) - (3) * var(--math-1))");
        assert_eq!(helpers, vec![("--math-1".to_string(), "calc((var(--a)) / 3 - 0.5 + 0.000001)".to_string())]);
        assert_eq!(natives, vec!["order: mod(1, 1)"]);

        let (text, helpers, _) = value("round(up, var(--a), 10px)");
        assert_eq!(text, "calc(var(--math-1) * (10px))");
        assert_eq!(helpers[0].1, "calc(tan(atan2(var(--a), 10px)) + 0.5 - 0.000001)");

        let (text, _, _) = value("round(var(--a))");
        assert_eq!(text, "var(--math-1)");

        let (text, helpers, natives) = value("calc(rem(var(--a), 4) + abs(var(--b)))");
        assert_eq!(text, "calc(calc((var(--a)) - (4) * var(--math-1) * clamp(-1, ((var(--a)) / 4) * 1000000000, 1)) + max(var(--b), -1 * (var(--b))))");
        assert_eq!(helpers.len(), 1);
        assert_eq!(natives, vec!["order: rem(1, 1)", "order: abs(1)"]);
    }

    #[test]
    fn a_declaration_gets_its_helpers_and_the_native_value() {
        let mut ast = crate::css_parser::parse(".a { width: round(var(--w), 10px); }").unwrap();
        let mut lowering = MathLowering::new(all());
        lowering.lower(&mut ast, false, false);
        assert_eq!(
            crate::to_css::to_css(&ast),
            ".a {\n  --math-1: calc(tan(atan2(var(--w), 10px)) + 0.000001);\n  width: calc(var(--math-1) * (10px));\n  @supports (order: round(1, 1)) {\n    width: round(var(--w), 10px);\n  }\n}\n"
        );
        assert_eq!(lowering.properties.len(), 1);
    }
}
