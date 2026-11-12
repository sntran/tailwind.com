//! A part of the utilities of `utilities.ts` of Tailwind CSS: from `sr-only`
//! to `grid-rows`.

use super::{functional_utility, property, spacing_utility, NamedValue, SpacingOptions, Utilities, UtilityDescription};
use crate::ast::{at_root, at_rule, decl, AstNode};
use crate::candidate::{FunctionalCandidate, UtilityValue};
use crate::design_system::DesignSystem;
use crate::switches;
use crate::theme::Theme;
use crate::utils::compare_breakpoints::{compare_breakpoints, Direction};
use crate::utils::infer_data_type::{
    infer_data_type, is_positive_integer, is_strict_positive_integer, is_valid_spacing_multiplier, DataType,
};
use crate::utils::segment::segment;

/// Register a static utility that has only declarations.
fn st(utilities: &mut Utilities, name: &str, declarations: &[(&str, &str)]) {
    let declarations: Vec<(String, String)> =
        declarations.iter().map(|(p, v)| (p.to_string(), v.to_string())).collect();
    utilities.r#static(name, move |_, _| Some(declarations.iter().map(|(p, v)| decl(p.as_str(), v.as_str())).collect()));
}

/// Register a static utility that has a node of `first`, then declarations.
fn st_with(utilities: &mut Utilities, name: &str, first: fn() -> AstNode, declarations: &[(&str, &str)]) {
    let declarations: Vec<(String, String)> =
        declarations.iter().map(|(p, v)| (p.to_string(), v.to_string())).collect();
    utilities.r#static(name, move |_, _| {
        let mut nodes = vec![first()];
        nodes.extend(declarations.iter().map(|(p, v)| decl(p.as_str(), v.as_str())));
        Some(nodes)
    });
}

/// A bare value that is a positive integer.
fn integer(value: &NamedValue, _: &DesignSystem) -> Option<String> {
    is_positive_integer(value.value).then(|| value.value.to_string())
}

/// A bare value that is a positive integer, with a unit.
fn integer_with(unit: &'static str) -> impl Fn(&NamedValue, &DesignSystem) -> Option<String> {
    move |value, _| is_positive_integer(value.value).then(|| format!("{}{unit}", value.value))
}

/// A bare value that is a multiplier of the spacing scale.
fn spacing_multiplier(value: &NamedValue, ds: &DesignSystem) -> Option<String> {
    ds.theme.resolve(None, &["--spacing"]).filter(|m| !m.is_empty())?;
    if !is_valid_spacing_multiplier(value.value) {
        return None;
    }
    Some(format!("--spacing({})", value.value))
}

/// A bare value of `auto-rows-*` and `auto-cols-*`.
fn auto_track_spacing(value: &NamedValue, ds: &DesignSystem) -> Option<String> {
    if !crate::switches::AUTO_TRACKS_BARE_SPACING.on() {
        return None;
    }
    spacing_multiplier(value, ds)
}

/// A bare value for `grid-cols-*` and `grid-rows-*`.
fn grid_repeat(value: &NamedValue, _: &DesignSystem) -> Option<String> {
    is_strict_positive_integer(value.value).then(|| format!("repeat({}, minmax(0, 1fr))", value.value))
}

/// A description that sets one property.
/// `col-*` and `row-*` take an integer and a negative value.
/// (Before 4.0.10, only a theme value or an arbitrary value.)
fn grid_line_numbers(description: UtilityDescription) -> UtilityDescription {
    if switches::GRID_LINE_NUMBERS.on() {
        description.negative().bare(integer)
    } else {
        description
    }
}

fn one_property(property: &'static str) -> UtilityDescription {
    UtilityDescription::new(move |value, _, _| Some(vec![decl(property, value)]))
}

/// Fixed values that set one property.
fn values(property: &'static str, values: &[(&'static str, &'static str)]) -> Vec<(&'static str, Vec<AstNode>)> {
    values.iter().map(|(name, value)| (*name, vec![decl(property, *value)])).collect()
}

fn negative() -> SpacingOptions {
    SpacingOptions { supports_negative: true, ..Default::default() }
}

fn fractions() -> SpacingOptions {
    SpacingOptions { supports_fractions: true, ..Default::default() }
}

fn negative_fractions() -> SpacingOptions {
    SpacingOptions { supports_negative: true, supports_fractions: true, ..Default::default() }
}

fn border_spacing_properties() -> AstNode {
    at_root(vec![
        property("--tw-border-spacing-x", Some("0"), Some("<length>")),
        property("--tw-border-spacing-y", Some("0"), Some("<length>")),
    ])
}

fn translate_properties() -> AstNode {
    at_root(vec![
        property("--tw-translate-x", Some("0"), None),
        property("--tw-translate-y", Some("0"), None),
        property("--tw-translate-z", Some("0"), None),
    ])
}

fn scale_properties() -> AstNode {
    at_root(vec![
        property("--tw-scale-x", Some("1"), None),
        property("--tw-scale-y", Some("1"), None),
        property("--tw-scale-z", Some("1"), None),
    ])
}

fn transform_properties() -> AstNode {
    // Before 4.1.4, each property had an initial value.
    let initial = |value: &'static str| (!switches::TRANSFORM_EMPTY_FALLBACKS.on()).then_some(value);
    at_root(vec![
        property("--tw-rotate-x", initial("rotateX(0)"), None),
        property("--tw-rotate-y", initial("rotateY(0)"), None),
        property("--tw-rotate-z", initial("rotateZ(0)"), None),
        property("--tw-skew-x", initial("skewX(0)"), None),
        property("--tw-skew-y", initial("skewY(0)"), None),
    ])
}

fn touch_properties() -> AstNode {
    at_root(vec![
        property("--tw-pan-x", None, None),
        property("--tw-pan-y", None, None),
        property("--tw-pinch-zoom", None, None),
    ])
}

fn snap_properties() -> AstNode {
    at_root(vec![property("--tw-scroll-snap-strictness", Some("proximity"), Some("*"))])
}

