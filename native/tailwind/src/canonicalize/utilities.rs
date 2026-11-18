//! The canonical form of a utility with no variants:
//! `UTILITY_CANONICALIZATIONS` of `canonicalize-candidates.ts`, the
//! converter of `theme(…)`, and the spacing scale.

use super::constant_fold_declaration::{constant_fold_declaration, constant_fold_declaration_ast};
use super::signatures::{pre_computed_utilities, utility_signature};
use super::{InternalOptions, Signature, SignatureOptions, Spacing};
use crate::candidate::{print_modifier, Candidate, CandidateModifier, UtilityValue, ValueKind};
use crate::design_system::DesignSystem;
use crate::utilities::UtilityKind;
use crate::utils::dimensions::dimensions;
use crate::utils::infer_data_type::{is_positive_integer, is_valid_spacing_multiplier};
use crate::utils::js::{js_is_integer, js_number, js_number_to_string as num, js_parse_float, js_to_fixed, js_trim};
use crate::utils::segment::segment;
use crate::value_parser::{self as vp, ValueAstNode};
use crate::walk::{walk, walk_siblings, WalkAction};
use std::rc::Rc;

/// The candidate with no variants and no important flag:
/// `{ ...candidate, variants: [], important: false }`.
pub fn without_variants(candidate: &Candidate) -> Candidate {
    let mut candidate = candidate.clone();
    match &mut candidate {
        Candidate::Arbitrary(c) => {
            c.variants.clear();
            c.important = false;
        }
        Candidate::Static(c) => {
            c.variants.clear();
            c.important = false;
        }
        Candidate::Functional(c) => {
            c.variants.clear();
            c.important = false;
        }
    }
    candidate
}

/// The local `parseCandidate`: the readings of a candidate, with the
/// prefix of the theme when it does not have it.
pub fn parse_candidate(ds: &DesignSystem, input: &str) -> Vec<Candidate> {
    let input = match ds.theme.prefix.as_deref().filter(|p| !p.is_empty()) {
        Some(prefix) if !input.starts_with(&format!("{prefix}:")) => format!("{prefix}:{input}"),
        _ => input.to_string(),
    };
    ds.parse_candidate(&input).iter().map(|c| (**c).clone()).collect()
}

/// `printUnprefixedCandidate`.
fn print_unprefixed_candidate(ds: &DesignSystem, candidate: &Candidate) -> String {
    let text = ds.print_candidate(candidate);
    match ds.theme.prefix.as_deref().filter(|p| !p.is_empty()) {
        Some(prefix) if text.starts_with(&format!("{prefix}:")) => text[prefix.len() + 1..].to_string(),
        _ => text,
    }
}

