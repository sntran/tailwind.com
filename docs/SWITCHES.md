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
| 4.3.3 | Rust compiler | 18 more |
| 4.3.2 | Rust compiler | 5 more |
| 4.3.1 | Rust compiler | 3 more |
| 4.3.0 | Rust compiler | 8 more |
| 4.2.4 | Rust compiler | 8 more |
| 4.2.3 | Rust compiler | 0 more |
| 4.2.2 | Rust compiler | 2 more |
| 4.2.1 | Rust compiler | 1 more |
| 4.2.0 | Rust compiler | 1 more |
| 4.0.0 to 4.1.18 | script engine | not counted |

All the 49 switches of Tailwind CSS 4.2 and 4.3 are there. Each of these
versions uses the Rust compiler also for a stylesheet with `@plugin` or
`@config`: see "The plugin layer" below.

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
- **A behavior of one version only** needs two switches, one for its
  start and one for its end. (In the list, `addbase-variant-eager` is
  only in 4.3.1, but it is in the plugin layer. `UTILITY_ROOT_ENDING_DASH`
  is only in 4.2.0, the oldest version, so it needs one switch.)
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

## The changes of Tailwind CSS 4.2 and 4.3

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
| `EMPTY_EXTRA_MODIFIER_IS_INVALID` | fix | S | edge | `bg-red-500/50/` was valid |
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

The scanner of the source files follows no version, in each engine. Its
changes in these releases: the Markdown and Ruby pre-processors, and the
`.jsonl` and `.ndjson` files.

## What the list shows

- **56 changes** in the eleven releases from 4.2.0 to 4.3.5. 7 of them
  are only in the JavaScript of the plugin layer and need no switch. So
  there are **49 switches**, and all are there: the Rust compiler serves
  each version of 4.2 and 4.3.
- **Most of the changes are fixes** of faults in rare inputs. 9 change
  the form of valid output, 6 are new features, 4 are constants, and 1
  has no visible change.
- **7 change the output of a normal project** (the rows with "each
  build" or "common"). Each other one needs an input that few projects
  have.
- **Most are one condition or one constant.** Of the 49 switches, 42
  are one condition, one constant or one small helper, and 7 keep a
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
- **An older line is more work.** With no switch for it, the Rust
  compiler is different from the official 4.1.18 in 111 of 600 fuzz
  cases, and from 4.0.17 in 375 of 600 (4.2.4 before its switches: 35 of
  1,500). 4.1.0 to 4.2.0 changes about 4,000 lines of the compiler of
  Tailwind CSS.
- **A behavior that is a fault of an old version is kept**, when its
  result is CSS: a custom property at the end of the input loses its
  last character as 4.3.3, as in the official 4.3.3. And before 4.3.3, a
  rule with two equal declarations gets no `color-mix(…)` fallback, as in
  the official compiler of those versions.
