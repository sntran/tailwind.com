# The Rust compiler as an older version: the switches

The Rust compiler is a port of one version of Tailwind CSS (the file
`UPSTREAM`). It can also compile as an older version. Each behavior that
changed in a release of Tailwind CSS is a **switch**: one named entry in
[`native/tailwind/src/switches.rs`](../native/tailwind/src/switches.rs),
and one condition at the place of the change.

```rust
/// `<!--` and `-->` between the rules of a stylesheet are skipped.
pub const HTML_COMMENT_MARKS: Switch = Switch::since(4, 3, 5, "…");
```

```rust
if switches::HTML_COMMENT_MARKS.on() { /* as the newest version */ } else { /* as before */ }
```

The compiler follows a version for each compile
(`native/tailwind/src/version.rs`). A switch is on when that version has
the release of the change.

`Version::OLDEST` is the oldest version that the Rust compiler serves.
The program uses the Rust compiler for each version from `OLDEST` to the
version of the port, and the script engine for each other version
([`VERSIONS.md`](VERSIONS.md)). So a switch that is not there makes
nothing wrong: that version is only slower.

| Version | Engine | Switches |
|---|---|---|
| 4.3.5 | Rust compiler | the port |
| 4.3.4 | Rust compiler | 3 |
| 4.3.3 | Rust compiler | 19 more |
| 4.3.2 | Rust compiler | 6 more |
| 4.3.1 | Rust compiler | 3 more |
| 4.3.0 | Rust compiler | 8 more |
| 4.2.4 | Rust compiler | 8 more |
| 4.2.3 | Rust compiler | 0 more |
| 4.2.2 | Rust compiler | 2 more |
| 4.2.1 | Rust compiler | 1 more |
| 4.2.0 | Rust compiler | 1 more |
| 4.1.18 | Rust compiler | 12 more |
| 4.1.17 | Rust compiler | 6 more |
| 4.1.16 | Rust compiler | 0 more |
| 4.1.15 | Rust compiler | 2 more |
| 4.1.14 | Rust compiler | 6 more |
| 4.1.13 | Rust compiler | 6 more |
| 4.1.12 | Rust compiler | 5 more |
| 4.1.11 | Rust compiler | 4 more |
| 4.1.10 | Rust compiler | 0 more |
| 4.1.9 | Rust compiler | 1 more |
| 4.1.8 | Rust compiler | 2 more |
| 4.1.7 | Rust compiler | 1 more |
| 4.1.6 | Rust compiler | 1 more |
| 4.1.5 | Rust compiler | 7 more |
| 4.1.4 | Rust compiler | 3 more |
| 4.1.3 | Rust compiler | 6 more |
| 4.1.2 | Rust compiler | 0 more |
| 4.1.1 | Rust compiler | 5 more |
| 4.1.0 | Rust compiler | 2 more |
| 4.0.17 | Rust compiler | 15 more |
| 4.0.0 to 4.0.16 | script engine | not counted |

All the 135 switches of Tailwind CSS 4.1.0 to 4.3.5 are there. Each of
these versions uses the Rust compiler also for a stylesheet with
`@plugin` or `@config`: see "The plugin layer" below.

## How to write a switch

You need the official packages of two versions: the version before the
change and the version with it.

```sh
cargo build --release --bin tailwind --example fuzz_build
sh tests/fuzz/install.sh build/official-4.3.3 4.3.3
sh tests/fuzz/install.sh build/official-4.3.4 4.3.4
```

1. **Find a difference.** The fuzzer compares the Rust compiler with the
   official compiler of a version. The Rust compiler compiles as the
   version of the package that it compares with. Each difference is a
   behavior with no switch, and the fuzzer makes the case small:

   ```sh
   node tests/fuzz/fuzz.mjs --official build/official-4.3.3 \
     --builder target/release/examples/fuzz_build --program target/release/tailwind \
     --seed 1 --cases 600 --projects 0 --out build/fuzz-4.3.3
   ```

2. **Find the change of Tailwind CSS.** The list below names the file
   and the function for the 4.3 releases. For another release:
   `sh scripts/sync-upstream.sh report` shows each changed file of
   Tailwind CSS and the Rust file that follows it, and the changelog of
   Tailwind CSS names the pull request.

3. **Write the case** as one line of `tests/upstream/switches.jsonl`
   (`name`, `css`, `candidates`). A case must be complete in itself:
   `@theme` and `@tailwind utilities`, and no `@import "tailwindcss"`.

4. **Write the switch**: the entry in `switches.rs` (with the version of
   the release, and what was before), and the condition in the code.

5. **Compare and record** with the two versions. Each case must be the
   same as the official compiler of each version:

   ```sh
   cargo build --release --example fuzz_build
   for v in 4.3.3 4.3.4; do
     node tests/fuzz/compare.mjs --official build/official-$v \
       --builder target/release/examples/fuzz_build --record tests/upstream/switches.jsonl
   done
   cargo test --release --test switches
   ```

6. **When the fuzzer finds no more difference** for a version, and each
   change of that release in the list has its switch or its reason for
   none, lower `Version::OLDEST` to that version. `tests/versions.sh`
   then compares the two engines for it on each system.

## The rules

- **The newest behavior is the port.** A switch only adds the old branch.
  A sync to a new version of Tailwind CSS (`SYNC.md`) adds one switch for
  each behavior that it changes, with the old code in the other branch.
- **Data comes from the package.** `theme.css` and `preflight.css` of a
  version come from its package in the file, so a change of a theme
  value needs no switch.
- **Faults that stop the compiler.** Where an old version of Tailwind CSS
  stops with a JavaScript exception of its own code (a fault that a
  later release fixed), the Rust compiler does not stop: it gives the
  result of the fixed version. A wrong output of an old version, which
  is valid CSS, is kept as it was.
- **Output that is not valid CSS.** Where an old version of Tailwind CSS
  leaves one of its own at-rules in the output (a fault that a later
  release fixed), the Rust compiler gives the result of the fixed
  version. Before 4.1.14, an `@variant` rule in the body of
  `@custom-variant` stayed in the output. The fuzzer knows this fault
  (`hasKnownFault` in `tests/fuzz/fuzz.mjs`): when the official result
  has an `@variant` rule, it compares the case again with no candidate
  that shows the fault.
- **A behavior of one version only** needs two switches, one for its
  start and one for its end. (In the list, `addbase-variant-eager` is
  only in 4.3.1, but it is in the plugin layer. The `& { … }` rule in the
  `@supports` rule of a `color-mix()` fallback is only in 4.1.15:
  `COLOR_MIX_FALLBACK_AMPERSAND` is its start and
  `COLOR_MIX_FALLBACK_NO_AMPERSAND` is its end.)
- **The plugin layer has no switches: each version has its own.** See
  the next section.

## The plugin layer

The plugin layer is JavaScript: `packages/tailwindcss/src/compat` of
Tailwind CSS, with the shims of `native/tailwind/js`, in one file
(`native/tailwind/assets/compat.js` for the version of the port). The
program has that file for each version from
`Version::OLDEST_WITH_PLUGINS` to the version of the port, made from
the source of that version with no change. So the Rust compiler as
version N runs the plugin layer of version N, and a change of Tailwind
CSS in its plugin layer needs no switch and no work: the list marks it
`JS`.

- `sh scripts/compat.sh UPSTREAM_CLONE` makes all the files. An older
  version is a patch file (`native/tailwind/assets/compat/VERSION.patch`,
  3 KB to 12 KB): the difference from the file of the next newer
  version, in the format of the packages (`src/patch.rs`). The patch
  files are in the program (76 KB in all), and `compat/js/bundles.rs`
  reads them.
- The files do not depend on the computer. The job "test" of
  `.github/workflows/build.yml` makes them again from the source of
  Tailwind CSS and compares them with the repository.
- A change of `native/tailwind/js` (the shims) changes each file: run
  the script again.
- The same shims must build with the source of each version. One version
  needed its own shim: only the plugin layer of 4.3.1 calls
  `substituteAtVariant`, which is in the host (`js/shims/variants.ts`).
- The parts of `compat/` that are Rust have switches as each other code:
  the default theme of `theme(fontFamily.sans)`
  (`DEFAULT_THEME_SYSTEM_FONTS`) and the old utilities `start` and `end`
  (`LEGACY_INSET_NEEDS_VALUE`).
- For a version that is older than `Version::OLDEST_WITH_PLUGINS`, the
  Rust compiler does not compile a stylesheet with `@plugin` or
  `@config`: it gives the error `ScriptEngineNeeded` before it loads a
  module, and the program uses the script engine for that stylesheet.
  Now `OLDEST_WITH_PLUGINS` is `OLDEST`, so this does not occur.

