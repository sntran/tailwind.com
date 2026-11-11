//! A part of the utilities of `utilities.ts` of Tailwind CSS: from the
//! transition utilities to `@container`.

use super::{
    as_color, functional_utility, property, replace_alpha, resolve_theme_color, spacing_utility, static_decls,
    static_utility, with_alpha, Compiled, SpacingOptions, StaticPart, Utilities, UtilityDescription,
};
use crate::ast::{at_root, at_rule, decl, decl_full, rule, AstNode};
use crate::candidate::{CandidateModifier, FunctionalCandidate, UtilityValue, ValueKind};
use crate::theme::Theme;
use crate::utils::infer_data_type::{
    infer_data_type, is_positive_integer, is_valid_opacity_value, is_valid_spacing_multiplier, DataType,
};
use crate::utils::js::js_trim;
use crate::utils::replace_shadow_colors::replace_shadow_colors;
use crate::utils::segment::segment;

/// A static part that is a declaration.
fn d(property: &'static str, value: &'static str) -> StaticPart {
    StaticPart::Decl(property, value)
}

/// A static part that is a node from a function.
fn n(f: fn() -> AstNode) -> StaticPart {
    StaticPart::Node(Box::new(f))
}

/// An `@property` rule with no initial value.
fn prop(ident: &str) -> AstNode {
    property(ident, None, None)
}

/// A value that is not `null` and not the empty text (a truthy text).
fn truthy(value: Option<String>) -> Option<String> {
    value.filter(|v| !v.is_empty())
}

/// The data type of an arbitrary value: the type of the candidate, or the
/// first type of `types` that the value has.
fn type_of<'a>(data_type: Option<&'a str>, value: &str, types: &[DataType]) -> Option<&'a str> {
    match data_type {
        Some(t) => Some(t),
        None => infer_data_type(value, types).map(DataType::as_str),
    }
}

/// The opacity of a shadow from the modifier of the candidate.
fn shadow_alpha(modifier: Option<&CandidateModifier>) -> Option<String> {
    let modifier = modifier?;
    if modifier.kind == ValueKind::Arbitrary {
        Some(modifier.value.clone())
    } else if super::is_shadow_alpha(&modifier.value) {
        Some(format!("{}%", modifier.value))
    } else {
        None
    }
}

/// The declarations of a shadow, with the opacity `alpha` in its colors.
fn alpha_replaced_shadow_properties(
    property: &str,
    value: &str,
    alpha: Option<&str>,
    var_injector: &dyn Fn(&str) -> String,
    prefix: &str,
) -> Vec<AstNode> {
    let mut requires_fallback = false;
    let replaced_value = replace_shadow_colors(value, |color| {
        let Some(alpha) = alpha else {
            return var_injector(color);
        };

        // `currentcolor` uses `color-mix`.
        if color.starts_with("current") {
            return var_injector(&with_alpha(color, alpha));
        }

        // A dynamic value needs a fallback for the relative color syntax.
        if color.starts_with("var(") || alpha.starts_with("var(") {
            requires_fallback = true;
        }

        var_injector(&replace_alpha(color, alpha))
    });

    let apply_prefix = |x: String| -> String {
        if prefix.is_empty() {
            return x;
        }
        // Before 4.1.2, the prefix was only before the first shadow.
        if !crate::switches::INSET_SHADOW_LIST.on() {
            return format!("{} {x}", js_trim(prefix));
        }
        // Before 4.2.0, the prefix was `inset ` and each shadow stayed as
        // it was.
        if !crate::switches::INSET_SHADOW_LIST_SPACES.on() {
            return segment(&x, b',').into_iter().map(|value| format!("{} {value}", js_trim(prefix))).collect::<Vec<_>>().join(",");
        }
        segment(&x, b',')
            .into_iter()
            .map(|value| format!("{} {}", js_trim(prefix), js_trim(value)))
            .collect::<Vec<_>>()
            .join(", ")
    };

    if requires_fallback {
        vec![
            decl(property, apply_prefix(replace_shadow_colors(value, var_injector))),
            rule("@supports (color: lab(from red l a b))", vec![decl(property, apply_prefix(replaced_value))]),
        ]
    } else {
        vec![decl(property, apply_prefix(replaced_value))]
    }
}

