# CSS functions and mixins

[CSS Custom Functions and Mixins Module Level 1](https://drafts.csswg.org/css-mixins-1/)
adds `@function` and `@mixin` to CSS. A function gives a value. A
mixin gives declarations and rules for a style rule:

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
- **The browsers decide**, as for each other lowering of Lightning CSS.
  The step lowers a rule when a target browser of the optimizer does not
  have it. The targets are those of Tailwind CSS (Chrome 111, Safari
  16.4, Firefox 128), so the step lowers the two rules for each project:

  | Rule | The first browsers | With the targets of Tailwind CSS |
  |---|---|---|
  | `@function` | Chrome and Edge 139 | Lowered |
  | `@mixin`, `@apply --name` | None yet | Lowered |

  The table is `FUNCTIONS` and `MIXINS` in `css_mixins.rs`. When a new
  release of Tailwind CSS raises its targets, the decision follows.

A stylesheet with no `@function` and no `@mixin` does not change. With no
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