## The changes of Tailwind CSS, release by release

This list comes from the source of Tailwind CSS: each change between two
releases that can change the result of `compile` and `build`.
Kind: `form` (a valid input gives another text), `fix` (the old behavior
was a fault), `feature` (a new input works), `data` (a constant or a
table). Effort: `S` (one condition or one constant), `M` (one function in
two forms), `L` (an algorithm in two forms). How often: in a normal
project. `JS` marks a change in the plugin layer.

### 4.3.5 (3 switches, all there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `COLOR_MIX_NEEDS_METHOD` | fix | S | edge | `color-mix(…)` with no `in …` got a fallback from the wrong node |
| `HTML_COMMENT_MARKS` | fix | S | edge | `-->` started a custom property; `<!--` was a part of the next rule |
| `ONE_CANDIDATE_NO_VARIANT_ORDER` | (no visible change found) | S | edge | The variant order was computed also for one candidate |

### 4.3.4 (19 switches, all there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `AT_PROPERTY_SUPPORTS_BLOCK` | data | S | each build | The `@supports` rule of the fallbacks had `margin-trim: inline`, not `block` |
| `EMPTY_VAR_FALLBACK_SPACE` | data | S | common | `var(--tw-blur,)`, not `var(--tw-blur, )` (8 utilities) |
| `GROUP_PEER_TARGET_IN_IS` | form | S | common | `.x:is(:where(.group):hover *)`, not `:is(:where(.group):hover .x)` |
| `SCOPE_HOISTING` | fix | S | rare | `@scope` stayed nested in the rule, with its contents as they were |
| `DEAD_PSEUDO_ELEMENT_RULES` | fix | S | rare | A rule after a pseudo-element (`::selection > *`) was in the output |
| `UNUSED_MODIFIER_IS_INVALID` | fix | S | rare | `rounded-sm/50` gave the class with the modifier ignored |
| `EMPTY_EXTRA_MODIFIER_IS_INVALID` | fix | S | edge | `bg-red-500/50/` was valid. Two variants that differed only in an empty second modifier kept the sequence of the candidates |
| `BALANCED_ARBITRARY_VALUE` | fix | M | rare | `w-[calc(1px]` was valid: only `;` and a closing bracket at depth 0 were errors |
| `SUPPORTS_KEYWORDS_AS_FUNCTIONS` | fix | M | rare | `and`, `or`, `not` got spaces around them in all of a `supports-[…]` value |
| `BREAKPOINT_DECIMAL_SORT` | fix | S | edge | `40.25rem` and `40.5rem` were equal in the sort (`parseInt`) |
| `COMMENT_BACKSLASH_END` | fix | S | edge | `\` before `*/` kept a comment open |
| `IMPORTANT_AT_THE_END` | fix | M | edge | The first `!important` in a value cut the value |
| `CUSTOM_PROPERTY_AT_EOF` | fix | M | edge | A custom property at the end of the input lost its last character |
| `DECLARATION_AT_EOF` | fix | S | edge | A declaration with no `;` at the end of the input was dropped |
| `UNESCAPE_ONE_HEX_DIGIT` | fix | S | edge | `\0` … `\f` gave the digit, not the code point |
| `MORE_LENGTH_UNITS` | data | S | rare | `dvmin`, `svb`, `cap`, … were not lengths: `text-[10dvmin]` was a color |
| `MOZ_ANY_NESTING` | fix | S | edge | `&` in `:-moz-any(…)` was not replaced |
| `DEFAULT_FN_WHITESPACE` | fix | S | rare | White space in `--default(…)` of `@utility` was removed |
| `THEME_FN_NAMESPACE_ROOT` (also in the plugin layer) | fix | S | rare | `theme(spacing)` with `--spacing` of `@theme default` was an error, and `theme(blur)` gave the value of the default configuration. Plugin `theme('shadow')` gave the DEFAULT text, not the object |

### 4.3.3 (6 switches, all there; 1 change in the plugin layer; data changes with no switch)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `NESTING_PASS` | form | M | each build | No pass that flattens rules: `.hover\:x { &:hover { @media (hover: hover) { … } } }`. The old code also removed equal declarations in each rule, and replaced a `&` rule by its children |
| `SPACING_ZERO_PX` | form | S | common | `--spacing(0)` gave `0`, not `0px` |
| `SHADOW_FRACTIONAL_ALPHA` | fix | S | rare | `shadow-sm/12.5` gave the class with no alpha |
| `ARIA_DATA_ATTRIBUTE_CHECK` | fix | S | edge | `data-[foo bar]:flex` gave a rule with a selector that is not valid |
| `DEFAULT_THEME_SYSTEM_FONTS` | data | S | edge | `theme(fontFamily.sans)` with no `--font-sans` gave `ui-sans-serif, system-ui, …` |
| selector-nth-child-of (JS) | form | M | edge | Classes after `of` in `:nth-child(…)` got no prefix in `addUtilities` |
| `THEME_FN_PREFERS_DEFAULT` (also in the plugin layer) | fix | S | edge | `theme(colors.foo / 50%)` had no opacity when `--color-foo-bar` was there too. Plugin `theme('colors.foo')` gave an object |

Data, from the package: the new `--font-sans` stack (`theme.css`,
`preflight.css`), `:-moz-focusring:where(:not(iframe))` in
`preflight.css`, and `oklch(L 0 none)` for 13 gray colors. The same
values are also in `tailwindcss/colors` and `tailwindcss/defaultTheme` of
the plugin layer: the plugin layer of each version has its own values.

### 4.3.2 (3 switches, all there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `AUTO_TRACKS_BARE_SPACING` | feature | S | rare | `auto-rows-12` gave nothing |
| `SPACING_FN_IS_LENGTH` (also in the plugin layer) | fix | S | edge | `text-[--spacing(2)]` was a color |
| `ALPHA_FN_IS_COLOR` (also in the plugin layer) | fix | S | edge | `--alpha(…)` was not a color for the type of a value |

### 4.3.1 (8 switches, all there; 2 changes in the plugin layer)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `SPACING_FN_FOLD` | form | S | each build | `m-1` gave `calc(var(--spacing) * 1)`, and `m-0` gave `calc(var(--spacing) * 0)` |
| `SPACE_ZERO` | form | S | common | `space-x-0` gave two `calc(…)` values, not `0` |
| `MASK_ANGLE_FOLD` | form | S | rare | `mask-linear-1` gave `calc(1deg * 1)` |
| `INSET_SHADOW_NONE_INSET` | fix | S | rare | `inset-shadow-none` had no `inset` |
| `APPLY_CSS_MIXINS` | feature | S | edge | `@apply --my-mixin` was an error |
| `APPLY_BODY_ERROR` | fix | S | edge | `@apply underline { … }` dropped the body with no error |
| `SHADOW_COLOR_DETECTION` | fix | M | rare | The color of a shadow was the first part that is not a length |
| `NOT_CONTAINER_VALUE_PARSER` | fix | M | rare | `not-*` on `@container style(…)` gave a condition that is not valid |
| addbase-variant-eager (JS) | fix | M | rare | Only in 4.3.1: `@variant` in `addBase` was replaced when the plugin loaded |
| plugin-selector-whitespace (JS) | form | L | rare | Selectors of plugins kept their text: `&>*`, not `& > *` |

### 4.3.0 (8 switches, all there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `ZOOM_TAB_SCROLLBAR` | feature, data | S | rare | No `zoom-*`, `tab-*`, `scrollbar-*`, `scrollbar-thumb-*`, `scrollbar-track-*`, `scrollbar-gutter-*`; their five properties had no place in the property order |
| `CONTAINER_SIZE` | feature | S | rare | No `@container-size` in a release |
| `CSS_UTILITY_DEFAULT` | feature | M | rare | A functional `@utility` had no `--default(…)`, and did not match a candidate with no value |
| `CSS_UTILITY_NEEDS_VALUE` | fix | S | edge | A functional `@utility` with no `--value(…)` matched each value |
| `UTILITY_NULL_FALLTHROUGH` | fix | S | edge | A utility that was not valid stopped the other utilities with the same name |
| `AT_VARIANT_LIST` | feature | S | rare | `@variant hover, focus` and `@variant hover:focus` were errors |
| `NEGATIVE_CALC_WHITESPACE` | form | S | edge | `-m-[1px+2px]` gave `calc(1px+2px * -1)` |
| `LEGACY_INSET_NEEDS_VALUE` | fix | S | edge | The old utilities `start` and `end` with no value gave the value of `--inset` or `--spacing` |

### 4.2.4 (no change of the compiler)

### 4.2.3 (2 switches, all there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `PLACEHOLDER_COLOR_KEY` | fix | S | rare | `placeholder-*` read `--background-color`, not `--placeholder-color` |
| `UNESCAPE_NUL_IS_REPLACED` | fix | S | edge | An escape of the code point 0 gave that code point, not U+FFFD |

No switch: an escape above U+10FFFF stopped the official compiler (a
fault that stops the compiler). `NODE_PATH` with more than one directory
is in the module loader of the program, which is the same for each
version.

### 4.2.2 (1 switch, there; 2 changes in the plugin layer)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `FRACTION_WHITE_SPACE` | fix | S | edge | `1 / 2` was not a fraction for the type of a value (a fault in the pattern) |
| plugin-values-own-property (JS) | fix | S | edge | `matchUtilities` and `matchVariant` took `constructor` and `toString` as values |
| theme-get-own-property (JS) | fix | S | edge | `theme('colors.constructor')` gave a function |

No switch: a static value with the name of a property of `Object`
(`auto-rows-constructor`) stopped the official compiler.

### 4.2.1 (1 switch, there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `UTILITY_ROOT_ENDING_DASH` | fix | S | edge | Only in 4.2.0: `@utility foo--*` was not a valid name |

### 4.2.0 (12 switches, all there; 1 change in the plugin layer; data changes with no switch)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `FRACTION_SPACES` | form | S | common | `w-1/2` gave `calc(1/2 * 100%)`, not `calc(1 / 2 * 100%)` |
| `LOGICAL_UTILITIES` | feature, data | S | rare | No `inset-s-*`, `inset-e-*`, `inset-bs-*`, `inset-be-*`, `mbs-*`, `mbe-*`, `pbs-*`, `pbe-*`, `scroll-mbs-*`, `scroll-mbe-*`, `scroll-pbs-*`, `scroll-pbe-*`, `border-bs-*`, `border-be-*`, `inline-*`, `block-*`, `min-inline-*`, `max-inline-*`, `min-block-*`, `max-block-*`; the `…-block-start` and `…-block-end` properties had no place in the property order |
| `START_END_ARE_OLD_UTILITIES` | form | S | edge | `start-*` and `end-*` were spacing utilities as `inset-x-*`: `start` with no value and `start-4/[5]` gave nothing |
| `FONT_FEATURES` | feature, data | S | rare | No `font-features-*`; `font-feature-settings` had no place in the property order |
| `RATIO_VALUE_SPACES` | form | S | rare | `--value(ratio)` of `@utility` gave `1/2`, not `1 / 2`, and `foo-01/2` gave nothing |
| `ASPECT_FRACTION_DECIMALS` | feature | S | rare | `aspect-8.5/11` gave nothing |
| `INSET_SHADOW_LIST_SPACES` | form | S | rare | A list of inset shadows had `,` and not `, ` between its shadows, and each shadow kept its spaces |
| `UTILITY_NAME_ESCAPES` | fix | S | rare | `@utility push-1\/2` was an error |
| `UTILITY_NAME_SCANNER_RULES` | fix | S | edge | The name of `@utility` had one pattern: `ui/button/sm`, `a.b`, `a-` and `a.5-*` were valid names |
| `SUPPORTS_COLOR_MIX_NO_FALLBACK` | fix | S | rare | A declaration in `@supports (color: color-mix(…))` got a fallback and a second `@supports` rule |
| `SOURCE_INLINE_WHITESPACE` | fix | S | edge | `@source inline( "underline" )` was the error "`@source` paths must be quoted." |
| `SLOT_NODES_STAY` | form | S | edge | An `@keyframes` or `@property` rule in the nodes that replace `@slot` went out to the root |
| colors (JS) | data | S | rare | `tailwindcss/colors` had no `mauve`, `olive`, `mist` and `taupe` |

No switch: an `@custom-variant` whose `@slot` gets nodes with an `@slot`
(`@custom-variant b { @variant a { @slot; } }`, where `a` is also an
`@custom-variant` with a body) made an endless loop in the official
compiler (a fault that stops the compiler). The new walk of the tree
(`walk.ts`) gives the same result. Data, from the package: the colors
`mauve`, `olive`, `mist` and `taupe` of `theme.css`.

### 4.1.18 (6 switches, all there; 1 change in the plugin layer)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `KEYFRAMES_NO_COLOR_MIX_FALLBACK` | fix | S | rare | A declaration with `color-mix(…)` in `@keyframes` got a fallback and an `@supports` rule |
| `NAMED_VALUE_CHARACTERS` | fix | S | edge | `bg-foo=bar` and `data-foo=bar:flex` were valid: a named value had no check of its characters |
| `CSS_ERROR_LOCATION` | form | S | rare | The message of a CSS syntax error had no `file:line:column: ` at its start (the compiler has the file only with the `from` option) |
| `CONTENT_THEME_KEY` | feature | S | rare | `content-*` did not read `--content`: `content-slash` gave nothing |
| `THEME_FN_DEFAULT_NAMESPACE` | fix | S | edge | `theme(ringColor.DEFAULT)` read `--ring-color`, not `--default-ring-color` (also `borderWidth`, `outlineWidth`, `ringWidth`, `transitionDuration`, `transitionTimingFunction`) |
| `THEME_FN_KEEPS_KEY_CASE` | fix | S | edge | `theme(colors.fooBar)` read `--color-foo-bar`, not `--color-fooBar` |
| config-theme-keys (JS) | fix | M | rare | The keys of the theme of a config changed to kebab case, and `ringColor.DEFAULT` of a config was `--ring-color` |

No switch: the new function `candidatesToAst` and the file `cartesian.ts`
are for the editor tools and the canonical form of candidates. The
check of `source(…)` against the directory of its file, and its new
message, are in the code of the command line (`cli/mod.rs`). That code
has no switches: it gives the result of 4.1.18 for each version.

### 4.1.17 (no switch)

No switch: an `@variant` rule in the rules of a candidate
(`[@variant_hover]:flex`) stayed in the output, which was not valid CSS.
The Rust compiler replaces it in each version, as 4.1.17 does.

### 4.1.16 (2 switches, all there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `COLOR_MIX_FALLBACK_NO_AMPERSAND` | form | S | common | The `@supports` rule of a `color-mix(…)` fallback had the declaration in a `& { … }` rule |
| `EMPTY_DATA_TYPE_IS_INVALID` | fix | S | edge | `bg-[:red]` gave `background-color: red` |

No switch: the changes of `canonicalize-candidates.ts`, `signatures.ts`,
`expand-declaration.ts` and `intellisense.ts` do not change the result of
a build.

### 4.1.15 (6 switches, all there; 1 change only in the plugin layer)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `COLOR_MIX_FALLBACK_AMPERSAND` | form | S | common | Only in 4.1.15: the `@supports` rule of a `color-mix(…)` fallback has the declaration in a `& { … }` rule. Before, it had the declaration itself, as since 4.1.16 |
| `COMPOUND_MODIFIER_TO_INNER_VARIANT` | fix | S | rare | `not-group-hover/name:flex` gave nothing: the modifier stayed on `not-*`, `has-*` or `in-*` |
| `AT_RULE_NAME_ENDS_AT_TAB` | fix | S | edge | A tab after the name of an at-rule was a part of the name: `@media\tprint` kept the tab, and `@apply\tflex` stayed in the output |
| `STATIC_VALUE_COPIES` | fix | M | rare | Only in 4.1.14: `z-auto!` made the declarations of each `z-auto` rule `!important`, also `hover:z-auto` and a later `@apply z-auto` |
| `THEME_FN_OBJECT_DEFAULT` (also in the plugin layer) | fix | S | edge | `theme(colors.foo)` with `--color-foo` and `--color-foo-bar` was an error, or gave its fallback |
| `THEME_FN_NO_STRING_INDEX` (also in the plugin layer) | fix | S | edge | `theme(colors.what.1)` gave the second character of the value of `--color-what` |
| selector-parser (JS) | fix | S | edge | In the selector parser of `addUtilities`, `&` and `*` were not nodes of their own: `&[x]` was one node |

No switch: the new walk of the tree (`walk.ts`) gives the same result.
It visits the context nodes, and the code that needs the parent or the
context reads them with `cssContext`. The value parser makes `/` a word
and not a separator: the text of a value stays the same.
`cloneAstNode` replaces `structuredClone`. `break-words` is now an old
utility of the plugin layer, which is there with no plugin: the result
is the same. The files `canonicalize-candidates.ts`, `signatures.ts`,
`constant-fold-declaration.ts`, `attribute-selector-parser.ts`,
`utils/dimensions.ts` and `intellisense.ts` are for the editor tools and
the canonical form of candidates. `selector-parser.ts` is for the plugin
layer.

`STATIC_VALUE_COPIES` is the end of a fault of one version. In 4.1.14,
the fixed values of a utility gave the same node objects to each
candidate. The Rust compiler has no shared objects: it gives each such
declaration an identity, keeps the identities with `!important` in the
design system, and gives the flag to each copy at the end of
`compile_candidates` (`compile::share_declarations`).

### 4.1.14 (6 switches, all there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `STATIC_VALUES_AFTER_THEME` | fix | S | rare | Each fixed value of a functional utility was a static utility: `rounded-full` with `--radius-full` gave two rules (also `z-auto`, `order-first`, `leading-none`, `transition-all` and the other fixed values) |
| `ORIGIN_PERCENTAGES` | form | S | common | `origin-top-left` gave `top left`, not `0 0` (six values of `origin-*` and of `perspective-origin-*`) |
| `GRID_LINE_THEME_KEYS` | fix | S | edge | `col-start-foo` with `--grid-column-start-foo` also gave `grid-column: var(--grid-column-start-foo)` (also `col-end-*`, `row-start-*`, `row-end-*`) |
| `AT_THEME_IS_A_FEATURE` | fix | S | rare | A stylesheet with `@theme` and no other feature of Tailwind CSS gave its input text with no change |
| `CUSTOM_VARIANT_CYCLE_ERROR` | fix | S | edge | Custom variants that use each other in a circle were no error |
| `CUSTOM_VARIANT_AT_VARIANT_COMPOUNDS` | fix | S | edge | `group-a:flex` gave nothing when the body of `@custom-variant a` had an `@variant` rule |

No switch: an `@variant` rule in the body of `@custom-variant` stayed in
the output, which was not valid CSS. The Rust compiler replaces it in
each version, as 4.1.14 does, and the fuzzer knows this fault (see "The
rules"). There is one exception: for a version before 4.1.14 and a
stylesheet with a circle of custom variants, the Rust compiler keeps
each `@variant` rule of a custom variant, as the official compiler of
that version did. For a circle, the fixed version gives only an error.
When a candidate uses a custom variant whose `@variant` rule names a
variant that is not there, `build` gives the error of Tailwind CSS
("Cannot use `@variant` with unknown variant") in each version.

No switch: the new suggestions of `@utility` are for the editor tools.
`@container-size` was only in a build that was not a release
(`CONTAINER_SIZE`, 4.3.0).

The scanner of the source files follows no version, in each engine. Its
changes in these releases: the Markdown and Ruby pre-processors, and the
`.jsonl` and `.ndjson` files.

### 4.1.13 (5 switches, all there; 2 changes in the plugin layer)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `EQUAL_DECLARATIONS_REMOVED` | form | S | common | A rule kept each of its equal declarations: `.a { color: red; color: red; }` stayed as it was, and `@apply` of two utilities with the same declaration gave it two times |
| `AT_DASH_IS_INVALID` | fix | S | edge | `@-md:flex` was `@md:flex` |
| `VARIANT_NAME_START_AND_END` | fix | S | edge | `@custom-variant foo-`, `-foo`, `_foo` and `Foo` were valid names, and the message for a name that is not valid had no rule about the start |
| `SR_ONLY_CLIP_PATH` | data | S | common | `sr-only` gave `clip: rect(0, 0, 0, 0)`, and `not-sr-only` gave `clip: auto` |
| `TRANSITION_NO_VISIBILITY` | data | S | common | From 4.1.5: the default of `transition` had `visibility` after `display` |
| variant-name (JS) | fix | S | edge | `addVariant` had the old rule and the old message for a name |
| match-variant-unknown-value (JS) | fix | S | edge | `matchVariant` gave a rule for a named value that is not in `values`, and for a value that is not a text |

No switch: the suggestions of `matchVariant` and `matchUtilities` are for
the editor tools. `css-functions.ts` has only a new comment. The scanner
of the source files ignores `.vercel` directories: it follows no version.

`EQUAL_DECLARATIONS_REMOVED` also changes the `color-mix(…)` fallbacks.
From 4.1.13 to 4.3.2, the step that removes equal declarations made a
new list of nodes for a rule with such declarations. The fallback went
into the old list, so that rule has no fallback (see `NESTING_PASS`).
Before 4.1.13, the rule keeps its list and gets the fallback. A
declaration in a rule with the selector `&` gets no fallback from 4.1.4
to 4.3.2, before and after this change: its list is not in the tree.

### 4.1.12 (4 switches, all there; 1 change in the plugin layer; data changes with no switch)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `APPLY_IGNORES_IMPORTANT_OPTION` | fix | S | rare | With the `important` option, each declaration of `@apply` was `!important`. `@apply` and the build had one cache of the rules of a candidate |
| `EXTRA_SEMICOLON_IGNORED` | fix | S | rare | `color: red;;` and a `;` after a rule were the error "Unexpected semicolon" |
| `UTILITY_NAME_ERROR_REASONS` | form | S | edge | `@utility foo*` and `@utility my-*-utility` had the general message for a name that is not valid |
| `DIVIDE_BORDER_COLOR_KEY` | fix | S | rare | `divide-*` did not read `--border-color-*` |
| plugin-false-values (JS) | fix | S | edge | A property with the value `false` or `undefined` in a CSS object of a plugin gave a declaration |

No switch: the suggestions of `flex-*` are for the editor tools. The
source locations of the nodes of `@plugin` and `@config` are for the
source maps. Data, from the package: the icon of an input with a list
(`::-webkit-calendar-picker-indicator` in `preflight.css`).

The one cache of `APPLY_IGNORES_IMPORTANT_OPTION` has a visible effect
before 4.1.6 (`APPLY_NESTED_WALK`). There, an `@apply` can compile a
utility before the `@apply` rules of that utility are done. The utility
keeps that form when it is also a candidate of the build.

### 4.1.11 (no change of the compiler)

### 4.1.10 (1 switch, there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `MATH_PERCENT_AND_UPPERCASE_UNITS` | fix | S | common | Only in 4.1.9: `w-[calc(100%-var(--x))]` gave `calc(100%-var(--x))`. The `-` after `%` or an uppercase unit, and before a name, got no spaces |

### 4.1.9 (2 switches, all there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `MATH_OPERATORS_NEED_VALUES` | fix | M | rare | Each `+`, `-`, `*` and `/` in a math function got spaces: `min(fit-content,1px)` gave `min(fit - content, 1px)`. `round(to-zero,1+2)` gave `round(to-zero,1 + 2)` |
| `CUSTOM_PROPERTY_STRINGS` | fix | S | rare | A `;` in a string of a custom property ended the value: `--x: 'data:text/plain;base64,…'` was the error "Unterminated string" |

No switch: `compat/container.ts` has only a new comment.

### 4.1.8 (1 switch, there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `APPLY_ERROR_REASONS` | form | S | rare | The error of `@apply` was always `Cannot apply unknown utility class: NAME` |

No switch: `order-none` moved to the old utilities of the plugin layer,
with the same result. `@tailwind utilities` becomes a context node after
the `@apply` step, with no visible change. The value parser stopped at a
`)` with no `(` (a fault that stops the compiler; the CSS parser and the
candidate parser refuse such a value first).

### 4.1.7 (1 switch, there; 1 change in the plugin layer)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `ROTATE_NEGATIVE_ARBITRARY` | fix | S | rare | `-rotate-[var(--r)]` gave `rotate: var(--r)`: only a value with the type angle was negated |
| merge-custom-variant (JS) | fix | S | edge | A variant with `:merge(…)` of a plugin was not ignored |

### 4.1.6 (7 switches, all there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `CRLF_STAYS` | form | S | rare | The parser replaced each `\r\n` of the input with `\n`, also in a comment, in a string and after a `\` |
| `PAREN_VALUE_NEEDS_TWO_DASHES` | fix | S | edge | `w-(-x)` and `w-(a-b)` were valid: one `-` in the first two characters was sufficient. `w-(--x;y)` was valid |
| `PAREN_MODIFIER_KEEPS_UNDERSCORES` | fix | S | rare | `bg-red-500/(--my_opacity)` gave `var(--my opacity)` |
| `SCALE_NEGATIVE_ARBITRARY` | fix | S | rare | `-scale-[var(--s)]` gave `scale: var(--s)` |
| `REFERENCE_NO_UTILITIES` | fix | S | rare | `@tailwind utilities` in a reference was the utilities node: a later `@tailwind utilities` gave nothing |
| `APPLY_NESTED_WALK` | fix | M | rare | An `@utility` that another `@utility` applied lost its nested rules with `@apply`. An `@apply` at the top level of a stylesheet gave nothing |
| `APPLY_CANDIDATES_AS_KEYS` | form | S | edge | The first candidate of `@apply` that is not valid gave the error, in the sequence of the text: now a number comes first |

No switch: the source maps (`ast.ts`, `css-parser.ts`, `at-import.ts`,
`theme.ts`, `source-maps/`), and `printCandidate` and `printVariant` of
`candidate.ts` (only the upgrade tool uses them).

`APPLY_NESTED_WALK` needs the one cache of the compiler of before 4.1.12
for a utility that is also a candidate: `@apply` and the build shared
the compiled rules (`APPLY_IGNORES_IMPORTANT_OPTION`).

### 4.1.5 (3 switches, all there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `FULL_ALPHA_NO_COLOR_MIX` | form | S | common | `bg-red-500/100` gave `color-mix(in oklab, … 100%, transparent)` |
| `HEIGHT_LH` | feature | S | rare | No `h-lh`, `min-h-lh`, `max-h-lh` |
| `TRANSITION_DISPLAY` | data | S | common | The default of `transition` ended at `backdrop-filter` |

No switch: the suggestions of the editor tools (the default shadows, and
`_` between digits in a theme key of `@utility`).

### 4.1.4 (6 switches, all there; 1 change in the plugin layer)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `COLOR_MIX_FALLBACK_LAST` | fix | M | rare | The fallback of `color-mix(…)` was made at the declaration: a theme variable with a fallback was not in its list, and the removal of an unused one took the last node of the list |
| `COLOR_MIX_VARIABLE_CHAIN` | fix | S | rare | A theme variable with the value `var(--other)` went into the fallback as it was |
| `AT_ROOT_WITH_DASH` | fix | S | rare | `@foo-bar:flex` and `@[calc(1rem-1px)]:flex` were not valid: an input with a dash did not get the `@` root |
| `TRANSFORM_EMPTY_FALLBACKS` | form | S | common | `transform: var(--tw-rotate-x) …`, and `@property --tw-rotate-x` with the initial value `rotateX(0)` |
| `SHADOW_INHERIT` | fix | S | rare | `shadow-inherit` gave `color-mix(in oklab, inherit var(--tw-shadow-alpha), transparent)` |
| `BRACE_RANGE_DIRECTION` | fix | S | edge | `{10..0..5}` and `{0..10..-5}` in `@source inline(…)` were empty |
| font-size-tuple (JS) | fix | S | rare | A `fontSize` array of a config gave one theme value with the two texts |

### 4.1.3 (no switch)

A `--value(…)` of `@utility` with a data type that is not valid for a
bare value gives a warning (`console.warn`). The result is the same. The
Rust compiler gives the warning as each version.

### 4.1.2 (5 switches, all there; 1 change in the plugin layer)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `COLOR_MIX_FALLBACK_FOR_EACH_COLOR` | fix | M | common | Only a variable with a value in the theme gave a fallback: `bg-current/50` and `bg-(--x)/50` had none |
| `PROPERTIES_LAYER` | form | S | each build | The fallbacks of `@property` were `@supports … { @layer base { … } }` near the start, not `@layer properties` at the end |
| `CURRENTCOLOR_LOWERCASE` | data | S | common | `currentColor`: `bg-current`, `ring`, `inset-ring`, `theme(borderColor.DEFAULT)` |
| `DROP_SHADOW_LIST` | fix | S | rare | A `--drop-shadow-*` value with two shadows gave one `drop-shadow(a, b)` |
| `INSET_SHADOW_LIST` | fix | S | edge | `inset-shadow-[a,b]` had `inset` only before the first shadow |
| plugin-current-color (JS) | data | S | rare | `currentColor` in `colors.current` and in the plugin API |

Data, from the package: `currentcolor` in `preflight.css`. No switch:
before 4.1.2, a `color-mix(…)` with less than three parts
(`color-mix(red)`) stopped the official compiler.

### 4.1.1 (2 switches, all there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `FALLBACKS_AFTER_IMPORTS` | fix | S | rare | The fallbacks of `@property` went after the comments at the start: before an `@charset`, an `@import` or an `@layer` with no body |
| `BRACE_RANGE_NO_PADDING` | fix | S | rare | `{0..10}` in `@source inline(…)` gave `00`, `01`, … `10` |

### 4.1.0 (15 switches, all there; 3 changes in the plugin layer; data changes with no switch)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `AT_PROPERTY_FALLBACKS` | feature | S | each build | The `@property` rules had no fallbacks: no `@supports … { @layer base { … } }` rule |
| `COLOR_MIX_FALLBACK` | feature | S | common | A declaration with `color-mix(…)` and a theme variable had no fallback and no `@supports` rule |
| `GRADIENT_POSITION_FALLBACK` | form | S | common | `bg-linear-to-r` gave one `--tw-gradient-position: to right in oklab`, with no `@supports` rule |
| `SHADOW_ALPHA` | feature, form | M | common | `shadow-red-500` gave `--tw-shadow-color: var(--color-red-500)`, `shadow-sm/50` gave nothing, `shadow/50` and `shadow-[…]/50` ignored the modifier, and `--tw-shadow-alpha` and `--tw-inset-shadow-alpha` had no `@property` rule |
| `DROP_SHADOW_COLOR` | feature, form | M | common | `drop-shadow-*` took only a theme value or an arbitrary value: no color, no modifier, and no `--tw-drop-shadow-color`, `--tw-drop-shadow-alpha` and `--tw-drop-shadow-size` (each filter utility had three fewer `@property` rules) |
| `TEXT_SHADOW` | feature | S | rare | No `text-shadow-*`; `text-shadow-sm` was the font size `--text-shadow-sm` |
| `MASK_UTILITIES` | feature, data | S | rare | No `mask-*`; the `mask-*` and `--tw-mask-*` properties had no place in the property order |
| `SAFE_AND_LAST_BASELINE_ALIGNMENT` | feature | S | rare | No `items-center-safe`, `justify-end-safe` and the 16 other `…-safe` utilities; no `items-baseline-last` and `self-baseline-last` |
| `WRAP_UTILITIES` | feature | S | rare | No `wrap-anywhere`, `wrap-break-word`, `wrap-normal` |
| `BG_SIZE_AND_POSITION` | feature | S | rare | No `bg-size-*` and `bg-position-*` |
| `CORNER_POSITION_NAMES` | feature | S | rare | No `bg-top-left`, `bg-top-right`, `bg-bottom-left`, `bg-bottom-right`, and no `object-*` with these names: only `bg-left-top`, `object-left-top`, … |
| `STATIC_VARIANTS_OF_4_1` | feature | S | rare | No `details-content`, `user-valid`, `user-invalid`, `inverted-colors`, `noscript`, `pointer-none`, `pointer-coarse`, `pointer-fine`, `any-pointer-none`, `any-pointer-coarse`, `any-pointer-fine` |
| `SOURCE_NOT_AND_INLINE` | feature | S | rare | `@source not "…"`, `@source inline(…)` and `@source not inline(…)` were the error "`@source` paths must be quoted." |
| `ARBITRARY_VALUE_CHECK` | fix | S | edge | An arbitrary value had no check: `[color:red;background:blue]`, `w-[1px)]` and `[--x:{a:b}]` were valid |
| `THEME_VARIABLE_AFTER_KEY_PATH` | fix | S | each build with a plugin | After the first `theme(colors.red.500)` in a stylesheet, and in each stylesheet with `@plugin` or `@config`, `--theme(--color-red-500)` gave the value and not `var(--color-red-500)`, and did not mark the variable as used. So `preflight.css` had the font lists and not `var(--default-font-family, …)` |
| theme-variable-with-plugins (JS) | fix | S | rare | With a plugin or a config, each `--theme(--color-red-500)` gave the value and not the variable |
| config-content-not (JS) | feature | S | rare | A `content` entry of a config that starts with `!` was a source with the `!` in its pattern, not a negated source |
| colors (JS) | data | S | rare | `tailwindcss/colors` had the lightness of each `oklch(…)` as a number (`0.984`), not as a percentage (`98.4%`) |

No switch: before 4.1.0, a `}` with no open bracket before it in an
arbitrary value (`w-[}]`, `[color:red}html{color:blue]`, `[&}]:flex`)
went into the output and closed the rule too early, which was not valid
CSS. The Rust compiler refuses such a value in each version, as 4.1.0
does (`utils/is_valid_arbitrary.rs`), and the fuzzer knows this fault
(`closesARuleEarly` in `tests/fuzz/fuzz.mjs`). So for a version before
4.1.0, `@apply w-[}]` is the error of `@apply` for a class that is not
known. Each other part of the check is in `ARBITRARY_VALUE_CHECK`.

No switch: `bg-left-top`, `object-left-top` and the six other corner
positions moved to the old utilities of the plugin layer, with the same
result. The feature flags (`feature-flags.ts`) were off in the released
package of 4.0.17, so their utilities and variants are new in 4.1.0; the
variants `scripting`, `scripting-none` and `scripting-initial` were in
no release. The `polyfills` option of `compile` is new: before 4.1.0,
the compiler had no fallbacks to switch off. A source of `@source` has
the new field `negated`, which only the command line uses (`cli/mod.rs`,
with no switches). The scanner and the optimizer follow no version:
`node_modules` and `.gitignore`, symbolic links, Razor files, and the
new targets of Lightning CSS.

`THEME_VARIABLE_AFTER_KEY_PATH` is in the Rust part of the plugin layer
(`compat.rs`): Tailwind CSS makes the full configuration at the first
`theme(…)` with a key path, also with no plugin. Before 4.1.0, the
theme function of that configuration then read each path.

Data, from the package: the lightness of each color of `theme.css` as a
percentage, and the new `--text-shadow-*` values.

The scanner of the source files follows no version, in each engine. Its
changes in these releases: the Markdown and Ruby pre-processors, and the
`.jsonl` and `.ndjson` files.

### 4.0.17 (no change of the compiler)

### 4.0.16 (3 switches, all there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `MARKER_DETAILS_MARKER` | form | S | common | `marker:*` had only the two `::marker` selectors, with no `::-webkit-details-marker` |
| `KEYFRAME_NAMES_AFTER_COMMA` | fix | S | rare | Only white space ended a keyframes name: with `--animate-both: fade 1s,slide 2s`, the `@keyframes slide` rule of the theme was removed |
| `CSS_UTILITY_LITERALS` | feature | S | rare | `--value('auto')` and `--modifier('auto')` of `@utility` matched nothing |

No switch: the suggestions of `--spacing(--value(integer))` and of the
literal values are for the editor tools.

### 4.0.15 (6 switches, all there; data changes with no switch)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `THEME_FN_GIVES_VARIABLE` | form | S | common | `--theme(--color-red-500)` gave the value (`red`), as `theme(…)` does, with no `inline`, no fallback in `var(…)`, and the error text of `theme(…)` ("Consider checking if the path is correct") |
| `THEME_INITIAL_NOT_EMITTED` | fix | S | rare | A theme variable with the value `initial` (from `--theme(--x, initial)`) was in the output |
| `BG_CONIC_NEGATIVE_ANGLE` | fix | S | rare | `-bg-conic-45` gave `from calc(45 * -1)`, an angle with no unit |
| `LINE_WIDTH_LIST` | fix | S | rare | `border-[12px_4px]` gave `border-color: 12px 4px`: only `thin`, `medium` and `thick` had the type line width |
| `CSS_UTILITY_ONE_REMOVAL` | fix | M | edge | In a functional `@utility`, a `--modifier(…)` with no modifier in the candidate removed its declaration and the walk of the value went on. Each next function with no result removed the node after the declaration |
| `VARIABLE_CANDIDATE_REBUILDS` | fix | S | rare | A later `build` with only a new theme variable as a candidate (`--color-red-500`) gave the output of the build before |

No switch: `replaceWith` of the walks takes only its first call, which is
the fix of `CSS_UTILITY_ONE_REMOVAL` (no other code calls it two times).
The flags for `items-center-safe` and `items-baseline-last` were off in a
release (`SAFE_AND_LAST_BASELINE_ALIGNMENT`, 4.1.0). `PluginUtils` is a
type. The plugin layer gives `forceInline` to its `theme` function as a
default value, which changes no result. Data, from the package:
`--default-font-family: --theme(--font-sans, initial)` and the five
other `--default-*` font variables of `theme.css`, and `preflight.css`
(`--theme(…)` for the font variables, no `line-height: inherit` for
`body`, and the color of `::placeholder` in an `@supports` rule).

`VARIABLE_CANDIDATE_REBUILDS` needs two builds with one compiler, so its
case is a test of `native/tailwind/tests/switches.rs`, not a record.

### 4.0.14 (no switch)

No switch: `@source inline(…)` and `@source not inline(…)` were behind a
flag that was off in a release (`SOURCE_NOT_AND_INLINE`, 4.1.0). The
scanner of the source files follows no version.

### 4.0.13 (no change of the compiler)

### 4.0.12 (1 switch, there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `SORT_IGNORES_EMPTY_DECLARATIONS` | fix | S | common | The property of a declaration with no value was in the sort: `text-sm` with no `--text-sm--line-height` sorted also by `line-height`, `letter-spacing` and `font-weight`, so it came before `text-[13px]` |

### 4.0.11 (no change of the compiler)

### 4.0.10 (7 switches, all there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `APPLY_IN_ITS_OWN_PARENT` | fix | M | rare | The walk of a rule with `@apply` also replaced the `@apply` rules of its nested rules, before their utilities were ready: `.a { @apply underline; div { @apply b; } }` with `@utility b { @apply flex; }` gave no rule for the candidate `b`. An `@apply` at the top level gave its declarations at the top level |
| `NOT_KEEPS_IS` | fix | S | common | `not-*` removed `&:is(` and `)` from the ends of a selector: `not-group-hover:flex` gave `&:not(:where(.group):hover *)`, and `not-[&:is(.a):is(.b)]:flex` gave `&:not(.a):is(.b)` |
| `ANIMATE_KEYFRAMES_WITH_PREFIX` | fix | S | rare | With a prefix, the `@keyframes` rule of a used `--tw-animate-*` variable was removed |
| `IMPORTANT_CUSTOM_PROPERTIES` | form | S | common | `!important` did not go on a custom property: `translate-x-2!` gave `--tw-translate-x: …` with no `!important` |
| `GRID_LINE_NUMBERS` | feature | S | common | `col-2`, `row-3`, `-col-2` and `-col-[2]` gave nothing: `col-*` and `row-*` took only a theme value or an arbitrary value |
| `NO_MAX_SIZE_AUTO` | fix | S | rare | `max-w-auto` gave `max-width: auto`, and `max-h-auto` gave `max-height: auto` |
| `OUTLINE_HIDDEN_STYLE_VARIABLE` | fix | S | common | `outline-hidden` did not set `--tw-outline-style: none` |

No switch: the flags for `details-content`, `pointer-*` and `scripting`
were off in a release (`STATIC_VARIANTS_OF_4_1`, 4.1.0). `prefixKey` of
the theme is now public, with the same result.

`APPLY_IN_ITS_OWN_PARENT` is the start of the walk of 4.0.10 to 4.1.5
(`APPLY_NESTED_WALK`). Before 4.0.10, the walk was the walk of 4.1.6, but
the nodes that replaced an `@apply` rule were the nodes of the cache of
the compiler. A utility that a nested `@apply` compiled too early kept
its own `@apply` rules in the cache. The walk then replaced these rules
in the tree. In the cache, it replaced only the ones in a nested rule of
the utility: an `@apply` rule that is a child of the utility stayed, and
it gives nothing for the candidate of the build. The Rust compiler has
no shared nodes: it writes each such change back to the cache
(`DesignSystem::replace_compiled_node`).

### 4.0.9 (6 switches, all there; 1 change in the plugin layer)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `REFERENCE_VAR_FALLBACK` | form | S | common | A value of `@theme reference` gave `var(--color-potato)`, with no fallback |
| `REFERENCE_IMPORT_THEME` | form | M | common | An `@theme` in a reference import (`@reference "…"`) was a normal `@theme`. Each reference import at the top level gave a copy of the rule of the first `@theme`, with the used variables (in the at-rules above that rule, when the rule was in the import) |
| `REFERENCE_THEME_KEYFRAMES` | fix | S | rare | The `@keyframes` rules of `@theme reference` were not in the output |
| `THEME_KEYFRAMES_AT_THE_END` | form | S | common | The `@keyframes` rules of the theme were in the rule of the first `@theme`: in the output they were before the `@property` rules |
| `BOM_IS_SKIPPED` | fix | S | edge | A byte order mark was a part of the first rule or declaration: before `@tailwind utilities;` or `@import "…";` it gave the error "Invalid declaration" |
| `IMPORTANT_DASH_PROPERTIES` | fix | S | common | `!important` did not go on a property with `-` as its first or second character: `z-10!` gave `z-index: 10`, and `-webkit-line-clamp` of `line-clamp-2!` had no `!important` |
| selector-parser-nested-function (JS) | fix | S | edge | In the selector parser of `addUtilities`, a function in a function (`:is(:not(.a))`) went to the top level of the selector |

No switch: the flags for `wrap-anywhere` and `inverted-colors` were off in
a release (4.1.0).

Before 4.0.9, a byte order mark before a rule with a body changed
nothing: the parser removes the white space around a selector and around
the name of an at-rule, and the mark is white space for JavaScript.

Before 4.0.9, the rule of the first `@theme` in a reference import had
its `@keyframes` rules, so each copy of the rule gave them again. From
4.0.9, that rule is not in the output, and `THEME_KEYFRAMES_AT_THE_END`
keeps the `@keyframes` rules out of it.

### 4.0.8 (8 switches, all there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `UNUSED_THEME_VARIABLES_REMOVED` | form | M | each build | In 4.0.6 and 4.0.7, each theme variable and each `@keyframes` rule of the theme was in the output |
| `REFERENCE_IMPORT_THEME_IS_NORMAL` | form | S | common | An `@theme` in a reference import was an `@theme reference`, and a reference import gave nothing (as from 4.0.9: `REFERENCE_IMPORT_THEME` is the end) |
| `THEME_RULE_AT_FIRST_THEME` | form | S | rare | The rule of the theme was at the place of the first `@theme` that is not a reference |
| `IMPORT_THEME_OPTION_WITH_RULES` | feature | S | rare | An import with `theme(…)` and a style rule or a declaration was the error "Files imported with `@import "…" theme(…)` must only contain `@theme` blocks." |
| `IMPORTANT_SKIPS_DASH_PROPERTIES` | form | S | common | `!important` went on each declaration (as from 4.0.10: `IMPORTANT_DASH_PROPERTIES` and `IMPORTANT_CUSTOM_PROPERTIES` are the end) |
| `SORT_BY_EACH_PROPERTY` | fix | S | common | The sort compared only the first property of two rules, and then the count of the different properties with a place in the order (not the count of the declarations) |
| `TRANSLATE_Z_PROPERTY_ORDER` | data | S | rare | `--tw-translate-z` had no place in the order of the properties |
| `TRANSLATE_Z_PX_ONE_RULE` | fix | S | rare | `translate-z-px` and `-translate-z-px` gave two equal rules each |

No switch: `extractUsedVariables` is the same code in a new file.

The removal of 4.0.8 is not the removal of 4.0.5: see 4.0.5.

### 4.0.7 (3 switches, all there; 1 change in the plugin layer)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `COMPARE_WHOLE_NUMBERS` | fix | S | common | The sort by name skipped equal characters first, so a number started at its first digit that is different: `p-100` (`00`) came before `p-12` (`2`) |
| `DROP_SHADOW_PROPERTY` | fix | S | common | The filter utilities had no `@property` rule for `--tw-drop-shadow` |
| `DEFAULT_OUTLINE_WIDTH` | feature | S | rare | `outline` gave `outline-width: 1px`, also with `--default-outline-width` in the theme |
| nested-utility-name (JS) | fix | S | rare | A nested rule of `addUtilities` with the name of the utility in its selector did not get the name of the candidate |

No switch: the flag for `user-valid` and `user-invalid` was off in a
release (`STATIC_VARIANTS_OF_4_1`, 4.1.0). The suggestions (fractions,
`_` in a theme key) are for the editor tools. `escape` has the same
result. The types of `tailwindcss/plugin` are not in the output.

Before 4.0.7, the order of the names was not consistent (`p-[1]` before
`p-[02]`, `p-[02]` before `p-[0a]`, `p-[0a]` before `p-[1]`). So the Rust
compiler sorts with the algorithm of V8 (`utils/v8_sort.rs`) for these
versions, and the result follows the sequence of the candidates, as in
the official compiler.

### 4.0.6 (1 switch, there; data changes with no switch)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `UNUSED_THEME_VARIABLES_KEPT` | form | S | each build | In 4.0.5, the theme variables with no use were removed (`UNUSED_THEME_VARIABLES_REMOVED_FIRST`) |

Data, from the package: the line breaks of `--font-sans` and
`--font-mono` in `theme.css`, and comments of `preflight.css`.

### 4.0.5 (3 switches, all there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `UNUSED_THEME_VARIABLES_REMOVED_FIRST` | form | M | each build | Each theme variable was in the output |
| `THEME_KEYFRAMES_ALL_COLLECTED` | form | S | common | A `@keyframes` rule of the theme was in the output only when its name was a word in the value of an `--animate-*` variable of the theme, also a variable with no use |
| `VALUE_PARSER_ESCAPES` | fix | S | edge | `\` was a normal character in the parser of values: `w-[a\(b]` gave `width: a\()b`, and `w-[var(--a\,_b)]` gave `var(--a\, b)` |

No switch: `@theme static` came in 4.0.5. Before, the word had no effect,
and no variable was removed. A candidate that is a variable
(`--color-red-500`) marks the variable as used: before 4.0.5 it was a
candidate that is not valid, with the same output. `getOptions` of the
theme reads a key with a prefix: only the removal and the plugin layer
call it.

The removal of 4.0.5 has three differences from the removal of 4.0.8.
A variable in the value of a theme variable is a use (`--a: var(--b)`
keeps `--b`, also when `--a` has no use). A custom property in an
`@keyframes` rule of the theme is removed when it is not a theme variable
in use. And only a rule of the theme at the top level is removed when it
is empty: in `@layer theme`, the rule `:root, :host` stays with no
declaration. In 4.0.5, a `@keyframes` rule of the theme stays when a
declaration of `animation` in the output has its name, or an
`--animate-*` variable in use has it.

Before 4.0.5, `\)` with no open function stopped the parser of values
with a JavaScript exception. The Rust compiler keeps the two characters
there, as 4.0.5 does.

### 4.0.4 (7 switches, all there; 2 changes in the plugin layer)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `ORDER_FIRST_LAST_NUMBERS` | form | S | common | `order-first` gave `calc(-infinity)`, and `order-last` gave `calc(infinity)` |
| `ANIMATE_NAMES_AT_WHITE_SPACE` | fix | S | rare | Only a space was between the words of an `--animate-*` value: a name next to a line break did not keep its `@keyframes` rule |
| `VAR_NAME_UNESCAPES_UNDERSCORE` | fix | S | rare | `w-[var(--a\_b)]` gave `var(--a\_b)`: the first argument of `var()` and `theme()` stayed as it was |
| `WALK_ENTERS_AT_ROOT` | fix | S | rare | `theme()` in an `@keyframes` rule of the theme stayed in the output, and an `@apply` rule there was removed with no error |
| `AT_RULES_IN_MEDIA` | fix | S | rare | The compiler did not look in an `@media` rule that stays: `@tailwind utilities`, `@source`, and an `@utility` or `@custom-variant` with no body were ignored there, and `@media important` in an `@media` rule stayed an `@media` rule |
| `CSS_UTILITY_RESULT_NOT_VISITED` | fix | S | edge | In a functional `@utility`, the walk visited the result of `--value(…)` again: `foo-[--modifier([*])]/[3]` gave the value `3` |
| `AT_VARIANT_IN_IMPORT_ONCE` | fix | S | rare | In 4.0.2 and 4.0.3, an `@variant` rule in an import with options got its variant two times |
| config-containers-key (JS) | fix | S | rare | The key `containers` of a JavaScript configuration did not go to the `--container-*` namespace |
| null-theme-value (JS) | fix | S | edge | A `null` value in the theme of a JavaScript configuration stopped the compiler |

No switch: the functions of a candidate (`theme()` in an arbitrary value)
are replaced in the cache of the design system, with the same result.
The walk of the selector parser of the plugin layer has the same change
as the other walks.

The walk of the tree changed in 4.0.4, and four of the switches come
from that change. Before 4.0.4, the walk did not go into an at-root
node (`WALK_ENTERS_AT_ROOT`), it visited the nodes that replace a node
also when the visitor asked to skip (`CSS_UTILITY_RESULT_NOT_VISITED`,
`AT_VARIANT_IN_IMPORT_ONCE`), and the visitor of `@media` asked to skip
its nodes (`AT_RULES_IN_MEDIA`).

Before 4.0.4, an at-rule of Tailwind CSS with a body (`@theme`,
`@utility`, `@custom-variant`) in an `@media` rule stayed in the output,
which is not valid CSS. There, the Rust compiler gives the result of
4.0.4. Before 4.0.2, an at-rule with no body stayed in the output too
(`@tailwind utilities;`, `@source "…";`): the same rule. The fuzzer knows
this fault (`hasKnownFault`: a line of the official result that starts
with one of these at-rules).

Before 4.0.4, the walk also went into the body of an `@custom-variant`
rule after its removal. So an `@variant` rule in that body got its
variant (from 4.0.4 to 4.1.13 it stayed in the output, see 4.1.14), and
each other at-rule of Tailwind CSS in that body was read (an `@media`
with import options, a nested `@custom-variant` as an error). The Rust
compiler replaces the `@variant` rule in each version, and does not read
the other at-rules there.

Before 4.0.4, where the result of `--value(…)` had a `--value(…)` that
gives the same result again (`foo-[--value([*])]`), the official compiler
did not stop. The Rust compiler gives the result of 4.0.4 there.

In 4.0.2 and 4.0.3, an `@variant` rule that is the old form of
`@custom-variant` (no body, or a body with `@slot`) had a fault in an
import with options. With no body, the official compiler gave the error
"Cannot use `@variant` with unknown variant". With `@slot`, it stopped
with a stack overflow. The Rust compiler gives the result of 4.0.4 for
the two.

### 4.0.3 (1 switch, there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `EMPTY_IMPORT_STAYS` | fix | S | rare | In 4.0.2, `@import url(…);` was removed, as each other at-rule with no body |

### 4.0.2 (7 switches, all there; 2 changes in the plugin layer)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `EMPTY_RULES_REMOVED` | form | S | common | A rule with no declaration and an at-rule with no body stayed in the output: `.a {` and `}`, `@supports (display: grid);`, `@font-face ;` |
| `DECLARATION_NEEDS_COLON` | fix | S | edge | A text with no `:` was a declaration: `foo;` gave `fo: foo`, and `foo` before `}` gave `foo: foo` |
| `THEME_KEYS_UNESCAPED` | fix | M | rare | The keys of the theme were the text of the stylesheet with its escapes. A value with a `.` was read only with `_`: `p-1.5` read `--spacing-1_5`, not `--spacing-1\.5` |
| `GRID_TRACKS_NOT_ZERO` | fix | S | rare | `grid-cols-0` gave `repeat(0, minmax(0, 1fr))` |
| `GRADIENT_POSITION_NO_COMMA` | form | S | common | `--tw-gradient-position` had a comma at its end (`to right in oklab,`), and the stops had `var(--tw-gradient-position,) …`. The same for the old utilities `bg-gradient-to-*` |
| `REFERENCE_THEME_NO_KEYFRAMES` | fix | S | rare | The `@keyframes` rules of `@theme reference` were in the output, as each other `@keyframes` rule of the theme |
| `AT_VARIANT_IN_IMPORT_TWICE` | (fault of 4.0.2 and 4.0.3) | S | rare | An `@variant` rule in an import with options got its variant one time |
| custom-property-case (JS) | fix | S | rare | The name of a custom property of a JavaScript plugin got `-` before each capital letter |
| config-theme-keys (JS) | fix | S | edge | The keys of the theme of a JavaScript configuration had escapes |

No switch: before 4.0.2, an `@variant` rule at the top level with no
`@slot`, and an `@variant` rule in an `@media` rule, stayed in the
output, which is not valid CSS (see 4.1.14 for the rule of the fuzzer).

Before 4.0.2, a declaration with no `:` and with a property that is
empty (`;;` gave `: ;`, and `a;` gave `: a;`) is not valid CSS. There,
the Rust compiler gives the error of 4.0.2 ("Unexpected semicolon",
"Invalid declaration"). The fuzzer knows this fault (`hasKnownFault`: a
line of the official result that starts with `: `).

### 4.0.1 (6 switches, all there)

| Switch | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| `PAREN_MODIFIER_NEEDS_DASH` | fix | S | rare | The short form of a variable in a modifier and in the value of a variant had no check: `bg-red-500/(foo)` gave `var(foo)`, and `data-(foo):flex` gave `&[data-var(foo)]` |
| `THEME_ROOT_AND_HOST` | form | S | each build | The rule of the theme was `:root`, not `:root, :host` |
| `NO_MIN_SIZE_NONE` | fix | S | rare | `min-w-none` gave `min-width: none`, and `min-h-none` gave `min-height: none` |
| `TEXT_MODIFIER_IS_LINE_HEIGHT` | fix | S | rare | A modifier of a font size with no line height was ignored: `text-sm/foo` and `text-sm/none` gave only the font size |
| `NO_FORCE_VARIANT` | fix | S | edge | `force:flex` gave the rule of `flex` with no change |
| `OPEN_VARIANT_OPEN_PSEUDO` | form | S | common | `open:*` gave `&:is([open], :popover-open)`, with no `:open` |

No switch: before 4.0.1, an `@custom-variant` with an empty selector
(`@custom-variant foo ();`, `(&:hover, , &:focus)`) gave a rule with an
empty selector, which is not valid CSS. The Rust compiler gives the error
of 4.0.1 ("selector is invalid"). The fuzzer knows this fault
(`hasKnownFault`: a selector of the official result that is empty or has
an empty part). The suggestions are for the editor tools.

## What the list shows

- **56 changes** in the eleven releases from 4.2.0 to 4.3.5. 5 of them
  are only in the JavaScript of the plugin layer and need no switch. So
  there are **51 switches** for these releases.
- **84 more switches** for the twenty releases from 4.1.0 to 4.2.0: 12
  for 4.2.0, 57 for 4.1.1 to 4.1.18, and 15 for 4.1.0. All the 135
  switches are there: the Rust compiler serves each version from 4.0.17
  to 4.3.5.
- **Most of the changes are fixes** of faults in rare inputs. 9 change
  the form of valid output, 6 are new features, 4 are constants, and 1
  has no visible change.
- **7 change the output of a normal project** (the rows with "each
  build" or "common"). Each other one needs an input that few projects
  have.
- **Most are one condition or one constant.** Of the 51 switches of 4.2
  and 4.3, 44 are one condition, one constant or one small helper, and 7
  keep a
  function in two forms (the declaration with `!important`, the custom
  property at the end of the input, the brackets of an arbitrary value,
  the words of `supports-[…]`, the color of a shadow, the negation of a
  `@container` condition, and the end of a style rule before the pass
  that flattens rules). The two largest changes of Tailwind CSS in these
  releases (the pass that flattens rules, and `@scope`) are new passes:
  the old behavior is "do not run the pass", and only a small step of
  the old code must come back (about 50 lines for the pass that flattens
  rules).
- **The work for one release was small.** The 15 switches of 4.3.1,
  4.3.2 and 4.3.3 are about 320 lines of Rust, with their comments, and
  the 12 switches of 4.2.1 to 4.3.0 are about 200 lines. The fuzzer
  found no difference after them: the list from the source of Tailwind
  CSS was almost complete. One switch came later
  (`DEFAULT_THEME_SYSTEM_FONTS`): its value is in the Rust part of the
  plugin layer, and the fuzzer has no `theme(fontFamily.sans)`.
- **An older line has more failures but few causes.** With no switch
  for the 4.1 line, the Rust compiler was different from the official
  4.1.18 in 181 of 600 fuzz cases, from 4.1.15 in 536, from 4.1.0 in
  580, and from 4.0.17 in 584 (cases with no plugin). A small number of
  changes made most of these: the spaces in a fraction (4.2.0), the
  `color-mix()` fallback (it came in 4.1.0 and changed in 4.1.2, 4.1.4,
  4.1.13, 4.1.15 and 4.1.16), the `@layer properties` rule (4.1.2), and
  the utilities that were new in 4.1.0 and 4.2.0. A group of new
  utilities is one switch (`LOGICAL_UTILITIES`, `MASK_UTILITIES`).
- **Three kinds of old behavior get no switch**: a fault that stops the
  compiler, output that is not valid CSS ("The rules"), and a change
  that is only in the JavaScript of the plugin layer.
- **A behavior that is a fault of an old version is kept**, when its
  result is CSS: a custom property at the end of the input loses its
  last character as 4.3.3, as in the official 4.3.3. And from 4.1.13 to
  4.3.2, a rule with two equal declarations gets no `color-mix(…)`
  fallback, as in the official compiler of those versions.
