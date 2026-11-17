# The sort of classes

`tailwind.com` sorts the classes of templates as the official Prettier
plugin of Tailwind CSS (`prettier-plugin-tailwindcss`) sorts them:

- **The order:** the order of the rules of Tailwind CSS in the output. A
  class that is not a class of Tailwind CSS comes first, and `...` (or
  `…`) comes last.
- **The design system of the project:** its theme, its utilities and
  variants, its plugins, its prefix, and its version of Tailwind CSS. A
  project on Tailwind CSS 4.1.0 gets the order of 4.1.0.
- **The whitespace:** one space between the classes, and none at the
  start or at the end.
- **The duplicates:** a class of Tailwind CSS that is there two times
  goes. A class that is not a class of Tailwind CSS stays.

The sort does not change the CSS: the compiler sorts the rules itself,
and `class="p-4 flex"` gives the same CSS as `class="flex p-4"`. The sort
changes only the templates.

## Phoenix: `mix format`

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
<div class="p-4 shadow flex bg-brand rounded-lg hover:bg-white items-center">
  <span class={["font-bold text-lg", @active && "underline text-red-500"]}>{@title}</span>
  <p class={"mt-2 #{@size} text-sm text-gray-600 block"}>Body</p>
</div>
```

becomes

```heex
<div class="flex items-center rounded-lg bg-brand p-4 shadow hover:bg-white">
  <span class={["text-lg font-bold", @active && "text-red-500 underline"]}>{@title}</span>
  <p class={"mt-2 #{@size} block text-sm text-gray-600"}>Body</p>
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
| `:profile` | the only profile | The profile of the configuration of `:tailwind`: its `--input`, `--cwd`, `cd` and `env`. With more than one profile, give it. |
| `:attributes` | `["class"]` | The names of the attributes to sort, for example `["class", "input_class"]`. |
| `:preserve_whitespace` | `false` | Keep the whitespace between the classes. |
| `:preserve_duplicates` | `false` | Keep a second copy of a class. |

The plugin needs `tailwind.com` (`mix tailwind.install`) or the NIF. It
starts `tailwind.com --sort-classes` one time for a run of `mix format`,
and that run loads the design system one time, with the plugins of the
stylesheet. Each file is then one request. In a test project with
Phoenix LiveView, `mix format` with the two plugins took 0.7 s.

## Erlang: `tailwind_sorter`

`tailwind_sorter` is a `gen_server` for a profile of the configuration:

```erlang
{ok, Sorter} = tailwind_sorter:start_link(default),
#{<<"attributes">> := [<<"flex p-4">>]} =
    tailwind_sorter:sort(Sorter, #{<<"attributes">> => [<<"p-4 flex">>]}).
```

It runs the program as a NIF when `tailwind:nif()` is true, else as a
port. More than one process can call one sorter: it answers in the
sequence of the calls.

## The program: `--sort-classes`

```sh
tailwind.com --sort-classes -i assets/css/app.css
```

The program loads the design system of the input, and then reads one
request on each line of its standard input, and writes one answer on
each line of its standard output, until the end of the input. A request
is a JSON object, and each part is optional:

```json
{"attributes": ["py-3 p-1 px-3"],
 "templates": [["px-3 ", " p-1 m-2"]],
 "lists": [["py-3", "p-1", "p-1"]],
 "preserveWhitespace": false,
 "preserveDuplicates": false}
```

| Part | What | In the plugin |
|---|---|---|
| `attributes` | The values of `class` attributes. | `sortClassAttributes` of its sorter API (`prettier-plugin-tailwindcss/sorter`) |
| `templates` | For each string with interpolations, its fixed parts: `"px-3 #{@a} p-1"` has `"px-3 "` and `" p-1"`. | `sortTemplateLiteral` |
| `lists` | Lists of classes. | `sortClassLists` of its sorter API |

The answer has the same parts, sorted:

```json
{"attributes":["p-1 px-3 py-3"],"templates":[["px-3 "," m-2 p-1"]],"lists":[["p-1","py-3"]]}
```

A request that is not valid gets `{"error": "…"}`. The sort needs the
Rust compiler: with the script engine (`TAILWIND_COM_ENGINE=script`),
the program gives an error.

## The tests

- `native/tailwind/tests/class_order.rs` compares the order of each class
  (`getClassOrder` of the design system) with the record of the official
  compiler: 250 lists on four stylesheets
  (`tests/upstream/tools/gen-class-order.mjs`).
- The same file compares the sort with the record of the real Prettier
  plugin 0.8.1: 486 values of `class` attributes (with each option), 120
  lists and 80 template literals
  (`tests/upstream/tools/gen-class-sort.mjs`).
- `tests/tailwind_formatter_test.exs` tests the plugin of `mix format`,
  and `tests/tailwind_sorter_test.exs` the `gen_server`, with the program
  and, in the job `nif` of CI, with the NIF.
