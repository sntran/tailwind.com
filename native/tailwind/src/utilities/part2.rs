//! A part of the utilities of `utilities.ts` of Tailwind CSS: from
//! `flex-row` to the function that makes the mask stop utilities.

use super::{
    as_color, color_utility, functional_utility, property, resolve_theme_color, spacing_utility, static_decls,
    static_utility, SpacingOptions, StaticPart, Utilities, UtilityDescription,
};
use crate::ast::{at_root, decl, rule, style_rule, AstNode};
use crate::candidate::{CandidateModifier, FunctionalCandidate, UtilityValue, ValueKind};
use crate::design_system::DesignSystem;
use crate::theme::Theme;
use crate::utils::dimensions::dimensions;
use crate::utils::infer_data_type::{
    infer_data_type, is_length, is_positive_integer, is_valid_spacing_multiplier, DataType,
};

/// The selector of the children that get a space or a divide line.
const CHILDREN: &str = ":where(& > :not(:last-child))";

/// The type of an arbitrary value: the type hint, or the inferred type.
fn arbitrary_type<'a>(data_type: Option<&'a str>, value: &str, types: &[DataType]) -> Option<&'a str> {
    match data_type {
        Some(t) => Some(t),
        None => infer_data_type(value, types).map(DataType::as_str),
    }
}

/// `isPositiveInteger(value.slice(0, -1))`: the check without the last
/// UTF-16 code unit of the value.
fn is_positive_integer_without_last(value: &str) -> bool {
    match value.chars().next_back() {
        None => is_positive_integer(""),
        // One half of a surrogate pair stays: the text is not a number.
        Some(c) if c.len_utf16() == 2 => false,
        Some(c) => is_positive_integer(&value[..value.len() - c.len_utf8()]),
    }
}

/// `true` when a space value is zero.
fn is_zero_space(value: &str) -> bool {
    if !crate::switches::SPACE_ZERO.on() {
        return false;
    }
    if value == "--spacing(0)" || value == "--spacing(-0)" {
        return true;
    }
    matches!(dimensions(value), Some((n, unit)) if n == 0.0 && (unit.is_none() || is_length(value)))
}

fn space_utility(utilities: &mut Utilities, axis: &'static str, sort: &'static str, start: &'static str, end: &'static str) {
    spacing_utility(
        utilities,
        &format!("space-{axis}"),
        &["--space", "--spacing"],
        move |value, _| {
            let zero = is_zero_space(value);
            let reverse = format!("--tw-space-{axis}-reverse");
            Some(vec![
                at_root(vec![property(&reverse, Some("0"), None)]),
                style_rule(
                    CHILDREN,
                    vec![
                        decl("--tw-sort", sort),
                        decl(reverse.as_str(), "0"),
                        decl(start, if zero { "0".to_string() } else { format!("calc({value} * var({reverse}))") }),
                        decl(
                            end,
                            if zero { "0".to_string() } else { format!("calc({value} * calc(1 - var({reverse})))") },
                        ),
                    ],
                ),
            ])
        },
        SpacingOptions { supports_negative: true, ..Default::default() },
    );
}

fn border_properties() -> AstNode {
    at_root(vec![property("--tw-border-style", Some("solid"), None)])
}

/// Register a utility for the width and the color of a border side.
fn border_side_utility(
    utilities: &mut Utilities,
    class_root: &str,
    style_property: &'static str,
    width_property: &'static str,
    color_property: &'static str,
) {
    let width = move |value: &str| {
        vec![border_properties(), decl(style_property, "var(--tw-border-style)"), decl(width_property, value)]
    };
    let color = move |value: &str| vec![decl(color_property, value)];

    utilities.functional(class_root, move |candidate, ds| {
        let theme = &ds.theme;
        let Some(candidate_value) = &candidate.value else {
            if candidate.modifier.is_some() {
                return None;
            }
            let value = theme.get(&["--default-border-width"]).unwrap_or_else(|| "1px".to_string());
            return Some(width(&value));
        };

        let named = match candidate_value {
            UtilityValue::Arbitrary { data_type, value } => {
                let ty = arbitrary_type(
                    data_type.as_deref(),
                    value,
                    &[DataType::Color, DataType::LineWidth, DataType::Length],
                );
                return match ty {
                    Some("line-width" | "length") => {
                        if candidate.modifier.is_some() {
                            return None;
                        }
                        Some(width(value))
                    }
                    _ => {
                        let value = as_color(value, candidate.modifier.as_ref(), theme)?;
                        Some(color(&value))
                    }
                };
            }
            UtilityValue::Named { value, .. } => value,
        };

        // The `border-color` property.
        if let Some(value) =
            resolve_theme_color(candidate, theme, &["--border-color", "--color"]).filter(|v| !v.is_empty())
        {
            return Some(color(&value));
        }

        // The `border-width` property.
        if candidate.modifier.is_some() {
            return None;
        }
        if let Some(value) = theme.resolve(Some(named), &["--border-width"]).filter(|v| !v.is_empty()) {
            return Some(width(&value));
        }
        if is_positive_integer(named) {
            return Some(width(&format!("{named}px")));
        }
        None
    });
}

