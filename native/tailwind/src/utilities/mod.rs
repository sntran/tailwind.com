//! The utilities: `utilities.ts` of Tailwind CSS.
//!
//! This file has the registry and the functions that make utilities. The
//! other files of this directory register the utilities, in the sequence
//! of `utilities.ts`.

pub mod css_utility;
mod part1;
mod part2;
mod part3;
mod part4;

use crate::ast::{at_rule, decl, AstNode};
use crate::candidate::{CandidateModifier, FunctionalCandidate, StaticCandidate, UtilityValue, ValueKind};
use crate::design_system::DesignSystem;
use crate::theme::Theme;
use crate::utils::infer_data_type::{is_positive_integer, is_valid_opacity_value, is_valid_spacing_multiplier};
use crate::utils::js::{js_number, js_number_to_string};
use crate::utils::math_operators::add_whitespace_around_math_operators;
use crate::utils::segment::segment;
use indexmap::IndexMap;
use std::rc::Rc;

/// True for a bare modifier that is the opacity of a shadow.
pub(crate) fn is_shadow_alpha(value: &str) -> bool {
    if crate::switches::SHADOW_FRACTIONAL_ALPHA.on() {
        is_valid_opacity_value(value)
    } else {
        is_positive_integer(value)
    }
}

/// A value with empty `var()` fallbacks, as the selected version writes
/// them: `var(--tw-blur, )`, and before 4.3.4 `var(--tw-blur,)`.
pub(crate) fn empty_fallbacks(text: &'static str) -> &'static str {
    if crate::switches::EMPTY_VAR_FALLBACK_SPACE.on() {
        return text;
    }
    thread_local! {
        static OLD: std::cell::RefCell<std::collections::HashMap<&'static str, &'static str>> = Default::default();
    }
    OLD.with(|old| *old.borrow_mut().entry(text).or_insert_with(|| Box::leak(text.replace(", )", ",)").into_boxed_str())))
}

/// The kind of a utility.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UtilityKind {
    Static,
    Functional,
}

/// The result of the compile function of a utility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Compiled {
    /// The declarations and rules of the utility.
    Nodes(Vec<AstNode>),
    /// The utility does not match this candidate (`undefined` upstream).
    Undefined,
    /// The candidate is not valid for this utility (`null` upstream).
    Null,
}

impl From<Option<Vec<AstNode>>> for Compiled {
    fn from(nodes: Option<Vec<AstNode>>) -> Self {
        match nodes {
            Some(nodes) => Compiled::Nodes(nodes),
            None => Compiled::Undefined,
        }
    }
}

pub type StaticCompileFn = Rc<dyn Fn(&StaticCandidate, &DesignSystem) -> Compiled>;
pub type FunctionalCompileFn = Rc<dyn Fn(&FunctionalCandidate, &DesignSystem) -> Compiled>;

/// The compile function of a utility.
#[derive(Clone)]
pub enum CompileFn {
    Static(StaticCompileFn),
    Functional(FunctionalCompileFn),
}

/// The options of a utility.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UtilityOptions {
    pub types: Vec<String>,
}

/// One registered utility.
#[derive(Clone)]
pub struct Utility {
    pub kind: UtilityKind,
    pub compile_fn: CompileFn,
    pub options: Option<UtilityOptions>,
}

/// The registry of the utilities, in the sequence of their registration.
#[derive(Default)]
pub struct Utilities {
    utilities: IndexMap<String, Vec<Utility>>,
}

