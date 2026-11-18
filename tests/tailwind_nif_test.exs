defmodule TailwindNifTest do
  # The NIF (`:tailwind_nif`). These tests run when TAILWIND_NIF gives the
  # path of the NIF (tests/test_helper.exs). Then tests/tailwind_test.exs
  # also runs the program as a NIF.
  use ExUnit.Case, async: false

  import ExUnit.CaptureIO

  @moduletag :nif
  @version Tailwind.latest_version()

  # A stylesheet whose build takes some seconds: 400,000 candidates.
  @slow ~s(@import "tailwindcss" source\(none\);\n) <>
          ~s[@source inline("{,hover:,focus:,md:,lg:}{p,m,px,py,mx,my,w,h}-{0..10000}");\n]

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

  # The text with no color codes. As in the official program, the
  # messages have colors when the variable CI is set.
  defp plain(text), do: String.replace(text, ~r/\e\[[0-9;]*m/, "")

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
    assert plain(text(output, :stderr)) =~ "tailwindcss v#{@version}"
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

  test "an input that nests too deep is an error, and the VM goes on", %{dir: dir} do
    # 20,000 nested rules: before, the stack of the run overflowed, and
    # the VM stopped.
    File.write!(
      Path.join(dir, "deep.css"),
      String.duplicate(".a {", 20_000) <> "color: red;" <> String.duplicate("}", 20_000)
    )

    env = [{"NO_COLOR", "1"}]
    {1, output} = :tailwind_nif.run(~w(-i deep.css), %{cd: dir, env: env}, &collect/3, [])
    assert text(output, :stderr) =~ "RangeError: Maximum call stack size exceeded"

    # 200,000 nested `var()`: the official program compiles it.
    value = String.duplicate("var(--x, ", 200_000) <> "1px" <> String.duplicate(")", 200_000)
    File.write!(Path.join(dir, "var.css"), "@tailwind utilities;\n.a { width: #{value}; }\n")
    {0, output} = :tailwind_nif.run(~w(-i var.css), %{cd: dir}, &collect/3, [])
    assert text(output, :stdout) =~ value

    {0, _} = :tailwind_nif.run(~w(-i app.css -o out.css), %{cd: dir}, &collect/3, [])
  end

  test "the option timeout stops a build", %{dir: dir} do
    File.write!(Path.join(dir, "slow.css"), @slow)
    start = System.monotonic_time(:millisecond)

    assert {:timeout, _} =
             :tailwind_nif.run(
               ~w(-i slow.css -o slow.out.css),
               %{cd: dir, timeout: 100},
               &collect/3,
               []
             )

    assert System.monotonic_time(:millisecond) - start < 2_000

    capture_io(fn ->
      assert :tailwind_nif.run(~w(-i slow.css), %{cd: dir, timeout: 0}) == :timeout
    end)

    # No message of the run comes after the timeout, and the run writes no
    # output.
    Process.sleep(1_000)
    refute_received {:tailwind_nif, _, _, _}
    refute File.exists?(Path.join(dir, "slow.out.css"))

    # A run that ends before the time gives its status.
    assert {0, _} = :tailwind_nif.run(~w(-i app.css), %{cd: dir, timeout: 60_000}, &collect/3, [])

    assert_raise ArgumentError, fn ->
      :tailwind_nif.run(~w(-i app.css), %{cd: dir, timeout: -1})
    end
  end

  test "a build stops when its process ends", %{dir: dir} do
    File.write!(Path.join(dir, "slow.css"), @slow)

    pid =
      spawn(fn ->
        :tailwind_nif.run(~w(-i slow.css -o slow.out.css), %{cd: dir}, &collect/3, [])
      end)

    Process.sleep(100)
    Process.exit(pid, :kill)
    # The garbage collector frees the resource of the run, and the build
    # stops before its output (a full build takes some seconds).
    Process.sleep(5_000)
    refute File.exists?(Path.join(dir, "slow.out.css"))
  end

  test "a run of open/2 reads its standard input", %{dir: dir} do
    {run, id} = :tailwind_nif.open(~w(canonicalize --stream --css app.css), %{cd: dir})
    assert :tailwind_nif.write(run, "py-3 p-1 px-3\n")
    assert_receive {:tailwind_nif, ^id, :stdout, "p-3\n"}, 10_000
    assert :tailwind_nif.write(run, ["w-[16px] h-[16px]", ?\n])
    assert_receive {:tailwind_nif, ^id, :stdout, "size-[16px]\n"}, 10_000
    :ok = :tailwind_nif.close(run)
    assert_receive {:tailwind_nif, ^id, :exit, 0}, 10_000
    refute :tailwind_nif.write(run, "flex\n")

    # A stop ends a run that waits for its input.
    {run, id} = :tailwind_nif.open(~w(canonicalize --stream --css app.css), %{cd: dir})
    :ok = :tailwind_nif.stop(run)
    refute_receive {:tailwind_nif, ^id, _, _}, 500
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
