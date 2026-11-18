defmodule Tailwind.Formatter do
  @moduledoc """
  A plugin of `mix format` that sorts the classes of HEEx templates, as the
  official Prettier plugin of Tailwind CSS (`prettier-plugin-tailwindcss`)
  sorts them: the order of the rules of Tailwind CSS, with the design
  system of the project (its theme, plugins and version of Tailwind CSS),
  and with no second copy of a class of Tailwind CSS.

  In `.formatter.exs`, put the plugin before `Phoenix.LiveView.HTMLFormatter`:

      [
        plugins: [Tailwind.Formatter, Phoenix.LiveView.HTMLFormatter],
        inputs: ["*.{heex,ex,exs}", "{config,lib,test}/**/*.{heex,ex,exs}"],
        tailwind: [profile: :my_app]
      ]

  The plugin sorts `.heex` files and `~H` sigils:

    * `class="…"` and `class='…'`.
    * Each string in `class={…}`, for example `class={["p-4 flex", @x && "m-2 block"]}`.
    * The fixed parts of a string with interpolations: in
      `class={"px-3 \#{@size} p-1"}`, `px-3` and `p-1` stay at the side of
      the interpolation, as in a template literal of JavaScript.

  It does not change comments, EEx tags (`<%= … %>`), `<script>` and
  `<style>`.

  ## Options

  The key `:tailwind` of `.formatter.exs`:

    * `:profile` - the profile of the configuration of `:tailwind` (its
      `--input`, `cd` and `env`). Default: the only profile of the
      configuration.
    * `:attributes` - the names of the attributes to sort. Default:
      `["class"]`.
    * `:preserve_whitespace` - keep the whitespace between the classes.
      Default: `false`.
    * `:preserve_duplicates` - keep a second copy of a class. Default:
      `false`.
    * `:canonicalize` - give each class list its canonical form, as the
      command `canonicalize` of the official program does: `px-4 py-4`
      becomes `p-4`, `w-[16px] h-[16px]` becomes `size-4`. The CSS stays
      the same. Default: `false`. See below.

  The order comes from a normal build of the profile, one time for each
  run of `mix format` (as a NIF when the project has one): the place of a
  class is the place of its first rule in the layer `utilities` of the
  output. A class with no rule (a class of the project, or a class that
  the build does not see) comes first. `docs/FORMATTER.md` has more.

  ## The canonical form

  With `canonicalize: true`, the plugin gives each class list with no
  interpolation to the command `canonicalize` of `tailwind.com`. The
  result replaces the list: it has the canonical form of each class,
  one class for more classes when the CSS is the same, no second copy of
  a class, and the order of the official Prettier plugin. A list with
  `...` and a string with interpolations get the sort only. A
  replacement is always a class of the version of Tailwind CSS of the
  profile. One process runs the command with
  `--stream` for all the files of a run of `mix format`: the first list
  takes about 0.7 s, and each other list a few milliseconds.
  """

  @behaviour Mix.Tasks.Format

  @impl Mix.Tasks.Format
  def features(_opts), do: [sigils: [:H], extensions: [".heex"]]

  @impl Mix.Tasks.Format
  def format(contents, opts) do
    config = Keyword.get(opts, :tailwind, [])
    names = config |> Keyword.get(:attributes, ["class"]) |> Enum.map(&to_string/1)

    case scan(contents, names) do
      [] -> contents
      slots -> sort(contents, slots, config)
    end
  end

  # The sort of the values of a template, with the order of the build.
  defp sort(contents, slots, config) do
    text = fn {start, stop} -> binary_part(contents, start, stop - start) end
    profile = profile(config)
    places = order(profile)
    place = fn class -> Map.get(places, escape(class)) end

    canonical =
      if Keyword.get(config, :canonicalize, false) do
        values = for {:attribute, range} <- slots, value = text.(range), canonical?(value), do: value
        canonical_forms(profile, values)
      else
        %{}
      end

    options = [
      remove_duplicates: not Keyword.get(config, :preserve_duplicates, false),
      collapse: not Keyword.get(config, :preserve_whitespace, false)
    ]

    replacements =
      Enum.flat_map(slots, fn
        {:attribute, range} ->
          value = text.(range)

          case Map.fetch(canonical, group(value)) do
            {:ok, form} -> [{range, form}]
            :error -> [{range, sort_attribute(value, place, options)}]
          end

        {:template, ranges} ->
          Enum.zip(ranges, sort_template(Enum.map(ranges, text), place, options))
      end)

    replace(contents, Enum.sort_by(replacements, fn {{start, _}, _} -> start end))
  end

  # The text with each range replaced, from the start.
  defp replace(contents, replacements) do
    {parts, last} =
      Enum.reduce(replacements, {[], 0}, fn {{start, stop}, value}, {parts, offset} ->
        {[value, binary_part(contents, offset, start - offset) | parts], stop}
      end)

    IO.iodata_to_binary(
      Enum.reverse([binary_part(contents, last, byte_size(contents) - last) | parts])
    )
  end

  # The order lives this long: one run of `mix format` builds one time,
  # and an editor that formats again later gets the new classes.
  @lifetime_ms 10_000

  ## The canonical form

  # A value with classes, no Mustache text and no `...`.
  defp canonical?(value) do
    group = group(value)
    group != "" and not String.contains?(value, "{{") and
      not Enum.any?(String.split(group, " "), &(&1 in ["...", "…"]))
  end

  # The classes of a value, with one space between them.
  defp group(value), do: value |> String.split(~r/[\t\r\f\n ]+/, trim: true) |> Enum.join(" ")

  @doc false
  # The canonical form of each value (by its group of classes), from the
  # process of the command `canonicalize` of the profile.
  def canonical_forms(profile, values) do
    groups = values |> Enum.map(&group/1) |> Enum.uniq()
    Tailwind.Formatter.Canonical.forms(profile, groups)
  end

  @doc false
  # The path of the input of the arguments of a profile, relative to the
  # working directory of the profile: `--cwd` and `--input`.
  def css_path(args) do
    args = input_args(args)

    {input, cwd} =
      Enum.reduce(Enum.chunk_every(split_args(args), 2), {nil, nil}, fn
        [flag, value], {_input, cwd} when flag in [~c"--input", ~c"-i"] -> {value, cwd}
        [~c"--cwd", value], {input, _} -> {input, value}
        _, acc -> acc
      end)

    if input, do: if(cwd, do: Path.join(to_string(cwd), to_string(input)), else: to_string(input)) |> String.to_charlist()
  end

  # `--name=value` is `--name value`.
  defp split_args(args) do
    Enum.flat_map(args, fn arg ->
      case :string.split(arg, ~c"=") do
        [flag, value] -> [flag, value]
        _ -> [arg]
      end
    end)
  end

  ## The order of the classes

  @doc false
  # The place of each class, from a build of the profile: escaped class
  # (as in a selector) to place. One process builds, and the others wait.
  def order(profile) do
    key = {__MODULE__, profile}

    case fresh(key) do
      {:ok, places} ->
        places

      :error ->
        :global.trans({key, self()}, fn ->
          case fresh(key) do
            {:ok, places} ->
              places

            :error ->
              places = places(build(profile))
              :persistent_term.put(key, {System.monotonic_time(:millisecond), places})
              places
          end
        end)
    end
  end

  defp fresh(key) do
    case :persistent_term.get(key, nil) do
      {time, places} ->
        if System.monotonic_time(:millisecond) - time < @lifetime_ms,
          do: {:ok, places},
          else: :error

      nil ->
        :error
    end
  end

  # A normal build of the profile, with the options of the official
  # program only: its input and its working directory, and an output in a
  # temporary file. The result is the CSS.
  defp build(profile) do
    %{args: args, cd: cd, env: env} = :tailwind.config_for(profile)
    env = Map.merge(%{~c"TAILWINDCSS_VERSION" => :tailwind.configured_version()}, env)

    output =
      Path.join(System.tmp_dir!(), "tailwind_formatter_#{System.unique_integer([:positive])}.css")

    argv = input_args(args) ++ [~c"--output=" ++ String.to_charlist(output)]

    {status, messages} =
      if :tailwind.nif() do
        :tailwind_nif.run(
          argv,
          %{cd: cd, env: Map.to_list(env)},
          fn _stream, data, acc -> [acc, data] end,
          []
        )
      else
        unless File.exists?(:tailwind.bin_path()) do
          raise ArgumentError,
                "Tailwind.Formatter: tailwind.com is not installed. Run `mix tailwind.install`."
        end

        {exe, port_args} = :tailwind.command(argv)

        port =
          Port.open({:spawn_executable, exe}, [
            {:args, port_args},
            {:cd, cd},
            {:env, Map.to_list(env)},
            :binary,
            :exit_status,
            :stderr_to_stdout,
            :use_stdio,
            :hide
          ])

        collect(port, [])
      end

    try do
      if status != 0 do
        raise ArgumentError,
              "Tailwind.Formatter: the build of the profile #{inspect(profile)} failed:\n" <>
                IO.iodata_to_binary(messages)
      end

      File.read!(output)
    after
      File.rm(output)
    end
  end

  defp collect(port, acc) do
    receive do
      {^port, {:data, data}} -> collect(port, [acc, data])
      {^port, {:exit_status, status}} -> {status, acc}
    end
  end

  # The input and the working directory of the arguments of a profile.
  defp input_args([]), do: []

  defp input_args([flag, value | rest]) when flag in [~c"--input", ~c"-i", ~c"--cwd"],
    do: [flag, value | input_args(rest)]

  defp input_args([arg | rest]) do
    if Enum.any?([~c"--input=", ~c"-i=", ~c"--cwd="], &List.starts_with?(arg, &1)),
      do: [arg | input_args(rest)],
      else: input_args(rest)
  end

  @doc false
  # The place of each class in the CSS of a build: the place of the first
  # style rule (at any depth: the `@media` of a variant is around the rule)
  # of the layer `utilities` that has the class. The class of a rule is the
  # longest class of its selector: it has the text of the classes that its
  # variants name (`group-hover:flex` has `group`). The keys are the
  # classes as the selectors write them (`escape/1`).
  def places(css) do
    case :binary.match(css, "@layer utilities {") do
      {start, length} -> rules(css, start + length, start + length, 0, 0, %{})
      :nomatch -> %{}
    end
  end

  @class ~r/\.((?:\\[0-9a-fA-F]{1,6} ?|\\.|[\w-]|[^\x00-\x7f])+)/u

  defp rules(css, i, _from, _depth, _rank, places) when i >= byte_size(css), do: places

  defp rules(css, i, from, depth, rank, places) do
    case :binary.at(css, i) do
      ?\\ ->
        rules(css, i + 2, from, depth, rank, places)

      quote when quote in [?", ?'] ->
        rules(css, find_escaped(css, i + 1, quote) + 1, from, depth, rank, places)

      ?{ ->
        prelude = css |> binary_part(from, i - from) |> String.trim()

        {places, rank} =
          if String.starts_with?(prelude, "@") do
            {places, rank}
          else
            case Regex.scan(@class, prelude, capture: :all_but_first) do
              [] ->
                {places, rank + 1}

              classes ->
                owner = classes |> List.flatten() |> Enum.max_by(&byte_size/1)
                {Map.put_new(places, owner, rank), rank + 1}
            end
          end

        rules(css, i + 1, i + 1, depth + 1, rank, places)

      ?} ->
        if depth == 0, do: places, else: rules(css, i + 1, i + 1, depth - 1, rank, places)

      ?; ->
        rules(css, i + 1, i + 1, depth, rank, places)

      _ ->
        rules(css, i + 1, from, depth, rank, places)
    end
  end

  @doc false
  # `CSS.escape`, as `escape` of Tailwind CSS: a class as a selector writes
  # it.
  def escape("-"), do: "\\-"

  def escape(value) do
    first = :binary.first(value <> "x")

    value
    |> :binary.bin_to_list()
    |> Enum.with_index()
    |> Enum.map(fn {code, index} ->
      numeric? =
        code in 0x01..0x1F or code == 0x7F or (index == 0 and code in ?0..?9) or
          (index == 1 and code in ?0..?9 and first == ?-)

      cond do
        code == 0 ->
          "�"

        numeric? ->
          "\\" <> String.downcase(Integer.to_string(code, 16)) <> " "

        code >= 0x80 or code in [?-, ?_] or code in ?a..?z or code in ?A..?Z or code in ?0..?9 ->
          <<code>>

        true ->
          <<?\\, code>>
      end
    end)
    |> IO.iodata_to_binary()
  end

  ## The sort of the Prettier plugin

  @dots ["...", "…"]

  # `\s` of JavaScript.
  @js_space "[\\t\\n\\v\\f\\r \\x{a0}\\x{1680}\\x{2000}-\\x{200a}\\x{2028}\\x{2029}\\x{202f}\\x{205f}\\x{3000}\\x{feff}]"

  @doc false
  # `sortClassList` of the plugin: no place first, then by place, and `...`
  # last, with a stable sort. With `remove_duplicates`, a second copy of a
  # class with a place goes. The result has the classes and the indexes
  # (in the sorted list) of the classes that went.
  def sort_class_list(classes, place, remove_duplicates) do
    sorted =
      classes
      |> Enum.map(&{&1, place.(&1)})
      |> Enum.sort_by(fn
        {name, _} when name in @dots -> {2, 0}
        {_, nil} -> {0, 0}
        {_, place} -> {1, place}
      end)

    if remove_duplicates do
      {kept, removed, _} =
        sorted
        |> Enum.with_index()
        |> Enum.reduce({[], [], MapSet.new()}, fn {{name, place}, index}, {kept, removed, seen} ->
          cond do
            MapSet.member?(seen, name) -> {kept, [index | removed], seen}
            place == nil -> {[name | kept], removed, seen}
            true -> {[name | kept], removed, MapSet.put(seen, name)}
          end
        end)

      {Enum.reverse(kept), MapSet.new(removed)}
    else
      {Enum.map(sorted, &elem(&1, 0)), MapSet.new()}
    end
  end

  @doc false
  # `sortClasses` of the plugin: the value of one attribute. The options:
  # `remove_duplicates`, `collapse` (`true`, `false`, or `{start, end}`),
  # `ignore_first` and `ignore_last`.
  def sort_attribute(value, place, options) do
    collapse =
      case Keyword.get(options, :collapse, true) do
        true -> {true, true}
        other -> other
      end

    cond do
      value == "" or String.contains?(value, "{{") ->
        value

      collapse != false and Regex.match?(~r/\A[\t\r\f\n ]+\z/, value) ->
        " "

      true ->
        sort_parts(value, place, collapse, options)
    end
  end

  defp sort_parts(value, place, collapse, options) do
    # `split(/([\t\r\f\n ]+)/)`: the classes at the even places, and the
    # whitespace between them at the odd places.
    parts = Regex.split(~r/([\t\r\f\n ]+)/, value, include_captures: true)
    classes = Enum.take_every(parts, 2)
    whitespace = parts |> Enum.drop(1) |> Enum.take_every(2)
    classes = if List.last(classes) == "", do: Enum.drop(classes, -1), else: classes
    whitespace = if collapse != false, do: Enum.map(whitespace, fn _ -> " " end), else: whitespace

    {prefix, classes, whitespace} =
      if Keyword.get(options, :ignore_first, false) do
        {first, classes} = pop_first(classes)
        {space, whitespace} = pop_first(whitespace)
        {first <> space, classes, whitespace}
      else
        {"", classes, whitespace}
      end

    {suffix, classes, whitespace} =
      if Keyword.get(options, :ignore_last, false) do
        {space, whitespace} = pop_last(whitespace)
        {last, classes} = pop_last(classes)
        {space <> last, classes, whitespace}
      else
        {"", classes, whitespace}
      end

    {list, removed} =
      sort_class_list(classes, place, Keyword.get(options, :remove_duplicates, true))

    # The plugin removes the whitespace before each removed class with the
    # indexes of the sorted list: `!removedIndices.has(index + 1)`.
    whitespace =
      whitespace
      |> Enum.with_index()
      |> Enum.reject(fn {_, index} -> MapSet.member?(removed, index + 1) end)
      |> Enum.map(&elem(&1, 0))

    result =
      list
      |> Enum.with_index()
      |> Enum.map(fn {class, index} -> [class, Enum.at(whitespace, index, "")] end)
      |> IO.iodata_to_binary()

    case collapse do
      false ->
        prefix <> result <> suffix

      {start, stop} ->
        trailing = Regex.compile!(@js_space <> "+\\z", "u")
        leading = Regex.compile!("\\A" <> @js_space <> "+", "u")
        prefix = Regex.replace(trailing, prefix, " ")
        suffix = Regex.replace(leading, suffix, " ")
        result = Regex.replace(leading, result, if(start, do: "", else: " "))
        result = Regex.replace(trailing, result, if(stop, do: "", else: " "))
        prefix <> result <> suffix
    end
  end

  defp pop_first([]), do: {"", []}
  defp pop_first([first | rest]), do: {first, rest}

  defp pop_last([]), do: {"", []}
  defp pop_last(list), do: {List.last(list), Enum.drop(list, -1)}

  @doc false
  # `sortTemplateLiteral` of the plugin: the fixed parts of a string with
  # interpolations. A part keeps its first class when it touches the
  # interpolation before it, and its last class when it touches the
  # interpolation after it. The whitespace collapses at the start of the
  # first part and at the end of the last part only.
  def sort_template(parts, place, options) do
    count = length(parts)
    starts_with_space = Regex.compile!("\\A" <> @js_space, "u")
    ends_with_space = Regex.compile!(@js_space <> "\\z", "u")

    parts
    |> Enum.with_index()
    |> Enum.map(fn {part, index} ->
      sort_attribute(part, place,
        remove_duplicates: Keyword.get(options, :remove_duplicates, true),
        ignore_first: index > 0 and not Regex.match?(starts_with_space, part),
        ignore_last: index + 1 < count and not Regex.match?(ends_with_space, part),
        collapse: Keyword.get(options, :collapse, true) and {index == 0, index + 1 >= count}
      )
    end)
  end

  # The profile of the options, or the only profile of the configuration.
  defp profile(config) do
    if profiles() == [] and Code.ensure_loaded?(Mix.Task), do: Mix.Task.run("loadconfig")

    case {Keyword.get(config, :profile), profiles()} do
      {nil, [profile]} ->
        profile

      {nil, profiles} ->
        raise ArgumentError,
              "Tailwind.Formatter: give the profile in .formatter.exs, as " <>
                "`tailwind: [profile: :my_app]`. The profiles of the configuration: #{inspect(profiles)}"

      {profile, _} ->
        profile
    end
  end

  defp profiles do
    for {key, value} <- Application.get_all_env(:tailwind),
        is_list(value) or is_map(value),
        Keyword.keyword?(value) or is_map(value),
        value[:args] != nil,
        do: key
  end

  ## The scanner

  @doc false
  # The places of the values to sort, in the sequence of the text:
  # `{:attribute, {start, stop}}` for a value with no interpolation, and
  # `{:template, [{start, stop}, …]}` for the fixed parts of a string with
  # interpolations. The positions are byte offsets.
  def scan(contents, names), do: text(contents, 0, names, [])

  defp text(src, i, _names, acc) when i >= byte_size(src), do: Enum.reverse(acc)

  defp text(src, i, names, acc) do
    cond do
      starts?(src, i, "<!--") ->
        text(src, skip_to(src, i + 4, "-->"), names, acc)

      starts?(src, i, "<%") ->
        text(src, skip_to(src, i + 2, "%>"), names, acc)

      starts?(src, i, "</") ->
        text(src, skip_to(src, i + 2, ">"), names, acc)

      byte(src, i) == ?< and tag_start?(byte(src, i + 1)) ->
        {tag, j} = read_name(src, i + 1)
        {j, closed?, acc} = attributes(src, j, names, acc)

        j =
          if tag in ["script", "style"] and not closed?,
            do: skip_to(src, j, "</" <> tag),
            else: j

        text(src, j, names, acc)

      byte(src, i) == ?{ ->
        {j, _} = expression(src, i + 1, false)
        text(src, j, names, acc)

      true ->
        text(src, i + 1, names, acc)
    end
  end

  # The attributes of a tag, to `>` or `/>`. The result has the place after
  # the tag, and true when the tag ends with `/>`.
  defp attributes(src, i, names, acc) do
    i = skip_space(src, i)

    cond do
      i >= byte_size(src) ->
        {i, false, acc}

      byte(src, i) == ?> ->
        {i + 1, false, acc}

      starts?(src, i, "/>") ->
        {i + 2, true, acc}

      byte(src, i) == ?{ ->
        {j, _} = expression(src, i + 1, false)
        attributes(src, j, names, acc)

      true ->
        {name, j} = read_attribute_name(src, i)

        if name == "" do
          attributes(src, i + 1, names, acc)
        else
          value(src, skip_space(src, j), name in names, names, acc)
        end
    end
  end

  # The value of an attribute, when there is `=`.
  defp value(src, j, sort?, names, acc) do
    if byte(src, j) == ?= do
      k = skip_space(src, j + 1)

      case byte(src, k) do
        quote when quote in [?", ?'] ->
          stop = find(src, k + 1, quote)
          acc = if sort?, do: [{:attribute, {k + 1, stop}} | acc], else: acc
          attributes(src, stop + 1, names, acc)

        ?{ ->
          {stop, slots} = expression(src, k + 1, sort?)
          attributes(src, stop, names, Enum.reverse(slots, acc))

        _ ->
          attributes(src, skip_value(src, k), names, acc)
      end
    else
      attributes(src, j, names, acc)
    end
  end

  # An Elixir expression, after its `{`, to its `}`. With `record?`, the
  # result has the strings of the expression. The result has the place
  # after the `}`.
  defp expression(src, i, record?), do: expression(src, i, 0, record?, [])

  defp expression(src, i, _depth, _record?, slots) when i >= byte_size(src),
    do: {i, Enum.reverse(slots)}

  defp expression(src, i, depth, record?, slots) do
    case byte(src, i) do
      ?} when depth == 0 ->
        {i + 1, Enum.reverse(slots)}

      ?} ->
        expression(src, i + 1, depth - 1, record?, slots)

      ?{ ->
        expression(src, i + 1, depth + 1, record?, slots)

      ?" ->
        if starts?(src, i, ~s(""")) do
          expression(src, skip_to(src, i + 3, ~s(""")), depth, record?, slots)
        else
          {stop, ranges, escaped?} = string(src, i + 1)

          slots =
            cond do
              not record? or escaped? -> slots
              match?([_], ranges) -> [{:attribute, hd(ranges)} | slots]
              true -> [{:template, ranges} | slots]
            end

          expression(src, stop, depth, record?, slots)
        end

      ?' ->
        expression(src, find_escaped(src, i + 1, ?') + 1, depth, record?, slots)

      ?? ->
        # A character, as `?{` or `?\\n`.
        skip = if byte(src, i + 1) == ?\\, do: 3, else: 2
        expression(src, i + skip, depth, record?, slots)

      ?~ ->
        expression(src, sigil(src, i + 1), depth, record?, slots)

      ?# ->
        expression(src, skip_to(src, i + 1, "\n"), depth, record?, slots)

      _ ->
        expression(src, i + 1, depth, record?, slots)
    end
  end

  # A string, after its `"`: the place after its end, the ranges of its
  # fixed parts (one more than its interpolations), and true when it has
  # an escape.
  defp string(src, i), do: string(src, i, i, [], false)

  defp string(src, i, start, ranges, escaped?) when i >= byte_size(src),
    do: {i, Enum.reverse([{start, i} | ranges]), escaped?}

  defp string(src, i, start, ranges, escaped?) do
    cond do
      byte(src, i) == ?\\ ->
        string(src, i + 2, start, ranges, true)

      starts?(src, i, "\#{") ->
        {j, _} = expression(src, i + 2, false)
        string(src, j, j, [{start, i} | ranges], escaped?)

      byte(src, i) == ?" ->
        {i + 1, Enum.reverse([{start, i} | ranges]), escaped?}

      true ->
        string(src, i + 1, start, ranges, escaped?)
    end
  end

  # A sigil, after its `~`: the place after it.
  defp sigil(src, i) do
    j = skip_while(src, i, &(&1 in ?a..?z or &1 in ?A..?Z))

    cond do
      starts?(src, j, ~s(""")) -> skip_to(src, j + 3, ~s("""))
      starts?(src, j, "'''") -> skip_to(src, j + 3, "'''")
      true -> close_sigil(src, j)
    end
  end

  @pairs %{?( => ?), ?[ => ?], ?{ => ?}, ?< => ?>}

  defp close_sigil(src, j) do
    case byte(src, j) do
      nil ->
        j

      open when is_map_key(@pairs, open) ->
        close = Map.fetch!(@pairs, open)
        nested(src, j + 1, open, close, 0) |> skip_modifiers(src)

      delimiter ->
        (find_escaped(src, j + 1, delimiter) + 1) |> skip_modifiers(src)
    end
  end

  defp nested(src, i, _open, _close, _depth) when i >= byte_size(src), do: i

  defp nested(src, i, open, close, depth) do
    case byte(src, i) do
      ?\\ -> nested(src, i + 2, open, close, depth)
      ^close when depth == 0 -> i + 1
      ^close -> nested(src, i + 1, open, close, depth - 1)
      ^open -> nested(src, i + 1, open, close, depth + 1)
      _ -> nested(src, i + 1, open, close, depth)
    end
  end

  defp skip_modifiers(i, src), do: skip_while(src, i, &(&1 in ?a..?z or &1 in ?A..?Z))

  ## The parts of the scanner

  defp byte(src, i) when i < byte_size(src), do: :binary.at(src, i)
  defp byte(_src, _i), do: nil

  defp starts?(src, i, prefix) do
    size = byte_size(prefix)
    i + size <= byte_size(src) and binary_part(src, i, size) == prefix
  end

  # The place after the first `pattern` from `i`, or the end.
  defp skip_to(src, i, pattern) do
    if i >= byte_size(src) do
      byte_size(src)
    else
      case :binary.match(src, pattern, scope: {i, byte_size(src) - i}) do
        {start, length} -> start + length
        :nomatch -> byte_size(src)
      end
    end
  end

  # The place of the first `char` from `i`, or the end.
  defp find(src, i, char) do
    case :binary.match(src, <<char>>, scope: {i, max(byte_size(src) - i, 0)}) do
      {start, _} -> start
      :nomatch -> byte_size(src)
    end
  end

  # The place of the first `char` from `i` with no `\` before it.
  defp find_escaped(src, i, _char) when i >= byte_size(src), do: byte_size(src)

  defp find_escaped(src, i, char) do
    case byte(src, i) do
      ?\\ -> find_escaped(src, i + 2, char)
      ^char -> i
      _ -> find_escaped(src, i + 1, char)
    end
  end

  defp skip_while(src, i, fun) do
    case byte(src, i) do
      nil -> i
      b -> if fun.(b), do: skip_while(src, i + 1, fun), else: i
    end
  end

  defp skip_space(src, i), do: skip_while(src, i, &(&1 in [?\s, ?\t, ?\n, ?\r, ?\f]))

  defp skip_value(src, i), do: skip_while(src, i, &(&1 not in [?\s, ?\t, ?\n, ?\r, ?\f, ?>]))

  defp tag_start?(nil), do: false
  defp tag_start?(b), do: b in ?a..?z or b in ?A..?Z or b in [?., ?:]

  defp read_name(src, i) do
    j = skip_while(src, i, &(&1 not in [?\s, ?\t, ?\n, ?\r, ?\f, ?>, ?/]))
    {binary_part(src, i, j - i), j}
  end

  defp read_attribute_name(src, i) do
    j = skip_while(src, i, &(&1 not in [?\s, ?\t, ?\n, ?\r, ?\f, ?>, ?/, ?=, ?{]))
    {binary_part(src, i, j - i), j}
  end
end