const CSS_BOX_SHADOW_VALUE: &str =
    "var(--tw-inset-shadow), var(--tw-inset-ring-shadow), var(--tw-ring-offset-shadow), var(--tw-ring-shadow), var(--tw-shadow)";
const NULL_SHADOW: &str = "0 0 #0000";

fn text_shadow_properties() -> AstNode {
    at_root(vec![prop("--tw-text-shadow-color"), property("--tw-text-shadow-alpha", Some("100%"), Some("<percentage>"))])
}

fn box_shadow_properties() -> AstNode {
    at_root(vec![
        property("--tw-shadow", Some(NULL_SHADOW), None),
        prop("--tw-shadow-color"),
        property("--tw-shadow-alpha", Some("100%"), Some("<percentage>")),
        property("--tw-inset-shadow", Some(NULL_SHADOW), None),
        prop("--tw-inset-shadow-color"),
        property("--tw-inset-shadow-alpha", Some("100%"), Some("<percentage>")),
        prop("--tw-ring-color"),
        property("--tw-ring-shadow", Some(NULL_SHADOW), None),
        prop("--tw-inset-ring-color"),
        property("--tw-inset-ring-shadow", Some(NULL_SHADOW), None),
        // Legacy
        prop("--tw-ring-inset"),
        property("--tw-ring-offset-width", Some("0px"), Some("<length>")),
        property("--tw-ring-offset-color", Some("#fff"), None),
        property("--tw-ring-offset-shadow", Some(NULL_SHADOW), None),
    ])
}

/// The data of one shadow utility (`text-shadow`, `shadow`, `inset-shadow`).
struct ShadowUtility {
    /// The property that gets the shadow.
    property: &'static str,
    alpha_property: &'static str,
    color_property: &'static str,
    /// The theme key of the sizes.
    theme_key: &'static str,
    color_theme_keys: [&'static str; 2],
    /// The value of the property for `none`.
    none_value: &'static str,
    /// `true` when the utility also sets `box-shadow`.
    box_shadow: bool,
    /// The prefix of each shadow of an arbitrary value.
    arbitrary_prefix: &'static str,
    properties: fn() -> AstNode,
}

impl ShadowUtility {
    /// The rules of a shadow value.
    fn shadow(&self, value: &str, alpha: Option<&String>, prefix: &str) -> Vec<AstNode> {
        let color_property = self.color_property;
        let mut nodes = vec![(self.properties)(), decl_full(self.alpha_property, alpha.cloned(), false)];
        nodes.extend(alpha_replaced_shadow_properties(
            self.property,
            value,
            alpha.map(String::as_str),
            &|color| format!("var({color_property}, {color})"),
            prefix,
        ));
        if self.box_shadow {
            nodes.push(decl("box-shadow", CSS_BOX_SHADOW_VALUE));
        }
        nodes
    }

    /// The rules of a shadow color.
    fn color(&self, value: &str) -> Vec<AstNode> {
        vec![
            (self.properties)(),
            decl(self.color_property, with_alpha(value, &format!("var({})", self.alpha_property))),
        ]
    }