fn divide_width_utility(utilities: &mut Utilities, theme: &Theme, axis: &'static str) {
    let default_width = theme.get(&["--default-border-width"]).unwrap_or_else(|| "1px".to_string());
    functional_utility(
        utilities,
        &format!("divide-{axis}"),
        UtilityDescription::new(move |value, _, _| {
            let reverse = format!("--tw-divide-{axis}-reverse");
            let mut nodes = vec![
                decl("--tw-sort", format!("divide-{axis}-width")),
                border_properties(),
                decl(reverse.as_str(), "0"),
            ];
            let start = format!("calc({value} * var({reverse}))");
            let end = format!("calc({value} * calc(1 - var({reverse})))");
            if axis == "x" {
                nodes.push(decl("border-inline-style", "var(--tw-border-style)"));
                nodes.push(decl("border-inline-start-width", start));
                nodes.push(decl("border-inline-end-width", end));
            } else {
                nodes.push(decl("border-bottom-style", "var(--tw-border-style)"));
                nodes.push(decl("border-top-style", "var(--tw-border-style)"));
                nodes.push(decl("border-top-width", start));
                nodes.push(decl("border-bottom-width", end));
            }
            Some(vec![at_root(vec![property(&reverse, Some("0"), None)]), style_rule(CHILDREN, nodes)])
        })
        .default_value(Some(&default_width))
        .theme_keys(&["--divide-width", "--border-width"])
        .bare(|v, _| is_positive_integer(v.value).then(|| format!("{}px", v.value))),
    );
}

/// The interpolation method of a gradient from the modifier.
fn resolve_interpolation_modifier(modifier: Option<&CandidateModifier>) -> String {
    match modifier {
        None => "in oklab".to_string(),
        Some(m) if m.kind == ValueKind::Named => match m.value.as_str() {
            "longer" | "shorter" | "increasing" | "decreasing" => format!("in oklch {} hue", m.value),
            _ => format!("in {}", m.value),
        },
        Some(m) => m.value.clone(),
    }
}

fn linear_gradient_direction(value: &str) -> Option<&'static str> {
    Some(match value {
        "to-t" => "to top",
        "to-tr" => "to top right",
        "to-r" => "to right",
        "to-br" => "to bottom right",
        "to-b" => "to bottom",
        "to-bl" => "to bottom left",
        "to-l" => "to left",
        "to-tl" => "to top left",
        _ => return None,
    })
}

fn handle_bg_linear(negative: bool) -> impl Fn(&FunctionalCandidate, &DesignSystem) -> Option<Vec<AstNode>> {
    move |candidate, _| {
        let named = match candidate.value.as_ref()? {
            UtilityValue::Arbitrary { data_type, value } => {
                if candidate.modifier.is_some() {
                    return None;
                }
                let value = match arbitrary_type(data_type.as_deref(), value, &[DataType::Angle]) {
                    Some("angle") if negative => format!("calc({value} * -1)"),
                    Some("angle") => value.clone(),
                    _ if negative => return None,
                    _ => value.clone(),
                };
                return Some(vec![
                    decl("--tw-gradient-position", value.as_str()),
                    decl("background-image", format!("linear-gradient(var(--tw-gradient-stops,{value}))")),
                ]);
            }
            UtilityValue::Named { value, .. } => value,
        };

        let value = match linear_gradient_direction(named) {
            Some(direction) if !negative => direction.to_string(),
            _ if is_positive_integer(named) => {
                if negative {
                    format!("calc({named}deg * -1)")
                } else {
                    format!("{named}deg")
                }
            }
            _ => return None,
        };

        let interpolation_method = resolve_interpolation_modifier(candidate.modifier.as_ref());

        // Before 4.1.0, there was no fallback with no method.
        if !crate::switches::GRADIENT_POSITION_FALLBACK.on() {
            return Some(vec![
                decl("--tw-gradient-position", format!("{value} {interpolation_method}")),
                decl("background-image", "linear-gradient(var(--tw-gradient-stops))"),
            ]);
        }

        Some(vec![
            decl("--tw-gradient-position", value.as_str()),
            rule(
                "@supports (background-image: linear-gradient(in lab, red, red))",
                vec![decl("--tw-gradient-position", format!("{value} {interpolation_method}"))],
            ),
            decl("background-image", "linear-gradient(var(--tw-gradient-stops))"),
        ])
    }
}

