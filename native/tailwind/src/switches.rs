//! The switches: each behavior of the compiler that changed in a release
//! of Tailwind CSS since `Version::OLDEST`.
//!
//! A switch has the first version with the new behavior and one sentence
//! about the change. The code of the new behavior is the port of the
//! newest version. At the place of the change, the code asks `NAME.on()`
//! and keeps the old behavior in the other branch:
//!
//! ```ignore
//! if switches::NAME.on() { /* as the newest version */ } else { /* as before */ }
//! ```
//!
//! How to find a switch that is not there: the fuzzer compares the Rust
//! compiler, with the version of an official package, with the compiler of
//! that package. Each difference is a behavior with no switch
//! (docs/VERSIONS.md, "The Rust compiler as an older version").

use crate::version::Switch;

// --- 4.3.5 ----------------------------------------------------------------

/// `color-mix(…)` with no color interpolation method (no `in …`) gets no
/// fallback, and a comment before `in` is not a part of the arguments.
/// Before: the fallback code ran for each `color-mix(…)`, and read the
/// color space from the third node. (Where that node was not there, the
/// official compiler stopped with a JavaScript exception: see "Faults that
/// stop the compiler" in docs/VERSIONS.md.)
pub const COLOR_MIX_NEEDS_METHOD: Switch = Switch::since(4, 3, 5, "color-mix() with no interpolation method gets no fallback");

/// `<!--` and `-->` between the rules of a stylesheet are skipped.
/// Before: `-->` started a custom property, and `<!--` was a part of the
/// next rule.
pub const HTML_COMMENT_MARKS: Switch = Switch::since(4, 3, 5, "<!-- and --> between top-level rules are skipped");

/// A build of one candidate does not compute the order of the variants.
/// Before: it always did. The order of the rules of one candidate with
/// more than one meaning can be different.
pub const ONE_CANDIDATE_NO_VARIANT_ORDER: Switch = Switch::since(4, 3, 5, "one candidate alone has no variant order");

// --- 4.3.4 ----------------------------------------------------------------

/// The `@supports` rule of the `@property` fallbacks asks for
/// `margin-trim: block`. Before: `margin-trim: inline`.
pub const AT_PROPERTY_SUPPORTS_BLOCK: Switch = Switch::since(4, 3, 4, "the @supports rule of the fallbacks has margin-trim: block");

/// An empty fallback of `var()` has a space: `var(--tw-blur, )`. Before:
/// `var(--tw-blur,)`, in the values of transform, touch-action, filter,
/// backdrop-filter, contain, font-variant-numeric, ring and ring-offset.
pub const EMPTY_VAR_FALLBACK_SPACE: Switch = Switch::since(4, 3, 4, "an empty var() fallback has a space");

/// `group-*` and `peer-*` put the target in `:is()`:
/// `:is(:where(.group):hover &)`. Before: `&:is(:where(.group):hover *)`.
pub const GROUP_PEER_TARGET_IN_IS: Switch = Switch::since(4, 3, 4, "group and peer variants have the target in :is()");

/// More than one `/` at the top level is not valid: `bg-red-500/50/`.
/// Before: only a third part that is not empty was an error, so an empty
/// one (and each part after it) was ignored.
pub const EMPTY_EXTRA_MODIFIER_IS_INVALID: Switch = Switch::since(4, 3, 4, "an empty second modifier is not valid");

/// An arbitrary value must have balanced brackets, and the check is on
/// the text before the decoding: `w-[calc(1px]` is not valid. Before:
/// only a `;` or a closing bracket at depth 0 was an error, and the check
/// was on the decoded text.
pub const BALANCED_ARBITRARY_VALUE: Switch = Switch::since(4, 3, 4, "an arbitrary value must have balanced brackets");

/// `supports-[…]` puts spaces around `and`, `or` and `not` only when they
/// are functions, and not in `selector(…)`. Before: around each of these
/// words in the value.
pub const SUPPORTS_KEYWORDS_AS_FUNCTIONS: Switch = Switch::since(4, 3, 4, "supports-[…] spaces only the functions and, or, not");

