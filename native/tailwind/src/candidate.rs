//! The parser of candidates and variants: `candidate.ts` of Tailwind CSS.
//!
//! A candidate is the text of a class, for example `md:hover:bg-red-500/50`.

use crate::switches;
use crate::design_system::DesignSystem;
use crate::utils::decode_arbitrary_value::decode_arbitrary_value;
use crate::utils::is_valid_arbitrary::is_valid_arbitrary;
use crate::utils::js::js_trim;
use crate::utils::segment::segment;
use std::rc::Rc;

/// The value of a functional utility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UtilityValue {
    /// `bg-[#0088cc]`, `bg-[color:var(--x)]`, `bg-(--x)`.
    Arbitrary { data_type: Option<String>, value: String },
    /// `bg-red-500`, `w-1/2` (`fraction` is `1/2`).
    Named { value: String, fraction: Option<String> },
}

impl UtilityValue {
    pub fn is_arbitrary(&self) -> bool {
        matches!(self, UtilityValue::Arbitrary { .. })
    }
    pub fn is_named(&self) -> bool {
        matches!(self, UtilityValue::Named { .. })
    }
    /// The text of the value, for the two kinds.
    pub fn value(&self) -> &str {
        match self {
            UtilityValue::Arbitrary { value, .. } | UtilityValue::Named { value, .. } => value,
        }
    }
    /// The fraction of a named value.
    pub fn fraction(&self) -> Option<&str> {
        match self {
            UtilityValue::Named { fraction, .. } => fraction.as_deref(),
            UtilityValue::Arbitrary { .. } => None,
        }
    }
    /// The data type of an arbitrary value.
    pub fn data_type(&self) -> Option<&str> {
        match self {
            UtilityValue::Arbitrary { data_type, .. } => data_type.as_deref(),
            UtilityValue::Named { .. } => None,
        }
    }
}

/// The kind of a modifier or of a variant value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    Arbitrary,
    Named,
}

/// A modifier: `bg-red-500/50` (named) or `bg-red-500/[50%]` (arbitrary).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateModifier {
    pub kind: ValueKind,
    pub value: String,
}

/// The value of a functional variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VariantValue {
    pub kind: ValueKind,
    pub value: String,
}

/// A variant, for example `hover`, `md`, `[&_p]`, `group-hover/name`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Variant {
    Arbitrary { selector: String, relative: bool },
    Static { root: String },
    Functional { root: String, value: Option<VariantValue>, modifier: Option<CandidateModifier> },
    Compound { root: String, modifier: Option<CandidateModifier>, variant: Rc<Variant> },
}

impl Variant {
    /// The root of a variant that is not arbitrary.
    pub fn root(&self) -> Option<&str> {
        match self {
            Variant::Arbitrary { .. } => None,
            Variant::Static { root }
            | Variant::Functional { root, .. }
            | Variant::Compound { root, .. } => Some(root),
        }
    }
    pub fn modifier(&self) -> Option<&CandidateModifier> {
        match self {
            Variant::Functional { modifier, .. } | Variant::Compound { modifier, .. } => modifier.as_ref(),
            _ => None,
        }
    }
    /// The value of a functional variant.
    pub fn value(&self) -> Option<&VariantValue> {
        match self {
            Variant::Functional { value, .. } => value.as_ref(),
            _ => None,
        }
    }
}

/// `[color:red]`, `[color:red]/50`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArbitraryCandidate {
    pub property: String,
    pub value: String,
    pub modifier: Option<CandidateModifier>,
    pub variants: Vec<Rc<Variant>>,
    pub important: bool,
    pub raw: String,
}

/// `underline`, `box-border`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaticCandidate {
    pub root: String,
    pub variants: Vec<Rc<Variant>>,
    pub important: bool,
    pub raw: String,
}