fn handle_bg_conic(negative: bool) -> impl Fn(&FunctionalCandidate, &DesignSystem) -> Option<Vec<AstNode>> {
    move |candidate, _| {
        if let Some(UtilityValue::Arbitrary { value, .. }) = &candidate.value {
            if candidate.modifier.is_some() {
                return None;
            }
            return Some(vec![
                decl("--tw-gradient-position", value.as_str()),
                decl("background-image", format!("conic-gradient(var(--tw-gradient-stops,{value}))")),
            ]);
        }

        let interpolation_method = resolve_interpolation_modifier(candidate.modifier.as_ref());

        let Some(candidate_value) = &candidate.value else {
            return Some(vec![
                decl("--tw-gradient-position", interpolation_method),
                decl("background-image", "conic-gradient(var(--tw-gradient-stops))"),
            ]);
        };

        let value = candidate_value.value();
        if !is_positive_integer(value) {
            return None;
        }
        let value = match negative {
            // Before 4.0.15, the negative angle had no unit.
            true if !crate::switches::BG_CONIC_NEGATIVE_ANGLE.on() => format!("calc({value} * -1)"),
            true => format!("calc({value}deg * -1)"),
            false => format!("{value}deg"),
        };

        Some(vec![
            decl("--tw-gradient-position", format!("from {value} {interpolation_method}")),
            decl("background-image", "conic-gradient(var(--tw-gradient-stops))"),
        ])
    }
}

fn gradient_stop_properties() -> AstNode {
    at_root(vec![
        property("--tw-gradient-position", None, None),
        property("--tw-gradient-from", Some("#0000"), Some("<color>")),
        property("--tw-gradient-via", Some("#0000"), Some("<color>")),
        property("--tw-gradient-to", Some("#0000"), Some("<color>")),
        property("--tw-gradient-stops", None, None),
        property("--tw-gradient-via-stops", None, None),
        property("--tw-gradient-from-position", Some("0%"), Some("<length-percentage>")),
        property("--tw-gradient-via-position", Some("50%"), Some("<length-percentage>")),
        property("--tw-gradient-to-position", Some("100%"), Some("<length-percentage>")),
    ])
}

fn gradient_stop_utility(
    utilities: &mut Utilities,
    class_root: &str,
    color: impl Fn(&str) -> Vec<AstNode> + 'static,
    position: impl Fn(&str) -> Vec<AstNode> + 'static,
) {
    utilities.functional(class_root, move |candidate, ds| {
        let theme = &ds.theme;
        let named = match candidate.value.as_ref()? {
            UtilityValue::Arbitrary { data_type, value } => {
                let ty = arbitrary_type(
                    data_type.as_deref(),
                    value,
                    &[DataType::Color, DataType::Length, DataType::Percentage],
                );
                return match ty {
                    Some("length" | "percentage") => {
                        if candidate.modifier.is_some() {
                            return None;
                        }
                        Some(position(value))
                    }
                    _ => {
                        let value = as_color(value, candidate.modifier.as_ref(), theme)?;
                        Some(color(&value))
                    }
                };
            }
            UtilityValue::Named { value, .. } => value,
        };

        // The color stops.
        if let Some(value) =
            resolve_theme_color(candidate, theme, &["--background-color", "--color"]).filter(|v| !v.is_empty())
        {
            return Some(color(&value));
        }

        // The positions.
        if candidate.modifier.is_some() {
            return None;
        }
        if let Some(value) =
            theme.resolve(Some(named), &["--gradient-color-stop-positions"]).filter(|v| !v.is_empty())
        {
            return Some(position(&value));
        }
        if named.strip_suffix('%').is_some_and(is_positive_integer) {
            return Some(position(named));
        }
        None
    });
}

/// The `@property` rules of the mask gradients.
pub(crate) fn mask_properties_gradient() -> AstNode {
    at_root(vec![
        property("--tw-mask-linear", Some("linear-gradient(#fff, #fff)"), None),
        property("--tw-mask-radial", Some("linear-gradient(#fff, #fff)"), None),
        property("--tw-mask-conic", Some("linear-gradient(#fff, #fff)"), None),
    ])
}

/// Register a utility for a stop of a mask gradient: a color or a position.
pub(crate) fn mask_stop_utility(
    utilities: &mut Utilities,
    class_root: &str,
    color: impl Fn(&str) -> Vec<AstNode> + 'static,
    position: impl Fn(&str) -> Vec<AstNode> + 'static,
) {
    utilities.functional(class_root, move |candidate, ds| {
        let theme = &ds.theme;
        let named = match candidate.value.as_ref()? {
            UtilityValue::Arbitrary { data_type, value } => {
                let ty = arbitrary_type(
                    data_type.as_deref(),
                    value,
                    &[DataType::Length, DataType::Percentage, DataType::Color],
                );
                return match ty {
                    Some("color") => {
                        let value = as_color(value, candidate.modifier.as_ref(), theme)?;
                        Some(color(&value))
                    }
                    Some("percentage") => {
                        if candidate.modifier.is_some() || !is_positive_integer_without_last(value) {
                            return None;
                        }
                        Some(position(value))
                    }
                    _ => {
                        if candidate.modifier.is_some() {
                            return None;
                        }
                        Some(position(value))
                    }
                };
            }
            UtilityValue::Named { value, .. } => value,
        };

        // The color stops.
        if let Some(value) =
            resolve_theme_color(candidate, theme, &["--background-color", "--color"]).filter(|v| !v.is_empty())
        {
            return Some(color(&value));
        }

        // The positions.
        if candidate.modifier.is_some() {
            return None;
        }
        match infer_data_type(named, &[DataType::Number, DataType::Percentage])? {
            DataType::Number => {
                theme.resolve(None, &["--spacing"]).filter(|m| !m.is_empty())?;
                if !is_valid_spacing_multiplier(named) {
                    return None;
                }
                Some(position(&format!("--spacing({named})")))
            }
            DataType::Percentage => {
                if !is_positive_integer_without_last(named) {
                    return None;
                }
                Some(position(named))
            }
            _ => None,
        }
    });
}

