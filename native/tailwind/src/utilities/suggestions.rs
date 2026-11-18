//! The suggestions of the utilities for the editor tools: the `suggest`
//! calls of `utilities.ts`, in their sequence, as data. The functions that
//! make utilities (`functional_utility`, `color_utility`,
//! `spacing_utility`) add their own suggestions. The class list
//! (`intellisense.rs`) and the canonical form of candidates use them.
//!
//! Only the sequence of the groups of one utility changes the class list.
//! Upstream calls `suggest` for a utility after the function that makes
//! it, so the calls of this file come after all the utilities.

use super::{color, suggest, DefaultSuggestion, Group, Utilities, Values, GROUP, OPACITY_MODIFIERS};

const fn list(values: &'static [&'static str]) -> Values {
    Values::List(values)
}

/// The percentages: `0%` to `100%` in steps of 5.
const PERCENTAGES: Values = Values::Steps { length: 21, step: 5, unit: "%" };
const SCALES: Values = list(&["0", "50", "75", "90", "95", "100", "105", "110", "125", "150", "200"]);
const ROTATIONS: Values = list(&["0", "1", "2", "3", "6", "12", "45", "90", "180"]);
const SKEWS: Values = list(&["0", "1", "2", "3", "6", "12"]);
/// `suggestedModifiers` of the gradients.
const GRADIENT_MODIFIERS: Values = list(&["oklab", "oklch", "srgb", "hsl", "longer", "shorter", "increasing", "decreasing"]);
const GRADIENT_ANGLES: Values = list(&["0", "30", "60", "90", "120", "150", "180", "210", "240", "270", "300", "330"]);
const BORDER_WIDTHS: Values = list(&["0", "2", "4", "8"]);
const WIDTHS: Values = list(&["0", "1", "2", "4", "8"]);
const QUARTERS: Values = list(&["0", "25", "50", "75", "100"]);
const TIMES: Values = list(&["75", "100", "150", "200", "300", "500", "700", "1000"]);
const BRIGHTNESS: Values = list(&["0", "50", "75", "90", "95", "100", "105", "110", "125", "150", "200"]);
const CONTRASTS: Values = list(&["0", "50", "75", "100", "125", "150", "200"]);
const HUES: Values = list(&["0", "15", "30", "60", "90", "180"]);
const SATURATIONS: Values = list(&["0", "50", "100", "150", "200"]);
const SEPIAS: Values = list(&["0", "50", "100"]);

const fn values(values: Values, theme_keys: &'static [&'static str]) -> Group {
    Group { values, theme_keys, ..GROUP }
}

const fn negative(values: Values, theme_keys: &'static [&'static str]) -> Group {
    Group { values, theme_keys, negative: true, ..GROUP }
}

const fn with_default(group: Group) -> Group {
    Group { default: DefaultSuggestion::Yes, ..group }
}

/// The colors of a background, and the positions of a gradient stop.
const STOP: &[Group] = &[color(&["--background-color", "--color"]), values(PERCENTAGES, &["--gradient-color-stop-positions"])];

/// The two `suggest` calls of `maskStopUtility`.
const MASK_STOP: &[Group] = STOP;
const MASK_STOP_VALUES: &[Group] =
    &[values(PERCENTAGES, &[]), values(Values::Spacing, &[]), color(&["--background-color", "--color"])];

/// The angles of `mask-linear` and `mask-conic`.
const MASK_ANGLES: &[Group] = &[negative(ROTATIONS, &[])];

/// The groups of a shadow (`shadow`, `inset-shadow`, `text-shadow`).
const fn shadow(colors: &'static [&'static str], key: &'static str, keys: &'static [&'static str]) -> [Group; 3] {
    [
        color(colors),
        values(list(&["none"]), &[]),
        Group { theme_keys: keys, modifiers: OPACITY_MODIFIERS, default: DefaultSuggestion::IfTheme(key), ..GROUP },
    ]
}