/// `bg-red-500`, `bg-[#0088cc]`, `w-1/2`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FunctionalCandidate {
    pub root: String,
    pub value: Option<UtilityValue>,
    pub modifier: Option<CandidateModifier>,
    pub variants: Vec<Rc<Variant>>,
    pub important: bool,
    pub raw: String,
}

/// A parsed candidate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Candidate {
    Arbitrary(ArbitraryCandidate),
    Static(StaticCandidate),
    Functional(FunctionalCandidate),
}

impl Candidate {
    pub fn variants(&self) -> &[Rc<Variant>] {
        match self {
            Candidate::Arbitrary(c) => &c.variants,
            Candidate::Static(c) => &c.variants,
            Candidate::Functional(c) => &c.variants,
        }
    }
    pub fn important(&self) -> bool {
        match self {
            Candidate::Arbitrary(c) => c.important,
            Candidate::Static(c) => c.important,
            Candidate::Functional(c) => c.important,
        }
    }
    pub fn raw(&self) -> &str {
        match self {
            Candidate::Arbitrary(c) => &c.raw,
            Candidate::Static(c) => &c.raw,
            Candidate::Functional(c) => &c.raw,
        }
    }
    /// The root of a candidate that is not arbitrary.
    pub fn root(&self) -> Option<&str> {
        match self {
            Candidate::Arbitrary(_) => None,
            Candidate::Static(c) => Some(&c.root),
            Candidate::Functional(c) => Some(&c.root),
        }
    }
}

/// `/^[a-zA-Z0-9_.%-]+$/`
fn is_valid_named_value(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'%' | b'-'))
}

fn blank(value: &str) -> bool {
    value.is_empty() || js_trim(value).is_empty()
}