/// Breakpoints sort by their value as a decimal number. Before: as an
/// integer (`parseInt`), so `40.25rem` and `40.5rem` were equal.
pub const BREAKPOINT_DECIMAL_SORT: Switch = Switch::since(4, 3, 4, "breakpoints sort as decimal numbers");

/// `\0` … `\f` (one hexadecimal digit) is the code point. Before: the
/// digit itself, when no white space follows.
pub const UNESCAPE_ONE_HEX_DIGIT: Switch = Switch::since(4, 3, 4, "an escape of one hexadecimal digit is a code point");

/// `cap`, `ic`, `rex`, `rch`, `rcap`, `ric` and the `sv*`, `lv*`, `dv*`
/// units for inline, block, min and max are lengths. Before: they were
/// not, so `text-[10dvmin]` was a color.
pub const MORE_LENGTH_UNITS: Switch = Switch::since(4, 3, 4, "more units are lengths");

/// A modifier that a utility does not use makes the candidate not valid:
/// `rounded-sm/50`, `shadow-sm/foo`, `stroke-2/50`. Before: the modifier
/// was ignored, and the class had the CSS of the class with no modifier.
pub const UNUSED_MODIFIER_IS_INVALID: Switch = Switch::since(4, 3, 4, "a modifier that the utility does not use is not valid");

/// `--default(…)` in `--value(…)` and `--modifier(…)` of `@utility` keeps
/// its white space. Before: the white space was removed
/// (`--default(box alphabetic)` gave `boxalphabetic`).
pub const DEFAULT_FN_WHITESPACE: Switch = Switch::since(4, 3, 4, "--default(…) keeps its white space");

/// A `\\` in a comment is a plain character: `/* C:\\temp\\*/` ends at its
/// `*/`. Before: the `\\` escaped the next character, so that comment
/// had no end.
pub const COMMENT_BACKSLASH_END: Switch = Switch::since(4, 3, 4, "a backslash in a comment escapes nothing");

/// `!important` is a flag only at the end of a value (a custom property
/// can have a comment after it), and an escaped space at the end of a
/// value stays. Before: the first `!important` after the colon cut the
/// value.
pub const IMPORTANT_AT_THE_END: Switch = Switch::since(4, 3, 4, "!important is a flag only at the end of a value");

/// A custom property with no `;` at the end of the input has all of its
/// value. Before: it lost its last character (the parser read that
/// character again as new text), and when a bracket closed at the end,
/// the property had no name and no value.
pub const CUSTOM_PROPERTY_AT_EOF: Switch = Switch::since(4, 3, 4, "a custom property at the end of the input is complete");

/// A declaration with no `;` at the end of the input stays. Before: it
/// was dropped.
pub const DECLARATION_AT_EOF: Switch = Switch::since(4, 3, 4, "a declaration at the end of the input stays");

/// `@scope` is an at-rule that goes out of a rule: a variant with `@scope`
/// wraps the utility, a nested `@scope` gets the selector of its parent,
/// and declarations in `@scope` get `:where(:scope)`. Before: `@scope`
/// stayed where it was, with its contents as they were.
pub const SCOPE_HOISTING: Switch = Switch::since(4, 3, 4, "@scope goes out of a rule, as @media does");

/// A nested rule that can never match (a combinator after a
/// pseudo-element: `::selection > *`) is not in the output. Before: it
/// was.
pub const DEAD_PSEUDO_ELEMENT_RULES: Switch = Switch::since(4, 3, 4, "a rule after a pseudo-element is dropped");

/// The arguments of `:-moz-any()` and `:-webkit-any()` are selectors, so a
/// `&` in them is replaced. Before: they were plain text.
pub const MOZ_ANY_NESTING: Switch = Switch::since(4, 3, 4, ":-moz-any() and :-webkit-any() have selectors as arguments");

// --- 4.3.3 ----------------------------------------------------------------

