//! A part of the utilities of `utilities.ts` of Tailwind CSS: from the edge
//! masks to the `backdrop-*` filters.

use super::part2::{mask_properties_gradient, mask_stop_utility};
use super::{
    as_color, color_utility, functional_utility, property, replace_alpha, resolve_theme_color, spacing_utility,
    static_decls, static_utility, with_alpha, NamedValue, SpacingOptions, StaticPart, Utilities, UtilityDescription,
};
use crate::ast::{at_root, decl, decl_full, rule, style_rule, AstNode};
use crate::candidate::{UtilityValue, ValueKind};
use crate::design_system::DesignSystem;
use crate::theme::Theme;
use crate::utils::infer_data_type::{
    infer_data_type, is_positive_integer, is_valid_opacity_value, is_valid_spacing_multiplier, DataType,
};
use crate::utils::js::{js_is_integer, js_number};
use crate::utils::replace_shadow_colors::replace_shadow_colors;
use crate::utils::segment::segment;

const MASK_IMAGE: &str = "var(--tw-mask-linear), var(--tw-mask-radial), var(--tw-mask-conic)";

const CSS_FILTER_VALUE: &str = "var(--tw-blur, ) var(--tw-brightness, ) var(--tw-contrast, ) var(--tw-grayscale, ) var(--tw-hue-rotate, ) var(--tw-invert, ) var(--tw-saturate, ) var(--tw-sepia, ) var(--tw-drop-shadow, )";

const CSS_BACKDROP_FILTER_VALUE: &str = "var(--tw-backdrop-blur, ) var(--tw-backdrop-brightness, ) var(--tw-backdrop-contrast, ) var(--tw-backdrop-grayscale, ) var(--tw-backdrop-hue-rotate, ) var(--tw-backdrop-invert, ) var(--tw-backdrop-opacity, ) var(--tw-backdrop-saturate, ) var(--tw-backdrop-sepia, )";

/// `value.slice(0, -1)` of JavaScript. `None` when the result is not a
/// complete string (the last character has two UTF-16 units).
fn without_last_unit(value: &str) -> Option<&str> {
    match value.chars().next_back() {
        None => Some(""),
        Some(c) if c.len_utf16() == 2 => None,
        Some(c) => Some(&value[..value.len() - c.len_utf8()]),
    }
}

/// `isPositiveInteger(value.slice(0, -1))`.
fn is_integer_percentage(value: &str) -> bool {
    without_last_unit(value).is_some_and(is_positive_integer)
}



fn mask_properties_edge() -> AstNode {
    at_root(vec![
        property("--tw-mask-left", Some("linear-gradient(#fff, #fff)"), None),
        property("--tw-mask-right", Some("linear-gradient(#fff, #fff)"), None),
        property("--tw-mask-bottom", Some("linear-gradient(#fff, #fff)"), None),
        property("--tw-mask-top", Some("linear-gradient(#fff, #fff)"), None),
    ])
}

/// The nodes of an edge mask. `edges` is: top, right, bottom, left. `kind`
/// is `color` or `position`.
fn mask_edge_nodes(stop: &str, edges: [bool; 4], kind: &str, value: &str) -> Vec<AstNode> {
    let mut nodes = vec![
        mask_properties_gradient(),
        mask_properties_edge(),
        decl("mask-image", MASK_IMAGE),
        decl("mask-composite", "intersect"),
        decl("--tw-mask-linear", "var(--tw-mask-left), var(--tw-mask-right), var(--tw-mask-bottom), var(--tw-mask-top)"),
    ];
    for (edge, active) in ["top", "right", "bottom", "left"].into_iter().zip(edges) {
        if !active {
            continue;
        }
        nodes.push(decl(
            format!("--tw-mask-{edge}"),
            format!("linear-gradient(to {edge}, var(--tw-mask-{edge}-from-color) var(--tw-mask-{edge}-from-position), var(--tw-mask-{edge}-to-color) var(--tw-mask-{edge}-to-position))"),
        ));
        nodes.push(at_root(vec![
            property(&format!("--tw-mask-{edge}-from-position"), Some("0%"), None),
            property(&format!("--tw-mask-{edge}-to-position"), Some("100%"), None),
            property(&format!("--tw-mask-{edge}-from-color"), Some("black"), None),
            property(&format!("--tw-mask-{edge}-to-color"), Some("transparent"), None),
        ]));
        nodes.push(decl(format!("--tw-mask-{edge}-{stop}-{kind}"), value));
    }
    nodes
}

fn mask_edge_utility(utilities: &mut Utilities, name: &str, stop: &'static str, edges: [bool; 4]) {
    mask_stop_utility(
        utilities,
        name,
        move |value| mask_edge_nodes(stop, edges, "color", value),
        move |value| mask_edge_nodes(stop, edges, "position", value),
    );
}

fn mask_properties_linear() -> AstNode {
    at_root(vec![
        property("--tw-mask-linear-position", Some("0deg"), None),
        property("--tw-mask-linear-from-position", Some("0%"), None),
        property("--tw-mask-linear-to-position", Some("100%"), None),
        property("--tw-mask-linear-from-color", Some("black"), None),
        property("--tw-mask-linear-to-color", Some("transparent"), None),
    ])
}

