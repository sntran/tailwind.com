defmodule TailwindNifTest do
  # The NIF (`:tailwind_nif`). These tests run when TAILWIND_NIF gives the
  # path of the NIF (tests/test_helper.exs). Then tests/tailwind_test.exs
  # also runs the program as a NIF.
  use ExUnit.Case, async: false

  import ExUnit.CaptureIO

  @moduletag :nif
  @version Tailwind.latest_version()

  setup do
    dir = Path.join(System.tmp_dir!(), "tailwind_nif_test_#{System.unique_integer([:positive])}")
    File.mkdir_p!(dir)
    File.write!(Path.join(dir, "app.css"), ~s(@import "tailwindcss";\n))
    File.write!(Path.join(dir, "index.html"), ~s(<p class="flex"></p>\n))
    previous = Application.get_all_env(:tailwind)

    on_exit(fn ->
      for {key, _} <- Application.get_all_env(:tailwind),
          do: Application.delete_env(:tailwind, key)

      for {key, value} <- previous, do: Application.put_env(:tailwind, key, value)
      File.rm_rf!(dir)
    end)

    {:ok, dir: dir}
  end

  defp collect(stream, data, acc), do: [{stream, data} | acc]

  defp text(output, stream) do
    output
    |> Enum.reverse()
    |> Enum.filter(&(elem(&1, 0) == stream))
    |> Enum.map_join(&elem(&1, 1))
  end

  test "the NIF is loaded" do
    assert :tailwind_nif.available()
    assert :tailwind.nif()
    assert {_, version} = :tailwind_nif.versions()
    assert version == @version
  end

  test "a run keeps the working directory and the environment of the VM", %{dir: dir} do
    cwd = File.cwd!()
    env = [{"TAILWIND_NIF_TEST", "1"}, {"NO_COLOR", "1"}]

    {status, _} =
      :tailwind_nif.run(~w(-i app.css -o out.css), %{cd: dir, env: env}, &collect/3, [])

    assert status == 0
    assert File.read!(Path.join(dir, "out.css")) =~ ".flex {"
    assert File.cwd!() == cwd
    assert System.get_env("TAILWIND_NIF_TEST") == nil
  end

  test "the standard output and the standard error are apart", %{dir: dir} do
    {0, output} = :tailwind_nif.run(~w(-i app.css), %{cd: dir}, &collect/3, [])
    assert text(output, :stdout) =~ ".flex {"
    assert text(output, :stderr) =~ "tailwindcss v#{@version}"
    refute text(output, :stderr) =~ ".flex {"
  end

  test "false and nil remove a variable", %{dir: dir} do
    for value <- [false, nil] do
      env = [{"TAILWINDCSS_VERSION", value}]
      {0, output} = :tailwind_nif.run(["--version"], %{cd: dir, env: env}, &collect/3, [])
      assert text(output, :stdout) =~ "(tailwindcss v#{@version})"
    end
  end

  test "run/2 sends the output to the group leader", %{dir: dir} do
    output =
      capture_io(fn -> assert :tailwind_nif.run(~w(-i app.css -o out.css), %{cd: dir}) == 0 end)

    assert output =~ "Done in "
  end

  test "load/1 is ok when the NIF is loaded" do
    assert :tailwind_nif.load("/no/such/libtailwind_nif") == :ok
  end

  test "with the NIF, there is no file to install", %{dir: dir} do
    Application.put_env(:tailwind, :path, Path.join(dir, "no/tailwind.com"))
    Application.put_env(:tailwind, :default, args: ~w(-i app.css -o out.css), cd: dir)
    assert Tailwind.bin_version() == {:ok, @version}
    capture_io(fn -> assert Tailwind.install_and_run(:default, []) == 0 end)
    refute File.exists?(Path.join(dir, "no/tailwind.com"))
    assert File.read!(Path.join(dir, "out.css")) =~ ".flex {"
  end

  test "the configuration nif: false uses the file", %{dir: dir} do
    Application.put_env(:tailwind, :nif, false)
    refute :tailwind.nif()
    Application.put_env(:tailwind, :path, Path.join(dir, "no/tailwind.com"))
    assert Tailwind.bin_version() == :error
  end
end