/// Each `suggest` call: the utilities and their groups.
const TABLE: &[(&[&str], &[Group])] = &[
    (&["z"], &[negative(list(&["0", "10", "20", "30", "40", "50"]), &["--z-index"])]),
    (&["order"], &[negative(Values::OneTo(12), &["--order"])]),
    (&["col-span"], &[values(Values::OneTo(12), &[])]),
    (&["col-start"], &[negative(Values::OneTo(13), &["--grid-column-start"])]),
    (&["col-end"], &[negative(Values::OneTo(13), &["--grid-column-end"])]),
    (&["row-span"], &[values(Values::OneTo(12), &[])]),
    (&["row-start"], &[negative(Values::OneTo(13), &["--grid-row-start"])]),
    (&["row-end"], &[negative(Values::OneTo(13), &["--grid-row-end"])]),
    (&["line-clamp"], &[values(list(&["1", "2", "3", "4", "5", "6"]), &["--line-clamp"])]),
    (&["flex"], &[Group { fractions: true, ..GROUP }, values(Values::OneTo(12), &[])]),
    (&["shrink"], &[with_default(values(list(&["0"]), &[]))]),
    (&["grow"], &[with_default(values(list(&["0"]), &[]))]),
    (&["scale", "scale-x", "scale-y", "scale-z"], &[negative(SCALES, &["--scale"])]),
    (&["rotate", "rotate-x", "rotate-y", "rotate-z"], &[negative(ROTATIONS, &["--rotate"])]),
    (&["skew", "skew-x", "skew-y"], &[negative(SKEWS, &["--skew"])]),
    (&["transform"], &[with_default(GROUP)]),
    (&["zoom"], &[values(list(&["50", "75", "90", "95", "100", "105", "110", "125", "150", "200"]), &[])]),
    (&["columns"], &[values(Values::OneTo(12), &["--columns", "--container"])]),
    (&["grid-cols"], &[values(Values::OneTo(12), &["--grid-template-columns"])]),
    (&["grid-rows"], &[values(Values::OneTo(12), &["--grid-template-rows"])]),
    (&["tab"], &[values(list(&["2", "4", "8"]), &[])]),
    // `borderSideUtility`.
    (
        &["border", "border-x", "border-y", "border-s", "border-e", "border-bs", "border-be", "border-t", "border-r", "border-b", "border-l"],
        &[with_default(color(&["--border-color", "--color"])), values(BORDER_WIDTHS, &["--border-width"])],
    ),
    (&["divide-x", "divide-y"], &[with_default(values(BORDER_WIDTHS, &["--divide-width", "--border-width"]))]),
    (
        &["bg-linear"],
        &[
            Group { values: list(&["to-t", "to-tr", "to-r", "to-br", "to-b", "to-bl", "to-l", "to-tl"]), modifiers: GRADIENT_MODIFIERS, ..GROUP },
            Group { values: GRADIENT_ANGLES, negative: true, modifiers: GRADIENT_MODIFIERS, ..GROUP },
        ],
    ),
    (
        &["bg-conic"],
        &[
            Group { default: DefaultSuggestion::Yes, modifiers: GRADIENT_MODIFIERS, ..GROUP },
            Group { values: GRADIENT_ANGLES, negative: true, modifiers: GRADIENT_MODIFIERS, ..GROUP },
        ],
    ),
    (&["bg-radial"], &[Group { default: DefaultSuggestion::Yes, modifiers: GRADIENT_MODIFIERS, ..GROUP }]),
    (&["bg"], &[color(&["--background-color", "--color"]), values(Values::None, &["--background-image"])]),
    // `gradientStopUtility`.
    (&["from", "via", "to"], STOP),
];

/// The `suggest` calls of the mask utilities, and of the utilities after
/// them. Each mask utility has two calls, in this sequence.
const MASK_ROOTS: &[&str] = &[
    "mask-x-from", "mask-x-to", "mask-y-from", "mask-y-to", "mask-t-from", "mask-t-to", "mask-r-from", "mask-r-to", "mask-b-from",
    "mask-b-to", "mask-l-from", "mask-l-to",
];

const SHADOW: [Group; 3] = shadow(&["--box-shadow-color", "--color"], "--shadow", &["--shadow"]);
const INSET_SHADOW: [Group; 3] = shadow(&["--box-shadow-color", "--color"], "--inset-shadow", &["--inset-shadow"]);
const TEXT_SHADOW: [Group; 3] = shadow(&["--text-shadow-color", "--color"], "--text-shadow", &["--text-shadow"]);