impl Utilities {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a static utility. `None` from the function is `undefined`.
    pub fn r#static(
        &mut self,
        name: impl Into<String>,
        compile_fn: impl Fn(&StaticCandidate, &DesignSystem) -> Option<Vec<AstNode>> + 'static,
    ) {
        self.static_full(name, move |c, ds| compile_fn(c, ds).into());
    }

    /// Register a static utility whose function can return `Compiled::Null`.
    pub fn static_full(
        &mut self,
        name: impl Into<String>,
        compile_fn: impl Fn(&StaticCandidate, &DesignSystem) -> Compiled + 'static,
    ) {
        self.utilities.entry(name.into()).or_default().push(Utility {
            kind: UtilityKind::Static,
            compile_fn: CompileFn::Static(Rc::new(compile_fn)),
            options: None,
        });
    }

    /// Register a functional utility. `None` from the function is `undefined`.
    pub fn functional(
        &mut self,
        name: impl Into<String>,
        compile_fn: impl Fn(&FunctionalCandidate, &DesignSystem) -> Option<Vec<AstNode>> + 'static,
    ) {
        self.functional_full(name, move |c, ds| compile_fn(c, ds).into(), None);
    }

    /// Register a functional utility with options, whose function can
    /// return `Compiled::Null`.
    pub fn functional_full(
        &mut self,
        name: impl Into<String>,
        compile_fn: impl Fn(&FunctionalCandidate, &DesignSystem) -> Compiled + 'static,
        options: Option<UtilityOptions>,
    ) {
        self.utilities.entry(name.into()).or_default().push(Utility {
            kind: UtilityKind::Functional,
            compile_fn: CompileFn::Functional(Rc::new(compile_fn)),
            options,
        });
    }

    pub fn has(&self, name: &str, kind: UtilityKind) -> bool {
        self.utilities.get(name).is_some_and(|fns| fns.iter().any(|u| u.kind == kind))
    }

    pub fn get(&self, name: &str) -> &[Utility] {
        self.utilities.get(name).map_or(&[], |v| v.as_slice())
    }

    /// The names of the utilities of one kind.
    pub fn keys(&self, kind: UtilityKind) -> Vec<&str> {
        self.utilities
            .iter()
            .filter(|(_, fns)| fns.iter().any(|u| u.kind == kind))
            .map(|(k, _)| k.as_str())
            .collect()
    }
}

/// An `@property` rule. `None` for `syntax` is `"*"`.
pub fn property(ident: &str, initial_value: Option<&str>, syntax: Option<&str>) -> AstNode {
    let mut nodes = vec![
        decl("syntax", match syntax {
            Some(s) if !s.is_empty() => format!("\"{s}\""),
            _ => "\"*\"".to_string(),
        }),
        decl("inherits", "false"),
    ];
    if let Some(v) = initial_value.filter(|v| !v.is_empty()) {
        nodes.push(decl("initial-value", v));
    }
    at_rule("@property", ident, nodes)
}

/// `alpha` as a percentage when it is a number: `0.5` gives `50%`.
fn alpha_percentage(alpha: &str) -> String {
    let n = js_number(alpha);
    if n.is_nan() {
        alpha.to_string()
    } else {
        format!("{}%", js_number_to_string(n * 100.0))
    }
}

/// Apply an opacity to a color with `color-mix`.
pub fn with_alpha(value: &str, alpha: &str) -> String {
    let alpha = alpha_percentage(alpha);
    if alpha == "100%" {
        return value.to_string();
    }
    format!("color-mix(in oklab, {value} {alpha}, transparent)")
}

/// Replace the opacity of a color with the relative color syntax.
pub fn replace_alpha(value: &str, alpha: &str) -> String {
    let alpha = alpha_percentage(alpha);
    format!("oklab(from {value} l a b / {alpha})")
}

/// A color with the opacity of the modifier. `None` is an invalid modifier.
pub fn as_color(value: &str, modifier: Option<&CandidateModifier>, theme: &Theme) -> Option<String> {
    let modifier = match modifier {
        None => return Some(value.to_string()),
        Some(m) => m,
    };

    if modifier.kind == ValueKind::Arbitrary {
        return Some(with_alpha(value, &modifier.value));
    }

    // A named opacity of the theme: `bg-red-500/half`.
    if let Some(alpha) = theme.resolve(Some(&modifier.value), &["--opacity"]).filter(|a| !a.is_empty()) {
        return Some(with_alpha(value, &alpha));
    }

    if !is_valid_opacity_value(&modifier.value) {
        return None;
    }

    // The modifier is a percentage: `bg-red-500/50`.
    Some(with_alpha(value, &format!("{}%", modifier.value)))
}