/// The last step of the compiler flattens the rules: a nested rule gets
/// the selector of its parent, and an at-rule goes out of the rules.
/// Before: the rules stayed nested (`.a { &:hover { @media … { … } } }`).
/// The step before it removed equal declarations in each rule (it kept
/// the last), and replaced a rule with the selector `&` by its children.
/// A fallback of `color-mix(…)` or an unused theme variable in a rule
/// that this old step changed stayed as it was.
pub const NESTING_PASS: Switch = Switch::since(4, 3, 3, "the output has no nested rules");

/// `--spacing(0)` is `0px`: a length. Before: `0`.
pub const SPACING_ZERO_PX: Switch = Switch::since(4, 3, 3, "--spacing(0) gives 0px");

/// A bare modifier of a shadow can be each opacity value: `shadow-sm/12.5`.
/// Before: only a positive integer.
pub const SHADOW_FRACTIONAL_ALPHA: Switch = Switch::since(4, 3, 3, "the alpha of a shadow can have a fraction");

/// The attribute selector of `aria-*` and `data-*` must be valid. Before:
/// there was no check: `data-[foo bar]:flex` gave a rule.
pub const ARIA_DATA_ATTRIBUTE_CHECK: Switch = Switch::since(4, 3, 3, "aria-* and data-* need a valid attribute selector");

/// The default theme of `theme(fontFamily.sans)` starts with the system
/// fonts (`-apple-system, BlinkMacSystemFont, …`). Before: `ui-sans-serif,
/// system-ui, …`. (The same values of `theme.css` and `preflight.css`
/// come from the package.)
pub const DEFAULT_THEME_SYSTEM_FONTS: Switch = Switch::since(4, 3, 3, "theme(fontFamily.sans) starts with the system fonts");

// --- 4.3.2 ----------------------------------------------------------------

/// `auto-rows-*` and `auto-cols-*` have bare values of the spacing scale:
/// `auto-rows-12`. Before: no bare values.
pub const AUTO_TRACKS_BARE_SPACING: Switch = Switch::since(4, 3, 2, "auto-rows-* and auto-cols-* have spacing values");

/// A value that starts with `--spacing(` is a length for the type of an
/// arbitrary value. Before: it was not, so `text-[--spacing(2)]` was a
/// color.
pub const SPACING_FN_IS_LENGTH: Switch = Switch::since(4, 3, 2, "--spacing(…) is a length");

/// A value that starts with `--alpha(` is a color for the type of an
/// arbitrary value. Before: it was not.
pub const ALPHA_FN_IS_COLOR: Switch = Switch::since(4, 3, 2, "--alpha(…) is a color");

// --- 4.3.1 ----------------------------------------------------------------

/// `--spacing(0)` is zero and `--spacing(1)` is the value of `--spacing`.
/// Before: always `calc(var(--spacing) * N)`, so `m-0` gave
/// `calc(var(--spacing) * 0)`.
pub const SPACING_FN_FOLD: Switch = Switch::since(4, 3, 1, "--spacing(0) and --spacing(1) have no calc()");

/// `space-x-0` and `space-y-0` give the margin `0`. Before: two `calc(…)`
/// values, as each other value.
pub const SPACE_ZERO: Switch = Switch::since(4, 3, 1, "space-x-0 and space-y-0 give 0");

/// The angles `0` and `1` of a mask are `0deg` and `1deg`:
/// `mask-linear-1`. Before: `calc(1deg * 1)`.
pub const MASK_ANGLE_FOLD: Switch = Switch::since(4, 3, 1, "the mask angles 0 and 1 have no calc()");

/// `inset-shadow-none` gives `inset 0 0 #0000`. Before: `0 0 #0000`.
pub const INSET_SHADOW_NONE_INSET: Switch = Switch::since(4, 3, 1, "inset-shadow-none has the keyword inset");

/// `@apply` with only dashed identifiers (`@apply --my-mixin`) is a CSS
/// mixin and stays, and mixins with utilities are an error. Before: a
/// dashed identifier was a utility as each other, and one with no
/// utility was an error.
pub const APPLY_CSS_MIXINS: Switch = Switch::since(4, 3, 1, "@apply with dashed identifiers is a CSS mixin");