/// A candidate, or the text of a candidate, for `hasSameSignature`.
pub enum Target<'a> {
    Candidate(&'a Candidate),
    Text(&'a str),
}

/// `COMPARE_CANDIDATES_KEY`: true when two candidates are valid and have
/// the same signature.
pub fn has_same_signature(ds: &DesignSystem, options: SignatureOptions, a: Target, b: Target) -> bool {
    let signature = |target: Target| {
        let text = match target {
            Target::Candidate(candidate) => ds.print_candidate(candidate),
            Target::Text(text) => text.to_string(),
        };
        utility_signature(ds, options, &text)
    };
    let a = signature(a);
    if !a.is_css() {
        return false;
    }
    let b = signature(b);
    b.is_css() && a == b
}

/// The signature of a candidate.
fn signature_of(ds: &DesignSystem, options: SignatureOptions, candidate: &Candidate) -> Signature {
    utility_signature(ds, options, &ds.print_candidate(candidate))
}

/// `CANONICALIZE_UTILITY_KEY`: the canonical form of a utility with no
/// variants.
pub fn canonicalize_utility(ds: &DesignSystem, options: InternalOptions, raw: &str) -> String {
    let key = (options, raw.to_string());
    if let Some(found) = ds.canonical.utilities.borrow().get(&key) {
        return found.clone();
    }
    let mut result = raw.to_string();
    let readings = ds.parse_candidate(raw);
    for candidate in readings.iter() {
        let mut replacement = (**candidate).clone();
        replacement = bg_gradient_to_linear(replacement);
        replacement = theme_to_var_utility(ds, replacement);
        replacement = calc_to_spacing_function(ds, replacement);
        replacement = optimize_arbitrary_value_expressions(ds, options, replacement);
        replacement = arbitrary_utilities(ds, options, replacement);
        replacement = bare_value_utilities(ds, options, replacement);
        replacement = deprecated_utilities(ds, options, replacement);
        replacement = drop_unnecessary_data_types(ds, options, replacement);
        replacement = arbitrary_value_to_bare_value_utility(ds, options, replacement);
        replacement = optimize_modifier(ds, options, replacement);

        let printed = ds.print_candidate(&replacement);
        if printed != raw {
            result = printed;
            break;
        }
    }
    ds.canonical.utilities.borrow_mut().insert(key, result.clone());
    result
}

// ---- bgGradientToLinear

const DIRECTIONS: &[&str] = &["t", "tr", "r", "br", "b", "bl", "l", "tl"];

fn bg_gradient_to_linear(mut candidate: Candidate) -> Candidate {
    if let Candidate::Static(c) = &mut candidate {
        if let Some(direction) = c.root.strip_prefix("bg-gradient-to-") {
            if DIRECTIONS.contains(&direction) {
                c.root = format!("bg-linear-to-{direction}");
            }
        }
    }
    candidate
}

// ---- themeToVarUtility and the converter of `theme(…)`

/// `Convert` of `canonicalize-candidates.ts`.
pub type Convert = u8;
pub const CONVERT_ALL: Convert = 0;
pub const CONVERT_MIGRATE_MODIFIER: Convert = 1 << 0;
pub const CONVERT_MIGRATE_THEME_ONLY: Convert = 1 << 1;

fn theme_to_var_utility(ds: &DesignSystem, mut candidate: Candidate) -> Candidate {
    match &mut candidate {
        Candidate::Arbitrary(c) => {
            let flags = if c.modifier.is_none() { CONVERT_MIGRATE_MODIFIER } else { CONVERT_ALL };
            let (value, modifier) = convert(ds, &c.value, flags);
            if value != c.value {
                c.value = value;
                if modifier.is_some() {
                    c.modifier = modifier;
                }
            }
        }
        Candidate::Functional(c) => {
            if let Some(UtilityValue::Arbitrary { value, .. }) = &mut c.value {
                let flags = if c.modifier.is_none() { CONVERT_MIGRATE_MODIFIER } else { CONVERT_ALL };
                let (new_value, modifier) = convert(ds, value, flags);
                if new_value != *value {
                    *value = new_value;
                    if modifier.is_some() {
                        c.modifier = modifier;
                    }
                }
            }
        }
        Candidate::Static(_) => {}
    }
    candidate
}

/// The converter (`createConverter`): each `theme(…)` of a value is a
/// `var(…)`, a `--spacing(…)` or a `--theme(…)`. The modifier of one
/// `theme(…)` with a `/` can become the modifier of the candidate.
pub fn convert(ds: &DesignSystem, input: &str, options: Convert) -> (String, Option<CandidateModifier>) {
    let mut ast = vp::parse(input);

    // In a variant, `var(…)` cannot go in the `@media (…)` part: only the
    // new `--theme(…)` form.
    if options & CONVERT_MIGRATE_THEME_ONLY != 0 {
        return (substitute_functions_in_value(&mut ast, &mut |path, fallback| to_theme(ds, path, fallback)), None);
    }

    let mut theme_usage_count = 0;
    let mut theme_modifier_count = 0;
    walk(&mut ast, |node, _| {
        let ValueAstNode::Function(function) = node else { return WalkAction::Continue };
        if function.value != "theme" {
            return WalkAction::Continue;
        }
        theme_usage_count += 1;
        // A `,` is a fallback, a `/` is a modifier.
        for child in &function.nodes {
            match child {
                ValueAstNode::Separator(s) if s.value.contains(',') => break,
                ValueAstNode::Word(w) if w.value == "/" => {
                    theme_modifier_count += 1;
                    break;
                }
                _ => {}
            }
        }
        WalkAction::Continue
    });

    if theme_usage_count == 0 {
        return (input.to_string(), None);
    }
    if theme_modifier_count == 0 {
        return (substitute_functions_in_value(&mut ast, &mut |path, fallback| to_var(ds, path, fallback)), None);
    }
    // More than one modifier: no modifier of the candidate, and no
    // `var(…)` that would lose a modifier.
    if theme_modifier_count > 1 {
        return (substitute_functions_in_value(&mut ast, &mut |path, fallback| to_theme(ds, path, fallback)), None);
    }

    // One `theme(…)` with a modifier: it becomes the modifier of the
    // candidate.
    let top_level_count = ast.len();
    let mut modifier = None;
    let result = substitute_functions_in_value(&mut ast, &mut |path, fallback| {
        let parts: Vec<&str> = segment(path, b'/').into_iter().map(js_trim).collect();
        // More than one `/`: a path that is not valid.
        if parts.len() > 2 {
            return None;
        }
        let mut path = path.to_string();
        // `theme(colors.red.500/50%)` is a color with a modifier, but only
        // when `theme(…)` is all of the value.
        if top_level_count == 1 && parts.len() == 2 && options & CONVERT_MIGRATE_MODIFIER != 0 {
            let (path_part, modifier_part) = (parts[0], parts[1]);
            modifier = Some(if is_percentage_digits(modifier_part) {
                to_opacity_modifier(ds, js_number(&modifier_part[..modifier_part.len() - 1]))
            } else if is_fraction_of_one(modifier_part) {
                to_opacity_modifier(ds, to_percentage(js_number(modifier_part)))
            } else {
                CandidateModifier { kind: ValueKind::Arbitrary, value: modifier_part.to_string() }
            });
            path = path_part.to_string();
        }
        to_var(ds, &path, fallback).or_else(|| to_theme(ds, &path, fallback))
    });
    (result, modifier)
}

/// `/^\d+%$/`
fn is_percentage_digits(value: &str) -> bool {
    value.len() >= 2 && value.ends_with('%') && value[..value.len() - 1].bytes().all(|b| b.is_ascii_digit())
}

/// `/^0?\.\d+$/`
fn is_fraction_of_one(value: &str) -> bool {
    let rest = value.strip_prefix('0').unwrap_or(value);
    match rest.strip_prefix('.') {
        Some(digits) => !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()),
        None => false,
    }
}

