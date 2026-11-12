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

/// `theme(…)` with only a namespace of Tailwind CSS v3 (`theme(spacing)`)
/// reads the namespace as an object, and gives its `DEFAULT`: the bare
/// variable (`--spacing`). Before: a namespace with only the bare variable
/// gave that value alone. Then a value of `@theme default` lost against
/// the object of the default configuration, which is no value ("Could
/// not resolve value for theme function").
pub const THEME_FN_NAMESPACE_ROOT: Switch = Switch::since(4, 3, 4, "theme(spacing) reads the namespace as an object");

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

/// In `theme(…)` with a path of Tailwind CSS v3, an object of CSS values
/// with `DEFAULT` gives that value before the opacity modifier:
/// `theme(colors.foo / 50%)` with `--color-foo` and `--color-foo-bar` has
/// the opacity. Before: the object got no opacity, and its `DEFAULT` was
/// the result.
pub const THEME_FN_PREFERS_DEFAULT: Switch = Switch::since(4, 3, 3, "theme(colors.foo / 50%) of an object with DEFAULT has the opacity");

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

// --- 4.2.0 ----------------------------------------------------------------

/// The utilities of the logical properties: `inset-s-*`, `inset-e-*`,
/// `inset-bs-*`, `inset-be-*`, `mbs-*`, `mbe-*`, `pbs-*`, `pbe-*`,
/// `scroll-mbs-*`, `scroll-mbe-*`, `scroll-pbs-*`, `scroll-pbe-*`,
/// `border-bs-*`, `border-be-*`, `inline-*`, `block-*` and their `min-` and
/// `max-` forms, and the `…-block-start` and `…-block-end` properties in
/// the order of the properties. Before: no such utilities, and these
/// properties had no place in the order.
pub const LOGICAL_UTILITIES: Switch = Switch::since(4, 2, 0, "utilities of the logical properties");

/// `start-*` and `end-*` are old utilities of the plugin layer: `start`
/// with no value gives the value of `--inset` or `--spacing`, and a bare
/// value takes each modifier (`start-4/[5]`). Before: they were spacing
/// utilities as `inset-x-*`: no result for these two forms.
pub const START_END_ARE_OLD_UTILITIES: Switch = Switch::since(4, 2, 0, "start-* and end-* are old utilities");

/// The utility `font-features-*`, and `font-feature-settings` in the order
/// of the properties. Before: no such utility, and the property had no
/// place in the order.
pub const FONT_FEATURES: Switch = Switch::since(4, 2, 0, "font-features-* utility");

/// The fraction of a utility has spaces around the `/`: `w-1/2` gives
/// `calc(1 / 2 * 100%)`. Before: `calc(1/2 * 100%)`.
pub const FRACTION_SPACES: Switch = Switch::since(4, 2, 0, "the fraction of a utility has spaces around the /");

/// `--value(ratio)` of `@utility` gives the ratio with spaces around the
/// `/` (`1 / 2`), and each part is a number that is an integer (`01/2`).
/// Before: the text of the candidate (`1/2`), and each part was an integer
/// with no extra `0` at its start.
pub const RATIO_VALUE_SPACES: Switch = Switch::since(4, 2, 0, "--value(ratio) gives the ratio with spaces around the /");

/// The parts of an `aspect-*` fraction can be multiples of 0.25:
/// `aspect-8.5/11`. Before: only integers.
pub const ASPECT_FRACTION_DECIMALS: Switch = Switch::since(4, 2, 0, "aspect-* fractions can have multiples of 0.25");

/// Each shadow of an `inset-shadow-*` list gets `inset`, one space, and
/// the shadow with no space at its ends, and the list has `, ` between
/// its shadows. Before: `inset ` and the shadow as it was, with `,`
/// between the shadows.
pub const INSET_SHADOW_LIST_SPACES: Switch = Switch::since(4, 2, 0, "an inset-shadow-* list has regular spaces");

/// The name of `@utility` can have escapes: `@utility push-1\/2` is
/// `push-1/2`. Before: the name was the text as it was, so an escape was
/// an error.
pub const UTILITY_NAME_ESCAPES: Switch = Switch::since(4, 2, 0, "the name of @utility can have escapes");

/// The name of `@utility` follows the rules of the scanner: one `/` with a
/// character after it, `.` only between digits, `%` only at the end and
/// after a digit, no `-` at the end, and a functional name has only a
/// root. Before: one pattern, `-?[a-z][a-zA-Z0-9/%._-]*` (with `-*` at the
/// end of a functional name).
pub const UTILITY_NAME_SCANNER_RULES: Switch = Switch::since(4, 2, 0, "the name of @utility follows the rules of the scanner");