/// `@apply` of utilities with a body is an error. Before: the body was
/// dropped with no error.
pub const APPLY_BODY_ERROR: Switch = Switch::since(4, 3, 1, "@apply of utilities must not have a body");

/// The color of a shadow is the part that is a color: a hexadecimal
/// color, a named color or a color function, in each sequence and in each
/// case. Before: the first part that was not a keyword or a length.
pub const SHADOW_COLOR_DETECTION: Switch = Switch::since(4, 3, 1, "the color of a shadow is the part that is a color");

/// `not-*` reads the condition of `@container` with the value parser, so
/// `style(…)` and a name before the query are correct. Before: it split
/// the condition at the spaces.
pub const NOT_CONTAINER_VALUE_PARSER: Switch = Switch::since(4, 3, 1, "not-* reads @container conditions with the value parser");

// --- 4.3.0 ----------------------------------------------------------------

/// The utilities `zoom-*`, `tab-*`, `scrollbar-auto`, `scrollbar-thin`,
/// `scrollbar-none`, `scrollbar-thumb-*`, `scrollbar-track-*` and
/// `scrollbar-gutter-*`, and their properties in the order of the
/// properties. Before: no such utilities, and `zoom`, `tab-size` and the
/// three `scrollbar-*` properties had no place in the order.
pub const ZOOM_TAB_SCROLLBAR: Switch = Switch::since(4, 3, 0, "zoom-*, tab-* and scrollbar-* utilities");

/// `@container-size`. Before: only in a build of Tailwind CSS that was
/// not a release.
pub const CONTAINER_SIZE: Switch = Switch::since(4, 3, 0, "@container-size");

/// A functional `@utility` can have a default: `--value(--default(4))`
/// and `--modifier(--default(…))` give a result for a candidate with no
/// value or no modifier. Before: a candidate with no value was not a
/// match, `--default(…)` had no meaning, and a declaration with
/// `--modifier(…)` was removed for a candidate with no modifier.
pub const CSS_UTILITY_DEFAULT: Switch = Switch::since(4, 3, 0, "--default(…) in a functional @utility");

/// A functional `@utility` must have a `--value(…)` with a result.
/// Before: one with no `--value(…)` was a match for each value.
pub const CSS_UTILITY_NEEDS_VALUE: Switch = Switch::since(4, 3, 0, "a functional @utility needs --value(…)");

/// When a utility is not valid for a candidate, the other utilities with
/// the same name still run (but a plugin utility with types stops them).
/// Before: the first utility that was not valid stopped them all.
pub const UTILITY_NULL_FALLTHROUGH: Switch = Switch::since(4, 3, 0, "a utility that is not valid does not stop the others");

/// `@variant` can have a list (`@variant hover, focus`) and a stack
/// (`@variant hover:focus`). Before: the text was one variant, so these
/// were errors.
pub const AT_VARIANT_LIST: Switch = Switch::since(4, 3, 0, "@variant with a list or a stack of variants");

/// The `calc(… * -1)` of a negative utility has spaces around its
/// operators. Before: the value stayed as it was (`-m-[1px+2px]`).
pub const NEGATIVE_CALC_WHITESPACE: Switch = Switch::since(4, 3, 0, "the calc() of a negative value has spaces around operators");

/// The old utilities `start` and `end` need a value. Before: with no
/// value they gave the value of `--inset` or `--spacing`.
pub const LEGACY_INSET_NEEDS_VALUE: Switch = Switch::since(4, 3, 0, "the old start and end utilities need a value");

// --- 4.2.3 ----------------------------------------------------------------

/// `placeholder-*` reads the theme keys `--placeholder-color` and
/// `--color`. Before: `--background-color` and `--color`.
pub const PLACEHOLDER_COLOR_KEY: Switch = Switch::since(4, 2, 3, "placeholder-* reads --placeholder-color");

