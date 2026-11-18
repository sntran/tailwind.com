//! The suggestions of the utilities for the editor tools: the `suggest`
//! calls of `utilities.ts`, in their sequence. The functions that make
//! utilities (`functional_utility`, `color_utility`, `spacing_utility`)
//! add their own suggestions. The class list (`intellisense.rs`) and the
//! canonical form of candidates use them.
//!
//! Only the sequence of the groups of one utility changes the class
//! list. Upstream calls `suggest` for a utility after the function that
//! makes it, so the calls of this file come after all the utilities.

use super::{one_to, steps, suggest, Suggestion, Utilities, DEFAULT_SPACING_SUGGESTIONS};

fn group(values: &[&str]) -> Suggestion {
    Suggestion::default().values(values)
}

/// The values of the scale utilities.
const SCALES: &[&str] = &["0", "50", "75", "90", "95", "100", "105", "110", "125", "150", "200"];
/// The values of the rotate utilities.
const ROTATIONS: &[&str] = &["0", "1", "2", "3", "6", "12", "45", "90", "180"];
/// The values of the skew utilities.
const SKEWS: &[&str] = &["0", "1", "2", "3", "6", "12"];
/// The modifiers of the gradients: `suggestedModifiers`.
const GRADIENT_MODIFIERS: &[&str] = &["oklab", "oklch", "srgb", "hsl", "longer", "shorter", "increasing", "decreasing"];
/// The angles of the gradients.
const GRADIENT_ANGLES: &[&str] = &["0", "30", "60", "90", "120", "150", "180", "210", "240", "270", "300", "330"];

fn gradient_modifiers() -> Vec<String> {
    GRADIENT_MODIFIERS.iter().map(|m| m.to_string()).collect()
}