/// `toOpacityModifier`: a named modifier reads the `--opacity-*` values of
/// the theme first: use it only when it gives the same opacity.
fn to_opacity_modifier(ds: &DesignSystem, percentage: f64) -> CandidateModifier {
    if js_is_integer(percentage) {
        let text = num(percentage);
        let Some(opacity) = ds.theme.resolve_value(Some(&text), &["--opacity"]) else {
            return CandidateModifier { kind: ValueKind::Named, value: text };
        };
        let theme_percentage = match opacity.strip_suffix('%') {
            Some(number) => js_number(number),
            None => to_percentage(js_number(&opacity)),
        };
        if theme_percentage == percentage {
            return CandidateModifier { kind: ValueKind::Named, value: text };
        }
    }
    CandidateModifier { kind: ValueKind::Arbitrary, value: format!("{}%", num(percentage)) }
}

/// `toPercentage`: with no noise of floating point: `.29 * 100` is not
/// `28.999999999999996`.
fn to_percentage(value: f64) -> f64 {
    js_number(&js_to_fixed(value * 100.0, 10))
}

/// `pathToVariableName`.
fn path_to_variable_name(ds: &DesignSystem, path: &str, should_prefix: bool) -> Option<String> {
    let key_path: Vec<String> = crate::utils::to_key_path::to_key_path(path).into_iter().map(String::from).collect();
    let property = crate::compat::theme_fn::key_path_to_css_property(&key_path).unwrap_or_else(|| "null".into());
    let variable = format!("--{property}");
    if ds.theme.get(&[variable.as_str()]).is_none_or(|v| v.is_empty()) {
        return None;
    }
    if should_prefix {
        if let Some(prefix) = ds.theme.prefix.as_deref().filter(|p| !p.is_empty()) {
            return Some(format!("--{prefix}-{}", &variable[2..]));
        }
    }
    Some(variable)
}

/// `toVar`.
fn to_var(ds: &DesignSystem, path: &str, fallback: Option<&str>) -> Option<String> {
    if let Some(variable) = path_to_variable_name(ds, path, true) {
        return Some(match fallback.filter(|f| !f.is_empty()) {
            Some(fallback) => format!("var({variable}, {fallback})"),
            None => format!("var({variable})"),
        });
    }
    let key_path = crate::utils::to_key_path::to_key_path(path);
    if key_path.first() == Some(&"spacing") && ds.theme.get(&["--spacing"]).is_some_and(|v| !v.is_empty()) {
        let multiplier = *key_path.get(1)?;
        if !is_valid_spacing_multiplier(multiplier) {
            return None;
        }
        return Some(format!("--spacing({multiplier})"));
    }
    None
}

/// `toTheme`.
fn to_theme(ds: &DesignSystem, path: &str, fallback: Option<&str>) -> Option<String> {
    let parts: Vec<&str> = segment(path, b'/').into_iter().map(js_trim).collect();
    let variable = path_to_variable_name(ds, parts[0], false)?;
    let modifier = if parts.len() > 1 { format!("/{}", parts[1..].join("/")) } else { String::new() };
    Some(match fallback.filter(|f| !f.is_empty()) {
        Some(fallback) => format!("--theme({variable}{modifier}, {fallback})"),
        None => format!("--theme({variable}{modifier})"),
    })
}

/// `substituteFunctionsInValue`: each `theme(…)` with the result of
/// `handle(path, fallback)`, when it is not `None`.
fn substitute_functions_in_value(
    ast: &mut Vec<ValueAstNode>,
    handle: &mut dyn FnMut(&str, Option<&str>) -> Option<String>,
) -> String {
    walk_siblings(
        ast,
        |siblings, ctx| {
            let index = ctx.index;
            let ValueAstNode::Function(node) = &mut siblings[index] else { return WalkAction::Continue };
            if node.value != "theme" || node.nodes.is_empty() {
                return WalkAction::Continue;
            }

            // No whitespace before the first argument.
            if matches!(&node.nodes[0], ValueAstNode::Separator(s) if js_trim(&s.value).is_empty()) {
                node.nodes.remove(0);
            }

            let Some(ValueAstNode::Word(path_node)) = node.nodes.first() else { return WalkAction::Continue };
            let mut path = path_node.value.clone();

            // The separators of the arguments have a comma: spaces go into
            // the path. `theme(--color-red-500 / 75%)`.
            let mut skip_until_index = 1;
            for i in 1..node.nodes.len() {
                if node.nodes[i].value().contains(',') {
                    break;
                }
                path.push_str(&vp::to_css(std::slice::from_ref(&node.nodes[i])));
                skip_until_index = i + 1;
            }

            let path = eventually_unquote(&path);
            let fallback_values = node.nodes.get(skip_until_index + 1..).unwrap_or(&[]);
            let fallback = (!fallback_values.is_empty()).then(|| vp::to_css(fallback_values));

            let Some(mut replacement) = handle(&path, fallback.as_deref()) else { return WalkAction::Continue };

            // After an operator, the replacement goes in parentheses:
            // `calc(100dvh-theme(spacing.2))` is `calc(100dvh-(--spacing(2)))`.
            let mut idx = index;
            while idx > 0 {
                idx -= 1;
                let previous = &siblings[idx];
                if matches!(previous, ValueAstNode::Separator(s) if js_trim(&s.value).is_empty()) {
                    continue;
                }
                if matches!(js_trim(previous.value()), "-" | "+" | "*" | "/") {
                    replacement = format!("({replacement})");
                }
                break;
            }

            WalkAction::Replace(vp::parse(&replacement))
        },
        |_, _| WalkAction::Continue,
    );
    vp::to_css(ast)
}