    fn compile(&self, candidate: &FunctionalCandidate, theme: &Theme) -> Option<Vec<AstNode>> {
        let alpha = shadow_alpha(candidate.modifier.as_ref());
        let invalid_alpha = candidate.modifier.is_some()
            && !alpha.as_ref().is_some_and(|a| !a.is_empty())
            && crate::switches::UNUSED_MODIFIER_IS_INVALID.on();

        let named = match &candidate.value {
            None => {
                let value = theme.get(&[self.theme_key])?;
                if invalid_alpha {
                    return None;
                }
                return Some(self.shadow(&value, alpha.as_ref(), ""));
            }
            Some(UtilityValue::Arbitrary { data_type, value }) => {
                return match type_of(data_type.as_deref(), value, &[DataType::Color]) {
                    Some("color") => {
                        let value = as_color(value, candidate.modifier.as_ref(), theme)?;
                        Some(self.color(&value))
                    }
                    _ => {
                        if invalid_alpha {
                            return None;
                        }
                        Some(self.shadow(value, alpha.as_ref(), self.arbitrary_prefix))
                    }
                };
            }
            Some(UtilityValue::Named { value, .. }) => value,
        };

        match named.as_str() {
            "none" => {
                if candidate.modifier.is_some() {
                    return None;
                }
                // Before 4.3.1, `inset-shadow-none` had no `inset`.
                let none_value = if self.arbitrary_prefix == "inset" && !crate::switches::INSET_SHADOW_NONE_INSET.on() {
                    NULL_SHADOW
                } else {
                    self.none_value
                };
                let mut nodes = vec![(self.properties)(), decl(self.property, none_value)];
                if self.box_shadow {
                    nodes.push(decl("box-shadow", CSS_BOX_SHADOW_VALUE));
                }
                return Some(nodes);
            }
            // Before 4.1.4, `inherit` was a color as each other.
            "inherit" if crate::switches::SHADOW_INHERIT.on() => {
                if candidate.modifier.is_some() {
                    return None;
                }
                return Some(vec![(self.properties)(), decl(self.color_property, "inherit")]);
            }
            _ => {}
        }

        // Shadow size
        if let Some(value) = truthy(theme.get(&[&format!("{}-{named}", self.theme_key)])) {
            if invalid_alpha {
                return None;
            }
            return Some(self.shadow(&value, alpha.as_ref(), ""));
        }

        // Shadow color
        let value = truthy(resolve_theme_color(candidate, theme, &self.color_theme_keys))?;
        Some(self.color(&value))
    }
}

/// The line height from the modifier of a `text-*` candidate. `None` is an
/// invalid modifier.
fn text_line_height(modifier: &CandidateModifier, theme: &Theme) -> Option<String> {
    let mut value = truthy(if modifier.kind == ValueKind::Arbitrary {
        Some(modifier.value.clone())
    } else {
        theme.resolve(Some(&modifier.value), &["--leading"])
    });

    if value.is_none() && is_valid_spacing_multiplier(&modifier.value) {
        truthy(theme.resolve(None, &["--spacing"]))?;
        value = Some(format!("--spacing({})", modifier.value));
    }

    // Shorthand for `leading-none`
    if value.is_none() && modifier.value == "none" {
        value = Some("1".to_string());
    }

    value
}

/// The `text-*` utility: a color or a font size.
fn compile_text(candidate: &FunctionalCandidate, theme: &Theme) -> Compiled {
    let named = match &candidate.value {
        None => return Compiled::Undefined,
        Some(UtilityValue::Arbitrary { data_type, value }) => {
            let types = [
                DataType::Color,
                DataType::Length,
                DataType::Percentage,
                DataType::AbsoluteSize,
                DataType::RelativeSize,
            ];
            return match type_of(data_type.as_deref(), value, &types) {
                Some("size" | "length" | "percentage" | "absolute-size" | "relative-size") => {
                    if let Some(modifier) = &candidate.modifier {
                        return match text_line_height(modifier, theme) {
                            Some(line_height) => {
                                Compiled::Nodes(vec![decl("font-size", value.as_str()), decl("line-height", line_height)])
                            }
                            None => Compiled::Null,
                        };
                    }
                    Compiled::Nodes(vec![decl("font-size", value.as_str())])
                }
                _ => as_color(value, candidate.modifier.as_ref(), theme).map(|value| vec![decl("color", value)]).into(),
            };
        }
        Some(UtilityValue::Named { value, .. }) => value,
    };

    // `color` property
    if let Some(value) = truthy(resolve_theme_color(candidate, theme, &["--text-color", "--color"])) {
        return Compiled::Nodes(vec![decl("color", value)]);
    }

    // `font-size` property
    let Some((font_size, options)) =
        theme.resolve_with(named, &["--text"], &["--line-height", "--letter-spacing", "--font-weight"])
    else {
        return Compiled::Undefined;
    };

    if let Some(modifier) = &candidate.modifier {
        return match text_line_height(modifier, theme) {
            Some(line_height) => Compiled::Nodes(vec![decl("font-size", font_size), decl("line-height", line_height)]),
            None => Compiled::Null,
        };
    }

    let option = |key: &str, variable: &str| -> Option<String> {
        options.iter().find(|(k, v)| k == key && !v.is_empty()).map(|(_, v)| format!("var({variable}, {v})"))
    };
    Compiled::Nodes(vec![
        decl("font-size", font_size),
        decl_full("line-height", option("--line-height", "--tw-leading"), false),
        decl_full("letter-spacing", option("--letter-spacing", "--tw-tracking"), false),
        decl_full("font-weight", option("--font-weight", "--tw-font-weight"), false),
    ])
}

fn transition_duration_property() -> AstNode {
    at_root(vec![prop("--tw-duration")])
}

fn transition_timing_function_property() -> AstNode {
    at_root(vec![prop("--tw-ease")])
}

fn css_contain_properties() -> AstNode {
    at_root(vec![
        prop("--tw-contain-size"),
        prop("--tw-contain-layout"),
        prop("--tw-contain-paint"),
        prop("--tw-contain-style"),
    ])
}

fn font_variant_numeric_properties() -> AstNode {
    at_root(vec![
        prop("--tw-ordinal"),
        prop("--tw-slashed-zero"),
        prop("--tw-numeric-figure"),
        prop("--tw-numeric-spacing"),
        prop("--tw-numeric-fraction"),
    ])
}

fn outline_properties() -> AstNode {
    at_root(vec![property("--tw-outline-style", Some("solid"), None)])
}

/// The rules of an outline width.
fn outline_width(value: impl Into<String>) -> Vec<AstNode> {
    vec![outline_properties(), decl("outline-style", "var(--tw-outline-style)"), decl("outline-width", value)]
}

/// A bare value that is a positive integer, with a unit.
fn bare_integer(unit: &'static str) -> impl Fn(&super::NamedValue, &crate::design_system::DesignSystem) -> Option<String> {
    move |v, _| {
        if !is_positive_integer(v.value) {
            return None;
        }
        Some(format!("{}{unit}", v.value))
    }
}

/// Register the utilities of this part.
pub fn register(utilities: &mut Utilities, theme: &Theme) {
    {
        let default_timing_function = format!(
            "var(--tw-ease, {})",
            theme.resolve(None, &["--default-transition-timing-function"]).unwrap_or_else(|| "ease".to_string())
        );
        let default_duration = format!(
            "var(--tw-duration, {})",
            theme.resolve(None, &["--default-transition-duration"]).unwrap_or_else(|| "0s".to_string())
        );
        let transition = move |value: &str| {
            vec![
                decl("transition-property", value),
                decl("transition-timing-function", default_timing_function.as_str()),
                decl("transition-duration", default_duration.as_str()),
            ]
        };
        const COLORS: &str = "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke, --tw-gradient-from, --tw-gradient-via, --tw-gradient-to";
        let static_values = vec![
            ("none", vec![decl("transition-property", "none")]),
            ("all", transition("all")),
            ("colors", transition(COLORS)),
            ("opacity", transition("opacity")),
            ("shadow", transition("box-shadow")),
            ("transform", transition("transform, translate, scale, rotate")),
        ];

        functional_utility(
            utilities,
            "transition",
            UtilityDescription::new(move |value, _, _| Some(transition(value)))
                .default_value(Some(
                    // Before 4.1.13, the list had `visibility`. Before
                    // 4.1.5, the list ended at `backdrop-filter`.
                    if crate::switches::TRANSITION_NO_VISIBILITY.on() {
                        "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke, --tw-gradient-from, --tw-gradient-via, --tw-gradient-to, opacity, box-shadow, transform, translate, scale, rotate, filter, -webkit-backdrop-filter, backdrop-filter, display, content-visibility, overlay, pointer-events"
                    } else if crate::switches::TRANSITION_DISPLAY.on() {
                        "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke, --tw-gradient-from, --tw-gradient-via, --tw-gradient-to, opacity, box-shadow, transform, translate, scale, rotate, filter, -webkit-backdrop-filter, backdrop-filter, display, visibility, content-visibility, overlay, pointer-events"
                    } else {
                        "color, background-color, border-color, outline-color, text-decoration-color, fill, stroke, --tw-gradient-from, --tw-gradient-via, --tw-gradient-to, opacity, box-shadow, transform, translate, scale, rotate, filter, -webkit-backdrop-filter, backdrop-filter"
                    },
                ))
                .theme_keys(&["--transition-property"])
                .static_values(static_values),
        );

        static_decls(utilities, "transition-discrete", &[("transition-behavior", "allow-discrete")]);
        static_decls(utilities, "transition-normal", &[("transition-behavior", "normal")]);

        functional_utility(
            utilities,
            "delay",
            UtilityDescription::new(|value, _, _| Some(vec![decl("transition-delay", value)]))
                .bare(bare_integer("ms"))
                .theme_keys(&["--transition-delay"]),
        );

        static_utility(
            utilities,
            "duration-initial",
            vec![n(transition_duration_property), d("--tw-duration", "initial")],
        );

        utilities.functional("duration", |candidate, ds| {
            // This utility has no modifier and no default value.
            if candidate.modifier.is_some() {
                return None;
            }
            let value = match candidate.value.as_ref()? {
                UtilityValue::Arbitrary { value, .. } => value.clone(),
                UtilityValue::Named { value, fraction } => {
                    match ds.theme.resolve(Some(fraction.as_deref().unwrap_or(value)), &["--transition-duration"]) {
                        Some(v) => v,
                        None if is_positive_integer(value) => format!("{value}ms"),
                        None => return None,
                    }
                }
            };

            Some(vec![
                transition_duration_property(),
                decl("--tw-duration", value.as_str()),
                decl("transition-duration", value),
            ])
        });
    }

    {
        let ease = |value: &str| {
            vec![transition_timing_function_property(), decl("--tw-ease", value), decl("transition-timing-function", value)]
        };
        functional_utility(
            utilities,
            "ease",
            UtilityDescription::new(move |value, _, _| Some(ease(value))).theme_keys(&["--ease"]).static_values(vec![
                ("initial", vec![transition_timing_function_property(), decl("--tw-ease", "initial")]),
                ("linear", ease("linear")),
            ]),
        );
    }

    static_decls(utilities, "will-change-auto", &[("will-change", "auto")]);
    static_decls(utilities, "will-change-scroll", &[("will-change", "scroll-position")]);
    static_decls(utilities, "will-change-contents", &[("will-change", "contents")]);
    static_decls(utilities, "will-change-transform", &[("will-change", "transform")]);
    functional_utility(
        utilities,
        "will-change",
        UtilityDescription::new(|value, _, _| Some(vec![decl("will-change", value)])),
    );

    static_decls(utilities, "content-none", &[("--tw-content", "none"), ("content", "none")]);
    functional_utility(
        utilities,
        "content",
        UtilityDescription::new(|value, _, _| {
            Some(vec![
                at_root(vec![property("--tw-content", Some("\"\""), None)]),
                decl("--tw-content", value),
                decl("content", "var(--tw-content)"),
            ])
        })
        // Before 4.1.18, `content-*` did not read the theme.
        .theme_keys(if crate::switches::CONTENT_THEME_KEY.on() { &["--content"] } else { &[] }),
    );

    {
        const CSS_CONTAIN_VALUE: &str =
            "var(--tw-contain-size, ) var(--tw-contain-layout, ) var(--tw-contain-paint, ) var(--tw-contain-style, )";

        static_decls(utilities, "contain-none", &[("contain", "none")]);
        static_decls(utilities, "contain-content", &[("contain", "content")]);
        static_decls(utilities, "contain-strict", &[("contain", "strict")]);

        for (name, variable, value) in [
            ("contain-size", "--tw-contain-size", "size"),
            ("contain-inline-size", "--tw-contain-size", "inline-size"),
            ("contain-layout", "--tw-contain-layout", "layout"),
            ("contain-paint", "--tw-contain-paint", "paint"),
            ("contain-style", "--tw-contain-style", "style"),
        ] {
            static_utility(
                utilities,
                name,
                vec![n(css_contain_properties), d(variable, value), d("contain", super::empty_fallbacks(CSS_CONTAIN_VALUE))],
            );
        }

        functional_utility(utilities, "contain", UtilityDescription::new(|value, _, _| Some(vec![decl("contain", value)])));
    }

    static_decls(utilities, "forced-color-adjust-none", &[("forced-color-adjust", "none")]);
    static_decls(utilities, "forced-color-adjust-auto", &[("forced-color-adjust", "auto")]);

    {
        let leading = |value: &str| {
            vec![at_root(vec![prop("--tw-leading")]), decl("--tw-leading", value), decl("line-height", value)]
        };
        spacing_utility(
            utilities,
            "leading",
            &["--leading", "--spacing"],
            move |value, _| Some(leading(value)),
            SpacingOptions { static_values: vec![("none", leading("1"))], ..Default::default() },
        );
    }

    functional_utility(
        utilities,
        "tracking",
        UtilityDescription::new(|value, _, _| {
            Some(vec![at_root(vec![prop("--tw-tracking")]), decl("--tw-tracking", value), decl("letter-spacing", value)])
        })
        .negative()
        .theme_keys(&["--tracking"]),
    );

    static_decls(
        utilities,
        "antialiased",
        &[("-webkit-font-smoothing", "antialiased"), ("-moz-osx-font-smoothing", "grayscale")],
    );
    static_decls(
        utilities,
        "subpixel-antialiased",
        &[("-webkit-font-smoothing", "auto"), ("-moz-osx-font-smoothing", "auto")],
    );

    {
        const CSS_FONT_VARIANT_NUMERIC_VALUE: &str = "var(--tw-ordinal, ) var(--tw-slashed-zero, ) var(--tw-numeric-figure, ) var(--tw-numeric-spacing, ) var(--tw-numeric-fraction, )";

        static_decls(utilities, "normal-nums", &[("font-variant-numeric", "normal")]);

        for (name, variable) in [
            ("ordinal", "--tw-ordinal"),
            ("slashed-zero", "--tw-slashed-zero"),
            ("lining-nums", "--tw-numeric-figure"),
            ("oldstyle-nums", "--tw-numeric-figure"),
            ("proportional-nums", "--tw-numeric-spacing"),
            ("tabular-nums", "--tw-numeric-spacing"),
            ("diagonal-fractions", "--tw-numeric-fraction"),
            ("stacked-fractions", "--tw-numeric-fraction"),
        ] {
            static_utility(
                utilities,
                name,
                vec![
                    n(font_variant_numeric_properties),
                    d(variable, name),
                    d("font-variant-numeric", super::empty_fallbacks(CSS_FONT_VARIANT_NUMERIC_VALUE)),
                ],
            );
        }
    }

    {
        utilities.r#static("outline-hidden", |_, _| {
            Some(vec![
                decl("--tw-outline-style", "none"),
                decl("outline-style", "none"),
                at_rule(
                    "@media",
                    "(forced-colors: active)",
                    vec![decl("outline", "2px solid transparent"), decl("outline-offset", "2px")],
                ),
            ])
        });