/// A declaration in `@supports (… color-mix(…) …)` gets no `color-mix(…)`
/// fallback. Before: it got a fallback and a second `@supports` rule.
pub const SUPPORTS_COLOR_MIX_NO_FALLBACK: Switch = Switch::since(4, 2, 0, "no color-mix() fallback in @supports with color-mix()");

/// `@source inline(…)` can have white space around its argument. Before:
/// that was the error "`@source` paths must be quoted."
pub const SOURCE_INLINE_WHITESPACE: Switch = Switch::since(4, 2, 0, "@source inline() can have white space around its argument");

/// The nodes that replace `@slot` in the body of `@custom-variant` stay as
/// they are. Before: an `@keyframes` or `@property` rule in these nodes
/// went out to the root, as the same rules of the body. (An `@slot` in
/// these nodes made an endless loop: see docs/SWITCHES.md.)
pub const SLOT_NODES_STAY: Switch = Switch::since(4, 2, 0, "the nodes that replace @slot stay as they are");

// --- 4.1.18 ---------------------------------------------------------------

/// A declaration in `@keyframes` gets no `color-mix(…)` fallback. Before:
/// it got a fallback and an `@supports` rule, as each other declaration.
pub const KEYFRAMES_NO_COLOR_MIX_FALLBACK: Switch = Switch::since(4, 1, 18, "no color-mix() fallback in @keyframes");

/// A named value of a utility or a variant, and a named modifier, have
/// only letters, digits, `_`, `.`, `%` and `-`: `bg-foo=bar` and
/// `data-foo=bar:flex` are not valid. Before: no check.
pub const NAMED_VALUE_CHARACTERS: Switch = Switch::since(4, 1, 18, "a named value has only letters, digits, _ . % and -");

/// The message of a CSS syntax error starts with the file, the line and
/// the column, when the compiler has the path of the stylesheet (`from`).
/// Before: only the message.
pub const CSS_ERROR_LOCATION: Switch = Switch::since(4, 1, 18, "a CSS syntax error has the file, the line and the column");

/// `content-*` reads the theme key `--content`: `content-slash`. Before:
/// only arbitrary values.
pub const CONTENT_THEME_KEY: Switch = Switch::since(4, 1, 18, "content-* reads --content");

/// `theme(…)` with a path of Tailwind CSS v3 reads the `DEFAULT` of
/// `borderWidth`, `outlineWidth`, `ringColor`, `ringWidth`,
/// `transitionDuration` and `transitionTimingFunction` from the
/// `--default-*` namespace: `theme(ringColor.DEFAULT)` is
/// `--default-ring-color`. Before: `--ring-color`.
pub const THEME_FN_DEFAULT_NAMESPACE: Switch = Switch::since(4, 1, 18, "theme(ringColor.DEFAULT) reads --default-ring-color");

/// `theme(…)` with a path of Tailwind CSS v3 keeps the case of a key:
/// `theme(colors.fooBar)` is `--color-fooBar`. Only the namespace, a
/// nested key of a tuple and `lineHeight` change to kebab case. Before:
/// each key changed to kebab case (`--color-foo-bar`).
pub const THEME_FN_KEEPS_KEY_CASE: Switch = Switch::since(4, 1, 18, "theme(colors.fooBar) keeps the case of the key");

// --- 4.1.16 ---------------------------------------------------------------

/// The `@supports` rule of a `color-mix(…)` fallback holds the declaration
/// itself. Before: the declaration was in a rule with the selector `&` in
/// that `@supports` rule.
pub const COLOR_MIX_FALLBACK_NO_AMPERSAND: Switch = Switch::since(4, 1, 16, "the @supports rule of a color-mix() fallback has no & rule");

/// An arbitrary value with an empty data type is not valid: `bg-[:red]`.
/// Before: the value was the text after the `:`, with no data type.
pub const EMPTY_DATA_TYPE_IS_INVALID: Switch = Switch::since(4, 1, 16, "an empty data type of an arbitrary value is not valid");

// --- 4.1.15 ---------------------------------------------------------------

/// Only in 4.1.15: the `@supports` rule of a `color-mix(…)` fallback holds
/// the declaration in a rule with the selector `&`
/// (`COLOR_MIX_FALLBACK_NO_AMPERSAND` is the end of this behavior). Before:
/// the `@supports` rule held the declaration itself, as it does now.
pub const COLOR_MIX_FALLBACK_AMPERSAND: Switch = Switch::since(4, 1, 15, "the @supports rule of a color-mix() fallback has a & rule");

