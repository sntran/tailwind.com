# The sort of classes

`mix format` can sort the classes of HEEx templates as the official
Prettier plugin of Tailwind CSS (`prettier-plugin-tailwindcss`) sorts
them:

- **The order:** the order of the rules of Tailwind CSS in the output. A
  class with no rule of Tailwind CSS (`group`, `peer`, a class of the
  CSS of the project) comes first, and `...` (or `…`) comes last.
- **The design system of the project:** its theme, its utilities and
  variants, its plugins (daisyUI), its prefix, and its version of
  Tailwind CSS.
- **The whitespace:** one space between the classes, and none at the
  start or at the end.
- **The duplicates:** a class of Tailwind CSS that is there two times
  goes. A class that is not a class of Tailwind CSS stays.

The sort changes only the templates. The CSS stays the same: the
compiler sorts the rules itself, and `class="p-4 flex"` gives the same
CSS as `class="flex p-4"`.

## Phoenix

`Tailwind.Formatter` is a plugin of `mix format`. In `.formatter.exs`,
put it before `Phoenix.LiveView.HTMLFormatter`:

```elixir
[
  import_deps: [:ecto, :ecto_sql, :phoenix],
  subdirectories: ["priv/*/migrations"],
  plugins: [Tailwind.Formatter, Phoenix.LiveView.HTMLFormatter],
  inputs: ["*.{heex,ex,exs}", "{config,lib,test}/**/*.{heex,ex,exs}", "priv/*/seeds.exs"]
]
```

Then `mix format` sorts the classes of the `.heex` files and of the
`~H` sigils:

```heex
<div class="p-4 shadow flex bg-brand rounded-lg hover:bg-white items-center group card">
  <span class={["font-bold text-lg", @active && "underline text-red-500"]}>{@title}</span>
  <p class={"mt-2 #{@size} text-sm text-gray-600 block group-hover:underline"}>Body</p>
</div>
```

becomes

```heex
<div class="group card flex items-center rounded-lg bg-brand p-4 shadow hover:bg-white">
  <span class={["text-lg font-bold", @active && "text-red-500 underline"]}>{@title}</span>
  <p class={"mt-2 #{@size} block text-sm text-gray-600 group-hover:underline"}>Body</p>
</div>
```

- `class="…"` and `class='…'`: the value is sorted.
- `class={…}`: each string of the expression is sorted, also in a list,
  in a condition (`@active && "…"`) and in a function call.
- A string with interpolations (`"mt-2 #{@size} text-sm"`): each fixed
  part is sorted, and a class at the side of an interpolation stays at
  its place, as in a template literal of JavaScript. `mt-2 ` ends with a
  space, so `mt-2` is a class of its own and can move; in
  `"mt-2#{@size}"`, `mt-2` is a part of the class that the
  interpolation makes, and it stays.
- The plugin does not change comments, EEx tags (`<%= … %>`), the body
  of `<script>` and `<style>`, a string with an escape (`\n`), a sigil
  (`~w(…)`), and the other attributes.

The options are in the key `:tailwind` of `.formatter.exs`:

| Option | Default | What |
|---|---|---|
| `:profile` | the only profile | The profile of the configuration of `:tailwind`: its `--input`, `--cwd`, `cd`, `env` and the version. With more than one profile, give it. |
| `:attributes` | `["class"]` | The names of the attributes to sort, for example `["class", "input_class"]`. |
| `:preserve_whitespace` | `false` | Keep the whitespace between the classes. |
| `:preserve_duplicates` | `false` | Keep a second copy of a class. |

## How the plugin gets the order

The plugin uses no option of the program that the official program does
not have:

1. It builds the CSS of the profile one time for a run of `mix format`:
   `tailwind.com --input=… --cwd=… --output=TEMPORARY_FILE`, with the
   `cd`, the `env` and the version of the profile, as `mix tailwind`
   does (as a NIF when the project has one). The other arguments of the
   profile, as `--minify` and `--watch`, are not for this build.
2. It reads each style rule of the layer `utilities` of the CSS, at any
   depth (the `@media` of a variant is around the rule). The class of a
   rule is the longest class of its selector: the selector of
   `group-hover:underline` also has `.group`, but
   `.group-hover\:underline` has the text of `.group` and more.
3. The place of a class is the place of its first rule. This is the
   order of `getClassOrder` of Tailwind CSS, which the Prettier plugin
   uses.
4. In Elixir, the plugin sorts each value as `sortClasses`,
   `sortClassList` and the sort of a template literal of the Prettier
   plugin do.

The order stays for 10 seconds: one run of `mix format` builds one time,
and an editor that formats again later gets the new classes.

**The limit:** the build sees the files of the `@source` of the
stylesheet, as they are on the disk. A class that the build does not see
(in a file that is not a source, or in an editor buffer that is not
saved) has no place, and comes first. Such a class also has no CSS in
the program until the build sees it.

In a Mix project with Phoenix LiveView and no plugin of Tailwind CSS,
`mix format` with `Tailwind.Formatter` and
`Phoenix.LiveView.HTMLFormatter` took about 0.95 s, and 0.75 s without
`Tailwind.Formatter`. The build of a Phoenix 1.8 project with daisyUI
takes about 120 ms, and the read of its CSS (78 KB) in Elixir about 3
ms.

## The tests

`tests/tailwind_formatter_test.exs`, with the program and, in the job
`nif` of CI, with the NIF:

- The order of a build against the record of the official compiler
  (`native/tailwind/tests/fixtures/class_order.json`, from
  `tests/upstream/tools/gen-class-order.mjs`): each list of a stylesheet
  that builds has the same sort, except the lists with `[--:1]` or
  `w-(--)`, two candidates that the scanner does not extract from a
  file.
- The sort against the record of the real Prettier plugin 0.8.1
  (`native/tailwind/tests/fixtures/class_sort.json`, from
  `tests/upstream/tools/gen-class-sort.mjs`): 486 values of `class`
  attributes (with each option), 120 lists and 80 template literals have
  the same result.
- The forms of the templates, a `~H` sigil through the formatter of
  `mix format`, and a build that fails.

The library crate also has the order of a design system:
`DesignSystem::get_class_order` (`native/tailwind/src/sort.rs`, the port
of `sort.ts`), with a test against the same record
(`native/tailwind/tests/class_order.rs`).