fn mask_properties_radial() -> AstNode {
    at_root(vec![
        property("--tw-mask-radial-from-position", Some("0%"), None),
        property("--tw-mask-radial-to-position", Some("100%"), None),
        property("--tw-mask-radial-from-color", Some("black"), None),
        property("--tw-mask-radial-to-color", Some("transparent"), None),
        property("--tw-mask-radial-shape", Some("ellipse"), None),
        property("--tw-mask-radial-size", Some("farthest-corner"), None),
        property("--tw-mask-radial-position", Some("center"), None),
    ])
}

fn mask_properties_conic() -> AstNode {
    at_root(vec![
        property("--tw-mask-conic-position", Some("0deg"), None),
        property("--tw-mask-conic-from-position", Some("0%"), None),
        property("--tw-mask-conic-to-position", Some("100%"), None),
        property("--tw-mask-conic-from-color", Some("black"), None),
        property("--tw-mask-conic-to-color", Some("transparent"), None),
    ])
}

/// The nodes of a stop of a linear, radial or conic mask. `shape` is
/// `linear`, `radial` or `conic`. `suffix` is, for example, `from-color`.
fn mask_gradient_stop_nodes(shape: &str, suffix: &str, value: &str) -> Vec<AstNode> {
    let (properties, start) = match shape {
        "linear" => (mask_properties_linear(), "var(--tw-mask-linear-position)"),
        "radial" => (
            mask_properties_radial(),
            "var(--tw-mask-radial-shape) var(--tw-mask-radial-size) at var(--tw-mask-radial-position)",
        ),
        _ => (mask_properties_conic(), "from var(--tw-mask-conic-position)"),
    };
    vec![
        mask_properties_gradient(),
        properties,
        decl("mask-image", MASK_IMAGE),
        decl("mask-composite", "intersect"),
        decl(
            format!("--tw-mask-{shape}-stops"),
            format!("{start}, var(--tw-mask-{shape}-from-color) var(--tw-mask-{shape}-from-position), var(--tw-mask-{shape}-to-color) var(--tw-mask-{shape}-to-position)"),
        ),
        decl(format!("--tw-mask-{shape}"), format!("{shape}-gradient(var(--tw-mask-{shape}-stops))")),
        decl(format!("--tw-mask-{shape}-{suffix}"), value),
    ]
}

/// Register `mask-<shape>-from` and `mask-<shape>-to`.
fn mask_gradient_stop_utilities(utilities: &mut Utilities, shape: &'static str) {
    for stop in ["from", "to"] {
        mask_stop_utility(
            utilities,
            &format!("mask-{shape}-{stop}"),
            move |value| mask_gradient_stop_nodes(shape, &format!("{stop}-color"), value),
            move |value| mask_gradient_stop_nodes(shape, &format!("{stop}-position"), value),
        );
    }
}

/// The bare value of an angle of a mask: `mask-linear-45`.
fn mask_angle(v: &NamedValue, negative: bool) -> Option<String> {
    if !is_positive_integer(v.value) {
        return None;
    }
    let number = js_number(v.value);
    let sign = if negative { "-" } else { "" };
    Some(if number == 0.0 {
        "0deg".to_string()
    } else if number == 1.0 {
        format!("{sign}1deg")
    } else {
        format!("calc(1deg * {sign}{})", v.value)
    })
}

/// The bare value `<integer>` with a unit: `brightness-50`.
fn integer_with_unit(unit: &'static str) -> impl Fn(&NamedValue, &DesignSystem) -> Option<String> {
    move |v, _| {
        if !is_positive_integer(v.value) {
            return None;
        }
        Some(format!("{}{unit}", v.value))
    }
}

fn filter_properties() -> AstNode {
    at_root(vec![
        property("--tw-blur", None, None),
        property("--tw-brightness", None, None),
        property("--tw-contrast", None, None),
        property("--tw-grayscale", None, None),
        property("--tw-hue-rotate", None, None),
        property("--tw-invert", None, None),
        property("--tw-opacity", None, None),
        property("--tw-saturate", None, None),
        property("--tw-sepia", None, None),
        property("--tw-drop-shadow", None, None),
        property("--tw-drop-shadow-color", None, None),
        property("--tw-drop-shadow-alpha", Some("100%"), Some("<percentage>")),
        property("--tw-drop-shadow-size", None, None),
    ])
}

fn backdrop_filter_properties() -> AstNode {
    at_root(vec![
        property("--tw-backdrop-blur", None, None),
        property("--tw-backdrop-brightness", None, None),
        property("--tw-backdrop-contrast", None, None),
        property("--tw-backdrop-grayscale", None, None),
        property("--tw-backdrop-hue-rotate", None, None),
        property("--tw-backdrop-invert", None, None),
        property("--tw-backdrop-opacity", None, None),
        property("--tw-backdrop-saturate", None, None),
        property("--tw-backdrop-sepia", None, None),
    ])
}