/// `eventuallyUnquote`.
fn eventually_unquote(value: &str) -> String {
    let chars: Vec<char> = value.chars().collect();
    let Some(&quote) = chars.first().filter(|c| **c == '\'' || **c == '"') else { return value.to_string() };
    let mut unquoted = String::new();
    let mut i = 1;
    while i + 1 < chars.len() {
        let current = chars[i];
        let next = chars[i + 1];
        if current == '\\' && (next == quote || next == '\\') {
            unquoted.push(next);
            i += 2;
        } else {
            unquoted.push(current);
            i += 1;
        }
    }
    unquoted
}

// ---- calcToSpacingFunction

fn calc_to_spacing_function(ds: &DesignSystem, mut candidate: Candidate) -> Candidate {
    match &mut candidate {
        Candidate::Arbitrary(c) => c.value = spacing_calc_to_spacing_function(&c.value, ds),
        Candidate::Functional(c) => {
            if let Some(UtilityValue::Arbitrary { value, .. }) = &mut c.value {
                *value = spacing_calc_to_spacing_function(value, ds);
            }
        }
        Candidate::Static(_) => {}
    }
    candidate
}

/// `calc(var(--spacing) * 2)` is `--spacing(2)`.
fn spacing_calc_to_spacing_function(input: &str, ds: &DesignSystem) -> String {
    let spacing_variable = match ds.theme.prefix.as_deref().filter(|p| !p.is_empty()) {
        Some(prefix) => format!("--{prefix}-spacing"),
        None => "--spacing".to_string(),
    };
    let mut ast = vp::parse(input);
    walk(&mut ast, |node, _| {
        let ValueAstNode::Function(function) = node else { return WalkAction::Continue };
        if function.value != "calc" || function.nodes.len() != 5 {
            return WalkAction::Continue;
        }
        if !matches!(&function.nodes[2], ValueAstNode::Word(w) if w.value == "*") {
            return WalkAction::Continue;
        }
        let is_spacing = matches!(&function.nodes[0], ValueAstNode::Function(var)
            if var.value == "var" && var.nodes.len() == 1 && matches!(&var.nodes[0], ValueAstNode::Word(w) if w.value == spacing_variable));
        if !is_spacing {
            return WalkAction::Continue;
        }
        WalkAction::Replace(vp::parse(&format!("--spacing({})", vp::to_css(std::slice::from_ref(&function.nodes[4])))))
    });
    vp::to_css(&ast)
}

// ---- The spacing scale

/// `MAX_BARE_VALUE_IN_PX`: a bare value up to the `2xl` breakpoint.
const MAX_BARE_VALUE_IN_PX: f64 = 1536.0;
const MAX_BARE_VALUE_IN_REM: f64 = MAX_BARE_VALUE_IN_PX / 16.0;

/// `createSpacingCache`: made one time for the design system, with the
/// `rem` of the first call.
pub fn prepare_spacing(ds: &DesignSystem, rem: Option<f64>) {
    if ds.canonical.spacing.borrow().is_some() {
        return;
    }
    let spacing = ds.resolve_theme_value("--spacing", true).and_then(|value| {
        let value = constant_fold_declaration(&value, rem);
        let (value, unit) = dimensions(&value)?;
        Some(Spacing { value, unit: unit.map(String::from), rem })
    });
    *ds.canonical.spacing.borrow_mut() = Some(spacing);
}

/// `SPACING_KEY`: the multiplier of the spacing scale for a value, or
/// `None` when the value is not a multiple of the scale.
pub fn spacing_multiplier(ds: &DesignSystem, input: &str) -> Option<f64> {
    let spacing = ds.canonical.spacing.borrow().clone().flatten()?;
    if let Some(found) = ds.canonical.spacing_values.borrow().get(input) {
        return *found;
    }
    let result = (|| {
        // A scale of 0: each multiplier is 0.
        if spacing.value == 0.0 {
            return None;
        }
        let folded = constant_fold_declaration(input, spacing.rem);
        let (value, unit) = dimensions(&folded)?;
        if unit != spacing.unit.as_deref() {
            return None;
        }
        Some(value / spacing.value)
    })();
    ds.canonical.spacing_values.borrow_mut().insert(input.to_string(), result);
    result
}

/// `isValidSpacingMultiplier` for a number.
fn is_valid_spacing_number(value: f64) -> bool {
    value >= 0.0 && value % 0.25 == 0.0
}

/// `isReasonableBareValue`: no large bare values such as `left-24999.75`.
fn is_reasonable_bare_value(value: f64, ds: &DesignSystem, rem: Option<f64>) -> bool {
    let Some(spacing) = ds.resolve_theme_value("--spacing", true) else { return false };
    let folded = constant_fold_declaration(&spacing, rem);
    let Some((spacing_value, spacing_unit)) = dimensions(&folded) else { return false };
    let pixels = value * spacing_value;
    match spacing_unit {
        Some("px") => pixels <= MAX_BARE_VALUE_IN_PX,
        Some("rem") => pixels <= MAX_BARE_VALUE_IN_REM,
        _ => false,
    }
}

// ---- arbitraryUtilities and bareValueUtilities

/// The modifier of a candidate.
fn modifier_of(candidate: &Candidate) -> Option<&CandidateModifier> {
    match candidate {
        Candidate::Arbitrary(c) => c.modifier.as_ref(),
        Candidate::Functional(c) => c.modifier.as_ref(),
        Candidate::Static(_) => None,
    }
}

