//! The utilities of older versions: `compat/legacy-utilities.ts` of
//! Tailwind CSS. Upstream registers them for each stylesheet.
//!
//! Upstream also hides these utilities from the suggestions of the editor
//! tools (`suggest`). This crate has no suggestions.

use crate::ast::{decl, AstNode};
use crate::candidate::{FunctionalCandidate, UtilityValue};
use crate::design_system::DesignSystem;
use crate::utils::infer_data_type::{is_positive_integer, is_valid_spacing_multiplier};
use crate::utils::segment::segment;

/// Register the utilities.
pub fn register_legacy_utilities(design_system: &mut DesignSystem) {
    let utilities = &mut design_system.utilities;

    for (value, direction) in [
        ("t", "top"),
        ("tr", "top right"),
        ("r", "right"),
        ("br", "bottom right"),
        ("b", "bottom"),
        ("bl", "bottom left"),
        ("l", "left"),
        ("tl", "top left"),
    ] {
        utilities.r#static(format!("bg-gradient-to-{value}"), move |_, _| {
            Some(vec![
                // (Before 4.0.2, the value had a comma at its end.)
                decl(
                    "--tw-gradient-position",
                    if crate::switches::GRADIENT_POSITION_NO_COMMA.on() {
                        format!("to {direction} in oklab")
                    } else {
                        format!("to {direction} in oklab,")
                    },
                ),
                decl("background-image", "linear-gradient(var(--tw-gradient-stops))"),
            ])
        });
    }

    let mut static_decls = |name: &str, declarations: &'static [(&'static str, &'static str)]| {
        utilities.r#static(name, move |_, _| Some(declarations.iter().map(|(p, v)| decl(*p, *v)).collect()));
    };

    // `background-position` utilities of v4.0 and before.
    static_decls("bg-left-top", &[("background-position", "left top")]);
    static_decls("bg-right-top", &[("background-position", "right top")]);
    static_decls("bg-left-bottom", &[("background-position", "left bottom")]);
    static_decls("bg-right-bottom", &[("background-position", "right bottom")]);

    // `object-position` utilities of v4.0 and before.
    static_decls("object-left-top", &[("object-position", "left top")]);
    static_decls("object-right-top", &[("object-position", "right top")]);
    static_decls("object-left-bottom", &[("object-position", "left bottom")]);
    static_decls("object-right-bottom", &[("object-position", "right bottom")]);

    utilities.functional("max-w-screen", |candidate, design_system| {
        let Some(UtilityValue::Named { value, .. }) = &candidate.value else { return None };
        let value = design_system.theme.resolve(Some(value), &["--breakpoint"]).filter(|v| !v.is_empty())?;
        Some(vec![decl("max-width", value)])
    });

    let mut static_decls = |name: &str, declarations: &'static [(&'static str, &'static str)]| {
        utilities.r#static(name, move |_, _| Some(declarations.iter().map(|(p, v)| decl(*p, *v)).collect()));
    };

    static_decls("overflow-ellipsis", &[("text-overflow", "ellipsis")]);

    static_decls("decoration-slice", &[("-webkit-box-decoration-break", "slice"), ("box-decoration-break", "slice")]);
    static_decls("decoration-clone", &[("-webkit-box-decoration-break", "clone"), ("box-decoration-break", "clone")]);

    for name in ["flex-shrink", "flex-grow"] {
        utilities.functional(name, move |candidate, _| {
            if candidate.modifier.is_some() {
                return None;
            }
            match &candidate.value {
                None => Some(vec![decl(name, "1")]),
                Some(UtilityValue::Arbitrary { value, .. }) => Some(vec![decl(name, value.clone())]),
                Some(UtilityValue::Named { value, .. }) if is_positive_integer(value) => Some(vec![decl(name, value.clone())]),
                Some(UtilityValue::Named { .. }) => None,
            }
        });

    }

    utilities.r#static("order-none", |_, _| Some(vec![decl("order", "0")]));
    utilities.r#static("break-words", |_, _| Some(vec![decl("overflow-wrap", "break-word")]));

    // The `start` and `end` inset utilities. `inset-s` and `inset-e`
    // replace them.
    // (Before 4.2.0, they were spacing utilities: `utilities/part1.rs`.)
    if !crate::switches::START_END_ARE_OLD_UTILITIES.on() {
        return;
    }
    for (name, property) in [("start", "inset-inline-start"), ("end", "inset-inline-end")] {
        utilities.r#static(format!("{name}-auto"), move |_, _| Some(vec![decl(property, "auto")]));
        utilities.r#static(format!("{name}-full"), move |_, _| Some(vec![decl(property, "100%")]));
        utilities.r#static(format!("-{name}-full"), move |_, _| Some(vec![decl(property, "-100%")]));
        utilities.r#static(format!("{name}-px"), move |_, _| Some(vec![decl(property, "1px")]));
        utilities.r#static(format!("-{name}-px"), move |_, _| Some(vec![decl(property, "-1px")]));

        utilities.functional(format!("-{name}"), move |candidate, design_system| {
            handle_inset(candidate, design_system, property, true)
        });
        utilities.functional(name, move |candidate, design_system| handle_inset(candidate, design_system, property, false));
    }
}

