defmodule TailwindFormatterTest do
  # The plugin of `mix format` (`Tailwind.Formatter`). It runs with the
  # program (a port) and, with TAILWIND_NIF, as a NIF.
  use ExUnit.Case, async: false

  @moduletag :program

  setup_all do
    dir = Path.join(System.tmp_dir!(), "tailwind_formatter_test_#{System.unique_integer([:positive])}")
    File.mkdir_p!(Path.join(dir, "assets"))
    File.write!(Path.join(dir, "assets/app.css"), ~s(@import "tailwindcss";\n))

    previous = Application.get_all_env(:tailwind)
    Application.put_env(:tailwind, :version, Tailwind.latest_version())
    Application.put_env(:tailwind, :path, Application.get_env(:tailwind, :test_program))
    Application.put_env(:tailwind, :default, args: ~w(--input=assets/app.css --output=priv/app.css), cd: dir)

    on_exit(fn ->
      # The sorter of the plugin has a name, and stays: stop it before its
      # directory goes.
      if pid = Process.whereis(Tailwind.Formatter.Sorter), do: :tailwind_sorter.stop(pid)

      for {key, _} <- Application.get_all_env(:tailwind),
          do: Application.delete_env(:tailwind, key)

      for {key, value} <- previous, do: Application.put_env(:tailwind, key, value)
      File.rm_rf!(dir)
    end)

    {:ok, dir: dir}
  end

  defp format(text, options \\ []) do
    Tailwind.Formatter.format(text, tailwind: Keyword.merge([profile: :default], options))
  end

  test "the features of the plugin" do
    assert Tailwind.Formatter.features([]) == [sigils: [:H], extensions: [".heex"]]
  end

  test "a class attribute in quotes" do
    assert format(~s(<div class="p-4 flex underline">x</div>)) ==
             ~s(<div class="flex p-4 underline">x</div>)

    assert format(~s(<div class='p-4 flex underline'></div>)) ==
             ~s(<div class='flex p-4 underline'></div>)

    assert format(~s(<.button id="p-4 flex" class="underline m-2" phx-click="go">Go</.button>)) ==
             ~s(<.button id="p-4 flex" class="m-2 underline" phx-click="go">Go</.button>)
  end

  test "the whitespace and the duplicates, as the Prettier plugin" do
    assert format(~s(<p class="  p-4\n    flex\n    m-2 flex  "></p>)) == ~s(<p class="m-2 flex p-4"></p>)

    assert format(~s(<p class=" p-4  flex flex"></p>), preserve_whitespace: true, preserve_duplicates: true) ==
             ~s(<p class=" flex  flex p-4"></p>)
  end

  test "the strings of an expression" do
    assert format(~s(<p class={"p-4 flex underline"}></p>)) == ~s(<p class={"flex p-4 underline"}></p>)

    assert format(~s(<p class={["p-4 flex", @active && "underline m-2", @kind == "primary"]}></p>)) ==
             ~s(<p class={["flex p-4", @active && "m-2 underline", @kind == "primary"]}></p>)
  end

  test "a string with interpolations, as a template literal" do
    assert format(~S(<p class={"p-4 #{@size} flex m-2"}></p>)) == ~S(<p class={"p-4 #{@size} m-2 flex"}></p>)
    assert format(~S(<p class={"m-2 flex#{@x} underline p-4"}></p>)) == ~S(<p class={"m-2 flex#{@x} p-4 underline"}></p>)
    # An interpolation with braces and a string in it.
    assert format(~S(<p class={"p-4 flex #{if @x, do: "a b", else: %{}} underline m-2"}></p>)) ==
             ~S(<p class={"flex p-4 #{if @x, do: "a b", else: %{}} m-2 underline"}></p>)
  end

  test "what the plugin does not change" do
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
      assert format(text) == text
    end
  end

  test "more attributes" do
    text = ~s(<.input class="p-4 flex" input_class="underline m-2" />)
    assert format(text) == ~s(<.input class="flex p-4" input_class="underline m-2" />)

    assert format(text, attributes: ["class", "input_class"]) ==
             ~s(<.input class="flex p-4" input_class="m-2 underline" />)
  end

  test "a ~H sigil, through the formatter of mix format", %{dir: dir} do
    File.write!(Path.join(dir, ".formatter.exs"), """
    [plugins: [Tailwind.Formatter], inputs: ["*.ex"], tailwind: [profile: :default]]
    """)

    file = Path.join(dir, "page.ex")

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
end