fn with_modifier(mut candidate: Candidate, modifier: Option<CandidateModifier>) -> Candidate {
    match &mut candidate {
        Candidate::Arbitrary(c) => c.modifier = modifier,
        Candidate::Functional(c) => c.modifier = modifier,
        Candidate::Static(_) => {}
    }
    candidate
}

/// A visitor of the replacements of a generator: `true` stops the
/// generator.
type Visit<'a> = dyn FnMut(Candidate) -> bool + 'a;

/// Visit each reading of a candidate. `true` when the visitor stops.
fn visit_readings(ds: &DesignSystem, input: &str, visit: &mut Visit) -> bool {
    parse_candidate(ds, input).into_iter().any(|candidate| visit(candidate))
}

/// The first part of `tryReplacements` of `arbitraryUtilities` and
/// `bareValueUtilities`: one replacement from the pre-computed utilities.
/// `Some(stopped)` when the generator ends here.
fn try_pre_computed(ds: &DesignSystem, replacements: &[String], visit: &mut Visit) -> Option<bool> {
    if replacements.len() > 1 {
        // A positive value is better than a negative value. With more
        // than one positive value, no replacement.
        let mut maybe: Option<&String> = None;
        for replacement in replacements {
            if replacement.starts_with('-') {
                continue;
            }
            if maybe.is_some() {
                return Some(false);
            }
            maybe = Some(replacement);
        }
        if let Some(maybe) = maybe {
            return Some(visit_readings(ds, maybe, visit));
        }
        return Some(false);
    }
    None
}

fn arbitrary_utilities(ds: &DesignSystem, options: InternalOptions, candidate: Candidate) -> Candidate {
    // Only arbitrary properties and arbitrary values.
    let is_arbitrary_value = matches!(&candidate, Candidate::Functional(c) if c.value.as_ref().is_some_and(UtilityValue::is_arbitrary));
    if !matches!(candidate, Candidate::Arbitrary(_)) && !is_arbitrary_value {
        return candidate;
    }

    let signature_options = options.signature_options;
    let Signature::Css(target) = signature_of(ds, signature_options, &candidate) else { return candidate };

    let mut found = None;
    try_arbitrary_replacements(ds, options, &target, &candidate, &mut |replacement| {
        if !has_same_signature(ds, signature_options, Target::Candidate(&candidate), Target::Candidate(&replacement)) {
            return false;
        }
        // The CSS variables of the value must stay in the replacement.
        if !all_variables_are_used(ds, &candidate, &replacement) {
            return false;
        }
        found = Some(replacement);
        true
    });
    found.unwrap_or(candidate)
}

/// `tryReplacements` of `arbitraryUtilities`. `true` when the visitor
/// stops.
fn try_arbitrary_replacements(
    ds: &DesignSystem,
    options: InternalOptions,
    target: &Rc<str>,
    candidate: &Candidate,
    visit: &mut Visit,
) -> bool {
    let signature_options = options.signature_options;
    let utilities = pre_computed_utilities(ds, signature_options);
    let replacements: &[String] = utilities.get(target).map_or(&[], |v| v.as_slice());

    if let Some(stopped) = try_pre_computed(ds, replacements, visit) {
        return stopped;
    }

    // With no replacement, try with no modifier, then add the modifier
    // again: `text-red-500/50` is not pre-computed, `text-red-500` is.
    if replacements.is_empty() {
        if let Some(modifier) = modifier_of(candidate).cloned() {
            let without = with_modifier(candidate.clone(), None);
            if let Signature::Css(target) = signature_of(ds, signature_options, &without) {
                let stopped = try_arbitrary_replacements(ds, options, &target, &without, &mut |replacement| {
                    visit(with_modifier(replacement, Some(modifier.clone())))
                });
                if stopped {
                    return true;
                }
            }
        }
    }

    if replacements.len() == 1 {
        return visit_readings(ds, &replacements[0], visit);
    }

    // A functional utility that sets the same property. An arbitrary
    // property sets one property.
    let value = match candidate {
        Candidate::Arbitrary(c) => c.value.clone(),
        Candidate::Functional(c) => match &c.value {
            Some(value) => value.value().to_string(),
            None => return false,
        },
        Candidate::Static(_) => return false,
    };
    let modifier = modifier_of(candidate).cloned();
    let is_arbitrary_value = matches!(candidate, Candidate::Functional(c) if c.value.as_ref().is_some_and(UtilityValue::is_arbitrary));

    // With `rem`, the values in `rem` and `px` are `px`: the multiplier of
    // the spacing scale too.
    if let Some(rem) = signature_options.rem() {
        if is_arbitrary_value {
            if let Some(bare) = spacing_multiplier(ds, &value) {
                if is_valid_spacing_number(bare) && is_reasonable_bare_value(bare, ds, Some(rem)) {
                    let mut replacement = candidate.clone();
                    if let Candidate::Functional(c) = &mut replacement {
                        c.value = Some(UtilityValue::Named { value: num(bare), fraction: None });
                    }
                    if visit(replacement) {
                        return true;
                    }
                }
            }
        }
    }

    let mut multiplier = spacing_multiplier(ds, &value);
    let mut root_prefix = "";
    if let Some(m) = multiplier.filter(|m| *m < 0.0) {
        root_prefix = "-";
        multiplier = Some(m.abs());
    }

    // `mt-*` before `-mt-*`: `-mt-[0px]` can be `mt-[0px]`.
    let mut roots: Vec<String> = ds.utilities.keys(UtilityKind::Functional).into_iter().map(String::from).collect();
    roots.sort_by_key(|root| root.starts_with('-'));

    let printed_modifier = print_modifier(modifier.as_ref());
    for root in roots {
        let root = format!("{root_prefix}{root}");

        // A bare value.
        if visit_readings(ds, &format!("{root}-{value}"), visit) {
            return true;
        }
        // A bare value with the modifier. (Upstream puts the modifier
        // object in the text: `[object Object]`.)
        if modifier.is_some() && visit_readings(ds, &format!("{root}-{value}[object Object]"), visit) {
            return true;
        }
        // A bare value of the spacing scale: `w-[64rem]` is `w-256`.
        if let Some(m) = multiplier {
            if is_valid_spacing_number(m) && is_reasonable_bare_value(m, ds, signature_options.rem()) {
                if visit_readings(ds, &format!("{root}-{}", num(m)), visit) {
                    return true;
                }
                if modifier.is_some() && visit_readings(ds, &format!("{root}-{}{printed_modifier}", num(m)), visit) {
                    return true;
                }
            }
        }
        // An arbitrary value.
        if visit_readings(ds, &format!("{root}-[{value}]"), visit) {
            return true;
        }
        if modifier.is_some() && visit_readings(ds, &format!("{root}-[{value}]{printed_modifier}"), visit) {
            return true;
        }
    }
    false
}