        for (name, style) in [
            ("outline-none", "none"),
            ("outline-solid", "solid"),
            ("outline-dashed", "dashed"),
            ("outline-dotted", "dotted"),
            ("outline-double", "double"),
        ] {
            static_decls(utilities, name, &[("--tw-outline-style", style), ("outline-style", style)]);
        }

        utilities.functional("outline", |candidate, ds| {
            let theme = &ds.theme;
            let named = match &candidate.value {
                None => {
                    if candidate.modifier.is_some() {
                        return None;
                    }
                    let value = theme.get(&["--default-outline-width"]).unwrap_or_else(|| "1px".to_string());
                    return Some(outline_width(value));
                }
                Some(UtilityValue::Arbitrary { data_type, value }) => {
                    let types = [DataType::Color, DataType::Length, DataType::Number, DataType::Percentage];
                    return match type_of(data_type.as_deref(), value, &types) {
                        Some("length" | "number" | "percentage") => {
                            if candidate.modifier.is_some() {
                                return None;
                            }
                            Some(outline_width(value.as_str()))
                        }
                        _ => {
                            let value = as_color(value, candidate.modifier.as_ref(), theme)?;
                            Some(vec![decl("outline-color", value)])
                        }
                    };
                }
                Some(UtilityValue::Named { value, .. }) => value,
            };

            // `outline-color` property
            if let Some(value) = truthy(resolve_theme_color(candidate, theme, &["--outline-color", "--color"])) {
                return Some(vec![decl("outline-color", value)]);
            }

            // `outline-width` property
            if candidate.modifier.is_some() {
                return None;
            }
            if let Some(value) = truthy(theme.resolve(Some(named), &["--outline-width"])) {
                Some(outline_width(value))
            } else if is_positive_integer(named) {
                Some(outline_width(format!("{named}px")))
            } else {
                None
            }
        });

