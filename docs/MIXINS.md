# CSS functions, mixins, conditions, tree counting and math

[CSS Custom Functions and Mixins Module Level 1](https://drafts.csswg.org/css-mixins-1/)
adds `@function` and `@mixin` to CSS. A function gives a value. A
mixin gives declarations and rules for a style rule. Two more drafts add
conditions: `if()` ([CSS Values 5](https://drafts.csswg.org/css-values-5/#if-notation))
for a value, and `@when` and `@else`
([CSS Conditional Rules 5](https://drafts.csswg.org/css-conditional-5/))
for rules. See [Conditions](#conditions). CSS Values 5 also adds
`sibling-index()` and `sibling-count()`: see [Tree counting](#tree-counting).
The math functions of CSS Values 4 that old browsers do not have are in
[Math functions](#math-functions).

```css
@function --gap(--n) {
  --step: 4px;
  @media (width >= 40rem) { --step: 8px; }
  result: calc(var(--n) * var(--step));
}

@mixin --card(--bg: white, --pad) {
  background: var(--bg);
  padding: var(--pad, 1rem);
  gap: --gap(2);
  &:hover { background: black; }
  @contents;
}

.panel {
  @apply --card(red, 2rem) { margin: 0; }
}
```

## What does what

- **Tailwind CSS** keeps these rules in its output with no change, as
  upstream does. `@apply` with only dashed names (`@apply --card`) is a
  mixin, not a list of utilities.
- **The optimizer** (`--minify` and `--optimize`) lowers them before
  Lightning CSS, in `native/tailwind/src/cli/css_mixins.rs`. Lightning CSS
  does not know them yet. When it has its own lowering, this step goes.
- **In the browser**, the page script of the npm package (`js/page.js`)
  lowers them too. Its program (`tailwind-browser.wasm`, the Cargo
  feature `lower`) has this step and no Lightning CSS: there, `--optimize`
  only lowers, and `--minify` gives an error.
- **The browsers decide**, as for each other lowering of Lightning CSS.
  The step lowers a rule when a target browser of the optimizer does not
  have it. The targets are those of Tailwind CSS (Chrome 111, Safari
  16.4, Firefox 128), so the step lowers each rule for each project:

  | Rule | The first browsers | With the targets of Tailwind CSS |
  |---|---|---|
  | `@function` | Chrome and Edge 139 | Lowered |
  | `@mixin`, `@apply --name` | None yet | Lowered |
  | `if()` | Chrome and Edge 137 | Lowered |
  | `@when`, `@else` | None yet | Lowered |
  | `sibling-index()`, `sibling-count()` | Chrome and Edge 138 | Lowered |
  | `round()`, `mod()`, `rem()` | Chrome 125, Safari 15.4, Firefox 118 | Lowered, for Chrome 111 to 124 |
  | `pow()`, `sqrt()`, `hypot()` | Chrome 120, Safari 15.4, Firefox 118 | Lowered, for Chrome 111 to 119 |
  | `abs()`, `sign()` | Chrome 138, Safari 15.4, Firefox 118 | Lowered, for Chrome 111 to 137 |
  | `progress()` | Chrome and Edge 138 | Lowered |

  The table is `FUNCTIONS`, `MIXINS`, `IF`, `WHEN`, `SIBLING`,
  `STEPPED`, `EXPONENTIAL`, `SIGN` and `PROGRESS` in `css_mixins.rs`. When a new release of Tailwind CSS raises its
  targets, the decision follows.

A stylesheet with none of these rules does not change. With no
`--minify` and no `--optimize`, the output is the output of Tailwind CSS,
with the rules in it.

The lowering of the example:

```css
.panel {
  background: red;
  gap: 8px;
  padding: 2rem;
}
@media (min-width: 40rem) {
  .panel { gap: 16px; }
}
.panel:hover { background: #000; }
.panel { margin: 0; }
```

## With Tailwind CSS

The two forms of `@apply` work together. The compiler tells them apart
by the name:

- `@apply flex gap-4 md:grid` (no dashed name): the utilities of Tailwind
  CSS, with their variants. Tailwind CSS compiles it.
- `@apply --button` (a dashed name): a CSS mixin. The optimizer lowers it.

One `@apply` cannot have the two. But the body of a mixin can have the
`@apply` of Tailwind CSS. So a mixin can use the utilities and the
variants:

```css
@mixin --button(--color: var(--color-sky-600), --text: white) {
  @apply inline-flex items-center gap-2 rounded-lg px-4 py-2 font-medium md:px-6;
  @apply bg-(--color) text-(--text) hover:bg-(--color)/90;
  @apply focus-visible:outline-2 focus-visible:outline-(--color);
  @contents;
}

.btn {
  @apply --button;
}

.btn-danger {
  @apply --button(var(--color-red-600)) {
    text-transform: uppercase;
  }
}
```

- The utilities with a custom property, `bg-(--color)`, give
  `var(--color)`. The parameter of the mixin then replaces it. So the
  arguments of the mixin go into the utilities, also with a variant and
  an opacity: `.btn-danger:hover` gets
  `color-mix(in oklab, var(--color-red-600) 90%, transparent)`.
- Each variant works: Tailwind CSS gives nested rules (`&:hover`,
  `@media`, `:is(& > *)`) in the body of the mixin, and the lowering
  keeps them.
- A parameter hides a custom property with the same name in the body,
  also in the utilities. A parameter `--spacing` changes each spacing
  utility of the mixin. Give the parameters names that are not the
  names of theme variables, or use this on purpose.
- With no optimizer, the output has the mixin after Tailwind CSS: the
  utilities are declarations in the body of the mixin.

Tailwind CSS tells you to use `@apply` only a little: a component of
your templates (for example a function component of Phoenix,
`<.button>`) keeps the utilities in the markup, and you can change them
there. A mixin is for the CSS that has no template: the content of a CMS
or of Markdown, the markup of a library, or a rule that must have
arguments. A mixin is better than an `@apply` of utilities in a class for
these reasons:

- It has parameters, default values and a block (`@contents`).
- It is standard CSS. When the browsers have mixins, the same CSS works
  with no lowering.
- A class with `@apply` is a utility class. A mixin is not a class, so a
  rule applies it where it must, also to an element that has no class
  (`.prose a`).

## In a Phoenix project

The lowering runs only with `--minify` or `--optimize`.
`mix assets.deploy` gives `--minify`, so a release has the lowered CSS.
The watcher of the endpoint in development gives neither. Then the
output keeps `@mixin`, and the browser does not apply it. Add
`--optimize` to the arguments of the profile: the optimizer runs, and
the output is not minified.

```elixir
config :tailwind,
  version: "4.3.5",
  my_app: [
    args: ~w(
      --input=assets/css/app.css
      --output=priv/static/assets/css/app.css
      --optimize
    ),
    cd: Path.expand("..", __DIR__)
  ]
```

With `--minify` from `mix assets.deploy` too, the output is minified, as
before.

## The functions

- Each call of a known function gets its `result`, with the arguments and
  the local variables in it. A `var()` of another name stays: it is a
  custom property of the element, as in the spec.
- A default value of a parameter replaces a missing argument. A missing
  argument with no default is the guaranteed-invalid value: `var(--a, x)`
  gives `x`, and with no fallback, the declaration becomes `unset`
  (`initial` for a custom property).
- An argument in braces can have commas: `--f({ 1px, 2px }, 3px)`.
- A `@media`, `@supports` or `@container` rule in the body gives a nested
  rule with the declaration for that condition, after the declaration.
  Lightning CSS then moves it out of the style rule.
- A function can call a function. A cycle, more than 32 calls in one
  stack, or too many arguments give the guaranteed-invalid value.

## The mixins

- Each `@apply` of a known mixin becomes the body of the mixin, at its
  place. `@contents` becomes the block of the `@apply`, or else the block
  of `@contents`. A mixin can apply a mixin.
- An argument or a property of `@private { … }` goes into the
  declarations of the element of the `@apply` as text. These are also
  the declarations of a nested rule for the same element: `&:hover`,
  `&.on`, `&[data-x]`.
- A nested rule for another element (`& > p`, `& + *`, `::before`) gets
  the argument from a custom property on the element: `--mixin-1-bg:
  red`, and `var(--mixin-1-bg)` in the rule. A descendant inherits it, and
  a sibling does not see it and uses the fallback, as the spec says. A
  typed parameter (`--size <length>`) also gets an `@property` rule, so a
  descendant gets the computed value of the element.
- An `@apply` of a mixin that is not in the stylesheet stays. A cycle or
  too many arguments remove the `@apply`. `@layer` in the body of a mixin
  is not valid, and is removed.

## Conditions

### `if()`

`if()` gives the value of the first branch with a true condition:

```css
.card {
  padding: if(media(width >= 40rem): 2rem; else: 1rem);
  display: if(supports(display: grid): grid; else: block);
}
```

The lowering:

```css
.card { padding: 1rem; }
@media (min-width: 40rem) { .card { padding: 2rem; } }
.card { display: block; }
@supports (display: grid) { .card { display: grid; } }
```

- Each branch with a `media()` or `supports()` condition becomes a
  nested rule after the declaration. The first branch comes last, so it
  applies when two conditions are true, as in `if()`.
- `else`, or no true branch, gives the declaration itself. With no true
  branch, the value of `if()` is empty: the declaration becomes `unset`
  (`initial` for a custom property), as in the browser, where a false
  `if()` does not go back to an earlier declaration.
- `not`, `and`, `or` and parentheses work. Media tests only, or supports
  tests only, give one rule. The two kinds together give one rule for
  each alternative.
- `style()` is known only in a function, for a parameter or a local
  variable: `if(style(--mode: dark): black; else: white)` in the body of
  `@function --ink(--mode)` gives `black` for `--ink(dark)`. A `style()`
  test of another property depends on the element: the `if()` stays.

### `@when` and `@else`

A chain of `@when` and `@else` rules applies the first rule with a true
condition:

```css
@when media(width >= 40rem) {
  .nav { display: flex; }
} @else supports(display: grid) {
  .nav { display: grid; }
} @else {
  .nav { display: block; }
}
```

Each rule of the chain becomes a `@media` or `@supports` rule with its
condition and with `not` of each condition before it. Media tests only
give one `@media` rule (`@media (not (width >= 40rem)) and (hover)`).
With the two kinds, a rule can become more than one rule (a condition
`not (a and b)` is `not a`, or `not b`). The rules then can apply
together, with the same declarations. An `@else` with no `@when`
before it is not valid, and goes. A chain with a condition that is not
valid stays.

## Tree counting

`sibling-index()` is the position of an element among its siblings, from
1, and `sibling-count()` is the number of siblings with the element:

```css
.list > li {
  animation-delay: calc(sibling-index() * 50ms);
  width: calc(100% / sibling-count());
}
```

The step uses the method of Roman Komarov ("Possible Future CSS:
Tree-Counting Functions and Random Values", kizu.dev), which the
CSS-Tricks article "How to Wait for the sibling-count() and
sibling-index() Functions" calls "flint and steel". The index is
`lo + 32 * hi`:

- `lo` comes from 32 rules `:nth-child(32n + r)`, one for each `r` from 1
  to 32.
- `hi` comes from 32 rules `:nth-child(n + 32k + 1)`, one for each `k`
  from 0 to 31. The later rule applies.

So 64 rules give the index of each of 1,024 siblings, where one rule for
each position needs 1,024 rules. `sibling-count()` adds 64 rules with
`:nth-last-child()`: the count is the index from the start plus the
index from the end, less one. The rules only set the custom properties
`--sibling-lo`, `--sibling-hi`, `--sibling-last-lo` and
`--sibling-last-hi`, and each function becomes a `calc()` of them.

- The rules for the style rules at the top of the stylesheet go to its
  end, with one list of selectors (`:is(.list > li, .grid > *)`, or
  the selector itself when there is one). A
  nested style rule gets its rules after it, in its parent.
- A pseudo-element (`li::before`) gets the value of its element: the
  rules are for the element, and a custom property is inherited.
- The size, with `--minify`: about 2.7 KB for `sibling-index()` (0.5 KB
  with gzip), and 6 KB when there is also `sibling-count()` (0.9 KB with
  gzip). A nested rule adds the same again.
- Chromium has the two functions. On lists of 1 to 300 elements, with a
  pseudo-element, nested rules, a `@media` rule and a `@function`, the
  lowered CSS gives the same computed values as the native functions.

## Math functions

Lightning CSS computes a math function with fixed values:
`round(10.4px, 1px)` is `10px`. A value with `var()`, or with two units,
is known only in the browser. Chrome 111, a target of Tailwind CSS, does
not have these functions, so `native/tailwind/src/cli/css_math.rs`
lowers them:

| Function | Lowering | Exact |
|---|---|---|
| `pow(A, n)`, with a whole number or a half `n` from -8 to 8 | `A` times itself `n` times, and `sqrt(A)` for the half | Yes |
| `sqrt(x)` | `tan(acos((1 - x) / (1 + x)) / 2)` | Yes |
| `abs(A)` | `max(A, -1 * (A))` | Yes |
| `sign(A)` | `sin(atan2(A, A * 0))`: 1, -1 or 0 | Yes |
| `hypot(A, B, …)` | `max / cos(atan2(min, max))`, with the maximum and the minimum of `\|A\|` and `\|B\|`, for each next argument | In floating point |
| `progress(A, B, C)` | `clamp(0, (A - B) / (C - B), 1)`; with `no-clamp`, no `clamp()` | In floating point |
| `round()`, `mod()`, `rem()` | A helper property with the quotient | In floating point |

For `round(down, A, B)`, `mod(A, B)` and the others, the step adds a
custom property to the declaration, `--math-1`, registered as an
`<integer>`. The browser rounds its value, so it is the whole quotient
`A / B`:

```css
.a { width: round(var(--w), 10px); }

/* lowered */
.a {
  --math-1: calc(tan(atan2(var(--w), 10px)) + 0.000001);
  width: calc(var(--math-1) * (10px));
}
@supports (order: round(1, 1)) {
  .a { width: round(var(--w), 10px); }
}
@property --math-1 { syntax: "<integer>"; inherits: true; initial-value: 0; }
```

- The quotient of two dimensions is `tan(atan2(A, B))`: old browsers
  cannot divide a length by a length. The quotient of a number by a
  plain number is `A / B`.
- A registered `<integer>` rounds a half away from zero, and `round()`
  rounds a half up. So the step uses `floor(q + 0.5)` for `nearest`, and
  the rounding of `q ± 0.5`, with a small number (0.000001) so that a
  whole quotient in floating point (1.9999999) is still whole.
- The `@supports` rule gives the native functions to each browser that
  has them. Only an old browser uses the helper property.
- In a custom property (`--size: round(var(--a), 5px)`), the helper
  property is inherited, so a descendant that uses `var(--size)` gets the
  value.
- In Chromium, on 81 elements with 12 declarations and a custom property,
  the lowered values are the same as the native values, with and without
  the `@supports` rules (972 values). With `B = 0.1`, the native
  function itself has an error of floating point: `round(up, -10, 0.1)`
  is `-9.9` there, and `-10` in the lowering.
- `sqrt()` uses the identity `tan(θ / 2) = sqrt((1 - cos θ) / (1 + cos θ))`,
  and `hypot()` the identity `cos(atan2(min, max)) = max / hypot`. Chrome
  111 has the functions of trigonometry.
- `progress()` with fixed values is a number at build time:
  `progress(25px, 0px, 100px)` is `0.25`. With a start and an end that
  are plain numbers, the quotient is a division. Else it is
  `tan(atan2())`. When the start and the end are the same, the result is
  0 (with `no-clamp`, an infinity), as in the spec.
- `hypot()` and `progress()` also get the `@supports` rule: in Firefox,
  `atan2()` of two units (rem and px) is 0 (bug 1802744 of Mozilla), so
  each browser that has the function uses it. The test of `no-clamp` has
  `no-clamp` in it: Chromium 153 has `progress()`, but not `no-clamp`.
- In Chromium, on 61 elements with `sqrt()`, `hypot()` (also with rem
  and px together), `pow()` with a half, and `progress()`, the lowered
  values are the same as the native values, with and without the
  `@supports` rules (488 values). For `progress(no-clamp, …)`, which
  Chromium does not have, the values are the same as the exact values.
- The division of two lengths (`calc(var(--w) / 1px)`) is not lowered:
  its usual use has two units, and `atan2()` of two units is 0 in
  Firefox 128, a target of Tailwind CSS. `log()`, `exp()` and `pow()`
  with another exponent have no exact lowering, and they stay. In
  `@keyframes`, a helper property and a nested rule are not valid: the
  value stays.

## Limits

- **Conditions together.** Each conditional rule of a function gives one
  variant. When two conditions are true together, the variant of the
  later rule applies, with the values of the other rule as when its
  condition is false.
- **An empty block.** Tailwind CSS prints `@apply --m {}` as
  `@apply --m;`, so the block of `@contents` applies, not the empty block.
- **Types.** The step does not check the type of an argument or of
  `returns`. A value of the wrong type is not the guaranteed-invalid
  value.
- **The spec is a draft.** The mixin part is "experimental and under
  active development". The step follows the Editor's Draft of
  2026-11-04. `@result` and the conditional rules in `@private` are not
  there.
- **Other stylesheets.** The step sees one output. A mixin or a function
  of another stylesheet stays as it is.
- **`style()` in a mixin.** A `style()` test of a parameter of a mixin is
  not known: the `if()` stays.
- **More than 1,024 siblings.** After 1,024 siblings, the index and the
  count are wrong.
- **The drafts.** `if()` follows the Editor's Draft of CSS Values 5, and
  `@when` the Editor's Draft of CSS Conditional Rules 5 (2026-11-10).

## Two faults of Tailwind CSS

These two faults stop a mixin before the optimizer sees it. This
program corrects them ("Fixes" in [`COMPATIBILITY.md`](COMPATIBILITY.md)).
When Tailwind CSS fixes them, the port of that version replaces these
fixes.

### `@apply` with spaces in the arguments

Tailwind CSS splits the params of `@apply` at each space, also in the
arguments of a mixin. So `@apply --card(red, 2rem)` is the mixin
`--card(red,` and the utility `2rem)`, and the error "You cannot use
`@apply` with both mixins and utilities". This program splits only at
the spaces that are not in parentheses, brackets, braces or strings
(`top_level_words` of `native/tailwind/src/apply.rs`). Where upstream
gives no error, the result is the same.

### `@contents;` with no block

The last pass of Tailwind CSS removes an at-rule with no body, except
the at-rules of a list. `@apply` is in the list, but `@contents` is not.
So `@contents;` goes from the body of a mixin, and the block of
`@apply` has no place. A mixin with only `@contents;` goes too. This
program adds `@contents` to the list (`native/tailwind/src/optimize.rs`).
The printer of Tailwind CSS gives `@contents ;`, with a space, which is
valid CSS.