/// Register a functional utility that sets one property to its value.
fn value_utility(utilities: &mut Utilities, class_root: &str, css_property: &'static str) {
    functional_utility(
        utilities,
        class_root,
        UtilityDescription::new(move |value, _, _| {
            if value.is_empty() {
                return None;
            }
            Some(vec![decl(css_property, value)])
        }),
    );
}

/// False for a static utility of this part that the selected version
/// does not have.
fn has_static_utility(name: &str) -> bool {
    // The utilities of 4.1.0.
    if name.ends_with("-safe") || name.ends_with("-baseline-last") {
        return crate::switches::SAFE_AND_LAST_BASELINE_ALIGNMENT.on();
    }
    if name.starts_with("wrap-") {
        return crate::switches::WRAP_UTILITIES.on();
    }
    if matches!(name, "bg-top-left" | "bg-top-right" | "bg-bottom-left" | "bg-bottom-right") {
        return crate::switches::CORNER_POSITION_NAMES.on();
    }
    true
}

/// Register the utilities of this part.
pub fn register(utilities: &mut Utilities, theme: &Theme) {
    let u = utilities;

    // Register static utilities that set one property.
    fn group(u: &mut Utilities, css_property: &'static str, values: &[(&str, &'static str)]) {
        for (name, value) in values {
            if has_static_utility(name) {
                static_decls(u, name, &[(css_property, value)]);
            }
        }
    }

    group(
        u,
        "flex-direction",
        &[
            ("flex-row", "row"),
            ("flex-row-reverse", "row-reverse"),
            ("flex-col", "column"),
            ("flex-col-reverse", "column-reverse"),
        ],
    );
    group(u, "flex-wrap", &[("flex-wrap", "wrap"), ("flex-nowrap", "nowrap"), ("flex-wrap-reverse", "wrap-reverse")]);
    group(
        u,
        "place-content",
        &[
            ("place-content-center", "center"),
            ("place-content-start", "start"),
            ("place-content-end", "end"),
            ("place-content-center-safe", "safe center"),
            ("place-content-end-safe", "safe end"),
            ("place-content-between", "space-between"),
            ("place-content-around", "space-around"),
            ("place-content-evenly", "space-evenly"),
            ("place-content-baseline", "baseline"),
            ("place-content-stretch", "stretch"),
        ],
    );
    group(
        u,
        "place-items",
        &[
            ("place-items-center", "center"),
            ("place-items-start", "start"),
            ("place-items-end", "end"),
            ("place-items-center-safe", "safe center"),
            ("place-items-end-safe", "safe end"),
            ("place-items-baseline", "baseline"),
            ("place-items-stretch", "stretch"),
        ],
    );
    group(
        u,
        "align-content",
        &[
            ("content-normal", "normal"),
            ("content-center", "center"),
            ("content-start", "flex-start"),
            ("content-end", "flex-end"),
            ("content-center-safe", "safe center"),
            ("content-end-safe", "safe flex-end"),
            ("content-between", "space-between"),
            ("content-around", "space-around"),
            ("content-evenly", "space-evenly"),
            ("content-baseline", "baseline"),
            ("content-stretch", "stretch"),
        ],
    );
    group(
        u,
        "align-items",
        &[
            ("items-center", "center"),
            ("items-start", "flex-start"),
            ("items-end", "flex-end"),
            ("items-center-safe", "safe center"),
            ("items-end-safe", "safe flex-end"),
            ("items-baseline", "baseline"),
            ("items-baseline-last", "last baseline"),
            ("items-stretch", "stretch"),
        ],
    );
    group(
        u,
        "justify-content",
        &[
            ("justify-normal", "normal"),
            ("justify-center", "center"),
            ("justify-start", "flex-start"),
            ("justify-end", "flex-end"),
            ("justify-center-safe", "safe center"),
            ("justify-end-safe", "safe flex-end"),
            ("justify-between", "space-between"),
            ("justify-around", "space-around"),
            ("justify-evenly", "space-evenly"),
            ("justify-baseline", "baseline"),
            ("justify-stretch", "stretch"),
        ],
    );
    group(
        u,
        "justify-items",
        &[
            ("justify-items-normal", "normal"),
            ("justify-items-center", "center"),
            ("justify-items-start", "start"),
            ("justify-items-end", "end"),
            ("justify-items-center-safe", "safe center"),
            ("justify-items-end-safe", "safe end"),
            ("justify-items-stretch", "stretch"),
        ],
    );

    for (name, css_property) in [("gap", "gap"), ("gap-x", "column-gap"), ("gap-y", "row-gap")] {
        spacing_utility(
            u,
            name,
            &["--gap", "--spacing"],
            move |value, _| Some(vec![decl(css_property, value)]),
            SpacingOptions::default(),
        );
    }

    space_utility(u, "x", "row-gap", "margin-inline-start", "margin-inline-end");
    space_utility(u, "y", "column-gap", "margin-block-start", "margin-block-end");

    for (axis, sort) in [("x", "row-gap"), ("y", "column-gap")] {
        let reverse = format!("--tw-space-{axis}-reverse");
        let reverse_rule = reverse.clone();
        static_utility(
            u,
            &format!("space-{axis}-reverse"),
            vec![
                StaticPart::Node(Box::new(move || at_root(vec![property(&reverse, Some("0"), None)]))),
                StaticPart::Node(Box::new(move || {
                    style_rule(CHILDREN, vec![decl("--tw-sort", sort), decl(reverse_rule.as_str(), "1")])
                })),
            ],
        );
    }

    static_decls(u, "accent-auto", &[("accent-color", "auto")]);
    color_utility(u, "accent", &["--accent-color", "--color"], |value, _| Some(vec![decl("accent-color", value)]));
    color_utility(u, "caret", &["--caret-color", "--color"], |value, _| Some(vec![decl("caret-color", value)]));
    // Before 4.1.12, `divide-*` did not read `--border-color`.
    let divide_keys: &[&'static str] = if crate::switches::DIVIDE_BORDER_COLOR_KEY.on() {
        &["--divide-color", "--border-color", "--color"]
    } else {
        &["--divide-color", "--color"]
    };
    color_utility(u, "divide", divide_keys, |value, _| {
        Some(vec![style_rule(CHILDREN, vec![decl("--tw-sort", "divide-color"), decl("border-color", value)])])
    });

    group(
        u,
        "place-self",
        &[
            ("place-self-auto", "auto"),
            ("place-self-start", "start"),
            ("place-self-end", "end"),
            ("place-self-center", "center"),
            ("place-self-end-safe", "safe end"),
            ("place-self-center-safe", "safe center"),
            ("place-self-stretch", "stretch"),
        ],
    );
    group(
        u,
        "align-self",
        &[
            ("self-auto", "auto"),
            ("self-start", "flex-start"),
            ("self-end", "flex-end"),
            ("self-center", "center"),
            ("self-end-safe", "safe flex-end"),
            ("self-center-safe", "safe center"),
            ("self-stretch", "stretch"),
            ("self-baseline", "baseline"),
            ("self-baseline-last", "last baseline"),
        ],
    );
    group(
        u,
        "justify-self",
        &[
            ("justify-self-auto", "auto"),
            ("justify-self-start", "flex-start"),
            ("justify-self-end", "flex-end"),
            ("justify-self-center", "center"),
            ("justify-self-end-safe", "safe flex-end"),
            ("justify-self-center-safe", "safe center"),
            ("justify-self-stretch", "stretch"),
        ],
    );

    for value in ["auto", "hidden", "clip", "visible", "scroll"] {
        static_decls(u, &format!("overflow-{value}"), &[("overflow", value)]);
        static_decls(u, &format!("overflow-x-{value}"), &[("overflow-x", value)]);
        static_decls(u, &format!("overflow-y-{value}"), &[("overflow-y", value)]);
    }

    for value in ["auto", "contain", "none"] {
        static_decls(u, &format!("overscroll-{value}"), &[("overscroll-behavior", value)]);
        static_decls(u, &format!("overscroll-x-{value}"), &[("overscroll-behavior-x", value)]);
        static_decls(u, &format!("overscroll-y-{value}"), &[("overscroll-behavior-y", value)]);
    }

    group(u, "scroll-behavior", &[("scroll-auto", "auto"), ("scroll-smooth", "smooth")]);
    let new_utilities = crate::switches::ZOOM_TAB_SCROLLBAR.on();
    // The utilities of 4.3.0 (`switches::ZOOM_TAB_SCROLLBAR`).
    if new_utilities {
        group(
            u,
            "scrollbar-width",
            &[("scrollbar-auto", "auto"), ("scrollbar-thin", "thin"), ("scrollbar-none", "none")],
        );

        for (name, variable) in
            [("scrollbar-thumb", "--tw-scrollbar-thumb"), ("scrollbar-track", "--tw-scrollbar-track")]
        {
            color_utility(u, name, &["--color"], move |value, _| {
                Some(vec![
                    at_root(vec![
                        property("--tw-scrollbar-thumb", Some("#0000"), Some("<color>")),
                        property("--tw-scrollbar-track", Some("#0000"), Some("<color>")),
                    ]),
                    decl(variable, value),
                    decl("scrollbar-color", "var(--tw-scrollbar-thumb) var(--tw-scrollbar-track)"),
                ])
            });
        }

        group(
            u,
            "scrollbar-gutter",
            &[
                ("scrollbar-gutter-auto", "auto"),
                ("scrollbar-gutter-stable", "stable"),
                ("scrollbar-gutter-both", "stable both-edges"),
            ],
        );
    }

    static_decls(u, "truncate", &[("overflow", "hidden"), ("text-overflow", "ellipsis"), ("white-space", "nowrap")]);

    group(u, "text-overflow", &[("text-ellipsis", "ellipsis"), ("text-clip", "clip")]);

    for value in ["none", "manual", "auto"] {
        static_decls(u, &format!("hyphens-{value}"), &[("-webkit-hyphens", value), ("hyphens", value)]);
    }

    group(
        u,
        "white-space",
        &[
            ("whitespace-normal", "normal"),
            ("whitespace-nowrap", "nowrap"),
            ("whitespace-pre", "pre"),
            ("whitespace-pre-line", "pre-line"),
            ("whitespace-pre-wrap", "pre-wrap"),
            ("whitespace-break-spaces", "break-spaces"),
        ],
    );

    if new_utilities {
        functional_utility(
            u,
            "tab",
            UtilityDescription::new(|value, _, _| Some(vec![decl("tab-size", value)]))
                .bare(|v, _| is_positive_integer(v.value).then(|| v.value.to_string())),
        );
    }

    group(
        u,
        "text-wrap",
        &[("text-wrap", "wrap"), ("text-nowrap", "nowrap"), ("text-balance", "balance"), ("text-pretty", "pretty")],
    );
    static_decls(u, "break-normal", &[("overflow-wrap", "normal"), ("word-break", "normal")]);
    group(u, "word-break", &[("break-all", "break-all"), ("break-keep", "keep-all")]);
    group(
        u,
        "overflow-wrap",
        &[("wrap-anywhere", "anywhere"), ("wrap-break-word", "break-word"), ("wrap-normal", "normal")],
    );

    // border-radius
    let radius: [(&str, &'static [&'static str]); 15] = [
        ("rounded", &["border-radius"]),
        ("rounded-s", &["border-start-start-radius", "border-end-start-radius"]),
        ("rounded-e", &["border-start-end-radius", "border-end-end-radius"]),
        ("rounded-t", &["border-top-left-radius", "border-top-right-radius"]),
        ("rounded-r", &["border-top-right-radius", "border-bottom-right-radius"]),
        ("rounded-b", &["border-bottom-right-radius", "border-bottom-left-radius"]),
        ("rounded-l", &["border-top-left-radius", "border-bottom-left-radius"]),
        ("rounded-ss", &["border-start-start-radius"]),
        ("rounded-se", &["border-start-end-radius"]),
        ("rounded-ee", &["border-end-end-radius"]),
        ("rounded-es", &["border-end-start-radius"]),
        ("rounded-tl", &["border-top-left-radius"]),
        ("rounded-tr", &["border-top-right-radius"]),
        ("rounded-br", &["border-bottom-right-radius"]),
        ("rounded-bl", &["border-bottom-left-radius"]),
    ];
    for (root, properties) in radius {
        let all = |value: &str| properties.iter().map(|p| decl(*p, value)).collect::<Vec<_>>();
        functional_utility(
            u,
            root,
            UtilityDescription::new(move |value, _, _| Some(properties.iter().map(|p| decl(*p, value)).collect()))
                .theme_keys(&["--radius"])
                .static_values(vec![("none", all("0")), ("full", all("calc(infinity * 1px)"))]),
        );
    }

    for value in ["solid", "dashed", "dotted", "double", "hidden", "none"] {
        static_decls(u, &format!("border-{value}"), &[("--tw-border-style", value), ("border-style", value)]);
    }

    // The width and the color of the borders.
    for (root, side) in [
        ("border", ""),
        ("border-x", "-inline"),
        ("border-y", "-block"),
        ("border-s", "-inline-start"),
        ("border-e", "-inline-end"),
        ("border-bs", "-block-start"),
        ("border-be", "-block-end"),
        ("border-t", "-top"),
        ("border-r", "-right"),
        ("border-b", "-bottom"),
        ("border-l", "-left"),
    ] {
        // The utilities of 4.2.0 (`switches::LOGICAL_UTILITIES`).
        if matches!(root, "border-bs" | "border-be") && !crate::switches::LOGICAL_UTILITIES.on() {
            continue;
        }
        let (style, width, color): (&'static str, &'static str, &'static str) = match side {
            "" => ("border-style", "border-width", "border-color"),
            "-inline" => ("border-inline-style", "border-inline-width", "border-inline-color"),
            "-block" => ("border-block-style", "border-block-width", "border-block-color"),
            "-inline-start" => {
                ("border-inline-start-style", "border-inline-start-width", "border-inline-start-color")
            }
            "-inline-end" => ("border-inline-end-style", "border-inline-end-width", "border-inline-end-color"),
            "-block-start" => ("border-block-start-style", "border-block-start-width", "border-block-start-color"),
            "-block-end" => ("border-block-end-style", "border-block-end-width", "border-block-end-color"),
            "-top" => ("border-top-style", "border-top-width", "border-top-color"),
            "-right" => ("border-right-style", "border-right-width", "border-right-color"),
            "-bottom" => ("border-bottom-style", "border-bottom-width", "border-bottom-color"),
            _ => ("border-left-style", "border-left-width", "border-left-color"),
        };
        border_side_utility(u, root, style, width, color);
    }

    divide_width_utility(u, theme, "x");
    divide_width_utility(u, theme, "y");

    for axis in ["x", "y"] {
        let reverse = format!("--tw-divide-{axis}-reverse");
        let reverse_rule = reverse.clone();
        static_utility(
            u,
            &format!("divide-{axis}-reverse"),
            vec![
                StaticPart::Node(Box::new(move || at_root(vec![property(&reverse, Some("0"), None)]))),
                StaticPart::Node(Box::new(move || style_rule(CHILDREN, vec![decl(reverse_rule.as_str(), "1")]))),
            ],
        );
    }

    for value in ["solid", "dashed", "dotted", "double", "none"] {
        static_utility(
            u,
            &format!("divide-{value}"),
            vec![StaticPart::Node(Box::new(move || {
                style_rule(
                    CHILDREN,
                    vec![decl("--tw-sort", "divide-style"), decl("--tw-border-style", value), decl("border-style", value)],
                )
            }))],
        );
    }

    group(u, "background-size", &[("bg-auto", "auto"), ("bg-cover", "cover"), ("bg-contain", "contain")]);
    // The utilities of 4.1.0 (`switches::BG_SIZE_AND_POSITION`).
    let bg_size_and_position = crate::switches::BG_SIZE_AND_POSITION.on();
    if bg_size_and_position {
        value_utility(u, "bg-size", "background-size");
    }

    group(u, "background-attachment", &[("bg-fixed", "fixed"), ("bg-local", "local"), ("bg-scroll", "scroll")]);

    group(
        u,
        "background-position",
        &[
            ("bg-top", "top"),
            ("bg-top-left", "left top"),
            ("bg-top-right", "right top"),
            ("bg-bottom", "bottom"),
            ("bg-bottom-left", "left bottom"),
            ("bg-bottom-right", "right bottom"),
            ("bg-left", "left"),
            ("bg-right", "right"),
            ("bg-center", "center"),
        ],
    );
    if bg_size_and_position {
        value_utility(u, "bg-position", "background-position");
    }

    group(
        u,
        "background-repeat",
        &[
            ("bg-repeat", "repeat"),
            ("bg-no-repeat", "no-repeat"),
            ("bg-repeat-x", "repeat-x"),
            ("bg-repeat-y", "repeat-y"),
            ("bg-repeat-round", "round"),
            ("bg-repeat-space", "space"),
        ],
    );

    static_decls(u, "bg-none", &[("background-image", "none")]);

    u.functional("-bg-linear", handle_bg_linear(true));
    u.functional("bg-linear", handle_bg_linear(false));

    u.functional("-bg-conic", handle_bg_conic(true));
    u.functional("bg-conic", handle_bg_conic(false));

    u.functional("bg-radial", |candidate, _| match &candidate.value {
        None => Some(vec![
            decl("--tw-gradient-position", resolve_interpolation_modifier(candidate.modifier.as_ref())),
            decl("background-image", "radial-gradient(var(--tw-gradient-stops))"),
        ]),
        Some(UtilityValue::Arbitrary { value, .. }) => {
            if candidate.modifier.is_some() {
                return None;
            }
            Some(vec![
                decl("--tw-gradient-position", value.as_str()),
                decl("background-image", format!("radial-gradient(var(--tw-gradient-stops,{value}))")),
            ])
        }
        Some(UtilityValue::Named { .. }) => None,
    });

    u.functional("bg", |candidate, ds| {
        let theme = &ds.theme;
        let named = match candidate.value.as_ref()? {
            UtilityValue::Arbitrary { data_type, value } => {
                let ty = arbitrary_type(
                    data_type.as_deref(),
                    value,
                    &[
                        DataType::Image,
                        DataType::Color,
                        DataType::Percentage,
                        DataType::Position,
                        DataType::BgSize,
                        DataType::Length,
                        DataType::Url,
                    ],
                );
                let css_property = match ty {
                    Some("percentage" | "position") => "background-position",
                    Some("bg-size" | "length" | "size") => "background-size",
                    Some("image" | "url") => "background-image",
                    _ => {
                        let value = as_color(value, candidate.modifier.as_ref(), theme)?;
                        return Some(vec![decl("background-color", value)]);
                    }
                };
                if candidate.modifier.is_some() {
                    return None;
                }
                return Some(vec![decl(css_property, value.as_str())]);
            }
            UtilityValue::Named { value, .. } => value,
        };

        // The `background-color` property.
        if let Some(value) =
            resolve_theme_color(candidate, theme, &["--background-color", "--color"]).filter(|v| !v.is_empty())
        {
            return Some(vec![decl("background-color", value)]);
        }

        // The `background-image` property.
        if candidate.modifier.is_some() {
            return None;
        }
        let value = theme.resolve(Some(named), &["--background-image"]).filter(|v| !v.is_empty())?;
        Some(vec![decl("background-image", value)])
    });

    const STOPS: &str = "var(--tw-gradient-via-stops, var(--tw-gradient-position), var(--tw-gradient-from) var(--tw-gradient-from-position), var(--tw-gradient-to) var(--tw-gradient-to-position))";

    gradient_stop_utility(
        u,
        "from",
        |value| {
            vec![
                gradient_stop_properties(),
                decl("--tw-sort", "--tw-gradient-from"),
                decl("--tw-gradient-from", value),
                decl("--tw-gradient-stops", STOPS),
            ]
        },
        |value| vec![gradient_stop_properties(), decl("--tw-gradient-from-position", value)],
    );
    static_decls(u, "via-none", &[("--tw-gradient-via-stops", "initial")]);
    gradient_stop_utility(
        u,
        "via",
        |value| {
            vec![
                gradient_stop_properties(),
                decl("--tw-sort", "--tw-gradient-via"),
                decl("--tw-gradient-via", value),
                decl(
                    "--tw-gradient-via-stops",
                    "var(--tw-gradient-position), var(--tw-gradient-from) var(--tw-gradient-from-position), var(--tw-gradient-via) var(--tw-gradient-via-position), var(--tw-gradient-to) var(--tw-gradient-to-position)",
                ),
                decl("--tw-gradient-stops", "var(--tw-gradient-via-stops)"),
            ]
        },
        |value| vec![gradient_stop_properties(), decl("--tw-gradient-via-position", value)],
    );
    gradient_stop_utility(
        u,
        "to",
        |value| {
            vec![
                gradient_stop_properties(),
                decl("--tw-sort", "--tw-gradient-to"),
                decl("--tw-gradient-to", value),
                decl("--tw-gradient-stops", STOPS),
            ]
        },
        |value| vec![gradient_stop_properties(), decl("--tw-gradient-to-position", value)],
    );

    // The utilities of 4.1.0 (`switches::MASK_UTILITIES`): all the rest of
    // this part.
    if !crate::switches::MASK_UTILITIES.on() {
        return;
    }

    // mask-image
    static_decls(u, "mask-none", &[("mask-image", "none")]);

    u.functional("mask", |candidate, _| {
        if candidate.modifier.is_some() {
            return None;
        }
        let Some(UtilityValue::Arbitrary { data_type, value }) = &candidate.value else {
            return None;
        };
        let ty = arbitrary_type(
            data_type.as_deref(),
            value,
            &[
                DataType::Image,
                DataType::Percentage,
                DataType::Position,
                DataType::BgSize,
                DataType::Length,
                DataType::Url,
            ],
        );
        let css_property = match ty {
            Some("percentage" | "position") => "mask-position",
            Some("bg-size" | "length" | "size") => "mask-size",
            _ => "mask-image",
        };
        Some(vec![decl(css_property, value.as_str())])
    });

    group(
        u,
        "mask-composite",
        &[
            ("mask-add", "add"),
            ("mask-subtract", "subtract"),
            ("mask-intersect", "intersect"),
            ("mask-exclude", "exclude"),
        ],
    );
    group(
        u,
        "mask-mode",
        &[("mask-alpha", "alpha"), ("mask-luminance", "luminance"), ("mask-match", "match-source")],
    );
    group(u, "mask-type", &[("mask-type-alpha", "alpha"), ("mask-type-luminance", "luminance")]);

    group(u, "mask-size", &[("mask-auto", "auto"), ("mask-cover", "cover"), ("mask-contain", "contain")]);
    value_utility(u, "mask-size", "mask-size");

    group(
        u,
        "mask-position",
        &[
            ("mask-top", "top"),
            ("mask-top-left", "left top"),
            ("mask-top-right", "right top"),
            ("mask-bottom", "bottom"),
            ("mask-bottom-left", "left bottom"),
            ("mask-bottom-right", "right bottom"),
            ("mask-left", "left"),
            ("mask-right", "right"),
            ("mask-center", "center"),
        ],
    );
    value_utility(u, "mask-position", "mask-position");

    group(
        u,
        "mask-repeat",
        &[
            ("mask-repeat", "repeat"),
            ("mask-no-repeat", "no-repeat"),
            ("mask-repeat-x", "repeat-x"),
            ("mask-repeat-y", "repeat-y"),
            ("mask-repeat-round", "round"),
            ("mask-repeat-space", "space"),
        ],
    );
    group(
        u,
        "mask-clip",
        &[
            ("mask-clip-border", "border-box"),
            ("mask-clip-padding", "padding-box"),
            ("mask-clip-content", "content-box"),
            ("mask-clip-fill", "fill-box"),
            ("mask-clip-stroke", "stroke-box"),
            ("mask-clip-view", "view-box"),
            ("mask-no-clip", "no-clip"),
        ],
    );
    group(
        u,
        "mask-origin",
        &[
            ("mask-origin-border", "border-box"),
            ("mask-origin-padding", "padding-box"),
            ("mask-origin-content", "content-box"),
            ("mask-origin-fill", "fill-box"),
            ("mask-origin-stroke", "stroke-box"),
            ("mask-origin-view", "view-box"),
        ],
    );
}