fn handle_inset(
    candidate: &FunctionalCandidate,
    design_system: &DesignSystem,
    property: &'static str,
    negative: bool,
) -> Option<Vec<AstNode>> {
    let negate = |value: String| if negative { format!("calc({value} * -1)") } else { value };

    // Before 4.3.0, `start` and `end` with no value gave the value of
    // `--inset` or `--spacing`.
    if candidate.value.is_none() && !crate::switches::LEGACY_INSET_NEEDS_VALUE.on() {
        if candidate.modifier.is_some() {
            return None;
        }
        let value = design_system.theme.resolve(None, &["--inset", "--spacing"])?;
        return Some(vec![decl(property, negate(value))]);
    }

    let (named, fraction) = match candidate.value.as_ref()? {
        UtilityValue::Arbitrary { value, .. } => {
            if candidate.modifier.is_some() {
                return None;
            }
            return Some(vec![decl(property, negate(value.clone()))]);
        }
        UtilityValue::Named { value, fraction } => (value.as_str(), fraction.as_deref()),
    };

    let theme = &design_system.theme;
    let mut value = theme.resolve(Some(fraction.unwrap_or(named)), &["--inset", "--spacing"]);

    // A fraction such as `start-1/2`.
    if value.is_none() {
        if let Some(fraction) = fraction.filter(|f| !f.is_empty()) {
            let parts = segment(fraction, b'/');
            let (lhs, rhs) = (parts.first().copied().unwrap_or(""), parts.get(1).copied().unwrap_or(""));
            if !is_positive_integer(lhs) || !is_positive_integer(rhs) {
                return None;
            }
            value = Some(format!("calc({fraction} * 100%)"));
        }
    }

    // A bare multiplier of the spacing scale such as `-start-4`.
    if value.is_none() && negative {
        let multiplier = theme.resolve(None, &["--spacing"]).filter(|m| !m.is_empty());
        if let Some(multiplier) = multiplier {
            if is_valid_spacing_multiplier(named) {
                return Some(vec![decl(property, format!("calc({multiplier} * -{named})"))]);
            }
        }
    }

    // A bare multiplier of the spacing scale such as `start-4`.
    if value.is_none() {
        let multiplier = theme.resolve(None, &["--spacing"]).filter(|m| !m.is_empty());
        if let Some(multiplier) = multiplier {
            if is_valid_spacing_multiplier(named) {
                value = Some(format!("calc({multiplier} * {named})"));
            }
        }
    }

    Some(vec![decl(property, negate(value?))])
}
