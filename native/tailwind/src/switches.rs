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
];