/// An escape of the code point 0 (`\\00`) is U+FFFD. Before: the code
/// point 0. (An escape above U+10FFFF stopped the official compiler: see
/// "Faults that stop the compiler" in docs/SWITCHES.md. An escape of a
/// surrogate is U+FFFD in each version of the Rust compiler, because a
/// Rust text cannot have one.)
pub const UNESCAPE_NUL_IS_REPLACED: Switch = Switch::since(4, 2, 3, "an escape of the code point 0 is U+FFFD");

// --- 4.2.2 ----------------------------------------------------------------

/// A fraction for the type of an arbitrary value can have white space
/// around the `/`: `1 / 2`. Before: a fault in the pattern permitted the
/// letter `s` there, not white space (`1s/s2`).
pub const FRACTION_WHITE_SPACE: Switch = Switch::since(4, 2, 2, "a fraction can have white space around the /");

// --- 4.2.1 ----------------------------------------------------------------

/// The root of a functional `@utility` can end with `-`: `@utility
/// foo--*`. Before (only in 4.2.0): the name was not valid.
pub const UTILITY_ROOT_ENDING_DASH: Switch = Switch::since(4, 2, 1, "the root of a functional @utility can end with -");

// --- 4.1.8 ----------------------------------------------------------------

/// The error of `@apply` for a candidate with no result gives the reason:
/// no prefix, a class that `@source not inline(…)` removes, a variant that
/// does not exist, or an empty theme. Before: always `Cannot apply unknown
/// utility class: NAME`.
pub const APPLY_ERROR_REASONS: Switch = Switch::since(4, 1, 8, "the error of @apply gives the reason");

// --- 4.1.7 ----------------------------------------------------------------

/// `-rotate-[…]` negates each arbitrary value that is not a vector:
/// `-rotate-[var(--r)]` gives `calc(var(--r) * -1)`. Before: only a value
/// with the type angle was negated, and each other value stayed as it was.
pub const ROTATE_NEGATIVE_ARBITRARY: Switch = Switch::since(4, 1, 7, "-rotate-[…] negates each arbitrary value");

// --- 4.1.6 ----------------------------------------------------------------

/// The parser keeps a `\r\n` that is in a comment, in a string, after a
/// `\\` or in the value of a custom property. Before: the parser replaced
/// each `\r\n` of the input with `\n` first.
pub const CRLF_STAYS: Switch = Switch::since(4, 1, 6, "the parser does not replace \\r\\n in the input");

/// The short form of a variable (`w-(--x)`, `bg-red-500/(--x)`,
/// `min-(--x):flex`) must start with `--`, and the short form of a value
/// has no `;` or `}` at the top level. Before: one `-` in the first two
/// characters was sufficient (`w-(-x)`, `w-(a-b)`), and the short form of
/// a value had no check of its own.
pub const PAREN_VALUE_NEEDS_TWO_DASHES: Switch = Switch::since(4, 1, 6, "the short form of a variable must start with --");

/// The short form of a variable in a modifier keeps the `_` of the name
/// of the variable: `bg-red-500/(--my_opacity)`. Before: each `_` was a
/// space (`var(--my opacity)`).
pub const PAREN_MODIFIER_KEEPS_UNDERSCORES: Switch = Switch::since(4, 1, 6, "a modifier with (--x_y) keeps the _ of the variable");

/// `-scale-[…]` negates its arbitrary value: `calc(… * -1)`. Before: the
/// value stayed as it was.
pub const SCALE_NEGATIVE_ARBITRARY: Switch = Switch::since(4, 1, 6, "-scale-[…] negates the arbitrary value");

/// `@tailwind utilities` in a reference (`@reference`, `@media reference`)
/// is removed. Before: it was the utilities node, so a later `@tailwind
/// utilities` was removed, and the stylesheet had no utilities.
pub const REFERENCE_NO_UTILITIES: Switch = Switch::since(4, 1, 6, "@tailwind utilities in a reference is removed");