/// The modifier of `not-*`, `has-*` and `in-*` goes to the inner variant:
/// `not-group-hover/name` is `not-*` of `group-hover/name`. Before: the
/// modifier stayed on the outer variant, which takes none, so the
/// candidate was not valid.
pub const COMPOUND_MODIFIER_TO_INNER_VARIANT: Switch = Switch::since(4, 1, 15, "the modifier of not-*, has-* and in-* goes to the inner variant");

/// A tab ends the name of an at-rule: `@media\tprint`. Before: only a
/// space or `(` did, so the tab and the text after it were a part of the
/// name, and the compiler did not know the at-rule.
pub const AT_RULE_NAME_ENDS_AT_TAB: Switch = Switch::since(4, 1, 15, "a tab ends the name of an at-rule");

/// A fixed value of a utility (`z-auto`, `leading-none`) gives new nodes
/// to each candidate. Before (only in 4.1.14, the release with
/// `STATIC_VALUES_AFTER_THEME`): each candidate got the same nodes, so the
/// `!important` of one candidate (`z-auto!`) made the declarations of each
/// candidate with that value important.
pub const STATIC_VALUE_COPIES: Switch = Switch::since(4, 1, 15, "a fixed value of a utility gives new nodes to each candidate");

/// `theme(…)` with a path of Tailwind CSS v3 gives the `DEFAULT` of an
/// object: `theme(colors.foo)` with `--color-foo` and `--color-foo-bar`.
/// Before: an object was no value ("Could not resolve value for theme
/// function").
pub const THEME_FN_OBJECT_DEFAULT: Switch = Switch::since(4, 1, 15, "theme(colors.foo) gives the DEFAULT of an object");

/// A key of a `theme(…)` path is not an index into a text. Before:
/// `theme(colors.what.1)` gave the second character of the value of
/// `--color-what`, when the theme had no better value.
pub const THEME_FN_NO_STRING_INDEX: Switch = Switch::since(4, 1, 15, "a key of a theme() path is not an index into a text");

// --- 4.1.14 ---------------------------------------------------------------

/// A fixed value of a functional utility (`origin-top-left`, `z-auto`,
/// `rounded-full`) is used only when the theme has no value with that
/// name. Before: each fixed value was a static utility, so a theme value
/// with the same name (`--radius-full`) gave a second rule.
pub const STATIC_VALUES_AFTER_THEME: Switch = Switch::since(4, 1, 14, "a theme value replaces a fixed value of a utility");

/// The corners and the sides of `origin-*` and `perspective-origin-*` are
/// percentages: `origin-top-left` gives `0 0`. Before: the keywords
/// (`top left`).
pub const ORIGIN_PERCENTAGES: Switch = Switch::since(4, 1, 14, "origin-* and perspective-origin-* give percentages");

/// `col-*` and `row-*` do not read `--grid-column-start-*`,
/// `--grid-column-end-*`, `--grid-row-start-*` and `--grid-row-end-*`.
/// Before: `col-start-foo` with `--grid-column-start-foo` also gave
/// `grid-column: var(--grid-column-start-foo)`.
pub const GRID_LINE_THEME_KEYS: Switch = Switch::since(4, 1, 14, "col-* and row-* do not read the keys of col-start-* and such");

/// A stylesheet with `@theme` and no other feature of Tailwind CSS is
/// compiled. Before: `@theme` was not a feature, so the output was the
/// input text with no change.
pub const AT_THEME_IS_A_FEATURE: Switch = Switch::since(4, 1, 14, "a stylesheet with only @theme is compiled");

/// Custom variants that use each other in a circle (`@variant b` in
/// `@custom-variant a`, and `@variant a` in `@custom-variant b`) are an
/// error. Before: no error. (An `@variant` rule in `@custom-variant` stayed
/// in the output before 4.1.14, which was not valid CSS. The Rust compiler
/// replaces the rule in each version, but not in a stylesheet with such a
/// circle: see docs/SWITCHES.md.)
pub const CUSTOM_VARIANT_CYCLE_ERROR: Switch = Switch::since(4, 1, 14, "a circle of custom variants is an error");

/// An `@variant` rule in the body of `@custom-variant` does not change
/// how the variant compounds (`group-*`, `not-*`). Before: it was an
/// at-rule as each other, so no compound variant took the custom variant.
pub const CUSTOM_VARIANT_AT_VARIANT_COMPOUNDS: Switch = Switch::since(4, 1, 14, "a custom variant with @variant can be in a compound variant");