const TABLE_AFTER_MASKS: &[(&[&str], &[Group])] = &[
    (&["fill"], &[color(&["--fill", "--color"])]),
    (&["stroke"], &[color(&["--stroke", "--color"]), values(list(&["0", "1", "2", "3"]), &["--stroke-width"])]),
    (&["font"], &[values(Values::None, &["--font"]), values(Values::None, &["--font-weight"])]),
    (&["font-stretch"], &[values(list(&["50%", "75%", "90%", "95%", "100%", "105%", "110%", "125%", "150%", "200%"]), &[])]),
    (&["decoration"], &[color(&["--text-decoration-color", "--color"]), values(list(&["0", "1", "2"]), &["--text-decoration-thickness"])]),
    (&["brightness"], &[values(BRIGHTNESS, &["--brightness"])]),
    (&["backdrop-brightness"], &[values(BRIGHTNESS, &["--backdrop-brightness", "--brightness"])]),
    (&["contrast"], &[values(CONTRASTS, &["--contrast"])]),
    (&["backdrop-contrast"], &[values(CONTRASTS, &["--backdrop-contrast", "--contrast"])]),
    (&["grayscale"], &[with_default(values(QUARTERS, &["--grayscale"]))]),
    (&["backdrop-grayscale"], &[with_default(values(QUARTERS, &["--backdrop-grayscale", "--grayscale"]))]),
    (&["hue-rotate"], &[values(HUES, &["--hue-rotate"])]),
    (&["backdrop-hue-rotate"], &[values(HUES, &["--backdrop-hue-rotate", "--hue-rotate"])]),
    (&["invert"], &[with_default(values(QUARTERS, &["--invert"]))]),
    (&["backdrop-invert"], &[with_default(values(QUARTERS, &["--backdrop-invert", "--invert"]))]),
    (&["saturate"], &[values(SATURATIONS, &["--saturate"])]),
    (&["backdrop-saturate"], &[values(SATURATIONS, &["--backdrop-saturate", "--saturate"])]),
    (&["sepia"], &[with_default(values(SEPIAS, &["--sepia"]))]),
    (&["backdrop-sepia"], &[with_default(values(SEPIAS, &["--backdrop-sepia", "--sepia"]))]),
    (&["drop-shadow"], &[color(&["--drop-shadow-color", "--color"]), values(Values::None, &["--drop-shadow"])]),
    (&["backdrop-opacity"], &[values(OPACITY_MODIFIERS, &["--backdrop-opacity", "--opacity"])]),
    (&["delay"], &[values(TIMES, &["--transition-delay"])]),
    (&["duration"], &[values(TIMES, &["--transition-duration"])]),
    (&["outline"], &[with_default(color(&["--outline-color", "--color"])), values(WIDTHS, &["--outline-width"])]),
    (&["outline-offset"], &[negative(WIDTHS, &["--outline-offset"])]),
    (&["opacity"], &[values(OPACITY_MODIFIERS, &["--opacity"])]),
    (&["underline-offset"], &[negative(WIDTHS, &["--text-underline-offset"])]),
    (
        &["text"],
        &[color(&["--text-color", "--color"]), Group { theme_keys: &["--text"], modifier_theme_keys: &["--leading"], ..GROUP }],
    ),
    (&["text-shadow"], &TEXT_SHADOW),
    (&["shadow"], &SHADOW),
    (&["inset-shadow"], &INSET_SHADOW),
    (&["ring", "inset-ring"], &[color(&["--ring-color", "--color"]), with_default(values(WIDTHS, &["--ring-width"]))]),
    (&["ring-offset"], &[color(&["--ring-offset-color", "--color"]), values(WIDTHS, &["--ring-offset-width"])]),
    (&["@container"], &[with_default(values(list(&["normal"]), &[]))]),
];

/// The `suggest` calls of `utilities.ts` that are not in a function that
/// makes utilities, and the calls of `borderSideUtility`,
/// `gradientStopUtility` and `maskStopUtility`.
pub(super) fn register(u: &mut Utilities) {
    let table = |u: &mut Utilities, table: &'static [(&'static [&'static str], &'static [Group])]| {
        for (roots, groups) in table {
            for root in *roots {
                suggest(u, root, groups);
            }
        }
    };
    table(u, TABLE);

    // `maskStopUtility`, also through `maskEdgeUtility`.
    let mask = |u: &mut Utilities, root: &str| {
        suggest(u, root, MASK_STOP);
        suggest(u, root, MASK_STOP_VALUES);
    };
    for root in MASK_ROOTS {
        mask(u, root);
    }
    suggest(u, "mask-linear", MASK_ANGLES);
    mask(u, "mask-linear-from");
    mask(u, "mask-linear-to");
    mask(u, "mask-radial-from");
    mask(u, "mask-radial-to");
    suggest(u, "mask-conic", MASK_ANGLES);
    mask(u, "mask-conic-from");
    mask(u, "mask-conic-to");

    table(u, TABLE_AFTER_MASKS);
}