        functional_utility(
            utilities,
            "outline-offset",
            UtilityDescription::new(|value, _, _| Some(vec![decl("outline-offset", value)]))
                .negative()
                .theme_keys(&["--outline-offset"])
                .bare(bare_integer("px")),
        );
    }

    functional_utility(
        utilities,
        "opacity",
        UtilityDescription::new(|value, _, _| Some(vec![decl("opacity", value)])).theme_keys(&["--opacity"]).bare(
            |v, _| {
                if !is_valid_opacity_value(v.value) {
                    return None;
                }
                Some(format!("{}%", v.value))
            },
        ),
    );

    functional_utility(
        utilities,
        "underline-offset",
        UtilityDescription::new(|value, _, _| Some(vec![decl("text-underline-offset", value)]))
            .negative()
            .theme_keys(&["--text-underline-offset"])
            .bare(bare_integer("px"))
            .static_values(vec![("auto", vec![decl("text-underline-offset", "auto")])]),
    );

    utilities.functional_full("text", |candidate, ds| compile_text(candidate, &ds.theme), None);

    static_utility(
        utilities,
        "text-shadow-initial",
        vec![n(text_shadow_properties), d("--tw-text-shadow-color", "initial")],
    );

    const TEXT_SHADOW: ShadowUtility = ShadowUtility {
        property: "text-shadow",
        alpha_property: "--tw-text-shadow-alpha",
        color_property: "--tw-text-shadow-color",
        theme_key: "--text-shadow",
        color_theme_keys: ["--text-shadow-color", "--color"],
        none_value: "none",
        box_shadow: false,
        arbitrary_prefix: "",
        properties: text_shadow_properties,
    };
    utilities.functional("text-shadow", |candidate, ds| TEXT_SHADOW.compile(candidate, &ds.theme));

    {
        static_utility(utilities, "shadow-initial", vec![n(box_shadow_properties), d("--tw-shadow-color", "initial")]);

        const SHADOW: ShadowUtility = ShadowUtility {
            property: "--tw-shadow",
            alpha_property: "--tw-shadow-alpha",
            color_property: "--tw-shadow-color",
            theme_key: "--shadow",
            color_theme_keys: ["--box-shadow-color", "--color"],
            none_value: NULL_SHADOW,
            box_shadow: true,
            arbitrary_prefix: "",
            properties: box_shadow_properties,
        };
        utilities.functional("shadow", |candidate, ds| SHADOW.compile(candidate, &ds.theme));

        static_utility(
            utilities,
            "inset-shadow-initial",
            vec![n(box_shadow_properties), d("--tw-inset-shadow-color", "initial")],
        );

        const INSET_SHADOW: ShadowUtility = ShadowUtility {
            property: "--tw-inset-shadow",
            alpha_property: "--tw-inset-shadow-alpha",
            color_property: "--tw-inset-shadow-color",
            theme_key: "--inset-shadow",
            color_theme_keys: ["--box-shadow-color", "--color"],
            none_value: "inset 0 0 #0000",
            box_shadow: true,
            arbitrary_prefix: "inset",
            properties: box_shadow_properties,
        };
        utilities.functional("inset-shadow", |candidate, ds| INSET_SHADOW.compile(candidate, &ds.theme));

        static_utility(utilities, "ring-inset", vec![n(box_shadow_properties), d("--tw-ring-inset", "inset")]);

        let default_ring_color = theme.get(&["--default-ring-color"]).unwrap_or_else(|| super::current_color().to_string());
        let ring_shadow = move |value: &str| {
            vec![
                box_shadow_properties(),
                decl(
                    "--tw-ring-shadow",
                    format!(
                        "{} 0 0 0 calc({value} + var(--tw-ring-offset-width)) var(--tw-ring-color, {default_ring_color})",
                        super::empty_fallbacks("var(--tw-ring-inset, )")
                    ),
                ),
                decl("box-shadow", CSS_BOX_SHADOW_VALUE),
            ]
        };
        utilities.functional("ring", move |candidate, ds| {
            let theme = &ds.theme;
            if candidate.value.is_none() {
                if candidate.modifier.is_some() {
                    return None;
                }
                let value = theme.get(&["--default-ring-width"]).unwrap_or_else(|| "1px".to_string());
                return Some(ring_shadow(&value));
            }
            ring_or_color(candidate, theme, "--tw-ring-color", &ring_shadow)
        });

        let inset_ring_shadow = |value: &str| {
            vec![
                box_shadow_properties(),
                decl(
                    "--tw-inset-ring-shadow",
                    format!("inset 0 0 0 {value} var(--tw-inset-ring-color, {})", super::current_color()),
                ),
                decl("box-shadow", CSS_BOX_SHADOW_VALUE),
            ]
        };
        utilities.functional("inset-ring", move |candidate, ds| {
            if candidate.value.is_none() {
                if candidate.modifier.is_some() {
                    return None;
                }
                return Some(inset_ring_shadow("1px"));
            }
            ring_or_color(candidate, &ds.theme, "--tw-inset-ring-color", &inset_ring_shadow)
        });

        const RING_OFFSET_SHADOW_VALUE: &str =
            "var(--tw-ring-inset, ) 0 0 0 var(--tw-ring-offset-width) var(--tw-ring-offset-color)";
        let ring_offset_width = |value: String| {
            vec![decl("--tw-ring-offset-width", value), decl("--tw-ring-offset-shadow", super::empty_fallbacks(RING_OFFSET_SHADOW_VALUE))]
        };
        utilities.functional("ring-offset", move |candidate, ds| {
            let theme = &ds.theme;
            let named = match candidate.value.as_ref()? {
                UtilityValue::Arbitrary { data_type, value } => {
                    return match type_of(data_type.as_deref(), value, &[DataType::Color, DataType::Length]) {
                        Some("length") => {
                            if candidate.modifier.is_some() {
                                return None;
                            }
                            Some(ring_offset_width(value.clone()))
                        }
                        _ => {
                            let value = as_color(value, candidate.modifier.as_ref(), theme)?;
                            Some(vec![decl("--tw-ring-offset-color", value)])
                        }
                    };
                }
                UtilityValue::Named { value, .. } => value,
            };

            // `--tw-ring-offset-width` property
            if let Some(value) = truthy(theme.resolve(Some(named), &["--ring-offset-width"])) {
                if candidate.modifier.is_some() {
                    return None;
                }
                return Some(ring_offset_width(value));
            } else if is_positive_integer(named) {
                if candidate.modifier.is_some() {
                    return None;
                }
                return Some(ring_offset_width(format!("{named}px")));
            }

            // `--tw-ring-offset-color` property
            let value = truthy(resolve_theme_color(candidate, theme, &["--ring-offset-color", "--color"]))?;
            Some(vec![decl("--tw-ring-offset-color", value)])
        });
    }

    utilities.functional("@container", |candidate, _| {
        let value = match &candidate.value {
            None => "inline-size",
            Some(UtilityValue::Arbitrary { value, .. }) => value.as_str(),
            Some(UtilityValue::Named { value, .. }) if value == "normal" => "normal",
            Some(UtilityValue::Named { value, .. }) if value == "size" && crate::switches::CONTAINER_SIZE.on() => "size",
            Some(_) => return None,
        };

        if let Some(modifier) = &candidate.modifier {
            return Some(vec![decl("container-type", value), decl("container-name", modifier.value.as_str())]);
        }

        Some(vec![decl("container-type", value)])
    });
}

