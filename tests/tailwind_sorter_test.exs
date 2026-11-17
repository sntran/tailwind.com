defmodule TailwindSorterTest do
  # The sorter (`:tailwind_sorter`): the sort of classes of the official
  # Prettier plugin, with the design system of a profile. It runs with the
  # program (a port) and, with TAILWIND_NIF, as a NIF.
  use ExUnit.Case, async: false

  @moduletag :program

  setup do
    dir = Path.join(System.tmp_dir!(), "tailwind_sorter_test_#{System.unique_integer([:positive])}")
    File.mkdir_p!(Path.join(dir, "assets"))

    File.write!(Path.join(dir, "assets/app.css"), """
    @import "tailwindcss";
    @theme {
      --color-brand: #123456;
    }
    @utility tab-* {
      tab-size: --value(integer);
    }
    """)

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

    :ok
  end

  # A sorter for the profile `default`. It stops before the directory of
  # the test goes: an `on_exit` of a test runs before the one of `setup`.
  defp sorter do
    {:ok, sorter} = :tailwind_sorter.start_link(:default)
    Process.unlink(sorter)
    on_exit(fn -> :tailwind_sorter.stop(sorter) end)
    sorter
  end

  test "the arguments of a profile for the sort" do
    assert :tailwind_sorter.sort_args(
             ~w(--input=a.css --output=b.css --watch -i c.css --cwd d --minify --cwd=e)c
           ) == ~w(--input=a.css -i c.css --cwd d --cwd=e --sort-classes)c
  end

  test "attributes, templates and lists, with the theme of the profile" do
    sorter = sorter()

    assert :tailwind_sorter.sort(sorter, %{
             "attributes" => ["  tab-4 p-1 foo bg-brand flex p-1 ... "],
             "templates" => [["px-3 py-1 ", " p-1 m-2"]],
             "lists" => [["py-3", "p-1", "p-1"]]
           }) == %{
             "attributes" => ["foo flex bg-brand p-1 tab-4 ..."],
             "templates" => [["px-3 py-1 ", " m-2 p-1"]],
             "lists" => [["p-1", "py-3"]]
           }
  end

  test "the options of the plugin" do
    sorter = sorter()

    assert :tailwind_sorter.sort(sorter, %{
             "attributes" => [" p-1  p-1 a"],
             "preserveWhitespace" => true,
             "preserveDuplicates" => true
           }) == %{"attributes" => [" a  p-1 p-1"]}
  end

  test "a request that is not valid" do
    sorter = sorter()
    assert %{"error" => _} = :tailwind_sorter.sort(sorter, %{"attributes" => "p-1"})
  end

  test "more processes use one sorter" do
    sorter = sorter()

    results =
      1..20
      |> Task.async_stream(fn n ->
        {n, :tailwind_sorter.sort(sorter, %{"lists" => [["py-#{n}", "p-1"]]})}
      end)
      |> Enum.map(fn {:ok, result} -> result end)

    for {n, answer} <- results, do: assert(answer == %{"lists" => [["p-1", "py-#{n}"]]})
  end
end