// --- 4.1.13 ---------------------------------------------------------------

/// A rule keeps only the last of its equal declarations (the same
/// property, value and `!important`). Before: each declaration stayed.
/// (From 4.1.13 to 4.3.2, a rule with equal declarations got a new list
/// of nodes, so it got no `color-mix(…)` fallback: see `NESTING_PASS`.
/// Before 4.1.13, each child of a rule with the selector `&` had a list
/// of its own.)
pub const EQUAL_DECLARATIONS_REMOVED: Switch = Switch::since(4, 1, 13, "a rule keeps only the last of its equal declarations");

/// `@-…` is not a variant and not a utility: `@-md:flex` gives nothing.
/// Before: the `@` root took the text after the dash, so `@-md:flex` was
/// `@md:flex`.
pub const AT_DASH_IS_INVALID: Switch = Switch::since(4, 1, 13, "@-md:flex is not valid");

/// The name of `@custom-variant` starts with a lowercase letter or a
/// digit, and does not end with `-` or `_`. Before: each sequence of
/// letters, digits, `-` and `_` was a name (also the empty name), and the
/// message of the error had no rule about the start.
pub const VARIANT_NAME_START_AND_END: Switch =
    Switch::since(4, 1, 13, "the name of @custom-variant has rules for its start and its end");

/// `sr-only` gives `clip-path: inset(50%)`, and `not-sr-only` gives
/// `clip-path: none`. Before: `clip: rect(0, 0, 0, 0)` and `clip: auto`.
pub const SR_ONLY_CLIP_PATH: Switch = Switch::since(4, 1, 13, "sr-only has clip-path, not clip");

/// The default of `transition` has no `visibility`. Before (from 4.1.5):
/// `visibility` was between `display` and `content-visibility`.
pub const TRANSITION_NO_VISIBILITY: Switch = Switch::since(4, 1, 13, "transition has no visibility");

// --- 4.1.12 ---------------------------------------------------------------

/// `@apply` does not follow the `important` option (`@import "tailwindcss"
/// important`): only `@apply flex!` gives `!important`. Before: each
/// declaration of `@apply` was `!important` with that option. `@apply` and
/// the build also had one cache of the rules of a candidate, so the build
/// got the rules of a utility as an earlier `@apply` made them.
pub const APPLY_IGNORES_IMPORTANT_OPTION: Switch = Switch::since(4, 1, 12, "@apply does not follow the important option");

/// A `;` with no declaration before it is ignored: `color: red;;`.
/// Before: the error "Unexpected semicolon".
pub const EXTRA_SEMICOLON_IGNORED: Switch = Switch::since(4, 1, 12, "a ; with no declaration before it is ignored");

/// The error of an `@utility` name with a `*` that is not `-*` at the end
/// gives the reason: "A functional utility must end in `-*`." or "The
/// dynamic portion marked by `-*` must appear once at the end." Before:
/// always "Utilities should be alphanumeric and start with a lowercase
/// letter."
pub const UTILITY_NAME_ERROR_REASONS: Switch = Switch::since(4, 1, 12, "the error of an @utility name with * gives the reason");

/// `divide-*` reads the theme keys `--divide-color`, `--border-color` and
/// `--color`. Before: `--divide-color` and `--color`.
pub const DIVIDE_BORDER_COLOR_KEY: Switch = Switch::since(4, 1, 12, "divide-* reads --border-color");

// --- 4.1.10 ---------------------------------------------------------------

/// In a math function, `%` and an uppercase letter after a number are a
/// part of the value: `calc(100%-var(--x))` gives `calc(100% - var(--x))`.
/// Before (only in 4.1.9): only a lowercase letter was, so the `-` after
/// `100%` or `100PX` and before a name got no spaces.
pub const MATH_PERCENT_AND_UPPERCASE_UNITS: Switch = Switch::since(4, 1, 10, "% and uppercase units end a value in a math function");

// --- 4.1.9 ----------------------------------------------------------------

/// An operator in a math function gets spaces only next to a value: a
/// digit, a number with a unit, a bracket or an operator. So a name with a
/// dash stays (`fit-content`, `calc-size(…)`, `-3.4e-2`). Before: each
/// operator in a math function got spaces (`fit - content`), but not in
/// `to-zero` and not in the name `anchor-size(`. The character after
/// `to-zero` stayed as it was (`round(to-zero,1 + 2)`).
pub const MATH_OPERATORS_NEED_VALUES: Switch = Switch::since(4, 1, 9, "an operator in a math function gets spaces only next to a value");