const TRANSFORM_VALUE: &str =
    "var(--tw-rotate-x, ) var(--tw-rotate-y, ) var(--tw-rotate-z, ) var(--tw-skew-x, ) var(--tw-skew-y, )";

/// The value of `transform` before 4.1.4: no empty fallbacks.
const TRANSFORM_VALUE_NO_FALLBACKS: &str =
    "var(--tw-rotate-x) var(--tw-rotate-y) var(--tw-rotate-z) var(--tw-skew-x) var(--tw-skew-y)";

/// The value of `transform`, as the selected version writes it.
fn transform_value() -> &'static str {
    if switches::TRANSFORM_EMPTY_FALLBACKS.on() {
        super::empty_fallbacks(TRANSFORM_VALUE)
    } else {
        TRANSFORM_VALUE_NO_FALLBACKS
    }
}

const TOUCH_ACTION: &str = "var(--tw-pan-x, ) var(--tw-pan-y, ) var(--tw-pinch-zoom, )";

const TRANSLATE_2D: &str = "var(--tw-translate-x) var(--tw-translate-y)";

fn negate(value: String, negative: bool) -> String {
    if negative {
        format!("calc({value} * -1)")
    } else {
        value
    }
}

/// The compile function of `scale-*` and `-scale-*`.
fn handle_scale(negative: bool) -> impl Fn(&FunctionalCandidate, &DesignSystem) -> Option<Vec<AstNode>> {
    move |candidate, ds| {
        if candidate.modifier.is_some() {
            return None;
        }
        let value = match candidate.value.as_ref()? {
            UtilityValue::Arbitrary { value, .. } => {
                let negative = negative && switches::SCALE_NEGATIVE_ARBITRARY.on();
                return Some(vec![decl("scale", negate(value.clone(), negative))]);
            }
            UtilityValue::Named { value: named, .. } => {
                let mut value = ds.theme.resolve(Some(named), &["--scale"]).filter(|v| !v.is_empty());
                if value.is_none() && is_positive_integer(named) {
                    value = Some(format!("{named}%"));
                }
                value?
            }
        };
        let value = negate(value, negative);
        Some(vec![
            scale_properties(),
            decl("--tw-scale-x", value.as_str()),
            decl("--tw-scale-y", value.as_str()),
            decl("--tw-scale-z", value.as_str()),
            decl("scale", "var(--tw-scale-x) var(--tw-scale-y)"),
        ])
    }
}

/// The compile function of `rotate-*` and `-rotate-*`.
fn handle_rotate(negative: bool) -> impl Fn(&FunctionalCandidate, &DesignSystem) -> Option<Vec<AstNode>> {
    move |candidate, ds| {
        if candidate.modifier.is_some() {
            return None;
        }
        let value = match candidate.value.as_ref()? {
            UtilityValue::Arbitrary { value, data_type } => {
                let is_vector = match data_type {
                    Some(data_type) => data_type == "vector",
                    None => infer_data_type(value, &[DataType::Angle, DataType::Vector]) == Some(DataType::Vector),
                };
                if is_vector {
                    return Some(vec![decl("rotate", format!("{value} var(--tw-rotate)"))]);
                }
                if !switches::ROTATE_NEGATIVE_ARBITRARY.on() {
                    let is_angle = match data_type {
                        Some(data_type) => data_type == "angle",
                        None => infer_data_type(value, &[DataType::Angle, DataType::Vector]) == Some(DataType::Angle),
                    };
                    if !is_angle {
                        return Some(vec![decl("rotate", value.as_str())]);
                    }
                }
                value.clone()
            }
            UtilityValue::Named { value: named, .. } => {
                let mut value = ds.theme.resolve(Some(named), &["--rotate"]).filter(|v| !v.is_empty());
                if value.is_none() && is_positive_integer(named) {
                    value = Some(format!("{named}deg"));
                }
                value?
            }
        };
        Some(vec![decl("rotate", negate(value, negative))])
    }
}