/// `allVariablesAreUsed`: each CSS variable of the value is also in the
/// replacement, and the replacement does not set it.
fn all_variables_are_used(ds: &DesignSystem, candidate: &Candidate, replacement: &Candidate) -> bool {
    let value = match candidate {
        Candidate::Functional(c) => match &c.value {
            Some(UtilityValue::Arbitrary { value, .. }) if value.contains("var(--") => value.clone(),
            _ => return true,
        },
        Candidate::Arbitrary(c) if c.value.contains("var(--") => c.value.clone(),
        _ => return true,
    };

    let css: Vec<String> =
        ds.candidates_to_css(&[ds.print_candidate(replacement).as_str()]).into_iter().map(|c| c.unwrap_or_default()).collect();
    let css = css.join("\n");

    let mut safe = true;
    walk(&mut vp::parse(&value), |node, _| {
        if let ValueAstNode::Function(function) = node {
            if function.value == "var" {
                let variable = function.nodes.first().map(|n| n.value().to_string()).unwrap_or_default();
                let used = css.contains(&format!("var({variable},")) || css.contains(&format!("var({variable})"));
                if !used || css.contains(&format!("{variable}:")) {
                    safe = false;
                    return WalkAction::Stop;
                }
            }
        }
        WalkAction::Continue
    });
    safe
}

fn bare_value_utilities(ds: &DesignSystem, options: InternalOptions, candidate: Candidate) -> Candidate {
    // Only functional utilities with a bare value.
    if !matches!(&candidate, Candidate::Functional(c) if c.value.as_ref().is_some_and(UtilityValue::is_named)) {
        return candidate;
    }
    let signature_options = options.signature_options;
    let Signature::Css(target) = signature_of(ds, signature_options, &candidate) else { return candidate };

    let mut found = None;
    try_bare_replacements(ds, options, &target, &candidate, &mut |replacement| {
        if !has_same_signature(ds, signature_options, Target::Candidate(&candidate), Target::Candidate(&replacement)) {
            return false;
        }
        found = Some(replacement);
        true
    });
    found.unwrap_or(candidate)
}

/// `tryReplacements` of `bareValueUtilities`.
fn try_bare_replacements(ds: &DesignSystem, options: InternalOptions, target: &Rc<str>, candidate: &Candidate, visit: &mut Visit) -> bool {
    let signature_options = options.signature_options;
    let utilities = pre_computed_utilities(ds, signature_options);
    let replacements: &[String] = utilities.get(target).map_or(&[], |v| v.as_slice());

    if let Some(stopped) = try_pre_computed(ds, replacements, visit) {
        return stopped;
    }

    if replacements.is_empty() {
        if let Some(modifier) = modifier_of(candidate).cloned() {
            let without = with_modifier(candidate.clone(), None);
            if let Signature::Css(target) = signature_of(ds, signature_options, &without) {
                let stopped = try_bare_replacements(ds, options, &target, &without, &mut |replacement| {
                    visit(with_modifier(replacement, Some(modifier.clone())))
                });
                if stopped {
                    return true;
                }
            }
        }
    }

    if replacements.len() == 1 {
        return visit_readings(ds, &replacements[0], visit);
    }
    false
}

// ---- deprecatedUtilities

/// `tryDeprecatedUtilities`: the new names of an old utility.
fn deprecated_names(candidate: &str) -> Vec<String> {
    let mut names = Vec::new();
    match candidate {
        "order-none" => names.push("order-0".to_string()),
        "break-words" => names.push("wrap-break-word".to_string()),
        "overflow-ellipsis" => names.push("text-ellipsis".to_string()),
        _ => {}
    }
    // `/^(-)?start-(.*?)$/` and `/^(-)?end-(.*?)$/`.
    for (old, new) in [("start-", "inset-s-"), ("end-", "inset-e-")] {
        let (sign, rest) = match candidate.strip_prefix('-') {
            Some(rest) if rest.starts_with(old) => ("-", rest),
            _ => ("", candidate),
        };
        if let Some(value) = rest.strip_prefix(old) {
            let replacement = format!("{sign}{new}{value}");
            if replacement != candidate {
                names.push(replacement);
            }
        }
    }
    names
}

