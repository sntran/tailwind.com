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

  The plugin runs `tailwind.com --sort-classes` one time for each run of
  `mix format` (`:tailwind_sorter`), as a NIF when the project has one.
  `docs/FORMATTER.md` has more.
  """

  @behaviour Mix.Tasks.Format

  @sorter Tailwind.Formatter.Sorter

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

  # The sort: one request for the values of a template.
  defp sort(contents, slots, config) do
    text = fn {start, stop} -> binary_part(contents, start, stop - start) end

    attributes = for {:attribute, range} <- slots, do: text.(range)
    templates = for {:template, ranges} <- slots, do: Enum.map(ranges, text)

    request = %{
      "attributes" => attributes,
      "templates" => templates,
      "preserveWhitespace" => Keyword.get(config, :preserve_whitespace, false),
      "preserveDuplicates" => Keyword.get(config, :preserve_duplicates, false)
    }

    case :tailwind_sorter.sort(sorter(config), request) do
      %{"attributes" => sorted_attributes, "templates" => sorted_templates} ->
        attribute_ranges = for {:attribute, range} <- slots, do: range
        template_ranges = for {:template, ranges} <- slots, range <- ranges, do: range

        replacements =
          Enum.zip(attribute_ranges, sorted_attributes) ++
            Enum.zip(template_ranges, List.flatten(sorted_templates))

        replace(contents, Enum.sort_by(replacements, fn {{start, _}, _} -> start end))

      %{"error" => message} ->
        raise ArgumentError, "Tailwind.Formatter: #{message}"
    end
  end

  # The text with each range replaced, from the start.
  defp replace(contents, replacements) do
    {parts, last} =
      Enum.reduce(replacements, {[], 0}, fn {{start, stop}, value}, {parts, offset} ->
        {[value, binary_part(contents, offset, start - offset) | parts], stop}
      end)

    IO.iodata_to_binary(Enum.reverse([binary_part(contents, last, byte_size(contents) - last) | parts]))
  end

  # The sorter of the run of `mix format`: one process for each file
  # calls it, so it has a name, and no link to the caller.
  defp sorter(config) do
    case Process.whereis(@sorter) do
      nil -> start_sorter(config)
      pid -> pid
    end
  end

  defp start_sorter(config) do
    profile = profile(config)

    unless :tailwind.nif() or File.exists?(:tailwind.bin_path()) do
      raise ArgumentError,
            "Tailwind.Formatter: tailwind.com is not installed. Run `mix tailwind.install`."
    end

    case :tailwind_sorter.start(profile, name: {:local, @sorter}) do
      {:ok, pid} ->
        pid

      {:error, {:already_started, pid}} ->
        pid

      {:error, reason} ->
        raise ArgumentError, "Tailwind.Formatter: the sorter does not start: #{inspect(reason)}"
    end
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