/// Register the utilities of this part.
/// The utilities of `inline-size`, `block-size` and their `min-` and `max-`
/// properties.
fn register_logical_sizes(u: &mut Utilities) {
    for (key, value) in [("full", "100%"), ("min", "min-content"), ("max", "max-content"), ("fit", "fit-content")] {

        st(u, &format!("inline-{key}"), &[("inline-size", value)]);
        st(u, &format!("block-{key}"), &[("block-size", value)]);
        st(u, &format!("min-inline-{key}"), &[("min-inline-size", value)]);
        st(u, &format!("min-block-{key}"), &[("min-block-size", value)]);
        st(u, &format!("max-inline-{key}"), &[("max-inline-size", value)]);
        st(u, &format!("max-block-{key}"), &[("max-block-size", value)]);
    }

    for (key, value) in [("svw", "100svw"), ("lvw", "100lvw"), ("dvw", "100dvw")] {
        st(u, &format!("inline-{key}"), &[("inline-size", value)]);
        st(u, &format!("min-inline-{key}"), &[("min-inline-size", value)]);
        st(u, &format!("max-inline-{key}"), &[("max-inline-size", value)]);
    }

    for (key, value) in [("svh", "100svh"), ("lvh", "100lvh"), ("dvh", "100dvh")] {
        st(u, &format!("block-{key}"), &[("block-size", value)]);
        st(u, &format!("min-block-{key}"), &[("min-block-size", value)]);
        st(u, &format!("max-block-{key}"), &[("max-block-size", value)]);
    }

    st(u, "inline-auto", &[("inline-size", "auto")]);
    st(u, "block-auto", &[("block-size", "auto")]);
    st(u, "min-inline-auto", &[("min-inline-size", "auto")]);
    st(u, "min-block-auto", &[("min-block-size", "auto")]);

    st(u, "block-lh", &[("block-size", "1lh")]);
    st(u, "min-block-lh", &[("min-block-size", "1lh")]);
    st(u, "max-block-lh", &[("max-block-size", "1lh")]);

    st(u, "inline-screen", &[("inline-size", "100vw")]);
    st(u, "min-inline-screen", &[("min-inline-size", "100vw")]);
    st(u, "max-inline-screen", &[("max-inline-size", "100vw")]);
    st(u, "block-screen", &[("block-size", "100vh")]);
    st(u, "min-block-screen", &[("min-block-size", "100vh")]);
    st(u, "max-block-screen", &[("max-block-size", "100vh")]);

    st(u, "max-inline-none", &[("max-inline-size", "none")]);
    st(u, "max-block-none", &[("max-block-size", "none")]);

    let logical_sizes: [(&str, &[&'static str], &'static str); 6] = [
        ("inline", &["--spacing", "--container"], "inline-size"),
        ("min-inline", &["--spacing", "--container"], "min-inline-size"),
        ("max-inline", &["--spacing", "--container"], "max-inline-size"),
        ("block", &["--spacing"], "block-size"),
        ("min-block", &["--spacing"], "min-block-size"),
        ("max-block", &["--spacing"], "max-block-size"),
    ];
    for (name, namespaces, prop) in logical_sizes {
        spacing_utility(u, name, namespaces, move |value, _| Some(vec![decl(prop, value)]), fractions());
    }
}

pub fn register(utilities: &mut Utilities, _theme: &Theme) {
    let u = utilities;

    // Before 4.1.13, `sr-only` and `not-sr-only` had the `clip` property.
    let clip_path = crate::switches::SR_ONLY_CLIP_PATH.on();
    st(
        u,
        "sr-only",
        &[
            ("position", "absolute"),
            ("width", "1px"),
            ("height", "1px"),
            ("padding", "0"),
            ("margin", "-1px"),
            ("overflow", "hidden"),
            if clip_path { ("clip-path", "inset(50%)") } else { ("clip", "rect(0, 0, 0, 0)") },
            ("white-space", "nowrap"),
            ("border-width", "0"),
        ],
    );
    st(
        u,
        "not-sr-only",
        &[
            ("position", "static"),
            ("width", "auto"),
            ("height", "auto"),
            ("padding", "0"),
            ("margin", "0"),
            ("overflow", "visible"),
            if clip_path { ("clip-path", "none") } else { ("clip", "auto") },
            ("white-space", "normal"),
        ],
    );

    // `pointer-events`
    st(u, "pointer-events-none", &[("pointer-events", "none")]);
    st(u, "pointer-events-auto", &[("pointer-events", "auto")]);

    // `visibility`
    st(u, "visible", &[("visibility", "visible")]);
    st(u, "invisible", &[("visibility", "hidden")]);
    st(u, "collapse", &[("visibility", "collapse")]);

    // `position`
    st(u, "static", &[("position", "static")]);
    st(u, "fixed", &[("position", "fixed")]);
    st(u, "absolute", &[("position", "absolute")]);
    st(u, "relative", &[("position", "relative")]);
    st(u, "sticky", &[("position", "sticky")]);

    // The utilities of 4.2.0 (`switches::LOGICAL_UTILITIES`).
    let logical_utilities = crate::switches::LOGICAL_UTILITIES.on();
    // Before 4.2.0, `start` and `end` were spacing utilities as the others
    // of this list. Now they are in `compat/legacy_utilities.rs`.
    let start_end_here = !crate::switches::START_END_ARE_OLD_UTILITIES.on();

    // `inset`
    for (name, prop) in [
        ("inset", "inset"),
        ("inset-x", "inset-inline"),
        ("inset-y", "inset-block"),
        ("start", "inset-inline-start"),
        ("end", "inset-inline-end"),
        ("inset-s", "inset-inline-start"),
        ("inset-e", "inset-inline-end"),
        ("inset-bs", "inset-block-start"),
        ("inset-be", "inset-block-end"),
        ("top", "top"),
        ("right", "right"),
        ("bottom", "bottom"),
        ("left", "left"),
    ] {
        match name {
            "start" | "end" if !start_end_here => continue,
            "inset-s" | "inset-e" | "inset-bs" | "inset-be" if !logical_utilities => continue,
            _ => {}
        }
        st(u, &format!("{name}-auto"), &[(prop, "auto")]);
        st(u, &format!("{name}-full"), &[(prop, "100%")]);
        st(u, &format!("-{name}-full"), &[(prop, "-100%")]);
        spacing_utility(u, name, &["--inset", "--spacing"], move |value, _| Some(vec![decl(prop, value)]), negative_fractions());
    }

    // `isolation`
    st(u, "isolate", &[("isolation", "isolate")]);
    st(u, "isolation-auto", &[("isolation", "auto")]);

    // `z-index`
    functional_utility(
        u,
        "z",
        one_property("z-index")
            .negative()
            .bare(integer)
            .theme_keys(&["--z-index"])
            .static_values(values("z-index", &[("auto", "auto")])),
    );

    // `order`
    functional_utility(
        u,
        "order",
        one_property("order")
            .negative()
            .bare(integer)
            .theme_keys(&["--order"])
            .static_values(values("order", &[("first", "-9999"), ("last", "9999")])),
    );

    // `grid-column`
    functional_utility(
        u,
        "col",
        grid_line_numbers(one_property("grid-column"))
            .theme_keys(&["--grid-column"])
            .static_values(values("grid-column", &[("auto", "auto")])),
    );
    functional_utility(
        u,
        "col-span",
        UtilityDescription::new(|value, _, _| Some(vec![decl("grid-column", format!("span {value} / span {value}"))]))
            .bare(integer)
            .static_values(values("grid-column", &[("full", "1 / -1")])),
    );
    functional_utility(
        u,
        "col-start",
        one_property("grid-column-start")
            .negative()
            .bare(integer)
            .theme_keys(&["--grid-column-start"])
            .static_values(values("grid-column-start", &[("auto", "auto")])),
    );
    functional_utility(
        u,
        "col-end",
        one_property("grid-column-end")
            .negative()
            .bare(integer)
            .theme_keys(&["--grid-column-end"])
            .static_values(values("grid-column-end", &[("auto", "auto")])),
    );

    // `grid-row`
    functional_utility(
        u,
        "row",
        grid_line_numbers(one_property("grid-row"))
            .theme_keys(&["--grid-row"])
            .static_values(values("grid-row", &[("auto", "auto")])),
    );
    functional_utility(
        u,
        "row-span",
        UtilityDescription::new(|value, _, _| Some(vec![decl("grid-row", format!("span {value} / span {value}"))]))
            .bare(integer)
            .static_values(values("grid-row", &[("full", "1 / -1")])),
    );
    functional_utility(
        u,
        "row-start",
        one_property("grid-row-start")
            .negative()
            .bare(integer)
            .theme_keys(&["--grid-row-start"])
            .static_values(values("grid-row-start", &[("auto", "auto")])),
    );
    functional_utility(
        u,
        "row-end",
        one_property("grid-row-end")
            .negative()
            .bare(integer)
            .theme_keys(&["--grid-row-end"])
            .static_values(values("grid-row-end", &[("auto", "auto")])),
    );

    // `float`
    st(u, "float-start", &[("float", "inline-start")]);
    st(u, "float-end", &[("float", "inline-end")]);
    st(u, "float-right", &[("float", "right")]);
    st(u, "float-left", &[("float", "left")]);
    st(u, "float-none", &[("float", "none")]);

    // `clear`
    st(u, "clear-start", &[("clear", "inline-start")]);
    st(u, "clear-end", &[("clear", "inline-end")]);
    st(u, "clear-right", &[("clear", "right")]);
    st(u, "clear-left", &[("clear", "left")]);
    st(u, "clear-both", &[("clear", "both")]);
    st(u, "clear-none", &[("clear", "none")]);

    // `margin`
    for (namespace, prop) in [
        ("m", "margin"),
        ("mx", "margin-inline"),
        ("my", "margin-block"),
        ("ms", "margin-inline-start"),
        ("me", "margin-inline-end"),
        ("mbs", "margin-block-start"),
        ("mbe", "margin-block-end"),
        ("mt", "margin-top"),
        ("mr", "margin-right"),
        ("mb", "margin-bottom"),
        ("ml", "margin-left"),
    ] {
        if matches!(namespace, "mbs" | "mbe") && !logical_utilities {
            continue;
        }
        st(u, &format!("{namespace}-auto"), &[(prop, "auto")]);
        spacing_utility(u, namespace, &["--margin", "--spacing"], move |value, _| Some(vec![decl(prop, value)]), negative());
    }

    // `box-sizing`
    st(u, "box-border", &[("box-sizing", "border-box")]);
    st(u, "box-content", &[("box-sizing", "content-box")]);

    // `line-clamp`
    functional_utility(
        u,
        "line-clamp",
        UtilityDescription::new(|value, _, _| {
            Some(vec![
                decl("overflow", "hidden"),
                decl("display", "-webkit-box"),
                decl("-webkit-box-orient", "vertical"),
                decl("-webkit-line-clamp", value),
            ])
        })
        .theme_keys(&["--line-clamp"])
        .bare(integer)
        .static_values(vec![(
            "none",
            vec![
                decl("overflow", "visible"),
                decl("display", "block"),
                decl("-webkit-box-orient", "horizontal"),
                decl("-webkit-line-clamp", "unset"),
            ],
        )]),
    );

    // `display`
    st(u, "block", &[("display", "block")]);
    st(u, "inline-block", &[("display", "inline-block")]);
    st(u, "inline", &[("display", "inline")]);
    st(u, "hidden", &[("display", "none")]);
    st(u, "inline-flex", &[("display", "inline-flex")]);
    st(u, "table", &[("display", "table")]);
    st(u, "inline-table", &[("display", "inline-table")]);
    st(u, "table-caption", &[("display", "table-caption")]);
    st(u, "table-cell", &[("display", "table-cell")]);
    st(u, "table-column", &[("display", "table-column")]);
    st(u, "table-column-group", &[("display", "table-column-group")]);
    st(u, "table-footer-group", &[("display", "table-footer-group")]);
    st(u, "table-header-group", &[("display", "table-header-group")]);
    st(u, "table-row-group", &[("display", "table-row-group")]);
    st(u, "table-row", &[("display", "table-row")]);
    st(u, "flow-root", &[("display", "flow-root")]);
    st(u, "flex", &[("display", "flex")]);
    st(u, "grid", &[("display", "grid")]);
    st(u, "inline-grid", &[("display", "inline-grid")]);
    st(u, "contents", &[("display", "contents")]);
    st(u, "list-item", &[("display", "list-item")]);

    // `field-sizing`
    st(u, "field-sizing-content", &[("field-sizing", "content")]);
    st(u, "field-sizing-fixed", &[("field-sizing", "fixed")]);

    // `aspect-ratio`
    functional_utility(
        u,
        "aspect",
        one_property("aspect-ratio")
            .theme_keys(&["--aspect"])
            .bare(|value, _| {
                let fraction = value.fraction?;
                let parts = segment(fraction, b'/');
                let (lhs, rhs) = (parts[0], parts.get(1).copied().unwrap_or(""));
                // Before 4.2.0, each part was an integer.
                let is_valid_part =
                    if crate::switches::ASPECT_FRACTION_DECIMALS.on() { is_valid_spacing_multiplier } else { is_positive_integer };
                if !is_valid_part(lhs) || !is_valid_part(rhs) {
                    return None;
                }
                Some(fraction.to_string())
            })
            .static_values(values("aspect-ratio", &[("auto", "auto"), ("square", "1 / 1")])),
    );

    // `size`, `width`, `height` and their `min-` and `max-` properties
    for (key, value) in [
        ("full", "100%"),
        ("svw", "100svw"),
        ("lvw", "100lvw"),
        ("dvw", "100dvw"),
        ("svh", "100svh"),
        ("lvh", "100lvh"),
        ("dvh", "100dvh"),
        ("min", "min-content"),
        ("max", "max-content"),
        ("fit", "fit-content"),
    ] {
        st(u, &format!("size-{key}"), &[("--tw-sort", "size"), ("width", value), ("height", value)]);
        st(u, &format!("w-{key}"), &[("width", value)]);
        st(u, &format!("h-{key}"), &[("height", value)]);
        st(u, &format!("min-w-{key}"), &[("min-width", value)]);
        st(u, &format!("min-h-{key}"), &[("min-height", value)]);
        st(u, &format!("max-w-{key}"), &[("max-width", value)]);
        st(u, &format!("max-h-{key}"), &[("max-height", value)]);
    }

    st(u, "size-auto", &[("--tw-sort", "size"), ("width", "auto"), ("height", "auto")]);
    st(u, "w-auto", &[("width", "auto")]);
    st(u, "h-auto", &[("height", "auto")]);
    st(u, "min-w-auto", &[("min-width", "auto")]);
    st(u, "min-h-auto", &[("min-height", "auto")]);
    // Before 4.0.10, also `max-w-auto` and `max-h-auto`: `auto` is not a
    // value of these properties.
    if !switches::NO_MAX_SIZE_AUTO.on() {
        st(u, "max-w-auto", &[("max-width", "auto")]);
        st(u, "max-h-auto", &[("max-height", "auto")]);
    }

    if switches::HEIGHT_LH.on() {
        st(u, "h-lh", &[("height", "1lh")]);
        st(u, "min-h-lh", &[("min-height", "1lh")]);
        st(u, "max-h-lh", &[("max-height", "1lh")]);
    }

    st(u, "w-screen", &[("width", "100vw")]);
    st(u, "min-w-screen", &[("min-width", "100vw")]);
    st(u, "max-w-screen", &[("max-width", "100vw")]);
    st(u, "h-screen", &[("height", "100vh")]);
    st(u, "min-h-screen", &[("min-height", "100vh")]);
    st(u, "max-h-screen", &[("max-height", "100vh")]);

    st(u, "max-w-none", &[("max-width", "none")]);
    st(u, "max-h-none", &[("max-height", "none")]);

    spacing_utility(
        u,
        "size",
        &["--size", "--spacing"],
        |value, _| Some(vec![decl("--tw-sort", "size"), decl("width", value), decl("height", value)]),
        fractions(),
    );

    let sizes: [(&str, &[&'static str], &'static str); 6] = [
        ("w", &["--width", "--spacing", "--container"], "width"),
        ("min-w", &["--min-width", "--spacing", "--container"], "min-width"),
        ("max-w", &["--max-width", "--spacing", "--container"], "max-width"),
        ("h", &["--height", "--spacing"], "height"),
        ("min-h", &["--min-height", "--height", "--spacing"], "min-height"),
        ("max-h", &["--max-height", "--height", "--spacing"], "max-height"),
    ];
    for (name, namespaces, prop) in sizes {
        spacing_utility(u, name, namespaces, move |value, _| Some(vec![decl(prop, value)]), fractions());
    }

    // `inline-size`, `block-size` and their `min-` and `max-` properties
    if logical_utilities {
        register_logical_sizes(u);
    }

    u.r#static("container", |_, ds| {
        let mut breakpoints: Vec<String> = ds.theme.namespace("--breakpoint").into_values().collect();
        breakpoints.sort_by(|a, z| compare_breakpoints(a, z, Direction::Asc));

        let mut decls = vec![decl("--tw-sort", "--tw-container-component"), decl("width", "100%")];
        for breakpoint in breakpoints {
            decls.push(at_rule("@media", format!("(width >= {breakpoint})"), vec![decl("max-width", breakpoint)]));
        }
        Some(decls)
    });

    // `flex`
    st(u, "flex-auto", &[("flex", "auto")]);
    st(u, "flex-initial", &[("flex", "0 auto")]);
    st(u, "flex-none", &[("flex", "none")]);

    // `flex` is `display: flex`, and `flex-1` is `flex: 1`.
    u.functional("flex", |candidate, _| match candidate.value.as_ref()? {
        UtilityValue::Arbitrary { value, .. } => {
            if candidate.modifier.is_some() {
                return None;
            }
            Some(vec![decl("flex", value.as_str())])
        }
        UtilityValue::Named { value, fraction } => {
            if let Some(fraction) = fraction.as_deref().filter(|f| !f.is_empty()) {
                let parts = segment(fraction, b'/');
                let (lhs, rhs) = (parts[0], parts.get(1).copied().unwrap_or(""));
                if !is_positive_integer(lhs) || !is_positive_integer(rhs) {
                    return None;
                }
                return Some(vec![decl("flex", format!("calc({fraction} * 100%)"))]);
            }
            if is_positive_integer(value) {
                if candidate.modifier.is_some() {
                    return None;
                }
                return Some(vec![decl("flex", value.as_str())]);
            }
            None
        }
    });

    // `flex-shrink`
    functional_utility(u, "shrink", one_property("flex-shrink").default_value(Some("1")).bare(integer));

    // `flex-grow`
    functional_utility(u, "grow", one_property("flex-grow").default_value(Some("1")).bare(integer));

    // `flex-basis`
    st(u, "basis-auto", &[("flex-basis", "auto")]);
    st(u, "basis-full", &[("flex-basis", "100%")]);
    spacing_utility(
        u,
        "basis",
        &["--flex-basis", "--spacing", "--container"],
        |value, _| Some(vec![decl("flex-basis", value)]),
        fractions(),
    );

    // `table-layout`
    st(u, "table-auto", &[("table-layout", "auto")]);
    st(u, "table-fixed", &[("table-layout", "fixed")]);

    // `caption-side`
    st(u, "caption-top", &[("caption-side", "top")]);
    st(u, "caption-bottom", &[("caption-side", "bottom")]);

    // `border-collapse`
    st(u, "border-collapse", &[("border-collapse", "collapse")]);
    st(u, "border-separate", &[("border-collapse", "separate")]);

    // `border-spacing`
    const BORDER_SPACING: &str = "var(--tw-border-spacing-x) var(--tw-border-spacing-y)";
    spacing_utility(
        u,
        "border-spacing",
        &["--border-spacing", "--spacing"],
        |value, _| {
            Some(vec![
                border_spacing_properties(),
                decl("--tw-border-spacing-x", value),
                decl("--tw-border-spacing-y", value),
                decl("border-spacing", BORDER_SPACING),
            ])
        },
        SpacingOptions::default(),
    );
    spacing_utility(
        u,
        "border-spacing-x",
        &["--border-spacing", "--spacing"],
        |value, _| {
            Some(vec![
                border_spacing_properties(),
                decl("--tw-border-spacing-x", value),
                decl("border-spacing", BORDER_SPACING),
            ])
        },
        SpacingOptions::default(),
    );
    spacing_utility(
        u,
        "border-spacing-y",
        &["--border-spacing", "--spacing"],
        |value, _| {
            Some(vec![
                border_spacing_properties(),
                decl("--tw-border-spacing-y", value),
                decl("border-spacing", BORDER_SPACING),
            ])
        },
        SpacingOptions::default(),
    );

    // `transform-origin` and `perspective-origin`
    const ORIGINS: [(&str, &str); 9] = [
        ("center", "center"),
        ("top", "top"),
        ("top-right", "100% 0"),
        ("right", "100%"),
        ("bottom-right", "100% 100%"),
        ("bottom", "bottom"),
        ("bottom-left", "0 100%"),
        ("left", "0"),
        ("top-left", "0 0"),
    ];
    // Before 4.1.14, the values were the keywords.
    const OLD_ORIGINS: [(&str, &str); 9] = [
        ("center", "center"),
        ("top", "top"),
        ("top-right", "top right"),
        ("right", "right"),
        ("bottom-right", "bottom right"),
        ("bottom", "bottom"),
        ("bottom-left", "bottom left"),
        ("left", "left"),
        ("top-left", "top left"),
    ];
    let origins = if crate::switches::ORIGIN_PERCENTAGES.on() { &ORIGINS } else { &OLD_ORIGINS };
    functional_utility(
        u,
        "origin",
        one_property("transform-origin")
            .theme_keys(&["--transform-origin"])
            .static_values(values("transform-origin", origins)),
    );
    functional_utility(
        u,
        "perspective-origin",
        one_property("perspective-origin")
            .theme_keys(&["--perspective-origin"])
            .static_values(values("perspective-origin", origins)),
    );

    // `perspective`
    functional_utility(
        u,
        "perspective",
        one_property("perspective")
            .theme_keys(&["--perspective"])
            .static_values(values("perspective", &[("none", "none")])),
    );

    // `translate`
    st(u, "translate-none", &[("translate", "none")]);
    st_with(
        u,
        "-translate-full",
        translate_properties,
        &[("--tw-translate-x", "-100%"), ("--tw-translate-y", "-100%"), ("translate", TRANSLATE_2D)],
    );
    st_with(
        u,
        "translate-full",
        translate_properties,
        &[("--tw-translate-x", "100%"), ("--tw-translate-y", "100%"), ("translate", TRANSLATE_2D)],
    );

    spacing_utility(
        u,
        "translate",
        &["--translate", "--spacing"],
        |value, _| {
            Some(vec![
                translate_properties(),
                decl("--tw-translate-x", value),
                decl("--tw-translate-y", value),
                decl("translate", TRANSLATE_2D),
            ])
        },
        negative_fractions(),
    );

    for axis in ["x", "y"] {
        let variable = format!("--tw-translate-{axis}");
        st_with(
            u,
            &format!("-translate-{axis}-full"),
            translate_properties,
            &[(variable.as_str(), "-100%"), ("translate", TRANSLATE_2D)],
        );
        st_with(
            u,
            &format!("translate-{axis}-full"),
            translate_properties,
            &[(variable.as_str(), "100%"), ("translate", TRANSLATE_2D)],
        );
        spacing_utility(
            u,
            &format!("translate-{axis}"),
            &["--translate", "--spacing"],
            move |value, _| {
                Some(vec![translate_properties(), decl(variable.as_str(), value), decl("translate", TRANSLATE_2D)])
            },
            negative_fractions(),
        );
    }

    spacing_utility(
        u,
        "translate-z",
        &["--translate", "--spacing"],
        |value, _| {
            Some(vec![
                translate_properties(),
                decl("--tw-translate-z", value),
                decl("translate", "var(--tw-translate-x) var(--tw-translate-y) var(--tw-translate-z)"),
            ])
        },
        negative(),
    );

    st_with(
        u,
        "translate-3d",
        translate_properties,
        &[("translate", "var(--tw-translate-x) var(--tw-translate-y) var(--tw-translate-z)")],
    );

    // `scale`
    st(u, "scale-none", &[("scale", "none")]);
    u.functional("-scale", handle_scale(true));
    u.functional("scale", handle_scale(false));

    for axis in ["x", "y", "z"] {
        let variable = format!("--tw-scale-{axis}");
        let scale = format!("var(--tw-scale-x) var(--tw-scale-y){}", if axis == "z" { " var(--tw-scale-z)" } else { "" });
        functional_utility(
            u,
            &format!("scale-{axis}"),
            UtilityDescription::new(move |value, _, _| {
                Some(vec![scale_properties(), decl(variable.as_str(), value), decl("scale", scale.as_str())])
            })
            .negative()
            .theme_keys(&["--scale"])
            .bare(integer_with("%")),
        );
    }

    st_with(u, "scale-3d", scale_properties, &[("scale", "var(--tw-scale-x) var(--tw-scale-y) var(--tw-scale-z)")]);

    // `rotate`
    st(u, "rotate-none", &[("rotate", "none")]);
    u.functional("-rotate", handle_rotate(true));
    u.functional("rotate", handle_rotate(false));

    for axis in ["x", "y", "z"] {
        let variable = format!("--tw-rotate-{axis}");
        let function = format!("rotate{}", axis.to_ascii_uppercase());
        functional_utility(
            u,
            &format!("rotate-{axis}"),
            UtilityDescription::new(move |value, _, _| {
                Some(vec![
                    transform_properties(),
                    decl(variable.as_str(), format!("{function}({value})")),
                    decl("transform", transform_value()),
                ])
            })
            .negative()
            .theme_keys(&["--rotate"])
            .bare(integer_with("deg")),
        );
    }

    // `transform` with `skew()`
    functional_utility(
        u,
        "skew",
        UtilityDescription::new(|value, _, _| {
            Some(vec![
                transform_properties(),
                decl("--tw-skew-x", format!("skewX({value})")),
                decl("--tw-skew-y", format!("skewY({value})")),
                decl("transform", transform_value()),
            ])
        })
        .negative()
        .theme_keys(&["--skew"])
        .bare(integer_with("deg")),
    );
    functional_utility(
        u,
        "skew-x",
        UtilityDescription::new(|value, _, _| {
            Some(vec![
                transform_properties(),
                decl("--tw-skew-x", format!("skewX({value})")),
                decl("transform", transform_value()),
            ])
        })
        .negative()
        .theme_keys(&["--skew"])
        .bare(integer_with("deg")),
    );
    functional_utility(
        u,
        "skew-y",
        UtilityDescription::new(|value, _, _| {
            Some(vec![
                transform_properties(),
                decl("--tw-skew-y", format!("skewY({value})")),
                decl("transform", transform_value()),
            ])
        })
        .negative()
        .theme_keys(&["--skew"])
        .bare(integer_with("deg")),
    );

    // `transform`
    u.functional("transform", |candidate, _| {
        if candidate.modifier.is_some() {
            return None;
        }
        let value = match &candidate.value {
            None => transform_value(),
            Some(UtilityValue::Arbitrary { value, .. }) => value.as_str(),
            Some(UtilityValue::Named { .. }) => return None,
        };
        Some(vec![transform_properties(), decl("transform", value)])
    });

    st(u, "transform-cpu", &[("transform", transform_value())]);
    st(u, "transform-gpu", &[("transform", &format!("translateZ(0) {}", transform_value()))]);
    st(u, "transform-none", &[("transform", "none")]);

    // `zoom`
    if crate::switches::ZOOM_TAB_SCROLLBAR.on() {
        functional_utility(u, "zoom", one_property("zoom").bare(integer_with("%")));
    }

    // `transform-style`
    st(u, "transform-flat", &[("transform-style", "flat")]);
    st(u, "transform-3d", &[("transform-style", "preserve-3d")]);

    // `transform-box`
    st(u, "transform-content", &[("transform-box", "content-box")]);
    st(u, "transform-border", &[("transform-box", "border-box")]);
    st(u, "transform-fill", &[("transform-box", "fill-box")]);
    st(u, "transform-stroke", &[("transform-box", "stroke-box")]);
    st(u, "transform-view", &[("transform-box", "view-box")]);

    // `backface-visibility`
    st(u, "backface-visible", &[("backface-visibility", "visible")]);
    st(u, "backface-hidden", &[("backface-visibility", "hidden")]);

    // `cursor`
    for value in [
        "auto",
        "default",
        "pointer",
        "wait",
        "text",
        "move",
        "help",
        "not-allowed",
        "none",
        "context-menu",
        "progress",
        "cell",
        "crosshair",
        "vertical-text",
        "alias",
        "copy",
        "no-drop",
        "grab",
        "grabbing",
        "all-scroll",
        "col-resize",
        "row-resize",
        "n-resize",
        "e-resize",
        "s-resize",
        "w-resize",
        "ne-resize",
        "nw-resize",
        "se-resize",
        "sw-resize",
        "ew-resize",
        "ns-resize",
        "nesw-resize",
        "nwse-resize",
        "zoom-in",
        "zoom-out",
    ] {
        st(u, &format!("cursor-{value}"), &[("cursor", value)]);
    }

    functional_utility(u, "cursor", one_property("cursor").theme_keys(&["--cursor"]));

    // `touch-action`
    for value in ["auto", "none", "manipulation"] {
        st(u, &format!("touch-{value}"), &[("touch-action", value)]);
    }

    for value in ["x", "left", "right"] {
        st_with(
            u,
            &format!("touch-pan-{value}"),
            touch_properties,
            &[("--tw-pan-x", &format!("pan-{value}")), ("touch-action", super::empty_fallbacks(TOUCH_ACTION))],
        );
    }

    for value in ["y", "up", "down"] {
        st_with(
            u,
            &format!("touch-pan-{value}"),
            touch_properties,
            &[("--tw-pan-y", &format!("pan-{value}")), ("touch-action", super::empty_fallbacks(TOUCH_ACTION))],
        );
    }

    st_with(
        u,
        "touch-pinch-zoom",
        touch_properties,
        &[("--tw-pinch-zoom", "pinch-zoom"), ("touch-action", super::empty_fallbacks(TOUCH_ACTION))],
    );

    // `user-select`
    for value in ["none", "text", "all", "auto"] {
        st(u, &format!("select-{value}"), &[("-webkit-user-select", value), ("user-select", value)]);
    }

    // `resize`
    st(u, "resize-none", &[("resize", "none")]);
    st(u, "resize-x", &[("resize", "horizontal")]);
    st(u, "resize-y", &[("resize", "vertical")]);
    st(u, "resize", &[("resize", "both")]);

    // `scroll-snap-type`
    st(u, "snap-none", &[("scroll-snap-type", "none")]);

    for value in ["x", "y", "both"] {
        st_with(
            u,
            &format!("snap-{value}"),
            snap_properties,
            &[("scroll-snap-type", &format!("{value} var(--tw-scroll-snap-strictness)"))],
        );
    }

    st_with(u, "snap-mandatory", snap_properties, &[("--tw-scroll-snap-strictness", "mandatory")]);
    st_with(u, "snap-proximity", snap_properties, &[("--tw-scroll-snap-strictness", "proximity")]);

    st(u, "snap-align-none", &[("scroll-snap-align", "none")]);
    st(u, "snap-start", &[("scroll-snap-align", "start")]);
    st(u, "snap-end", &[("scroll-snap-align", "end")]);
    st(u, "snap-center", &[("scroll-snap-align", "center")]);

    st(u, "snap-normal", &[("scroll-snap-stop", "normal")]);
    st(u, "snap-always", &[("scroll-snap-stop", "always")]);

    // `scroll-margin`
    for (namespace, prop) in [
        ("scroll-m", "scroll-margin"),
        ("scroll-mx", "scroll-margin-inline"),
        ("scroll-my", "scroll-margin-block"),
        ("scroll-ms", "scroll-margin-inline-start"),
        ("scroll-me", "scroll-margin-inline-end"),
        ("scroll-mbs", "scroll-margin-block-start"),
        ("scroll-mbe", "scroll-margin-block-end"),
        ("scroll-mt", "scroll-margin-top"),
        ("scroll-mr", "scroll-margin-right"),
        ("scroll-mb", "scroll-margin-bottom"),
        ("scroll-ml", "scroll-margin-left"),
    ] {
        if matches!(namespace, "scroll-mbs" | "scroll-mbe") && !logical_utilities {
            continue;
        }
        spacing_utility(
            u,
            namespace,
            &["--scroll-margin", "--spacing"],
            move |value, _| Some(vec![decl(prop, value)]),
            negative(),
        );
    }

    // `scroll-padding`
    for (namespace, prop) in [
        ("scroll-p", "scroll-padding"),
        ("scroll-px", "scroll-padding-inline"),
        ("scroll-py", "scroll-padding-block"),
        ("scroll-ps", "scroll-padding-inline-start"),
        ("scroll-pe", "scroll-padding-inline-end"),
        ("scroll-pbs", "scroll-padding-block-start"),
        ("scroll-pbe", "scroll-padding-block-end"),
        ("scroll-pt", "scroll-padding-top"),
        ("scroll-pr", "scroll-padding-right"),
        ("scroll-pb", "scroll-padding-bottom"),
        ("scroll-pl", "scroll-padding-left"),
    ] {
        if matches!(namespace, "scroll-pbs" | "scroll-pbe") && !logical_utilities {
            continue;
        }
        spacing_utility(
            u,
            namespace,
            &["--scroll-padding", "--spacing"],
            move |value, _| Some(vec![decl(prop, value)]),
            SpacingOptions::default(),
        );
    }

    st(u, "list-inside", &[("list-style-position", "inside")]);
    st(u, "list-outside", &[("list-style-position", "outside")]);

    functional_utility(
        u,
        "list",
        one_property("list-style-type")
            .theme_keys(&["--list-style-type"])
            .static_values(values("list-style-type", &[("none", "none"), ("disc", "disc"), ("decimal", "decimal")])),
    );

    functional_utility(
        u,
        "list-image",
        one_property("list-style-image")
            .theme_keys(&["--list-style-image"])
            .static_values(values("list-style-image", &[("none", "none")])),
    );

    st(u, "appearance-none", &[("appearance", "none")]);
    st(u, "appearance-auto", &[("appearance", "auto")]);

    st(u, "scheme-normal", &[("color-scheme", "normal")]);
    st(u, "scheme-dark", &[("color-scheme", "dark")]);
    st(u, "scheme-light", &[("color-scheme", "light")]);
    st(u, "scheme-light-dark", &[("color-scheme", "light dark")]);
    st(u, "scheme-only-dark", &[("color-scheme", "only dark")]);
    st(u, "scheme-only-light", &[("color-scheme", "only light")]);

    functional_utility(
        u,
        "columns",
        one_property("columns")
            .theme_keys(&["--columns", "--container"])
            .bare(integer)
            .static_values(values("columns", &[("auto", "auto")])),
    );

    for value in ["auto", "avoid", "all", "avoid-page", "page", "left", "right", "column"] {
        st(u, &format!("break-before-{value}"), &[("break-before", value)]);
    }

    for value in ["auto", "avoid", "avoid-page", "avoid-column"] {
        st(u, &format!("break-inside-{value}"), &[("break-inside", value)]);
    }

    for value in ["auto", "avoid", "all", "avoid-page", "page", "left", "right", "column"] {
        st(u, &format!("break-after-{value}"), &[("break-after", value)]);
    }

    st(u, "grid-flow-row", &[("grid-auto-flow", "row")]);
    st(u, "grid-flow-col", &[("grid-auto-flow", "column")]);
    st(u, "grid-flow-dense", &[("grid-auto-flow", "dense")]);
    st(u, "grid-flow-row-dense", &[("grid-auto-flow", "row dense")]);
    st(u, "grid-flow-col-dense", &[("grid-auto-flow", "column dense")]);

    const AUTO_TRACKS: [(&str, &str); 4] =
        [("auto", "auto"), ("min", "min-content"), ("max", "max-content"), ("fr", "minmax(0, 1fr)")];
    functional_utility(
        u,
        "auto-cols",
        one_property("grid-auto-columns")
            .theme_keys(&["--grid-auto-columns"])
            .bare(auto_track_spacing)
            .static_values(values("grid-auto-columns", &AUTO_TRACKS)),
    );
    functional_utility(
        u,
        "auto-rows",
        one_property("grid-auto-rows")
            .theme_keys(&["--grid-auto-rows"])
            .bare(auto_track_spacing)
            .static_values(values("grid-auto-rows", &AUTO_TRACKS)),
    );

    functional_utility(
        u,
        "grid-cols",
        one_property("grid-template-columns")
            .theme_keys(&["--grid-template-columns"])
            .bare(grid_repeat)
            .static_values(values("grid-template-columns", &[("none", "none"), ("subgrid", "subgrid")])),
    );
    functional_utility(
        u,
        "grid-rows",
        one_property("grid-template-rows")
            .theme_keys(&["--grid-template-rows"])
            .bare(grid_repeat)
            .static_values(values("grid-template-rows", &[("none", "none"), ("subgrid", "subgrid")])),
    );
}
