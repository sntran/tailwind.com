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
| 4.3.4 | Rust compiler | 3, all there |
| 4.3.3 | script engine | 19 to write |
| 4.3.2 | script engine | 6 more |
| 4.3.1 | script engine | 3 more |
| 4.3.0 | script engine | 10 more |
| 4.0.0 to 4.2.4 | script engine | not counted |

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
- **A behavior of one version only** (in the list: `addbase-variant-eager`
  is only in 4.3.1) needs two switches, one for its start and one for its
  end.
- **The plugin layer is JavaScript** (`assets/compat.js`, made from the
  source of Tailwind CSS of the port). A switch there needs a question to
  the host from the JavaScript side. No switch of this kind is written
  yet.

## The changes of Tailwind CSS 4.3

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

### 4.3.4 (19 switches, to write)

| Change | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| at-property-supports-query | data | S | each build | The `@supports` rule of the fallbacks had `margin-trim: inline`, not `block` |
| empty-var-fallback-space | data | S | common | `var(--tw-blur,)`, not `var(--tw-blur, )` (8 utilities) |
| group-peer-selector-form | form | S | common | `.x:is(:where(.group):hover *)`, not `:is(:where(.group):hover .x)` |
| scope-hoisting | fix | S | rare | `@scope` stayed nested in the rule, with its contents as they were |
| dead-pseudo-element-rules | fix | S | rare | A rule after a pseudo-element (`::selection > *`) was in the output |
| ignored-modifier-no-utility | fix | S | rare | `rounded-sm/50` gave the class with the modifier ignored |
| empty-extra-modifier | fix | S | edge | `bg-red-500/50/` was valid |
| unbalanced-arbitrary-value | fix | M | rare | `w-[calc(1px]` was valid: only `;` and a closing bracket at depth 0 were errors |
| supports-variant-keywords | fix | M | rare | `and`, `or`, `not` got spaces around them in all of a `supports-[…]` value |
| breakpoint-decimal-sort | fix | S | edge | `40.25rem` and `40.5rem` were equal in the sort (`parseInt`) |
| comment-backslash-end | fix | S | edge | `\` before `*/` kept a comment open |
| important-flag-at-end | fix | M | edge | The first `!important` in a value cut the value |
| custom-property-at-eof | fix | M | edge | A custom property at the end of the input lost its last character |
| declaration-at-eof | fix | S | edge | A declaration with no `;` at the end of the input was dropped |
| unescape-single-hex-digit | fix | S | edge | `\0` … `\f` gave the digit, not the code point |
| length-units-table | data | S | rare | `dvmin`, `svb`, `cap`, … were not lengths: `text-[10dvmin]` was a color |
| moz-any-nesting | fix | S | edge | `&` in `:-moz-any(…)` was not replaced |
| default-fn-whitespace | fix | S | rare | White space in `--default(…)` of `@utility` was removed |
| theme-root-namespace-lookup (JS) | fix | M | rare | Plugin `theme('shadow')` gave the DEFAULT text, not the object |

### 4.3.3 (6 switches, to write; 6 data changes with no switch)

| Change | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| nesting-pass | form | M | each build | No pass that flattens rules: `.hover\:x { &:hover { @media (hover: hover) { … } } }`. The old code also removed equal declarations in each rule, and replaced a `&` rule by its children |
| spacing-zero-px | form | S | common | `--spacing(0)` gave `0`, not `0px` |
| shadow-fractional-alpha | fix | S | rare | `shadow-sm/12.5` gave the class with no alpha |
| aria-data-attr-validate | fix | S | edge | `data-[foo bar]:flex` gave a rule with a selector that is not valid |
| selector-nth-child-of (JS) | form | M | edge | Classes after `of` in `:nth-child(…)` got no prefix in `addUtilities` |
| plugin-theme-default (JS) | fix | S | rare | Plugin `theme('colors.foo')` gave an object when `--color-foo-bar` was there too |

Data, from the package: the new `--font-sans` stack (`theme.css`,
`preflight.css`), `:-moz-focusring:where(:not(iframe))` in
`preflight.css`, and `oklch(L 0 none)` for 13 gray colors. The same
values are also in `tailwindcss/colors` and `tailwindcss/defaultTheme` of
the plugin layer: there they must come from the package too.

### 4.3.2 (3 switches, to write)

| Change | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| auto-rows-cols-bare-spacing | feature | S | rare | `auto-rows-12` gave nothing |
| infer-length-spacing-fn (also JS) | fix | S | edge | `text-[--spacing(2)]` was a color |
| infer-color-alpha-fn (also JS) | fix | S | edge | `--alpha(…)` was not a color for the type of a value |

### 4.3.1 (10 switches, to write)

| Change | Kind | Effort | How often | What was before |
|---|---|---|---|---|
| spacing-fn-fold-0-1 | form | S | each build | `m-1` gave `calc(var(--spacing) * 1)`, and `m-0` gave `calc(var(--spacing) * 0)` |
| space-zero | form | S | common | `space-x-0` gave two `calc(…)` values, not `0` |
| mask-angle-0-1 | form | S | rare | `mask-linear-1` gave `calc(1deg * 1)` |
| inset-shadow-none-inset | fix | S | rare | `inset-shadow-none` had no `inset` |
| apply-css-mixins | feature | S | edge | `@apply --my-mixin` was an error |
| apply-body-error | fix | S | edge | `@apply underline { … }` dropped the body with no error |
| shadow-color-detection | fix | M | rare | The color of a shadow was the first part that is not a length |
| not-container-negation | fix | M | rare | `not-*` on `@container style(…)` gave a condition that is not valid |
| addbase-variant-eager (JS) | fix | M | rare | Only in 4.3.1: `@variant` in `addBase` was replaced when the plugin loaded |
| plugin-selector-whitespace (JS) | form | L | rare | Selectors of plugins kept their text: `&>*`, not `& > *` |

## What the list shows

- **41 switches** for the six releases of 4.3: 3 are there, 38 are to
  write.
- **27 are fixes** of faults in rare inputs, 8 change the form of valid
  output, 2 are new features, 3 are constants, and 1 has no visible
  change.
- **7 change the output of a normal project** (the rows with "each
  build" or "common"). Each other one needs an input that few projects
  have.
- **30 are one condition or one constant, 10 are one function in two
  forms, and 1 is large.** The large one is in the JavaScript of the
  plugin layer. The two largest changes of Tailwind CSS in these
  releases (the pass that flattens rules, and `@scope`) are new passes:
  the old behavior is "do not run the pass", and no old algorithm must
  come back.
- **7 switches are in the plugin layer**, which is JavaScript, and 2
  data changes are there too.