/// The value of a custom property can have a string with `;` or `}`:
/// `--x: 'data:text/plain;base64,…'`. A string with no end is an error.
/// Before: the `;` in the string ended the value, and the parser read the
/// other part of the string as new text.
pub const CUSTOM_PROPERTY_STRINGS: Switch = Switch::since(4, 1, 9, "a string in a custom property can have ; and }");

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

// --- 4.1.5 ----------------------------------------------------------------

/// A color with an alpha of 100% has no `color-mix(…)`: `bg-red-500/100`.
/// Before: `color-mix(in oklab, … 100%, transparent)`.
pub const FULL_ALPHA_NO_COLOR_MIX: Switch = Switch::since(4, 1, 5, "an alpha of 100% gives no color-mix()");

/// The utilities `h-lh`, `min-h-lh` and `max-h-lh`. Before: no such
/// utilities.
pub const HEIGHT_LH: Switch = Switch::since(4, 1, 5, "h-lh, min-h-lh and max-h-lh");

/// The default of `transition` has also `display`, `content-visibility`,
/// `overlay` and `pointer-events`. Before: the list ended at
/// `backdrop-filter`.
pub const TRANSITION_DISPLAY: Switch = Switch::since(4, 1, 5, "transition has display, content-visibility, overlay, pointer-events");

// --- 4.1.4 ----------------------------------------------------------------

/// The fallbacks of `color-mix(…)` are made after the removal of the
/// unused theme variables. Before: at the declaration, so a theme variable
/// with a fallback was in an `@supports` rule at the time of the removal,
/// and the removal took the last node of the list.
pub const COLOR_MIX_FALLBACK_LAST: Switch = Switch::since(4, 1, 4, "color-mix() fallbacks are made after the theme cleanup");

/// The fallback of `color-mix(…)` follows a theme variable to its value:
/// `var(--a)` with `--a: var(--b)` gives the value of `--b`, and a value
/// `currentcolor` is known. Before: the value of the first variable went
/// into the fallback as it was.
pub const COLOR_MIX_VARIABLE_CHAIN: Switch = Switch::since(4, 1, 4, "color-mix() fallbacks follow a chain of theme variables");

/// A root with dashes can be a value of the `@` variant: `@foo-bar:flex`
/// with `--container-foo-bar`, `@[calc(1rem-1px)]:flex`. Before: only an
/// input with no dash got the `@` root.
pub const AT_ROOT_WITH_DASH: Switch = Switch::since(4, 1, 4, "the value of the @ variant can have a dash");

/// `transform` reads `var(--tw-rotate-x, )` and the four other variables
/// with an empty fallback, and their `@property` rules have no initial
/// value. Before: `var(--tw-rotate-x)`, and the initial values
/// `rotateX(0)`, `rotateY(0)`, `rotateZ(0)`, `skewX(0)`, `skewY(0)`.
pub const TRANSFORM_EMPTY_FALLBACKS: Switch = Switch::since(4, 1, 4, "the transform variables have empty fallbacks");

/// `shadow-inherit`, `inset-shadow-inherit`, `text-shadow-inherit` and
/// `drop-shadow-inherit` set the color variable to `inherit`. Before:
/// `inherit` was a color as each other: `color-mix(in oklab, inherit
/// var(--tw-shadow-alpha), transparent)`.
pub const SHADOW_INHERIT: Switch = Switch::since(4, 1, 4, "shadow-inherit sets the color variable to inherit");

/// A range of the brace expansion goes from its start to its end, also
/// when the step has the other sign: `{10..0..5}`. Before: the sign of the
/// step gave the direction, and such a range was empty.
pub const BRACE_RANGE_DIRECTION: Switch = Switch::since(4, 1, 4, "a brace range ignores the sign of its step");

// --- 4.1.2 ----------------------------------------------------------------

/// The fallback of `color-mix(…)` is made for each variable and for
/// `currentcolor`: a `color-mix(…)` with a color that is not known gives
/// its first color. Before: only a variable with a value in the theme gave
/// a fallback.
pub const COLOR_MIX_FALLBACK_FOR_EACH_COLOR: Switch = Switch::since(4, 1, 2, "color-mix() gets a fallback also for colors that are not known");

/// The fallbacks of `@property` are in `@layer properties` at the end of
/// the stylesheet, and an empty `@layer properties;` is at the start.
/// Before: one rule `@supports … { @layer base { … } }` near the start.
pub const PROPERTIES_LAYER: Switch = Switch::since(4, 1, 2, "the @property fallbacks are in @layer properties");