/// The color of a named candidate value from the theme, with the opacity of
/// the modifier. `inherit`, `transparent` and `current` need no theme.
pub fn resolve_theme_color(candidate: &FunctionalCandidate, theme: &Theme, theme_keys: &[&str]) -> Option<String> {
    let name = match &candidate.value {
        Some(UtilityValue::Named { value, .. }) => value.as_str(),
        _ => panic!("resolve_theme_color needs a named value"),
    };
    let value = match name {
        "inherit" => Some("inherit".to_string()),
        "transparent" => Some("transparent".to_string()),
        "current" => Some("currentcolor".to_string()),
        _ => theme.resolve(Some(name), theme_keys),
    };
    match value {
        Some(v) if !v.is_empty() => as_color(&v, candidate.modifier.as_ref(), theme),
        _ => None,
    }
}

/// A part of a static utility: a declaration, or a function that makes a node.
pub enum StaticPart {
    Decl(&'static str, &'static str),
    Node(Box<dyn Fn() -> AstNode>),
}

/// Register a static utility such as `justify-center`.
pub fn static_utility(utilities: &mut Utilities, class_name: &str, declarations: Vec<StaticPart>) {
    utilities.r#static(class_name, move |_, _| {
        Some(
            declarations
                .iter()
                .map(|part| match part {
                    StaticPart::Decl(p, v) => decl(*p, *v),
                    StaticPart::Node(f) => f(),
                })
                .collect(),
        )
    });
}

/// Register a static utility that has only declarations.
pub fn static_decls(utilities: &mut Utilities, class_name: &str, declarations: &[(&'static str, &'static str)]) {
    let declarations = declarations.to_vec();
    utilities.r#static(class_name, move |_, _| Some(declarations.iter().map(|(p, v)| decl(*p, *v)).collect()));
}

/// The default value of a functional utility.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum DefaultValue {
    /// No `defaultValue` key upstream: the value of the theme key itself.
    #[default]
    Theme,
    /// `defaultValue: null` upstream: the utility needs a value.
    Null,
    /// A fixed value.
    Value(String),
}

pub type BareValueFn = Rc<dyn Fn(&NamedValue, &DesignSystem) -> Option<String>>;
pub type HandleFn = Rc<dyn Fn(&str, Option<&str>, &DesignSystem) -> Option<Vec<AstNode>>>;

/// A named value of a candidate, as the bare value functions get it.
#[derive(Debug, Clone, Copy)]
pub struct NamedValue<'a> {
    pub value: &'a str,
    pub fraction: Option<&'a str>,
}