/// All the readings of a candidate. An empty result is an invalid candidate.
pub fn parse_candidate(input: &str, design_system: &DesignSystem) -> Vec<Candidate> {
    let mut result = Vec::new();

    // hover:focus:underline
    // ^^^^^ ^^^^^^           -> Variants
    //             ^^^^^^^^^  -> Base
    let mut raw_variants = segment(input, b':');

    // A prefix is a special variant at the start of the candidate.
    if let Some(prefix) = design_system.theme.prefix.as_deref().filter(|p| !p.is_empty()) {
        if raw_variants.len() == 1 {
            return result;
        }
        if raw_variants[0] != prefix {
            return result;
        }
        raw_variants.remove(0);
    }

    let mut base = raw_variants.pop().unwrap_or("");

    let mut variants: Vec<Rc<Variant>> = Vec::with_capacity(raw_variants.len());
    for raw in raw_variants.iter().rev() {
        match design_system.parse_variant(raw) {
            Some(variant) => variants.push(variant),
            None => return result,
        }
    }

    let mut important = false;
    if base.ends_with('!') {
        // The current syntax: the `!` is at the end.
        important = true;
        base = &base[..base.len() - 1];
    } else if base.starts_with('!') {
        // The legacy syntax: the `!` is at the start.
        important = true;
        base = &base[1..];
    }

    // A static utility has no value and no modifier.
    if design_system.utilities.has(base, crate::utilities::UtilityKind::Static) && !base.contains('[') {
        result.push(Candidate::Static(StaticCandidate {
            root: base.to_string(),
            variants: variants.clone(),
            important,
            raw: input.to_string(),
        }));
    }

    // The modifier is after the only `/` at the top level.
    let parts = segment(base, b'/');
    if has_extra_modifier(&parts) {
        return result;
    }
    let mut base_without_modifier = parts[0];
    let modifier_segment = parts.get(1).copied();

    let parsed_modifier = match modifier_segment {
        None => None,
        Some(m) => match parse_modifier(m) {
            Some(m) => Some(m),
            // An empty or invalid modifier: `bg-red-500/`, `bg-red-500/[]`.
            None => return result,
        },
    };

    // Arbitrary properties: `[color:red]`.
    if base_without_modifier.starts_with('[') {
        if !base_without_modifier.ends_with(']') {
            return result;
        }
        // A property starts with `-` or with a lowercase letter.
        let first = base_without_modifier.as_bytes().get(1).copied().unwrap_or(0);
        if first != b'-' && !first.is_ascii_lowercase() {
            return result;
        }
        base_without_modifier = &base_without_modifier[1..base_without_modifier.len() - 1];

        let idx = match base_without_modifier.find(':') {
            Some(idx) if idx != 0 && idx != base_without_modifier.len() - 1 => idx,
            _ => return result,
        };
        let property = &base_without_modifier[..idx];
        let value = match decode_valid_arbitrary_value(&base_without_modifier[idx + 1..]) {
            Some(v) => v,
            None => return result,
        };

        result.push(Candidate::Arbitrary(ArbitraryCandidate {
            property: property.to_string(),
            value,
            modifier: parsed_modifier,
            variants,
            important,
            raw: input.to_string(),
        }));
        return result;
    }

    let has_functional =
        |root: &str| design_system.utilities.has(root, crate::utilities::UtilityKind::Functional);

    // The pairs of root and value that can match.
    let roots: Vec<(String, Option<String>)> = if base_without_modifier.ends_with(']') {
        // `bg-[#0088cc]`: the root is before the `-[`.
        let idx = match base_without_modifier.find("-[") {
            Some(idx) => idx,
            None => return result,
        };
        let root = &base_without_modifier[..idx];
        if !has_functional(root) {
            return result;
        }
        vec![(root.to_string(), Some(base_without_modifier[idx + 1..].to_string()))]
    } else if base_without_modifier.ends_with(')') {
        // `bg-(--my-var)`: the short form of `bg-[var(--my-var)]`.
        let idx = match base_without_modifier.find("-(") {
            Some(idx) => idx,
            None => return result,
        };
        let root = &base_without_modifier[..idx];
        if !has_functional(root) {
            return result;
        }
        let inner = &base_without_modifier[idx + 2..base_without_modifier.len() - 1];
        let parts = segment(inner, b':');
        let (data_type, value) = if parts.len() == 2 { (Some(parts[0]), parts[1]) } else { (None, inner) };

        // The value must be a CSS variable.
        if !value.starts_with("--") {
            return result;
        }
        if !is_valid_arbitrary(value) {
            return result;
        }
        let value = match data_type {
            None => format!("[var({value})]"),
            Some(t) => format!("[{t}:var({value})]"),
        };
        vec![(root.to_string(), Some(value))]
    } else {
        find_roots(base_without_modifier, &has_functional)
    };

    for (root, value) in roots {
        let mut candidate = FunctionalCandidate {
            root,
            modifier: parsed_modifier.clone(),
            value: None,
            variants: variants.clone(),
            important,
            raw: input.to_string(),
        };

        let value = match value {
            None => {
                result.push(Candidate::Functional(candidate));
                continue;
            }
            Some(v) => v,
        };

        match value.find('[') {
            Some(start) => {
                if !value.ends_with(']') {
                    return result;
                }
                let mut arbitrary = match decode_valid_arbitrary_value(&value[start + 1..value.len() - 1]) {
                    Some(v) => v,
                    None => continue,
                };

                // A type hint is `-` and lowercase letters before a `:`.
                let mut typehint: Option<String> = None;
                for (i, code) in arbitrary.bytes().enumerate() {
                    if code == b':' {
                        typehint = Some(arbitrary[..i].to_string());
                        arbitrary = arbitrary[i + 1..].to_string();
                        break;
                    }
                    if code == b'-' || code.is_ascii_lowercase() {
                        continue;
                    }
                    break;
                }

                if blank(&arbitrary) {
                    continue;
                }
                if typehint.as_deref() == Some("") {
                    continue;
                }
                candidate.value = Some(UtilityValue::Arbitrary { data_type: typehint, value: arbitrary });
            }
            None => {
                // A named modifier can be the second part of a fraction.
                let fraction = match (&modifier_segment, &candidate.modifier) {
                    (None, _) => None,
                    (_, Some(m)) if m.kind == ValueKind::Arbitrary => None,
                    (Some(m), _) => Some(format!("{value}/{m}")),
                };
                if !is_valid_named_value(&value) {
                    continue;
                }
                candidate.value = Some(UtilityValue::Named { value, fraction });
            }
        }

        result.push(Candidate::Functional(candidate));
    }

    result
}