/// `currentcolor` is in lowercase: `bg-current`, the default color of
/// `ring-*` and `inset-ring-*`, and the default theme of
/// `theme(borderColor.DEFAULT)`. Before: `currentColor`.
pub const CURRENTCOLOR_LOWERCASE: Switch = Switch::since(4, 1, 2, "currentcolor is in lowercase");

/// A `--drop-shadow-*` theme value with more than one shadow gives one
/// `drop-shadow(…)` for each. Before: one `drop-shadow(…)` with all the
/// text.
pub const DROP_SHADOW_LIST: Switch = Switch::since(4, 1, 2, "one drop-shadow() for each shadow of a theme value");

/// `inset-shadow-[…]` with more than one shadow puts `inset` before each.
/// Before: only before the first.
pub const INSET_SHADOW_LIST: Switch = Switch::since(4, 1, 2, "inset before each shadow of inset-shadow-[…]");

// --- 4.1.1 ----------------------------------------------------------------

/// The fallbacks of `@property` go after the `@charset` rules, the
/// `@import` rules and the `@layer` rules with no body at the start, or
/// at the end of the rules. Before: after the comments at the start, or
/// at the start.
pub const FALLBACKS_AFTER_IMPORTS: Switch = Switch::since(4, 1, 1, "the @property fallbacks go after @import and @layer statements");

/// A number of a range of the brace expansion has no zeros before it.
/// Before: each number had the length of the longer limit: `{0..10}` gave
/// `00`, `01`, … `10`.
pub const BRACE_RANGE_NO_PADDING: Switch = Switch::since(4, 1, 1, "a brace range has no padding");

// --- 4.1.0 ----------------------------------------------------------------

/// Each `@property` rule gives a fallback for a browser with no
/// `@property`: a declaration with the initial value in one rule
/// `@supports … { @layer base { … } }`. Before: no fallbacks.
pub const AT_PROPERTY_FALLBACKS: Switch = Switch::since(4, 1, 0, "the @property rules have fallbacks");

/// A declaration with `color-mix(…)` and a theme variable gets a fallback
/// with the value of the variable, and the declaration goes into an
/// `@supports` rule. Before: no fallback.
pub const COLOR_MIX_FALLBACK: Switch = Switch::since(4, 1, 0, "color-mix() with a theme variable has a fallback");

/// `bg-linear-to-r` and `bg-linear-45` set `--tw-gradient-position` with no
/// interpolation method, and again with the method in an `@supports` rule.
/// Before: one declaration with the method.
pub const GRADIENT_POSITION_FALLBACK: Switch = Switch::since(4, 1, 0, "bg-linear-* has a fallback with no interpolation method");

/// An arbitrary value, an arbitrary modifier and an arbitrary variant have
/// no `;` and no closing bracket at the top level, and a `{` opens no
/// level: `[color:red;x:y]`, `w-[1px)]` and `[--x:{a:b}]` are not valid.
/// Before: no check. (A `}` with no open bracket before it is not valid
/// in each version: the output of the official compiler for it was not
/// valid CSS. See docs/SWITCHES.md.)
pub const ARBITRARY_VALUE_CHECK: Switch = Switch::since(4, 1, 0, "an arbitrary value has a check of its brackets and of ;");

/// `@source not "…"`, `@source inline("…")` and `@source not inline("…")`.
/// Before: each was the error "`@source` paths must be quoted."
pub const SOURCE_NOT_AND_INLINE: Switch = Switch::since(4, 1, 0, "@source not and @source inline()");

/// `--theme(--x)` and `theme(--x)` give the variable in each place.
/// Before: after the first `theme(…)` with a key path
/// (`theme(colors.red.500)`), the theme function of the plugin layer read
/// each next path: `--theme(--x)` gave the value and not `var(--x)`, and
/// the variable was not marked as used.
pub const THEME_VARIABLE_AFTER_KEY_PATH: Switch = Switch::since(4, 1, 0, "--theme(--x) gives the variable also after theme() with a key path");

/// `shadow-*` and `inset-shadow-*` take an opacity as a modifier
/// (`shadow-xl/50`), a shadow color is `color-mix(…)` with
/// `--tw-shadow-alpha`, and the two alpha variables have `@property`
/// rules. Before: the color as it was, no result for a theme shadow with a
/// modifier, and each other shadow ignored its modifier.
pub const SHADOW_ALPHA: Switch = Switch::since(4, 1, 0, "shadow-* and inset-shadow-* take an opacity modifier");