fn deprecated_utilities(ds: &DesignSystem, options: InternalOptions, candidate: Candidate) -> Candidate {
    let target = print_unprefixed_candidate(ds, &candidate);
    for name in deprecated_names(&target) {
        if !has_same_signature(ds, options.signature_options, Target::Candidate(&candidate), Target::Text(&name)) {
            continue;
        }
        if let Some(replacement) = parse_candidate(ds, &name).into_iter().next() {
            return replacement;
        }
    }
    candidate
}

// ---- dropUnnecessaryDataTypes

fn drop_unnecessary_data_types(ds: &DesignSystem, options: InternalOptions, mut candidate: Candidate) -> Candidate {
    let Candidate::Functional(c) = &candidate else { return candidate };
    let Some(UtilityValue::Arbitrary { data_type: Some(_), value }) = &c.value else { return candidate };

    let mut replacement = candidate.clone();
    if let Candidate::Functional(r) = &mut replacement {
        r.value = Some(UtilityValue::Arbitrary { data_type: None, value: value.clone() });
    }
    let signature_options = options.signature_options;
    if signature_of(ds, signature_options, &candidate) == signature_of(ds, signature_options, &replacement) {
        if let Candidate::Functional(c) = &mut candidate {
            if let Some(UtilityValue::Arbitrary { data_type, .. }) = &mut c.value {
                *data_type = None;
            }
        }
    }
    candidate
}

// ---- arbitraryValueToBareValueUtility

fn arbitrary_value_to_bare_value_utility(ds: &DesignSystem, options: InternalOptions, mut candidate: Candidate) -> Candidate {
    let Candidate::Functional(c) = &candidate else { return candidate };
    let Some(UtilityValue::Arbitrary { value, .. }) = &c.value else { return candidate };
    let value = value.clone();

    let signature_options = options.signature_options;
    let expected = signature_of(ds, signature_options, &candidate);

    let mut found = None;
    let mut seen = crate::hash::HashSet::default();
    try_value_replacements(&value, &mut seen, &mut |replacement| {
        let mut test = candidate.clone();
        if let Candidate::Functional(t) = &mut test {
            t.value = Some(replacement.clone());
        }
        if signature_of(ds, signature_options, &test) == expected {
            found = Some(replacement);
            return true;
        }
        false
    });
    if let (Some(value), Candidate::Functional(c)) = (found, &mut candidate) {
        c.value = Some(value);
    }
    candidate
}

/// `tryValueReplacements`: the bare values that an arbitrary value can be.
/// `true` when the visitor stops.
fn try_value_replacements(
    value: &str,
    seen: &mut crate::hash::HashSet<String>,
    visit: &mut dyn FnMut(UtilityValue) -> bool,
) -> bool {
    if !seen.insert(value.to_string()) {
        return false;
    }
    let named = |value: &str, fraction: Option<String>| UtilityValue::Named { value: value.to_string(), fraction };

    // 0. No square brackets. 1. A number.
    if visit(named(value, None)) {
        return true;
    }
    // 2. A percentage: no `%`.
    if let Some(number) = value.strip_suffix('%') {
        if is_valid_spacing_multiplier(number) && visit(named(number, None)) {
            return true;
        }
    }
    // 3. A ratio of whole numbers.
    if value.contains('/') {
        let parts: Vec<&str> = value.split('/').collect();
        let (numerator, denominator) = (parts[0], parts.get(1).copied().unwrap_or(""));
        if is_positive_integer(numerator) && is_positive_integer(denominator) && visit(named(numerator, Some(format!("{numerator}/{denominator}")))) {
            return true;
        }
    }

    // The numbers and the fractions in the value: `20px` can be `20`. The
    // shortest first.
    let mut options: Vec<String> = Vec::new();
    for found in numbers_and_fractions(value) {
        let found = js_trim(found).to_string();
        if !options.contains(&found) {
            options.push(found);
        }
    }
    options.sort_by_key(|option| crate::utils::js::js_string_length(option));
    for option in options {
        if try_value_replacements(&option, seen, visit) {
            return true;
        }
    }
    false
}

/// The matches of `/(\d+\/\d+)|(\d+\.?\d+)/g`.
fn numbers_and_fractions(value: &str) -> Vec<&str> {
    let bytes = value.as_bytes();
    let digits = |from: usize| bytes[from..].iter().take_while(|b| b.is_ascii_digit()).count();
    let mut result = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let n = digits(i);
        if n == 0 {
            i += 1;
            continue;
        }
        // `\d+\/\d+`
        if bytes.get(i + n) == Some(&b'/') && digits(i + n + 1) > 0 {
            let end = i + n + 1 + digits(i + n + 1);
            result.push(&value[i..end]);
            i = end;
            continue;
        }
        // `\d+\.?\d+`
        if bytes.get(i + n) == Some(&b'.') && digits(i + n + 1) > 0 {
            let end = i + n + 1 + digits(i + n + 1);
            result.push(&value[i..end]);
            i = end;
            continue;
        }
        if n >= 2 {
            result.push(&value[i..i + n]);
            i += n;
            continue;
        }
        i += 1;
    }
    result
}