/// The rules of `ring-*` and `inset-ring-*` for a candidate with a value:
/// a ring width (through `shadow`) or a ring color.
fn ring_or_color(
    candidate: &FunctionalCandidate,
    theme: &Theme,
    color_property: &str,
    shadow: &dyn Fn(&str) -> Vec<AstNode>,
) -> Option<Vec<AstNode>> {
    let named = match candidate.value.as_ref()? {
        UtilityValue::Arbitrary { data_type, value } => {
            return match type_of(data_type.as_deref(), value, &[DataType::Color, DataType::Length]) {
                Some("length") => {
                    if candidate.modifier.is_some() {
                        return None;
                    }
                    Some(shadow(value))
                }
                _ => {
                    let value = as_color(value, candidate.modifier.as_ref(), theme)?;
                    Some(vec![decl(color_property, value)])
                }
            };
        }
        UtilityValue::Named { value, .. } => value,
    };

    // Ring color
    if let Some(value) = truthy(resolve_theme_color(candidate, theme, &["--ring-color", "--color"])) {
        return Some(vec![decl(color_property, value)]);
    }

    // Ring width
    if candidate.modifier.is_some() {
        return None;
    }
    let mut value = theme.resolve(Some(named), &["--ring-width"]);
    if value.is_none() && is_positive_integer(named) {
        value = Some(format!("{named}px"));
    }
    let value = truthy(value)?;
    Some(shadow(&value))
}