/// The substitution of `@apply` in a parent does each `@apply` below it,
/// and each parent in the path depends on the applied utilities. Before:
/// only the children of each parent, so a utility that another `@utility`
/// applied could lose its nested rules with `@apply`, and an `@apply` at
/// the top level of a stylesheet gave nothing.
pub const APPLY_NESTED_WALK: Switch = Switch::since(4, 1, 6, "@apply in nested rules of an applied @utility");

/// The candidates of one `@apply` are the keys of a JavaScript object: a
/// candidate that is a number comes first. Before: they were a list, in
/// the sequence of the text. The first candidate that is not valid gives
/// the error.
pub const APPLY_CANDIDATES_AS_KEYS: Switch = Switch::since(4, 1, 6, "the candidates of @apply are in the sequence of object keys");

/// All the switches, the newest first: for the tests and the documents.
pub const ALL: &[&Switch] = &[
    &COLOR_MIX_NEEDS_METHOD,
    &HTML_COMMENT_MARKS,
    &ONE_CANDIDATE_NO_VARIANT_ORDER,
    &AT_PROPERTY_SUPPORTS_BLOCK,
    &EMPTY_VAR_FALLBACK_SPACE,
    &GROUP_PEER_TARGET_IN_IS,
    &EMPTY_EXTRA_MODIFIER_IS_INVALID,
    &BALANCED_ARBITRARY_VALUE,
    &SUPPORTS_KEYWORDS_AS_FUNCTIONS,
    &BREAKPOINT_DECIMAL_SORT,
    &UNESCAPE_ONE_HEX_DIGIT,
    &MORE_LENGTH_UNITS,
    &UNUSED_MODIFIER_IS_INVALID,
    &DEFAULT_FN_WHITESPACE,
    &COMMENT_BACKSLASH_END,
    &IMPORTANT_AT_THE_END,
    &CUSTOM_PROPERTY_AT_EOF,
    &DECLARATION_AT_EOF,
    &SCOPE_HOISTING,
    &DEAD_PSEUDO_ELEMENT_RULES,
    &MOZ_ANY_NESTING,
    &NESTING_PASS,
    &SPACING_ZERO_PX,
    &SHADOW_FRACTIONAL_ALPHA,
    &ARIA_DATA_ATTRIBUTE_CHECK,
    &DEFAULT_THEME_SYSTEM_FONTS,
    &AUTO_TRACKS_BARE_SPACING,
    &SPACING_FN_IS_LENGTH,
    &ALPHA_FN_IS_COLOR,
    &SPACING_FN_FOLD,
    &SPACE_ZERO,
    &MASK_ANGLE_FOLD,
    &INSET_SHADOW_NONE_INSET,
    &APPLY_CSS_MIXINS,
    &APPLY_BODY_ERROR,
    &SHADOW_COLOR_DETECTION,
    &NOT_CONTAINER_VALUE_PARSER,
    &ZOOM_TAB_SCROLLBAR,
    &CONTAINER_SIZE,
    &CSS_UTILITY_DEFAULT,
    &CSS_UTILITY_NEEDS_VALUE,
    &UTILITY_NULL_FALLTHROUGH,
    &AT_VARIANT_LIST,
    &NEGATIVE_CALC_WHITESPACE,
    &LEGACY_INSET_NEEDS_VALUE,
    &PLACEHOLDER_COLOR_KEY,
    &UNESCAPE_NUL_IS_REPLACED,
    &FRACTION_WHITE_SPACE,
    &UTILITY_ROOT_ENDING_DASH,
    &APPLY_ERROR_REASONS,
    &ROTATE_NEGATIVE_ARBITRARY,
    &CRLF_STAYS,
    &PAREN_VALUE_NEEDS_TWO_DASHES,
    &PAREN_MODIFIER_KEEPS_UNDERSCORES,
    &SCALE_NEGATIVE_ARBITRARY,
    &REFERENCE_NO_UTILITIES,
    &APPLY_NESTED_WALK,
    &APPLY_CANDIDATES_AS_KEYS,
];