/// `drop-shadow-*` takes a color (`drop-shadow-red-500`) and an opacity
/// modifier, and writes the shadow to `--tw-drop-shadow-size`. The filter
/// utilities have three more `@property` rules. Before: only a theme
/// value or an arbitrary value, as one `drop-shadow(…)` for each shadow.
pub const DROP_SHADOW_COLOR: Switch = Switch::since(4, 1, 0, "drop-shadow-* takes a color and an opacity modifier");

/// The utilities `text-shadow-*`, and `text-*` ignores the theme keys
/// `--text-shadow-*`. Before: no such utilities, and `text-shadow-sm` was
/// the font size `--text-shadow-sm`.
pub const TEXT_SHADOW: Switch = Switch::since(4, 1, 0, "text-shadow-* utilities");

/// The utilities `mask-*`, and the `mask-*` and `--tw-mask-*` properties in
/// the order of the properties. Before: no such utilities, and these
/// properties had no place in the order.
pub const MASK_UTILITIES: Switch = Switch::since(4, 1, 0, "mask-* utilities");

/// The alignment utilities with `safe` (`items-center-safe`,
/// `justify-end-safe`, …) and with `last baseline` (`items-baseline-last`,
/// `self-baseline-last`). Before: no such utilities.
pub const SAFE_AND_LAST_BASELINE_ALIGNMENT: Switch = Switch::since(4, 1, 0, "safe and last baseline alignment utilities");

/// The utilities `wrap-anywhere`, `wrap-break-word` and `wrap-normal`.
/// Before: no such utilities.
pub const WRAP_UTILITIES: Switch = Switch::since(4, 1, 0, "wrap-* utilities");

/// The utilities `bg-size-*` and `bg-position-*` for arbitrary values.
/// Before: no such utilities.
pub const BG_SIZE_AND_POSITION: Switch = Switch::since(4, 1, 0, "bg-size-* and bg-position-* utilities");

/// The corner positions have the vertical side first: `bg-top-left`,
/// `bg-top-right`, `bg-bottom-left`, `bg-bottom-right`, and the same for
/// `object-*`. Before: only `bg-left-top`, `object-left-top`, … (they
/// stay, as old utilities).
pub const CORNER_POSITION_NAMES: Switch = Switch::since(4, 1, 0, "bg-top-left and object-top-left are names of corner positions");

/// The variants `details-content`, `user-valid`, `user-invalid`,
/// `inverted-colors`, `noscript`, `pointer-none`, `pointer-coarse`,
/// `pointer-fine`, `any-pointer-none`, `any-pointer-coarse` and
/// `any-pointer-fine`. Before: no such variants.
pub const STATIC_VARIANTS_OF_4_1: Switch = Switch::since(4, 1, 0, "details-content, user-valid, noscript, pointer-* and more variants");

// --- 4.0.16 ---------------------------------------------------------------

/// `--value('auto')` and `--modifier('auto')` of `@utility` match a named
/// value with the same text. Before: a literal matched nothing.
pub const CSS_UTILITY_LITERALS: Switch = Switch::since(4, 0, 16, "--value('…') and --modifier('…') take literal values");

/// A comma also ends a keyframes name in `animation` and in `--animate-*`:
/// `animation: a 1s, b 1s` uses `a` and `b`. Before: only white space ended
/// a name, so the `@keyframes` rule of a name before a comma was removed.
pub const KEYFRAME_NAMES_AFTER_COMMA: Switch = Switch::since(4, 0, 16, "a comma ends a keyframes name in an animation value");