/// The nodes of a filter utility: `--tw-<name>: <function>(<value>)`.
fn filter_nodes(name: &str, function: &str, value: &str) -> Vec<AstNode> {
    vec![
        filter_properties(),
        decl(format!("--tw-{name}"), format!("{function}({value})")),
        decl("filter", CSS_FILTER_VALUE),
    ]
}

/// The nodes of a backdrop filter utility.
fn backdrop_filter_nodes(name: &str, function: &str, value: &str) -> Vec<AstNode> {
    vec![
        backdrop_filter_properties(),
        decl(format!("--tw-backdrop-{name}"), format!("{function}({value})")),
        decl("-webkit-backdrop-filter", CSS_BACKDROP_FILTER_VALUE),
        decl("backdrop-filter", CSS_BACKDROP_FILTER_VALUE),
    ]
}

/// The options of a pair of filter utilities.
struct FilterPair {
    name: &'static str,
    theme_key: &'static str,
    backdrop_theme_key: &'static str,
    unit: &'static str,
    default_value: Option<&'static str>,
    negative: bool,
}

/// Register a filter utility and its backdrop utility, for example
/// `brightness` and `backdrop-brightness`.
fn filter_pair(utilities: &mut Utilities, pair: FilterPair) {
    let FilterPair { name, theme_key, backdrop_theme_key, unit, default_value, negative } = pair;

    let mut desc = UtilityDescription::new(move |value, _, _| Some(filter_nodes(name, name, value)))
        .theme_keys(&[theme_key])
        .bare(integer_with_unit(unit));
    let mut backdrop = UtilityDescription::new(move |value, _, _| Some(backdrop_filter_nodes(name, name, value)))
        .theme_keys(&[backdrop_theme_key, theme_key])
        .bare(integer_with_unit(unit));
    if let Some(value) = default_value {
        desc = desc.default_value(Some(value));
        backdrop = backdrop.default_value(Some(value));
    }
    if negative {
        desc = desc.negative();
        backdrop = backdrop.negative();
    }
    functional_utility(utilities, name, desc);
    functional_utility(utilities, &format!("backdrop-{name}"), backdrop);
}

/// `alphaReplacedDropShadowProperties` of upstream, with no prefix.
fn alpha_replaced_drop_shadow_properties(
    property: &str,
    value: &str,
    alpha: Option<&str>,
    var_injector: impl Fn(&str) -> String,
) -> Vec<AstNode> {
    let mut requires_fallback = false;

    let replaced_value = segment(value, b',')
        .into_iter()
        .map(|value| {
            let replaced = replace_shadow_colors(value, |color| {
                let Some(alpha) = alpha else {
                    return var_injector(color);
                };
                if color.starts_with("current") {
                    return var_injector(&with_alpha(color, alpha));
                }
                if color.starts_with("var(") || alpha.starts_with("var(") {
                    requires_fallback = true;
                }
                var_injector(&replace_alpha(color, alpha))
            });
            format!("drop-shadow({replaced})")
        })
        .collect::<Vec<_>>()
        .join(" ");

    if requires_fallback {
        let fallback = segment(value, b',')
            .into_iter()
            .map(|value| format!("drop-shadow({})", replace_shadow_colors(value, &var_injector)))
            .collect::<Vec<_>>()
            .join(" ");
        vec![
            decl(property, fallback),
            rule("@supports (color: lab(from red l a b))", vec![decl(property, replaced_value)]),
        ]
    } else {
        vec![decl(property, replaced_value)]
    }
}

/// The value of `--tw-drop-shadow` for a shadow of the theme.
fn drop_shadow_list(resolved: &str) -> String {
    segment(resolved, b',').into_iter().map(|v| format!("drop-shadow({v})")).collect::<Vec<_>>().join(" ")
}

fn drop_shadow_size_nodes(value: &str, alpha: Option<&str>, drop_shadow: String) -> Vec<AstNode> {
    let mut nodes = vec![filter_properties(), decl_full("--tw-drop-shadow-alpha", alpha.map(str::to_string), false)];
    nodes.extend(alpha_replaced_drop_shadow_properties("--tw-drop-shadow-size", value, alpha, |color| {
        format!("var(--tw-drop-shadow-color, {color})")
    }));
    nodes.push(decl("--tw-drop-shadow", drop_shadow));
    nodes.push(decl("filter", CSS_FILTER_VALUE));
    nodes
}

