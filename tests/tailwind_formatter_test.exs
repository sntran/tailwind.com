defmodule TailwindFormatterTest do
  # The plugin of `mix format` (`Tailwind.Formatter`): the order of the
  # classes from a normal build, and the sort of the official Prettier
  # plugin. It runs with the program (a port) and, with TAILWIND_NIF, as a
  # NIF.
  use ExUnit.Case, async: false

  @moduletag :program

  @fixtures Path.expand("../native/tailwind/tests/fixtures", __DIR__)

  setup_all do
    dir =
      Path.join(
        System.tmp_dir!(),
        "tailwind_formatter_test_#{System.unique_integer([:positive])}"
      )

    File.mkdir_p!(Path.join(dir, "assets"))
    File.mkdir_p!(Path.join(dir, "lib"))
    File.write!(Path.join(dir, "assets/app.css"), ~s|@import "tailwindcss" source("../lib");\n|)

    previous = Application.get_all_env(:tailwind)
    Application.put_env(:tailwind, :version, Tailwind.latest_version())
    Application.put_env(:tailwind, :path, Application.get_env(:tailwind, :test_program))

    Application.put_env(:tailwind, :default,
      args: ~w(--input=assets/app.css --output=priv/app.css --watch),
      cd: dir
    )

    on_exit(fn ->
      for {key, _} <- Application.get_all_env(:tailwind),
          do: Application.delete_env(:tailwind, key)

      for {key, value} <- previous, do: Application.put_env(:tailwind, key, value)
      File.rm_rf!(dir)
    end)

    {:ok, dir: dir}
  end

  setup %{dir: dir} do
    # Each test has its own template, and the build must see it.
    for profile <- [:default, :fixture], do: :persistent_term.erase({Tailwind.Formatter, profile})
    File.rm_rf!(Path.join(dir, "lib"))
    File.mkdir_p!(Path.join(dir, "lib"))
    :ok
  end

  # Format a template that is a file of the project, as in `mix format`:
  # the build of the order sees it.
  defp format(dir, text, options \\ []) do
    File.write!(Path.join(dir, "lib/page.html.heex"), text)
    :persistent_term.erase({Tailwind.Formatter, Keyword.get(options, :profile, :default)})
    Tailwind.Formatter.format(text, tailwind: Keyword.merge([profile: :default], options))
  end

  test "the features of the plugin" do
    assert Tailwind.Formatter.features([]) == [sigils: [:H], extensions: [".heex"]]
  end

  test "a class attribute in quotes", %{dir: dir} do
    assert format(dir, ~s(<div class="p-4 flex underline">x</div>)) ==
             ~s(<div class="flex p-4 underline">x</div>)

    assert format(dir, ~s(<div class='p-4 flex underline'></div>)) ==
             ~s(<div class='flex p-4 underline'></div>)

    assert format(
             dir,
             ~s(<.button id="p-4 flex" class="underline m-2" phx-click="go">Go</.button>)
           ) ==
             ~s(<.button id="p-4 flex" class="m-2 underline" phx-click="go">Go</.button>)
  end

  test "group, peer and the classes of the project come first", %{dir: dir} do
    assert format(dir, ~s(<div class="p-4 group card flex peer group-hover:underline"></div>)) ==
             ~s(<div class="group card peer flex p-4 group-hover:underline"></div>)
  end

  test "the whitespace and the duplicates, as the Prettier plugin", %{dir: dir} do
    assert format(dir, ~s(<p class="  p-4\n    flex\n    m-2 flex  "></p>)) ==
             ~s(<p class="m-2 flex p-4"></p>)

    assert format(dir, ~s(<p class=" p-4  flex flex"></p>),
             preserve_whitespace: true,
             preserve_duplicates: true
           ) ==
             ~s(<p class=" flex  flex p-4"></p>)
  end

  test "the strings of an expression", %{dir: dir} do
    assert format(dir, ~s(<p class={"p-4 flex underline"}></p>)) ==
             ~s(<p class={"flex p-4 underline"}></p>)

    assert format(
             dir,
             ~s(<p class={["p-4 flex", @active && "underline m-2", @kind == "primary"]}></p>)
           ) ==
             ~s(<p class={["flex p-4", @active && "m-2 underline", @kind == "primary"]}></p>)
  end

  test "a string with interpolations, as a template literal", %{dir: dir} do
    assert format(dir, ~S(<p class={"p-4 #{@size} flex m-2"}></p>)) ==
             ~S(<p class={"p-4 #{@size} m-2 flex"}></p>)

    assert format(dir, ~S(<p class={"m-2 flex#{@x} underline p-4"}></p>)) ==
             ~S(<p class={"m-2 flex#{@x} p-4 underline"}></p>)

    # An interpolation with braces and a string in it.
    assert format(
             dir,
             ~S(<p class={"p-4 flex #{if @x, do: "a b", else: %{}} underline m-2"}></p>)
           ) ==
             ~S(<p class={"flex p-4 #{if @x, do: "a b", else: %{}} m-2 underline"}></p>)
  end

  test "what the plugin does not change", %{dir: dir} do
    for text <- [
          ~s(<!-- <div class="p-4 flex"> -->),
          ~s(<%= "p-4 flex" %>),
          ~s(<script>let s = '<div class="p-4 flex">'</script>),
          ~s(<style>.a { content: "p-4 flex" }</style>),
          ~s(<p>{"p-4 flex"}</p>),
          ~s(<p {@rest} title="p-4 flex"></p>),
          ~S(<p class={"p-4\\n flex"}></p>),
          ~s|<p class={~w(p-4 flex)}></p>|
        ] do
      assert format(dir, text) == text
    end
  end

  test "more attributes", %{dir: dir} do
    text = ~s(<.input class="p-4 flex" input_class="underline m-2" />)
    assert format(dir, text) == ~s(<.input class="flex p-4" input_class="underline m-2" />)

    assert format(dir, text, attributes: ["class", "input_class"]) ==
             ~s(<.input class="flex p-4" input_class="m-2 underline" />)
  end

  test "a ~H sigil, through the formatter of mix format", %{dir: dir} do
    File.write!(Path.join(dir, ".formatter.exs"), """
    [plugins: [Tailwind.Formatter], inputs: ["lib/*.ex"], tailwind: [profile: :default]]
    """)

    file = Path.join(dir, "lib/page.ex")

    File.write!(file, """
    defmodule Page do
      def render(assigns) do
        ~H\"\"\"
        <div class="p-4 flex underline">{@title}</div>
        \"\"\"
      end
    end
    """)

    {formatter, _options} = Mix.Tasks.Format.formatter_for_file(file, root: dir)
    assert formatter.(File.read!(file)) =~ ~s(<div class="flex p-4 underline">{@title}</div>)
  end

  test "the canonical form of the class lists", %{dir: dir} do
    :persistent_term.erase({Tailwind.Formatter, :canonical, :default})

    assert format(dir, ~s|<div class="py-3 p-1 px-3 flex"></div>\n|, canonicalize: true) ==
             ~s|<div class="flex p-3"></div>\n|

    # The strings of `class={…}`, one class for more classes, and the
    # classes of the project first. A list with `...` and a string with an
    # interpolation get the sort only.
    text = """
    <div class={["w-[16px]  h-[16px]", @open && "mt-2 mb-2 card"]}>
      <p class="mt-[1px] mb-[1px] ...">x</p>
      <p class={"pt-2 pb-2 \#{@x} flex"}>y</p>
    </div>
    """

    assert format(dir, text, canonicalize: true) == """
           <div class={["size-[16px]", @open && "card my-2"]}>
             <p class="mt-[1px] mb-[1px] ...">x</p>
             <p class={"pt-2 pb-2 \#{@x} flex"}>y</p>
           </div>
           """

    # With no option, only the sort.
    assert format(dir, ~s|<div class="py-3 p-1 px-3"></div>\n|) ==
             ~s|<div class="p-1 px-3 py-3"></div>\n|
  end

  test "a build that fails gives its messages", %{dir: dir} do
    File.write!(
      Path.join(dir, "assets/broken.css"),
      ~s(@import "tailwindcss";\n.a { @apply nothing-here; }\n)
    )

    Application.put_env(:tailwind, :broken, args: ~w(--input=assets/broken.css), cd: dir)

    error =
      assert_raise ArgumentError, fn ->
        format(dir, ~s(<p class="p-4 flex"></p>), profile: :broken)
      end

    assert error.message =~ "nothing-here"
  after
    Application.delete_env(:tailwind, :broken)
    :persistent_term.erase({Tailwind.Formatter, :broken})
  end

  # A profile with the stylesheet `css`, and a file with the classes, one on
  # each line: the build sees each class.
  defp fixture(dir, css, classes) do
    fixture = Path.join(dir, "fixture")
    File.rm_rf!(fixture)
    File.mkdir_p!(fixture)
    File.write!(Path.join(fixture, "classes.txt"), Enum.join(Enum.uniq(classes), "\n"))

    css = String.replace(css, ~s(@import "tailwindcss"), ~s|@import "tailwindcss" source(none)|)
    File.write!(Path.join(fixture, "app.css"), css <> ~s(\n@source "./classes.txt";\n))
    Application.put_env(:tailwind, :fixture, args: ~w(--input=app.css), cd: fixture)
    :persistent_term.erase({Tailwind.Formatter, :fixture})
    places = Tailwind.Formatter.order(:fixture)
    fn class -> Map.get(places, Tailwind.Formatter.escape(class)) end
  end

  # The sort of the tests of Tailwind CSS (`sort.test.ts`): no place first,
  # then by place.
  defp default_sort(classes, place) do
    Enum.sort_by(classes, fn class ->
      case place.(class) do
        nil -> {0, 0}
        rank -> {1, rank}
      end
    end)
  end

  test "the order of a build is the order of the official compiler", %{dir: dir} do
    records =
      Path.join(@fixtures, "class_order.json")
      |> File.read!()
      |> JSON.decode!()
      |> Enum.filter(&String.contains?(&1["css"], ~s(@import "tailwindcss")))

    assert length(records) >= 180

    different =
      records
      |> Enum.group_by(& &1["css"])
      |> Enum.flat_map(fn {css, records} ->
        place = fixture(dir, css, Enum.flat_map(records, & &1["classes"]))

        for record <- records,
            order = Map.new(record["order"], fn [class, place] -> {class, place} end),
            default_sort(record["classes"], place) != default_sort(record["classes"], &order[&1]),
            do: record["classes"]
      end)

    # `[--:1]` and `w-(--)` are candidates of Tailwind CSS that the scanner
    # does not extract from a file: the build has no rule for them.
    assert Enum.all?(different, fn classes -> "[--:1]" in classes or "w-(--)" in classes end),
           inspect(different)

    assert length(different) <= 3
  end

  test "the sort is the sort of the official Prettier plugin", %{dir: dir} do
    records = Path.join(@fixtures, "class_sort.json") |> File.read!() |> JSON.decode!()

    classes =
      Enum.flat_map(records["attributes"], &String.split(&1["input"])) ++
        Enum.flat_map(records["lists"], & &1["input"]) ++
        Enum.flat_map(records["templates"], fn record ->
          Enum.flat_map(record["input"], &String.split/1)
        end)

    place = fixture(dir, ~s(@import "tailwindcss";), classes)

    for record <- records["attributes"] do
      options =
        case record["options"] do
          "preserveWhitespace" -> [collapse: false]
          "preserveDuplicates" -> [remove_duplicates: false]
          _ -> []
        end

      assert Tailwind.Formatter.sort_attribute(record["input"], place, options) ==
               record["output"],
             inspect(record)
    end

    for record <- records["lists"] do
      {list, _} =
        Tailwind.Formatter.sort_class_list(
          record["input"],
          place,
          record["options"] != "preserveDuplicates"
        )

      assert list == record["output"], inspect(record)
    end

    for record <- records["templates"] do
      assert Tailwind.Formatter.sort_template(record["input"], place, []) == record["output"],
             inspect(record)
    end
  end
end