fn parse_modifier(modifier: &str) -> Option<CandidateModifier> {
    if modifier.starts_with('[') && modifier.ends_with(']') && modifier.len() >= 2 {
        let arbitrary = decode_valid_arbitrary_value(&modifier[1..modifier.len() - 1])?;
        if blank(&arbitrary) {
            return None;
        }
        return Some(CandidateModifier { kind: ValueKind::Arbitrary, value: arbitrary });
    }

    if modifier.starts_with('(') && modifier.ends_with(')') && modifier.len() >= 2 {
        let inner = &modifier[1..modifier.len() - 1];
        // The value must be a CSS variable.
        if !inner.starts_with("--") {
            return None;
        }
        if !is_valid_arbitrary(inner) {
            return None;
        }
        return Some(CandidateModifier {
            kind: ValueKind::Arbitrary,
            value: decode_arbitrary_value(&format!("var({inner})")).into_owned(),
        });
    }

    if !is_valid_named_value(modifier) {
        return None;
    }
    Some(CandidateModifier { kind: ValueKind::Named, value: modifier.to_string() })
}

/// One variant from its text. `None` is an invalid variant.
pub fn parse_variant(variant: &str, design_system: &DesignSystem) -> Option<Variant> {
    // Arbitrary variants: `[&_p]`, `[@media(width>=123px)]`.
    if variant.starts_with('[') && variant.ends_with(']') && variant.len() >= 2 {
        // An at-rule with a selector in one arbitrary variant is not valid.
        if variant.as_bytes()[1] == b'@' && variant.contains('&') {
            return None;
        }
        let mut selector = decode_valid_arbitrary_value(&variant[1..variant.len() - 1])?;
        if blank(&selector) {
            return None;
        }
        let first = selector.as_bytes()[0];
        let relative = matches!(first, b'>' | b'+' | b'~');

        // A selector with no `&` gets one: `&:is(…)`.
        if !relative && first != b'@' && !selector.contains('&') {
            selector = format!("&:is({selector})");
        }
        return Some(Variant::Arbitrary { selector, relative });
    }

    // group-hover/group-name
    // ^^^^^^^^^^^            -> Variant without modifier
    //             ^^^^^^^^^^ -> Modifier
    let parts = segment(variant, b'/');
    if has_extra_modifier(&parts) {
        return None;
    }
    let variant_without_modifier = parts[0];
    let mut modifier: Option<String> = parts.get(1).map(|m| m.to_string());

    let roots = find_roots(variant_without_modifier, &|root| design_system.variants.has(root));

    for (root, value) in roots {
        match design_system.variants.kind(&root) {
            Some(crate::variants::VariantKind::Static) => {
                // A static variant has no value and no modifier.
                if value.is_some() || modifier.is_some() {
                    return None;
                }
                return Some(Variant::Static { root });
            }
            Some(crate::variants::VariantKind::Functional) => {
                let parsed_modifier = match &modifier {
                    None => None,
                    Some(m) => Some(parse_modifier(m)?),
                };
                let value = match value {
                    None => {
                        return Some(Variant::Functional { root, modifier: parsed_modifier, value: None })
                    }
                    Some(v) => v,
                };

                if value.ends_with(']') {
                    if !value.starts_with('[') {
                        continue;
                    }
                    let arbitrary = decode_valid_arbitrary_value(&value[1..value.len() - 1])?;
                    if blank(&arbitrary) {
                        return None;
                    }
                    return Some(Variant::Functional {
                        root,
                        modifier: parsed_modifier,
                        value: Some(VariantValue { kind: ValueKind::Arbitrary, value: arbitrary }),
                    });
                }

                if value.ends_with(')') {
                    if !value.starts_with('(') {
                        continue;
                    }
                    let arbitrary = decode_valid_arbitrary_value(&value[1..value.len() - 1])?;
                    if blank(&arbitrary) {
                        return None;
                    }
                    if !arbitrary.starts_with("--") {
                        return None;
                    }
                    return Some(Variant::Functional {
                        root,
                        modifier: parsed_modifier,
                        value: Some(VariantValue {
                            kind: ValueKind::Arbitrary,
                            value: format!("var({arbitrary})"),
                        }),
                    });
                }

                if !is_valid_named_value(&value) {
                    continue;
                }
                return Some(Variant::Functional {
                    root,
                    modifier: parsed_modifier,
                    value: Some(VariantValue { kind: ValueKind::Named, value }),
                });
            }
            Some(crate::variants::VariantKind::Compound) => {
                let mut value = value?;

                // `not-group-hover/name` is `not-*` of `group-hover/name`:
                // the modifier belongs to the inner variant.
                if let Some(m) = &modifier {
                    if !m.is_empty() && (root == "not" || root == "has" || root == "in") {
                        value = format!("{value}/{m}");
                        modifier = None;
                    }
                }

                let sub_variant = design_system.parse_variant(&value)?;

                // These two variants must be compatible.
                if !design_system.variants.compounds_with(&root, &sub_variant) {
                    return None;
                }

                let parsed_modifier = match &modifier {
                    None => None,
                    Some(m) => Some(parse_modifier(m)?),
                };

                return Some(Variant::Compound { root, modifier: parsed_modifier, variant: sub_variant });
            }
            // An arbitrary variant has no root, and a root always exists here.
            Some(crate::variants::VariantKind::Arbitrary) | None => {}
        }
    }

    None
}