/// The description of a functional utility that reads the theme.
#[derive(Clone)]
pub struct UtilityDescription {
    pub supports_negative: bool,
    pub supports_fractions: bool,
    pub theme_keys: Vec<&'static str>,
    pub default_value: DefaultValue,
    /// Fixed values: the name of the value, and the nodes.
    pub static_values: Vec<(&'static str, Vec<AstNode>)>,
    pub handle_bare_value: Option<BareValueFn>,
    pub handle_negative_bare_value: Option<BareValueFn>,
    /// The parameters are the value and the data type.
    pub handle: HandleFn,
}

impl UtilityDescription {
    /// A description with this `handle` and no other option.
    pub fn new(handle: impl Fn(&str, Option<&str>, &DesignSystem) -> Option<Vec<AstNode>> + 'static) -> Self {
        Self {
            supports_negative: false,
            supports_fractions: false,
            theme_keys: Vec::new(),
            default_value: DefaultValue::Theme,
            static_values: Vec::new(),
            handle_bare_value: None,
            handle_negative_bare_value: None,
            handle: Rc::new(handle),
        }
    }
    pub fn negative(mut self) -> Self {
        self.supports_negative = true;
        self
    }
    pub fn fractions(mut self) -> Self {
        self.supports_fractions = true;
        self
    }
    pub fn theme_keys(mut self, keys: &[&'static str]) -> Self {
        self.theme_keys = keys.to_vec();
        self
    }
    pub fn default_value(mut self, value: Option<&str>) -> Self {
        self.default_value = match value {
            Some(v) => DefaultValue::Value(v.to_string()),
            None => DefaultValue::Null,
        };
        self
    }
    pub fn static_values(mut self, values: Vec<(&'static str, Vec<AstNode>)>) -> Self {
        self.static_values = values;
        self
    }
    pub fn bare(mut self, f: impl Fn(&NamedValue, &DesignSystem) -> Option<String> + 'static) -> Self {
        self.handle_bare_value = Some(Rc::new(f));
        self
    }
    pub fn negative_bare(mut self, f: impl Fn(&NamedValue, &DesignSystem) -> Option<String> + 'static) -> Self {
        self.handle_negative_bare_value = Some(Rc::new(f));
        self
    }
}

fn handle_functional_utility(desc: UtilityDescription, negative: bool) -> impl Fn(&FunctionalCandidate, &DesignSystem) -> Option<Vec<AstNode>> {
    move |candidate, ds| {
        let theme = &ds.theme;
        let mut value: Option<String>;
        let mut data_type: Option<&str> = None;

        match &candidate.value {
            None => {
                if candidate.modifier.is_some() {
                    return None;
                }
                // The default value, or the bare theme value.
                value = match &desc.default_value {
                    DefaultValue::Theme => theme.resolve(None, &desc.theme_keys),
                    DefaultValue::Null => None,
                    DefaultValue::Value(v) => Some(v.clone()),
                };
            }
            Some(UtilityValue::Arbitrary { data_type: t, value: v }) => {
                if candidate.modifier.is_some() {
                    return None;
                }
                value = Some(v.clone());
                data_type = t.as_deref();
            }
            Some(UtilityValue::Named { value: named, fraction }) => {
                let named_value = NamedValue { value: named, fraction: fraction.as_deref() };
                value = theme.resolve(Some(fraction.as_deref().unwrap_or(named)), &desc.theme_keys);

                // A value of the theme does not take a modifier.
                if value.is_some()
                    && candidate.modifier.is_some()
                    && fraction.is_none()
                    && crate::switches::UNUSED_MODIFIER_IS_INVALID.on()
                {
                    return None;
                }

                // The value is a fraction: `w-1/2`.
                if value.is_none() && desc.supports_fractions {
                    if let Some(fraction) = fraction {
                        let parts = segment(fraction, b'/');
                        let (lhs, rhs) = (parts[0], parts.get(1).copied().unwrap_or(""));
                        if !is_positive_integer(lhs) || !is_positive_integer(rhs) {
                            return None;
                        }
                        // Before 4.2.0, the fraction had no spaces.
                        value = Some(if crate::switches::FRACTION_SPACES.on() {
                            format!("calc({lhs} / {rhs} * 100%)")
                        } else {
                            format!("calc({fraction} * 100%)")
                        });
                    }
                }

                // A bare value of a negative utility: `-mt-4`.
                if value.is_none() && negative {
                    if let Some(f) = &desc.handle_negative_bare_value {
                        value = f(&named_value, ds);
                        if !value.as_deref().is_some_and(|v| v.contains('/')) && candidate.modifier.is_some() {
                            return None;
                        }
                        if let Some(v) = &value {
                            return (desc.handle)(v, None, ds);
                        }
                    }
                }

                // A bare value: `mt-4`.
                if value.is_none() {
                    if let Some(f) = &desc.handle_bare_value {
                        value = f(&named_value, ds);
                        if !value.as_deref().is_some_and(|v| v.contains('/')) && candidate.modifier.is_some() {
                            return None;
                        }
                    }
                }

                // A fixed value.
                if value.is_none() && !negative && candidate.modifier.is_none() {
                    if let Some((_, nodes)) = desc.static_values.iter().find(|(k, _)| k == named) {
                        // Only in 4.1.14, each candidate got the same nodes.
                        if !crate::switches::STATIC_VALUE_COPIES.on() {
                            crate::compile::share_declarations(nodes, ds);
                        }
                        return Some(nodes.clone());
                    }
                }
            }
        }

        // No value: the candidate does not match.
        let value = value?;

        if negative {
            let negated = format!("calc({value} * -1)");
            if crate::switches::NEGATIVE_CALC_WHITESPACE.on() {
                (desc.handle)(&add_whitespace_around_math_operators(&negated), data_type, ds)
            } else {
                (desc.handle)(&negated, data_type, ds)
            }
        } else {
            (desc.handle)(&value, data_type, ds)
        }
    }
}

/// Register a functional utility such as `max-w-*` that reads the theme.
pub fn functional_utility(utilities: &mut Utilities, class_root: &str, mut desc: UtilityDescription) {
    if !crate::switches::STATIC_VALUES_AFTER_THEME.on() {
        // Before 4.1.14, each fixed value was a static utility: a theme
        // value with the same name did not replace it.
        for (key, nodes) in std::mem::take(&mut desc.static_values) {
            utilities.r#static(format!("{class_root}-{key}"), move |_, _| Some(nodes.clone()));
        }
    } else if !crate::switches::STATIC_VALUE_COPIES.on() {
        // Only in 4.1.14, the nodes of a fixed value were the same objects
        // for each candidate: see `compile::share_declarations`.
        for (_, nodes) in &mut desc.static_values {
            crate::ast::share_identity(nodes);
        }
    }
    if desc.supports_negative {
        utilities.functional(format!("-{class_root}"), handle_functional_utility(desc.clone(), true));
    }
    utilities.functional(class_root, handle_functional_utility(desc, false));
}

/// Register a functional utility for a color. Such a utility accepts an
/// opacity modifier.
pub fn color_utility(
    utilities: &mut Utilities,
    class_root: &str,
    theme_keys: &[&'static str],
    handle: impl Fn(&str, &DesignSystem) -> Option<Vec<AstNode>> + 'static,
) {
    let theme_keys = theme_keys.to_vec();
    utilities.functional(class_root, move |candidate, ds| {
        let value = match candidate.value.as_ref()? {
            UtilityValue::Arbitrary { value, .. } => as_color(value, candidate.modifier.as_ref(), &ds.theme),
            UtilityValue::Named { .. } => resolve_theme_color(candidate, &ds.theme, &theme_keys),
        }?;
        handle(&value, ds)
    });
}

/// The options of `spacing_utility`.
#[derive(Clone, Default)]
pub struct SpacingOptions {
    pub supports_negative: bool,
    pub supports_fractions: bool,
    pub static_values: Vec<(&'static str, Vec<AstNode>)>,
}

/// Register a utility that reads the spacing scale, such as `mt-*`.
pub fn spacing_utility(
    utilities: &mut Utilities,
    name: &str,
    theme_keys: &[&'static str],
    handle: impl Fn(&str, &DesignSystem) -> Option<Vec<AstNode>> + 'static,
    options: SpacingOptions,
) {
    let handle = Rc::new(handle);
    if options.supports_negative {
        let h = handle.clone();
        utilities.r#static(format!("-{name}-px"), move |_, ds| h("-1px", ds));
    }
    let h = handle.clone();
    utilities.r#static(format!("{name}-px"), move |_, ds| h("1px", ds));

    let h = handle.clone();
    let mut desc = UtilityDescription::new(move |value, _, ds| h(value, ds))
        .theme_keys(theme_keys)
        .default_value(None)
        .static_values(options.static_values)
        .bare(|v, ds| {
            ds.theme.resolve(None, &["--spacing"]).filter(|m| !m.is_empty())?;
            if !is_valid_spacing_multiplier(v.value) {
                return None;
            }
            Some(format!("--spacing({})", v.value))
        })
        .negative_bare(|v, ds| {
            ds.theme.resolve(None, &["--spacing"]).filter(|m| !m.is_empty())?;
            if !is_valid_spacing_multiplier(v.value) {
                return None;
            }
            Some(format!("--spacing(-{})", v.value))
        });
    desc.supports_fractions = options.supports_fractions;
    desc.supports_negative = options.supports_negative;
    functional_utility(utilities, name, desc);
}

/// All the utilities of Tailwind CSS.
pub fn create_utilities(theme: &Theme) -> Utilities {
    let mut utilities = Utilities::new();
    register_all(&mut utilities, theme);
    utilities
}

/// Register the utilities, in the sequence of `utilities.ts`.
fn register_all(utilities: &mut Utilities, theme: &Theme) {
    part1::register(utilities, theme);
    part2::register(utilities, theme);
    part3::register(utilities, theme);
    part4::register(utilities, theme);
}
