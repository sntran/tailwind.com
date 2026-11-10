//! Color checks: `utils/is-color.ts` of Tailwind CSS.

/// The named colors, the color keywords and the system colors, in order.
const NAMED_COLORS: [&str; 169] = [
    "accentcolor",
    "accentcolortext",
    "activetext",
    "aliceblue",
    "antiquewhite",
    "aqua",
    "aquamarine",
    "azure",
    "beige",
    "bisque",
    "black",
    "blanchedalmond",
    "blue",
    "blueviolet",
    "brown",
    "burlywood",
    "buttonborder",
    "buttonface",
    "buttontext",
    "cadetblue",
    "canvas",
    "canvastext",
    "chartreuse",
    "chocolate",
    "coral",
    "cornflowerblue",
    "cornsilk",
    "crimson",
    "currentcolor",
    "cyan",
    "darkblue",
    "darkcyan",
    "darkgoldenrod",
    "darkgray",
    "darkgreen",
    "darkgrey",
    "darkkhaki",
    "darkmagenta",
    "darkolivegreen",
    "darkorange",
    "darkorchid",
    "darkred",
    "darksalmon",
    "darkseagreen",
    "darkslateblue",
    "darkslategray",
    "darkslategrey",
    "darkturquoise",
    "darkviolet",
    "deeppink",
    "deepskyblue",
    "dimgray",
    "dimgrey",
    "dodgerblue",
    "field",
    "fieldtext",
    "firebrick",
    "floralwhite",
    "forestgreen",
    "fuchsia",
    "gainsboro",
    "ghostwhite",
    "gold",
    "goldenrod",
    "gray",
    "graytext",
    "green",
    "greenyellow",
    "grey",
    "highlight",
    "highlighttext",
    "honeydew",
    "hotpink",
    "indianred",
    "indigo",
    "ivory",
    "khaki",
    "lavender",
    "lavenderblush",
    "lawngreen",
    "lemonchiffon",
    "lightblue",
    "lightcoral",
    "lightcyan",
    "lightgoldenrodyellow",
    "lightgray",
    "lightgreen",
    "lightgrey",
    "lightpink",
    "lightsalmon",
    "lightseagreen",
    "lightskyblue",
    "lightslategray",
    "lightslategrey",
    "lightsteelblue",
    "lightyellow",
    "lime",
    "limegreen",
    "linen",
    "linktext",
    "magenta",
    "mark",
    "marktext",
    "maroon",
    "mediumaquamarine",
    "mediumblue",
    "mediumorchid",
    "mediumpurple",
    "mediumseagreen",
    "mediumslateblue",
    "mediumspringgreen",
    "mediumturquoise",
    "mediumvioletred",
    "midnightblue",
    "mintcream",
    "mistyrose",
    "moccasin",
    "navajowhite",
    "navy",
    "oldlace",
    "olive",
    "olivedrab",
    "orange",
    "orangered",
    "orchid",
    "palegoldenrod",
    "palegreen",
    "paleturquoise",
    "palevioletred",
    "papayawhip",
    "peachpuff",
    "peru",
    "pink",
    "plum",
    "powderblue",
    "purple",
    "rebeccapurple",
    "red",
    "rosybrown",
    "royalblue",
    "saddlebrown",
    "salmon",
    "sandybrown",
    "seagreen",
    "seashell",
    "selecteditem",
    "selecteditemtext",
    "sienna",
    "silver",
    "skyblue",
    "slateblue",
    "slategray",
    "slategrey",
    "snow",
    "springgreen",
    "steelblue",
    "tan",
    "teal",
    "thistle",
    "tomato",
    "transparent",
    "turquoise",
    "violet",
    "visitedtext",
    "wheat",
    "white",
    "whitesmoke",
    "yellow",
    "yellowgreen",
];

/// The longest name in `NAMED_COLORS`.
const MAX_NAME_LENGTH: usize = 20;

/// The color functions, in lower case.
const COLOR_FUNCTIONS: [&str; 13] = [
    "rgb",
    "rgba",
    "hsl",
    "hsla",
    "hwb",
    "color",
    "lab",
    "lch",
    "oklab",
    "oklch",
    "light-dark",
    "color-mix",
    "--alpha",
];

/// `true` when the value starts with a call of a color function, in any case.
fn is_color_fn(value: &str) -> bool {
    match value.find('(') {
        Some(paren) => {
            let name = &value[..paren];
            COLOR_FUNCTIONS.iter().any(|f| name.eq_ignore_ascii_case(f))
        }
        None => false,
    }
}

/// `true` when the value is a color: a hex color, a color function or a
/// named color.
pub fn is_color(value: &str) -> bool {
    value.as_bytes().first() == Some(&b'#') || is_color_fn(value) || is_named_color(value)
}

/// `true` when the value is a named color, in any case.
pub fn is_named_color(value: &str) -> bool {
    if value.is_ascii() {
        let bytes = value.as_bytes();
        if bytes.len() > MAX_NAME_LENGTH || bytes.len() < 3 {
            return false;
        }
        let mut buffer = [0u8; MAX_NAME_LENGTH];
        let lower = &mut buffer[..bytes.len()];
        for (to, from) in lower.iter_mut().zip(bytes) {
            *to = from.to_ascii_lowercase();
        }
        let lower: &[u8] = lower;
        NAMED_COLORS.binary_search_by(|name| name.as_bytes().cmp(lower)).is_ok()
    } else {
        // JavaScript `toLowerCase` changes some characters to ASCII, for
        // example the Kelvin sign to `k`.
        let lower = value.to_lowercase();
        NAMED_COLORS.binary_search(&lower.as_str()).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_in_order() {
        assert!(NAMED_COLORS.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(NAMED_COLORS.iter().map(|n| n.len()).max(), Some(MAX_NAME_LENGTH));
        assert_eq!(NAMED_COLORS.iter().map(|n| n.len()).min(), Some(3));
    }
}