/// Register the utilities of this part.
pub fn register(utilities: &mut Utilities, _theme: &Theme) {
    // Edge masks. The edges are: top, right, bottom, left.
    mask_edge_utility(utilities, "mask-x-from", "from", [false, true, false, true]);
    mask_edge_utility(utilities, "mask-x-to", "to", [false, true, false, true]);
    mask_edge_utility(utilities, "mask-y-from", "from", [true, false, true, false]);
    mask_edge_utility(utilities, "mask-y-to", "to", [true, false, true, false]);
    mask_edge_utility(utilities, "mask-t-from", "from", [true, false, false, false]);
    mask_edge_utility(utilities, "mask-t-to", "to", [true, false, false, false]);
    mask_edge_utility(utilities, "mask-r-from", "from", [false, true, false, false]);
    mask_edge_utility(utilities, "mask-r-to", "to", [false, true, false, false]);
    mask_edge_utility(utilities, "mask-b-from", "from", [false, false, true, false]);
    mask_edge_utility(utilities, "mask-b-to", "to", [false, false, true, false]);
    mask_edge_utility(utilities, "mask-l-from", "from", [false, false, false, true]);
    mask_edge_utility(utilities, "mask-l-to", "to", [false, false, false, true]);

    // Linear masks.
    functional_utility(
        utilities,
        "mask-linear",
        UtilityDescription::new(|value, _, _| {
            Some(vec![
                mask_properties_gradient(),
                mask_properties_linear(),
                decl("mask-image", MASK_IMAGE),
                decl("mask-composite", "intersect"),
                decl("--tw-mask-linear", "linear-gradient(var(--tw-mask-linear-stops, var(--tw-mask-linear-position)))"),
                decl("--tw-mask-linear-position", value),
            ])
        })
        .default_value(None)
        .negative()
        .bare(|v, _| mask_angle(v, false))
        .negative_bare(|v, _| mask_angle(v, true)),
    );
    mask_gradient_stop_utilities(utilities, "linear");

    // Radial masks.
    static_decls(utilities, "mask-circle", &[("--tw-mask-radial-shape", "circle")]);
    static_decls(utilities, "mask-ellipse", &[("--tw-mask-radial-shape", "ellipse")]);
    static_decls(utilities, "mask-radial-closest-side", &[("--tw-mask-radial-size", "closest-side")]);
    static_decls(utilities, "mask-radial-farthest-side", &[("--tw-mask-radial-size", "farthest-side")]);
    static_decls(utilities, "mask-radial-closest-corner", &[("--tw-mask-radial-size", "closest-corner")]);
    static_decls(utilities, "mask-radial-farthest-corner", &[("--tw-mask-radial-size", "farthest-corner")]);
    static_decls(utilities, "mask-radial-at-top", &[("--tw-mask-radial-position", "top")]);
    static_decls(utilities, "mask-radial-at-top-left", &[("--tw-mask-radial-position", "top left")]);
    static_decls(utilities, "mask-radial-at-top-right", &[("--tw-mask-radial-position", "top right")]);
    static_decls(utilities, "mask-radial-at-bottom", &[("--tw-mask-radial-position", "bottom")]);
    static_decls(utilities, "mask-radial-at-bottom-left", &[("--tw-mask-radial-position", "bottom left")]);
    static_decls(utilities, "mask-radial-at-bottom-right", &[("--tw-mask-radial-position", "bottom right")]);
    static_decls(utilities, "mask-radial-at-left", &[("--tw-mask-radial-position", "left")]);
    static_decls(utilities, "mask-radial-at-right", &[("--tw-mask-radial-position", "right")]);
    static_decls(utilities, "mask-radial-at-center", &[("--tw-mask-radial-position", "center")]);
    functional_utility(
        utilities,
        "mask-radial-at",
        UtilityDescription::new(|value, _, _| Some(vec![decl("--tw-mask-radial-position", value)])).default_value(None),
    );

    functional_utility(
        utilities,
        "mask-radial",
        UtilityDescription::new(|value, _, _| {
            Some(vec![
                mask_properties_gradient(),
                mask_properties_radial(),
                decl("mask-image", MASK_IMAGE),
                decl("mask-composite", "intersect"),
                decl("--tw-mask-radial", "radial-gradient(var(--tw-mask-radial-stops, var(--tw-mask-radial-size)))"),
                decl("--tw-mask-radial-size", value),
            ])
        })
        .default_value(None),
    );
    mask_gradient_stop_utilities(utilities, "radial");

    // Conic masks.
    functional_utility(
        utilities,
        "mask-conic",
        UtilityDescription::new(|value, _, _| {
            Some(vec![
                mask_properties_gradient(),
                mask_properties_conic(),
                decl("mask-image", MASK_IMAGE),
                decl("mask-composite", "intersect"),
                decl("--tw-mask-conic", "conic-gradient(var(--tw-mask-conic-stops, var(--tw-mask-conic-position)))"),
                decl("--tw-mask-conic-position", value),
            ])
        })
        .default_value(None)
        .negative()
        .bare(|v, _| mask_angle(v, false))
        .negative_bare(|v, _| mask_angle(v, true)),
    );
    mask_gradient_stop_utilities(utilities, "conic");

    // `box-decoration-break`
    static_decls(
        utilities,
        "box-decoration-slice",
        &[("-webkit-box-decoration-break", "slice"), ("box-decoration-break", "slice")],
    );
    static_decls(
        utilities,
        "box-decoration-clone",
        &[("-webkit-box-decoration-break", "clone"), ("box-decoration-break", "clone")],
    );

    // `background-clip`
    static_decls(utilities, "bg-clip-text", &[("background-clip", "text")]);
    static_decls(utilities, "bg-clip-border", &[("background-clip", "border-box")]);
    static_decls(utilities, "bg-clip-padding", &[("background-clip", "padding-box")]);
    static_decls(utilities, "bg-clip-content", &[("background-clip", "content-box")]);

    static_decls(utilities, "bg-origin-border", &[("background-origin", "border-box")]);
    static_decls(utilities, "bg-origin-padding", &[("background-origin", "padding-box")]);
    static_decls(utilities, "bg-origin-content", &[("background-origin", "content-box")]);

    for value in [
        "normal",
        "multiply",
        "screen",
        "overlay",
        "darken",
        "lighten",
        "color-dodge",
        "color-burn",
        "hard-light",
        "soft-light",
        "difference",
        "exclusion",
        "hue",
        "saturation",
        "color",
        "luminosity",
    ] {
        static_decls(utilities, &format!("bg-blend-{value}"), &[("background-blend-mode", value)]);
        static_decls(utilities, &format!("mix-blend-{value}"), &[("mix-blend-mode", value)]);
    }

    static_decls(utilities, "mix-blend-plus-darker", &[("mix-blend-mode", "plus-darker")]);
    static_decls(utilities, "mix-blend-plus-lighter", &[("mix-blend-mode", "plus-lighter")]);

    static_decls(utilities, "fill-none", &[("fill", "none")]);
    utilities.functional("fill", |candidate, ds| {
        let theme = &ds.theme;
        match candidate.value.as_ref()? {
            UtilityValue::Arbitrary { value, .. } => {
                let value = as_color(value, candidate.modifier.as_ref(), theme)?;
                Some(vec![decl("fill", value)])
            }
            UtilityValue::Named { .. } => {
                let value = resolve_theme_color(candidate, theme, &["--fill", "--color"]).filter(|v| !v.is_empty())?;
                Some(vec![decl("fill", value)])
            }
        }
    });

    static_decls(utilities, "stroke-none", &[("stroke", "none")]);
    utilities.functional("stroke", |candidate, ds| {
        let theme = &ds.theme;
        let named = match candidate.value.as_ref()? {
            UtilityValue::Arbitrary { data_type, value } => {
                let ty = data_type.as_deref().or_else(|| {
                    infer_data_type(value, &[DataType::Color, DataType::Number, DataType::Length, DataType::Percentage])
                        .map(DataType::as_str)
                });
                return match ty {
                    Some("number" | "length" | "percentage") => {
                        if candidate.modifier.is_some() {
                            return None;
                        }
                        Some(vec![decl("stroke-width", value.as_str())])
                    }
                    _ => {
                        let value = as_color(value, candidate.modifier.as_ref(), theme)?;
                        Some(vec![decl("stroke", value)])
                    }
                };
            }
            UtilityValue::Named { value, .. } => value,
        };

        if let Some(value) = resolve_theme_color(candidate, theme, &["--stroke", "--color"]).filter(|v| !v.is_empty()) {
            return Some(vec![decl("stroke", value)]);
        }

        // A modifier is only valid as the opacity of a color.
        if candidate.modifier.is_some() {
            return None;
        }

        if let Some(value) = theme.resolve(Some(named), &["--stroke-width"]).filter(|v| !v.is_empty()) {
            Some(vec![decl("stroke-width", value)])
        } else if is_positive_integer(named) {
            Some(vec![decl("stroke-width", named.as_str())])
        } else {
            None
        }
    });

    static_decls(utilities, "object-contain", &[("object-fit", "contain")]);
    static_decls(utilities, "object-cover", &[("object-fit", "cover")]);
    static_decls(utilities, "object-fill", &[("object-fit", "fill")]);
    static_decls(utilities, "object-none", &[("object-fit", "none")]);
    static_decls(utilities, "object-scale-down", &[("object-fit", "scale-down")]);

    functional_utility(
        utilities,
        "object",
        UtilityDescription::new(|value, _, _| Some(vec![decl("object-position", value)]))
            .theme_keys(&["--object-position"])
            .static_values(vec![
                ("top", vec![decl("object-position", "top")]),
                ("top-left", vec![decl("object-position", "left top")]),
                ("top-right", vec![decl("object-position", "right top")]),
                ("bottom", vec![decl("object-position", "bottom")]),
                ("bottom-left", vec![decl("object-position", "left bottom")]),
                ("bottom-right", vec![decl("object-position", "right bottom")]),
                ("left", vec![decl("object-position", "left")]),
                ("right", vec![decl("object-position", "right")]),
                ("center", vec![decl("object-position", "center")]),
            ]),
    );

    for (name, property) in [
        ("p", "padding"),
        ("px", "padding-inline"),
        ("py", "padding-block"),
        ("ps", "padding-inline-start"),
        ("pe", "padding-inline-end"),
        ("pbs", "padding-block-start"),
        ("pbe", "padding-block-end"),
        ("pt", "padding-top"),
        ("pr", "padding-right"),
        ("pb", "padding-bottom"),
        ("pl", "padding-left"),
    ] {
        spacing_utility(
            utilities,
            name,
            &["--padding", "--spacing"],
            move |value, _| Some(vec![decl(property, value)]),
            SpacingOptions::default(),
        );
    }

    static_decls(utilities, "text-left", &[("text-align", "left")]);
    static_decls(utilities, "text-center", &[("text-align", "center")]);
    static_decls(utilities, "text-right", &[("text-align", "right")]);
    static_decls(utilities, "text-justify", &[("text-align", "justify")]);
    static_decls(utilities, "text-start", &[("text-align", "start")]);
    static_decls(utilities, "text-end", &[("text-align", "end")]);

    spacing_utility(
        utilities,
        "indent",
        &["--text-indent", "--spacing"],
        |value, _| Some(vec![decl("text-indent", value)]),
        SpacingOptions { supports_negative: true, ..SpacingOptions::default() },
    );

    static_decls(utilities, "align-baseline", &[("vertical-align", "baseline")]);
    static_decls(utilities, "align-top", &[("vertical-align", "top")]);
    static_decls(utilities, "align-middle", &[("vertical-align", "middle")]);
    static_decls(utilities, "align-bottom", &[("vertical-align", "bottom")]);
    static_decls(utilities, "align-text-top", &[("vertical-align", "text-top")]);
    static_decls(utilities, "align-text-bottom", &[("vertical-align", "text-bottom")]);
    static_decls(utilities, "align-sub", &[("vertical-align", "sub")]);
    static_decls(utilities, "align-super", &[("vertical-align", "super")]);

    functional_utility(
        utilities,
        "align",
        UtilityDescription::new(|value, _, _| Some(vec![decl("vertical-align", value)])),
    );

    utilities.functional("font", |candidate, ds| {
        let theme = &ds.theme;
        if candidate.modifier.is_some() {
            return None;
        }
        let font_weight = |value: &str| {
            vec![
                at_root(vec![property("--tw-font-weight", None, None)]),
                decl("--tw-font-weight", value),
                decl("font-weight", value),
            ]
        };
        let named = match candidate.value.as_ref()? {
            UtilityValue::Arbitrary { data_type, value } => {
                let ty = data_type.as_deref().or_else(|| {
                    infer_data_type(value, &[DataType::Number, DataType::GenericName, DataType::FamilyName])
                        .map(DataType::as_str)
                });
                return match ty {
                    Some("generic-name" | "family-name") => Some(vec![decl("font-family", value.as_str())]),
                    _ => Some(font_weight(value)),
                };
            }
            UtilityValue::Named { value, .. } => value,
        };

        if let Some((families, options)) =
            theme.resolve_with(named, &["--font"], &["--font-feature-settings", "--font-variation-settings"])
        {
            let option = |name: &str| options.iter().find(|(k, _)| k == name).map(|(_, v)| v.clone());
            return Some(vec![
                decl("font-family", families),
                decl_full("font-feature-settings", option("--font-feature-settings"), false),
                decl_full("font-variation-settings", option("--font-variation-settings"), false),
            ]);
        }

        let value = theme.resolve(Some(named), &["--font-weight"]).filter(|v| !v.is_empty())?;
        Some(font_weight(&value))
    });

    // `font-feature-settings`
    functional_utility(
        utilities,
        "font-features",
        UtilityDescription::new(|value, _, _| Some(vec![decl("font-feature-settings", value)])),
    );

    static_decls(utilities, "uppercase", &[("text-transform", "uppercase")]);
    static_decls(utilities, "lowercase", &[("text-transform", "lowercase")]);
    static_decls(utilities, "capitalize", &[("text-transform", "capitalize")]);
    static_decls(utilities, "normal-case", &[("text-transform", "none")]);

    static_decls(utilities, "italic", &[("font-style", "italic")]);
    static_decls(utilities, "not-italic", &[("font-style", "normal")]);
    static_decls(utilities, "underline", &[("text-decoration-line", "underline")]);
    static_decls(utilities, "overline", &[("text-decoration-line", "overline")]);
    static_decls(utilities, "line-through", &[("text-decoration-line", "line-through")]);
    static_decls(utilities, "no-underline", &[("text-decoration-line", "none")]);

    static_decls(utilities, "font-stretch-normal", &[("font-stretch", "normal")]);
    static_decls(utilities, "font-stretch-ultra-condensed", &[("font-stretch", "ultra-condensed")]);
    static_decls(utilities, "font-stretch-extra-condensed", &[("font-stretch", "extra-condensed")]);
    static_decls(utilities, "font-stretch-condensed", &[("font-stretch", "condensed")]);
    static_decls(utilities, "font-stretch-semi-condensed", &[("font-stretch", "semi-condensed")]);
    static_decls(utilities, "font-stretch-semi-expanded", &[("font-stretch", "semi-expanded")]);
    static_decls(utilities, "font-stretch-expanded", &[("font-stretch", "expanded")]);
    static_decls(utilities, "font-stretch-extra-expanded", &[("font-stretch", "extra-expanded")]);
    static_decls(utilities, "font-stretch-ultra-expanded", &[("font-stretch", "ultra-expanded")]);
    functional_utility(
        utilities,
        "font-stretch",
        UtilityDescription::new(|value, _, _| Some(vec![decl("font-stretch", value)])).bare(|v, _| {
            let number = js_number(v.value.strip_suffix('%')?);
            // Only 50% to 200% are valid.
            if !js_is_integer(number) || !(50.0..=200.0).contains(&number) {
                return None;
            }
            Some(v.value.to_string())
        }),
    );

    color_utility(utilities, "placeholder", &["--placeholder-color", "--color"], |value, _| {
        Some(vec![style_rule("&::placeholder", vec![decl("--tw-sort", "placeholder-color"), decl("color", value)])])
    });

    // `text-decoration-style`
    static_decls(utilities, "decoration-solid", &[("text-decoration-style", "solid")]);
    static_decls(utilities, "decoration-double", &[("text-decoration-style", "double")]);
    static_decls(utilities, "decoration-dotted", &[("text-decoration-style", "dotted")]);
    static_decls(utilities, "decoration-dashed", &[("text-decoration-style", "dashed")]);
    static_decls(utilities, "decoration-wavy", &[("text-decoration-style", "wavy")]);

    // `text-decoration-thickness`
    static_decls(utilities, "decoration-auto", &[("text-decoration-thickness", "auto")]);
    static_decls(utilities, "decoration-from-font", &[("text-decoration-thickness", "from-font")]);

    utilities.functional("decoration", |candidate, ds| {
        let theme = &ds.theme;
        let named = match candidate.value.as_ref()? {
            UtilityValue::Arbitrary { data_type, value } => {
                let ty = data_type.as_deref().or_else(|| {
                    infer_data_type(value, &[DataType::Color, DataType::Length, DataType::Percentage])
                        .map(DataType::as_str)
                });
                return match ty {
                    Some("length" | "percentage") => {
                        if candidate.modifier.is_some() {
                            return None;
                        }
                        Some(vec![decl("text-decoration-thickness", value.as_str())])
                    }
                    _ => {
                        let value = as_color(value, candidate.modifier.as_ref(), theme)?;
                        Some(vec![decl("text-decoration-color", value)])
                    }
                };
            }
            UtilityValue::Named { value, .. } => value,
        };

        // `text-decoration-thickness`
        if let Some(value) = theme.resolve(Some(named), &["--text-decoration-thickness"]).filter(|v| !v.is_empty()) {
            if candidate.modifier.is_some() {
                return None;
            }
            return Some(vec![decl("text-decoration-thickness", value)]);
        }
        if is_positive_integer(named) {
            if candidate.modifier.is_some() {
                return None;
            }
            return Some(vec![decl("text-decoration-thickness", format!("{named}px"))]);
        }

        // `text-decoration-color`
        let value = resolve_theme_color(candidate, theme, &["--text-decoration-color", "--color"])
            .filter(|v| !v.is_empty())?;
        Some(vec![decl("text-decoration-color", value)])
    });

    functional_utility(
        utilities,
        "animate",
        UtilityDescription::new(|value, _, _| Some(vec![decl("animation", value)]))
            .theme_keys(&["--animate"])
            .static_values(vec![("none", vec![decl("animation", "none")])]),
    );

    // The filters.
    utilities.functional("filter", |candidate, _| {
        if candidate.modifier.is_some() {
            return None;
        }
        match &candidate.value {
            None => Some(vec![filter_properties(), decl("filter", CSS_FILTER_VALUE)]),
            Some(UtilityValue::Arbitrary { value, .. }) => Some(vec![decl("filter", value.as_str())]),
            Some(UtilityValue::Named { value, .. }) if value == "none" => Some(vec![decl("filter", "none")]),
            Some(UtilityValue::Named { .. }) => None,
        }
    });

    utilities.functional("backdrop-filter", |candidate, _| {
        if candidate.modifier.is_some() {
            return None;
        }
        match &candidate.value {
            None => Some(vec![
                backdrop_filter_properties(),
                decl("-webkit-backdrop-filter", CSS_BACKDROP_FILTER_VALUE),
                decl("backdrop-filter", CSS_BACKDROP_FILTER_VALUE),
            ]),
            Some(UtilityValue::Arbitrary { value, .. }) => {
                Some(vec![decl("-webkit-backdrop-filter", value.as_str()), decl("backdrop-filter", value.as_str())])
            }
            Some(UtilityValue::Named { value, .. }) if value == "none" => {
                Some(vec![decl("-webkit-backdrop-filter", "none"), decl("backdrop-filter", "none")])
            }
            Some(UtilityValue::Named { .. }) => None,
        }
    });

    functional_utility(
        utilities,
        "blur",
        UtilityDescription::new(|value, _, _| Some(filter_nodes("blur", "blur", value)))
            .theme_keys(&["--blur"])
            .static_values(vec![(
                "none",
                vec![filter_properties(), decl("--tw-blur", " "), decl("filter", CSS_FILTER_VALUE)],
            )]),
    );

    functional_utility(
        utilities,
        "backdrop-blur",
        UtilityDescription::new(|value, _, _| Some(backdrop_filter_nodes("blur", "blur", value)))
            .theme_keys(&["--backdrop-blur", "--blur"])
            .static_values(vec![(
                "none",
                vec![
                    backdrop_filter_properties(),
                    decl("--tw-backdrop-blur", " "),
                    decl("-webkit-backdrop-filter", CSS_BACKDROP_FILTER_VALUE),
                    decl("backdrop-filter", CSS_BACKDROP_FILTER_VALUE),
                ],
            )]),
    );

    let pair = |name, theme_key, backdrop_theme_key, unit, default_value, negative| FilterPair {
        name,
        theme_key,
        backdrop_theme_key,
        unit,
        default_value,
        negative,
    };
    filter_pair(utilities, pair("brightness", "--brightness", "--backdrop-brightness", "%", None, false));
    filter_pair(utilities, pair("contrast", "--contrast", "--backdrop-contrast", "%", None, false));
    filter_pair(utilities, pair("grayscale", "--grayscale", "--backdrop-grayscale", "%", Some("100%"), false));
    filter_pair(utilities, pair("hue-rotate", "--hue-rotate", "--backdrop-hue-rotate", "deg", None, true));
    filter_pair(utilities, pair("invert", "--invert", "--backdrop-invert", "%", Some("100%"), false));
    filter_pair(utilities, pair("saturate", "--saturate", "--backdrop-saturate", "%", None, false));
    filter_pair(utilities, pair("sepia", "--sepia", "--backdrop-sepia", "%", Some("100%"), false));

    static_utility(
        utilities,
        "drop-shadow-none",
        vec![
            StaticPart::Node(Box::new(filter_properties)),
            StaticPart::Decl("--tw-drop-shadow", " "),
            StaticPart::Decl("filter", CSS_FILTER_VALUE),
        ],
    );

    utilities.functional("drop-shadow", |candidate, ds| {
        let theme = &ds.theme;
        let mut alpha: Option<String> = None;
        if let Some(modifier) = &candidate.modifier {
            if modifier.kind == ValueKind::Arbitrary {
                alpha = Some(modifier.value.clone());
            } else if is_valid_opacity_value(&modifier.value) {
                alpha = Some(format!("{}%", modifier.value));
            }
        }
        let alpha = alpha.as_deref();
        // `candidate.modifier && !alpha`
        let bad_modifier = candidate.modifier.is_some() && !alpha.is_some_and(|a| !a.is_empty());
        let size_var = || decl("--tw-drop-shadow", "var(--tw-drop-shadow-size)");
        let color_nodes = |color: String| vec![filter_properties(), decl("--tw-drop-shadow-color", color), size_var()];

        let named = match &candidate.value {
            None => {
                let value = theme.get(&["--drop-shadow"])?;
                let resolved = theme.resolve(None, &["--drop-shadow"])?;
                if bad_modifier {
                    return None;
                }
                return Some(drop_shadow_size_nodes(&value, alpha, drop_shadow_list(&resolved)));
            }
            Some(UtilityValue::Arbitrary { data_type, value }) => {
                let ty =
                    data_type.as_deref().or_else(|| infer_data_type(value, &[DataType::Color]).map(DataType::as_str));
                return match ty {
                    Some("color") => {
                        let value = as_color(value, candidate.modifier.as_ref(), theme)?;
                        Some(color_nodes(with_alpha(&value, "var(--tw-drop-shadow-alpha)")))
                    }
                    _ => {
                        if bad_modifier {
                            return None;
                        }
                        Some(drop_shadow_size_nodes(value, alpha, "var(--tw-drop-shadow-size)".to_string()))
                    }
                };
            }
            Some(UtilityValue::Named { value, .. }) => value,
        };

        // The size of the shadow.
        let value = theme.get(&[&format!("--drop-shadow-{named}")]).filter(|v| !v.is_empty());
        let resolved = theme.resolve(Some(named), &["--drop-shadow"]).filter(|v| !v.is_empty());
        if let (Some(value), Some(resolved)) = (value, resolved) {
            if bad_modifier {
                return None;
            }
            let drop_shadow = if alpha.is_some_and(|a| !a.is_empty()) {
                "var(--tw-drop-shadow-size)".to_string()
            } else {
                drop_shadow_list(&resolved)
            };
            return Some(drop_shadow_size_nodes(&value, alpha, drop_shadow));
        }

        // The color of the shadow.
        let value =
            resolve_theme_color(candidate, theme, &["--drop-shadow-color", "--color"]).filter(|v| !v.is_empty())?;
        if value == "inherit" {
            return Some(color_nodes("inherit".to_string()));
        }
        Some(color_nodes(with_alpha(&value, "var(--tw-drop-shadow-alpha)")))
    });

    functional_utility(
        utilities,
        "backdrop-opacity",
        UtilityDescription::new(|value, _, _| Some(backdrop_filter_nodes("opacity", "opacity", value)))
            .theme_keys(&["--backdrop-opacity", "--opacity"])
            .bare(|v, _| {
                if !is_valid_opacity_value(v.value) {
                    return None;
                }
                Some(format!("{}%", v.value))
            }),
    );
}