// ---- optimizeArbitraryValueExpressions

fn optimize_arbitrary_value_expressions(ds: &DesignSystem, options: InternalOptions, mut candidate: Candidate) -> Candidate {
    let Candidate::Functional(c) = &candidate else { return candidate };
    let Some(UtilityValue::Arbitrary { value, .. }) = &c.value else { return candidate };
    let signature_options = options.signature_options;

    let set_value = |candidate: &mut Candidate, text: String| {
        if let Candidate::Functional(c) = candidate {
            if let Some(UtilityValue::Arbitrary { value, .. }) = &mut c.value {
                *value = text;
            }
        }
    };
    let is_calc = |ast: &[ValueAstNode]| matches!(ast, [ValueAstNode::Function(f)] if f.value == "calc");

    let mut value_ast = vp::parse(value);

    // The fold of a `calc(…)` value. (Upstream changes the tree of the
    // value also when the result is not used.)
    if is_calc(&value_ast) {
        let (folded, folded_ast) = constant_fold_declaration_ast(value_ast, None, false);
        value_ast = folded_ast;
        if folded {
            let mut replacement = candidate.clone();
            set_value(&mut replacement, vp::to_css(&value_ast));
            if has_same_signature(ds, signature_options, Target::Candidate(&candidate), Target::Candidate(&replacement)) {
                candidate = replacement;
            }
        }
    }

    // The `-` sign goes into the arbitrary value.
    let root = match &candidate {
        Candidate::Functional(c) => c.root.clone(),
        _ => unreachable!(),
    };
    if root.starts_with('-') {
        // A `var(…)` stays as it is.
        if matches!(value_ast.as_slice(), [ValueAstNode::Function(f)] if f.value == "var") {
            return candidate;
        }
        let current = match &candidate {
            Candidate::Functional(c) => c.value.as_ref().map(|v| v.value().to_string()).unwrap_or_default(),
            _ => unreachable!(),
        };
        let (folded, folded_ast) = constant_fold_declaration_ast(vp::parse(&format!("calc({current} * -1)")), None, false);
        if folded {
            let mut replacement = candidate.clone();
            if let Candidate::Functional(r) = &mut replacement {
                r.root = r.root[1..].to_string();
            }
            set_value(&mut replacement, vp::to_css(&folded_ast));
            if has_same_signature(ds, signature_options, Target::Candidate(&candidate), Target::Candidate(&replacement)) {
                candidate = replacement;
                value_ast = folded_ast;
            }
        }
    }

    // The `-` sign goes out of the arbitrary value: `calc(arg * -1)` or
    // `calc(-1 * arg)`.
    if let [ValueAstNode::Function(calc)] = value_ast.as_slice() {
        if calc.value == "calc" {
            let args = &calc.nodes;
            let is = |node: &ValueAstNode, kind: &str, value: &str| node.kind() == kind && node.value() == value;
            if args.len() == 5 && is(&args[1], "separator", " ") && is(&args[2], "word", "*") && is(&args[3], "separator", " ") {
                let arg = if is(&args[4], "word", "-1") {
                    Some(&args[0])
                } else if is(&args[0], "word", "-1") {
                    Some(&args[4])
                } else {
                    None
                };
                if let Some(arg) = arg {
                    let root = match &candidate {
                        Candidate::Functional(c) => c.root.clone(),
                        _ => unreachable!(),
                    };
                    let mut replacement = candidate.clone();
                    if let Candidate::Functional(r) = &mut replacement {
                        r.root = format!("-{root}");
                    }
                    set_value(&mut replacement, vp::to_css(std::slice::from_ref(arg)));
                    if has_same_signature(ds, signature_options, Target::Candidate(&candidate), Target::Candidate(&replacement)) {
                        candidate = replacement;
                    }
                }
            }
        }
    }

    candidate
}

// ---- optimizeModifier

/// `/[25%]` is `/25`, `/[100%]` and `/100` are no modifier.
fn optimize_modifier(ds: &DesignSystem, options: InternalOptions, mut candidate: Candidate) -> Candidate {
    let Some(modifier) = modifier_of(&candidate).cloned() else { return candidate };
    if matches!(candidate, Candidate::Static(_)) {
        return candidate;
    }
    let signature_options = options.signature_options;
    let target = signature_of(ds, signature_options, &candidate);

    // 1. No modifier.
    if target == signature_of(ds, signature_options, &with_modifier(candidate.clone(), None)) {
        return with_modifier(candidate, None);
    }

    // 2. No square brackets and no `%`.
    let value = match modifier.value.strip_suffix('%') {
        Some(number) if modifier.value.contains('.') => num(js_number(number)),
        Some(number) => number.to_string(),
        None => modifier.value.clone(),
    };
    let new_modifier = CandidateModifier { kind: ValueKind::Named, value };
    if target == signature_of(ds, signature_options, &with_modifier(candidate.clone(), Some(new_modifier.clone()))) {
        return with_modifier(candidate, Some(new_modifier));
    }

    // 3. No square brackets, and the number times 100: `[0.16]` is `16`.
    let new_modifier = CandidateModifier { kind: ValueKind::Named, value: num(js_parse_float(&modifier.value) * 100.0) };
    if target == signature_of(ds, signature_options, &with_modifier(candidate.clone(), Some(new_modifier.clone()))) {
        candidate = with_modifier(candidate, Some(new_modifier));
    }
    candidate
}