/// The `suggest` calls of `utilities.ts` that are not in a function that
/// makes utilities, and the calls of `borderSideUtility`,
/// `gradientStopUtility` and `maskStopUtility`.
pub(super) fn register(u: &mut Utilities) {
    suggest(u, "z", |_| vec![group(&["0", "10", "20", "30", "40", "50"]).negative().theme_keys(&["--z-index"])]);
    suggest(u, "order", |_| vec![Suggestion::default().negative().values_owned(one_to(12)).theme_keys(&["--order"])]);
    suggest(u, "col-span", |_| vec![Suggestion::default().values_owned(one_to(12))]);
    suggest(u, "col-start", |_| vec![Suggestion::default().negative().values_owned(one_to(13)).theme_keys(&["--grid-column-start"])]);
    suggest(u, "col-end", |_| vec![Suggestion::default().negative().values_owned(one_to(13)).theme_keys(&["--grid-column-end"])]);
    suggest(u, "row-span", |_| vec![Suggestion::default().values_owned(one_to(12))]);
    suggest(u, "row-start", |_| vec![Suggestion::default().negative().values_owned(one_to(13)).theme_keys(&["--grid-row-start"])]);
    suggest(u, "row-end", |_| vec![Suggestion::default().negative().values_owned(one_to(13)).theme_keys(&["--grid-row-end"])]);
    suggest(u, "line-clamp", |_| vec![group(&["1", "2", "3", "4", "5", "6"]).theme_keys(&["--line-clamp"])]);
    suggest(u, "flex", |_| vec![Suggestion::default().fractions(), Suggestion::default().values_owned(one_to(12))]);
    suggest(u, "shrink", |_| vec![group(&["0"]).default_value()]);
    suggest(u, "grow", |_| vec![group(&["0"]).default_value()]);
    suggest(u, "scale", |_| vec![group(SCALES).negative().theme_keys(&["--scale"])]);
    for axis in ["x", "y", "z"] {
        suggest(u, &format!("scale-{axis}"), |_| vec![group(SCALES).negative().theme_keys(&["--scale"])]);
    }
    suggest(u, "rotate", |_| vec![group(ROTATIONS).negative().theme_keys(&["--rotate"])]);
    for axis in ["x", "y", "z"] {
        suggest(u, &format!("rotate-{axis}"), |_| vec![group(ROTATIONS).negative().theme_keys(&["--rotate"])]);
    }
    suggest(u, "skew", |_| vec![group(SKEWS).negative().theme_keys(&["--skew"])]);
    suggest(u, "skew-x", |_| vec![group(SKEWS).negative().theme_keys(&["--skew"])]);
    suggest(u, "skew-y", |_| vec![group(SKEWS).negative().theme_keys(&["--skew"])]);
    suggest(u, "transform", |_| vec![Suggestion::default().default_value()]);
    suggest(u, "zoom", |_| vec![group(&["50", "75", "90", "95", "100", "105", "110", "125", "150", "200"])]);
    suggest(u, "columns", |_| vec![Suggestion::default().values_owned(one_to(12)).theme_keys(&["--columns", "--container"])]);
    suggest(u, "grid-cols", |_| vec![Suggestion::default().values_owned(one_to(12)).theme_keys(&["--grid-template-columns"])]);
    suggest(u, "grid-rows", |_| vec![Suggestion::default().values_owned(one_to(12)).theme_keys(&["--grid-template-rows"])]);
    suggest(u, "tab", |_| vec![group(&["2", "4", "8"])]);

    // `borderSideUtility`.
    for root in ["border", "border-x", "border-y", "border-s", "border-e", "border-bs", "border-be", "border-t", "border-r", "border-b", "border-l"] {
        suggest(u, root, |_| {
            vec![
                Suggestion::color(&["--border-color", "--color"]).default_value(),
                group(&["0", "2", "4", "8"]).theme_keys(&["--border-width"]),
            ]
        });
    }

    suggest(u, "divide-x", |_| vec![group(&["0", "2", "4", "8"]).theme_keys(&["--divide-width", "--border-width"]).default_value()]);
    suggest(u, "divide-y", |_| vec![group(&["0", "2", "4", "8"]).theme_keys(&["--divide-width", "--border-width"]).default_value()]);

    suggest(u, "bg-linear", |_| {
        vec![
            group(&["to-t", "to-tr", "to-r", "to-br", "to-b", "to-bl", "to-l", "to-tl"]).modifiers(gradient_modifiers()),
            group(GRADIENT_ANGLES).negative().modifiers(gradient_modifiers()),
        ]
    });
    suggest(u, "bg-conic", |_| {
        vec![
            Suggestion::default().default_value().modifiers(gradient_modifiers()),
            group(GRADIENT_ANGLES).negative().modifiers(gradient_modifiers()),
        ]
    });
    suggest(u, "bg-radial", |_| vec![Suggestion::default().default_value().modifiers(gradient_modifiers())]);
    suggest(u, "bg", |_| {
        vec![Suggestion::color(&["--background-color", "--color"]), Suggestion::default().theme_keys(&["--background-image"])]
    });

    // `gradientStopUtility`.
    for root in ["from", "via", "to"] {
        suggest(u, root, |_| {
            vec![
                Suggestion::color(&["--background-color", "--color"]),
                Suggestion::default().values_owned(steps(21, 5, "%")).theme_keys(&["--gradient-color-stop-positions"]),
            ]
        });
    }

    // `maskStopUtility`, also through `maskEdgeUtility`.
    let mask_stops = |u: &mut Utilities, root: &str| {
        suggest(u, root, |_| {
            vec![
                Suggestion::color(&["--background-color", "--color"]),
                Suggestion::default().values_owned(steps(21, 5, "%")).theme_keys(&["--gradient-color-stop-positions"]),
            ]
        });
        suggest(u, root, |theme| {
            let spacing = theme.get(&["--spacing"]).is_some_and(|v| !v.is_empty());
            vec![
                Suggestion::default().values_owned(steps(21, 5, "%")),
                group(if spacing { DEFAULT_SPACING_SUGGESTIONS } else { &[] }),
                Suggestion::color(&["--background-color", "--color"]),
            ]
        });
    };
    for root in [
        "mask-x-from", "mask-x-to", "mask-y-from", "mask-y-to", "mask-t-from", "mask-t-to", "mask-r-from", "mask-r-to", "mask-b-from",
        "mask-b-to", "mask-l-from", "mask-l-to",
    ] {
        mask_stops(u, root);
    }
    suggest(u, "mask-linear", |_| vec![group(ROTATIONS).negative()]);
    mask_stops(u, "mask-linear-from");
    mask_stops(u, "mask-linear-to");
    mask_stops(u, "mask-radial-from");
    mask_stops(u, "mask-radial-to");
    suggest(u, "mask-conic", |_| vec![group(ROTATIONS).negative()]);
    mask_stops(u, "mask-conic-from");
    mask_stops(u, "mask-conic-to");

    suggest(u, "fill", |_| vec![Suggestion::color(&["--fill", "--color"])]);
    suggest(u, "stroke", |_| {
        vec![Suggestion::color(&["--stroke", "--color"]), group(&["0", "1", "2", "3"]).theme_keys(&["--stroke-width"])]
    });
    suggest(u, "font", |_| vec![Suggestion::default().theme_keys(&["--font"]), Suggestion::default().theme_keys(&["--font-weight"])]);
    suggest(u, "font-stretch", |_| vec![group(&["50%", "75%", "90%", "95%", "100%", "105%", "110%", "125%", "150%", "200%"])]);
    suggest(u, "decoration", |_| {
        vec![
            Suggestion::color(&["--text-decoration-color", "--color"]),
            group(&["0", "1", "2"]).theme_keys(&["--text-decoration-thickness"]),
        ]
    });

    // The filters.
    let brightness = &["0", "50", "75", "90", "95", "100", "105", "110", "125", "150", "200"];
    suggest(u, "brightness", |_| vec![group(brightness).theme_keys(&["--brightness"])]);
    suggest(u, "backdrop-brightness", |_| vec![group(brightness).theme_keys(&["--backdrop-brightness", "--brightness"])]);
    let contrast = &["0", "50", "75", "100", "125", "150", "200"];
    suggest(u, "contrast", |_| vec![group(contrast).theme_keys(&["--contrast"])]);
    suggest(u, "backdrop-contrast", |_| vec![group(contrast).theme_keys(&["--backdrop-contrast", "--contrast"])]);
    let quarters = &["0", "25", "50", "75", "100"];
    suggest(u, "grayscale", |_| vec![group(quarters).theme_keys(&["--grayscale"]).default_value()]);
    suggest(u, "backdrop-grayscale", |_| vec![group(quarters).theme_keys(&["--backdrop-grayscale", "--grayscale"]).default_value()]);
    let hues = &["0", "15", "30", "60", "90", "180"];
    suggest(u, "hue-rotate", |_| vec![group(hues).theme_keys(&["--hue-rotate"])]);
    suggest(u, "backdrop-hue-rotate", |_| vec![group(hues).theme_keys(&["--backdrop-hue-rotate", "--hue-rotate"])]);
    suggest(u, "invert", |_| vec![group(quarters).theme_keys(&["--invert"]).default_value()]);
    suggest(u, "backdrop-invert", |_| vec![group(quarters).theme_keys(&["--backdrop-invert", "--invert"]).default_value()]);
    let saturations = &["0", "50", "100", "150", "200"];
    suggest(u, "saturate", |_| vec![group(saturations).theme_keys(&["--saturate"])]);
    suggest(u, "backdrop-saturate", |_| vec![group(saturations).theme_keys(&["--backdrop-saturate", "--saturate"])]);
    let sepias = &["0", "50", "100"];
    suggest(u, "sepia", |_| vec![group(sepias).theme_keys(&["--sepia"]).default_value()]);
    suggest(u, "backdrop-sepia", |_| vec![group(sepias).theme_keys(&["--backdrop-sepia", "--sepia"]).default_value()]);
    suggest(u, "drop-shadow", |_| {
        vec![Suggestion::color(&["--drop-shadow-color", "--color"]), Suggestion::default().theme_keys(&["--drop-shadow"])]
    });
    suggest(u, "backdrop-opacity", |_| {
        vec![Suggestion::default().values_owned(steps(21, 5, "")).theme_keys(&["--backdrop-opacity", "--opacity"])]
    });

    let times = &["75", "100", "150", "200", "300", "500", "700", "1000"];
    suggest(u, "delay", |_| vec![group(times).theme_keys(&["--transition-delay"])]);
    suggest(u, "duration", |_| vec![group(times).theme_keys(&["--transition-duration"])]);

    let widths = &["0", "1", "2", "4", "8"];
    suggest(u, "outline", |_| {
        vec![
            Suggestion::color(&["--outline-color", "--color"]).default_value(),
            group(widths).theme_keys(&["--outline-width"]),
        ]
    });
    suggest(u, "outline-offset", |_| vec![group(widths).negative().theme_keys(&["--outline-offset"])]);
    suggest(u, "opacity", |_| vec![Suggestion::default().values_owned(steps(21, 5, "")).theme_keys(&["--opacity"])]);
    suggest(u, "underline-offset", |_| vec![group(widths).negative().theme_keys(&["--text-underline-offset"])]);
    suggest(u, "text", |_| {
        vec![
            Suggestion::color(&["--text-color", "--color"]),
            Suggestion::default().theme_keys(&["--text"]).modifier_theme_keys(&["--leading"]),
        ]
    });
    suggest(u, "text-shadow", |theme| {
        vec![
            Suggestion::color(&["--text-shadow-color", "--color"]),
            group(&["none"]),
            Suggestion::default()
                .theme_keys(&["--text-shadow"])
                .modifiers(steps(21, 5, ""))
                .default_value_if(theme.get(&["--text-shadow"]).is_some()),
        ]
    });
    suggest(u, "shadow", |theme| {
        vec![
            Suggestion::color(&["--box-shadow-color", "--color"]),
            group(&["none"]),
            Suggestion::default()
                .theme_keys(&["--shadow"])
                .modifiers(steps(21, 5, ""))
                .default_value_if(theme.get(&["--shadow"]).is_some()),
        ]
    });
    suggest(u, "inset-shadow", |theme| {
        vec![
            Suggestion::color(&["--box-shadow-color", "--color"]),
            group(&["none"]),
            Suggestion::default()
                .theme_keys(&["--inset-shadow"])
                .modifiers(steps(21, 5, ""))
                .default_value_if(theme.get(&["--inset-shadow"]).is_some()),
        ]
    });
    suggest(u, "ring", |_| {
        vec![Suggestion::color(&["--ring-color", "--color"]), group(widths).theme_keys(&["--ring-width"]).default_value()]
    });
    suggest(u, "inset-ring", |_| {
        vec![Suggestion::color(&["--ring-color", "--color"]), group(widths).theme_keys(&["--ring-width"]).default_value()]
    });
    suggest(u, "ring-offset", |_| {
        vec![Suggestion::color(&["--ring-offset-color", "--color"]), group(widths).theme_keys(&["--ring-offset-width"])]
    });
    suggest(u, "@container", |_| vec![group(&["normal"]).default_value()]);
}