/// The pairs of root and value of `input`, the longest root first.
fn find_roots(input: &str, exists: &dyn Fn(&str) -> bool) -> Vec<(String, Option<String>)> {
    let mut roots = Vec::new();

    // The full input can be a root: a utility or variant with no value.
    if exists(input) {
        roots.push((input.to_string(), None));
    }

    // Cut at each dash, from the end to the start.
    let bytes = input.as_bytes();
    let mut idx = input.rfind('-');
    while let Some(i) = idx {
        if i == 0 {
            break;
        }
        let maybe_root = &input[..i];
        if exists(maybe_root) {
            let value = &input[i + 1..];
            // `bg-` has an empty value, which is not valid.
            if value.is_empty() {
                break;
            }
            // `@-[200px]` is not valid: the `@` root does not take a dash.
            if maybe_root == "@" && exists("@") && bytes[i] == b'-' {
                break;
            }
            roots.push((maybe_root.to_string(), Some(value.to_string())));
        }
        idx = input[..i].rfind('-');
    }

    // The `@` root has no dash: `@md`, `@[200px]`.
    if bytes.first() == Some(&b'@') && exists("@") {
        roots.push(("@".to_string(), Some(input[1..].to_string())));
    }

    roots
}

/// True when the parts at `/` have more than one modifier. Before 4.3.4,
/// only a third part that is not empty was an error.
fn has_extra_modifier(parts: &[&str]) -> bool {
    if switches::EMPTY_EXTRA_MODIFIER_IS_INVALID.on() {
        parts.len() > 2
    } else {
        parts.get(2).is_some_and(|extra| !extra.is_empty())
    }
}

fn decode_valid_arbitrary_value(raw: &str) -> Option<String> {
    // Before 4.3.4, the check was on the decoded value.
    if !switches::BALANCED_ARBITRARY_VALUE.on() {
        let decoded = decode_arbitrary_value(raw);
        return is_valid_arbitrary(&decoded).then(|| decoded.into_owned());
    }
    if is_valid_arbitrary(raw) {
        Some(decode_arbitrary_value(raw).into_owned())
    } else {
        None
    }
}