/// `marker` also has the selectors `& *::-webkit-details-marker` and
/// `&::-webkit-details-marker`. Before: only the two `::marker` selectors.
pub const MARKER_DETAILS_MARKER: Switch = Switch::since(4, 0, 16, "the marker variant has ::-webkit-details-marker");

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
    &THEME_FN_NAMESPACE_ROOT,
    &NESTING_PASS,
    &SPACING_ZERO_PX,
    &SHADOW_FRACTIONAL_ALPHA,
    &ARIA_DATA_ATTRIBUTE_CHECK,
    &DEFAULT_THEME_SYSTEM_FONTS,
    &THEME_FN_PREFERS_DEFAULT,
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
    &LOGICAL_UTILITIES,
    &START_END_ARE_OLD_UTILITIES,
    &FONT_FEATURES,
    &FRACTION_SPACES,
    &RATIO_VALUE_SPACES,
    &ASPECT_FRACTION_DECIMALS,
    &INSET_SHADOW_LIST_SPACES,
    &UTILITY_NAME_ESCAPES,
    &UTILITY_NAME_SCANNER_RULES,
    &SUPPORTS_COLOR_MIX_NO_FALLBACK,
    &SOURCE_INLINE_WHITESPACE,
    &SLOT_NODES_STAY,
    &KEYFRAMES_NO_COLOR_MIX_FALLBACK,
    &NAMED_VALUE_CHARACTERS,
    &CSS_ERROR_LOCATION,
    &CONTENT_THEME_KEY,
    &THEME_FN_DEFAULT_NAMESPACE,
    &THEME_FN_KEEPS_KEY_CASE,
    &COLOR_MIX_FALLBACK_NO_AMPERSAND,
    &EMPTY_DATA_TYPE_IS_INVALID,
    &COLOR_MIX_FALLBACK_AMPERSAND,
    &COMPOUND_MODIFIER_TO_INNER_VARIANT,
    &AT_RULE_NAME_ENDS_AT_TAB,
    &STATIC_VALUE_COPIES,
    &THEME_FN_OBJECT_DEFAULT,
    &THEME_FN_NO_STRING_INDEX,
    &STATIC_VALUES_AFTER_THEME,
    &ORIGIN_PERCENTAGES,
    &GRID_LINE_THEME_KEYS,
    &AT_THEME_IS_A_FEATURE,
    &CUSTOM_VARIANT_CYCLE_ERROR,
    &CUSTOM_VARIANT_AT_VARIANT_COMPOUNDS,
    &EQUAL_DECLARATIONS_REMOVED,
    &AT_DASH_IS_INVALID,
    &VARIANT_NAME_START_AND_END,
    &SR_ONLY_CLIP_PATH,
    &TRANSITION_NO_VISIBILITY,
    &APPLY_IGNORES_IMPORTANT_OPTION,
    &EXTRA_SEMICOLON_IGNORED,
    &UTILITY_NAME_ERROR_REASONS,
    &DIVIDE_BORDER_COLOR_KEY,
    &MATH_PERCENT_AND_UPPERCASE_UNITS,
    &MATH_OPERATORS_NEED_VALUES,
    &CUSTOM_PROPERTY_STRINGS,
    &APPLY_ERROR_REASONS,
    &ROTATE_NEGATIVE_ARBITRARY,
    &CRLF_STAYS,
    &PAREN_VALUE_NEEDS_TWO_DASHES,
    &PAREN_MODIFIER_KEEPS_UNDERSCORES,
    &SCALE_NEGATIVE_ARBITRARY,
    &REFERENCE_NO_UTILITIES,
    &APPLY_NESTED_WALK,
    &APPLY_CANDIDATES_AS_KEYS,
    &FULL_ALPHA_NO_COLOR_MIX,
    &HEIGHT_LH,
    &TRANSITION_DISPLAY,
    &COLOR_MIX_FALLBACK_LAST,
    &COLOR_MIX_VARIABLE_CHAIN,
    &AT_ROOT_WITH_DASH,
    &TRANSFORM_EMPTY_FALLBACKS,
    &SHADOW_INHERIT,
    &BRACE_RANGE_DIRECTION,
    &COLOR_MIX_FALLBACK_FOR_EACH_COLOR,
    &PROPERTIES_LAYER,
    &CURRENTCOLOR_LOWERCASE,
    &DROP_SHADOW_LIST,
    &INSET_SHADOW_LIST,
    &FALLBACKS_AFTER_IMPORTS,
    &BRACE_RANGE_NO_PADDING,
    &AT_PROPERTY_FALLBACKS,
    &COLOR_MIX_FALLBACK,
    &GRADIENT_POSITION_FALLBACK,
    &ARBITRARY_VALUE_CHECK,
    &SOURCE_NOT_AND_INLINE,
    &THEME_VARIABLE_AFTER_KEY_PATH,
    &SHADOW_ALPHA,
    &DROP_SHADOW_COLOR,
    &TEXT_SHADOW,
    &MASK_UTILITIES,
    &SAFE_AND_LAST_BASELINE_ALIGNMENT,
    &WRAP_UTILITIES,
    &BG_SIZE_AND_POSITION,
    &CORNER_POSITION_NAMES,
    &STATIC_VARIANTS_OF_4_1,
    &CSS_UTILITY_LITERALS,
    &KEYFRAME_NAMES_AFTER_COMMA,
    &MARKER_DETAILS_MARKER,
];
