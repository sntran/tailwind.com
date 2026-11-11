defmodule TailwindTest do
  use ExUnit.Case, async: false

  import ExUnit.CaptureIO

  @version Tailwind.latest_version()

  setup do
    dir = Path.join(System.tmp_dir!(), "tailwind_test_#{System.unique_integer([:positive])}")
    File.mkdir_p!(Path.join(dir, "assets"))
    File.mkdir_p!(Path.join(dir, "lib"))

    File.write!(Path.join(dir, "assets/app.css"), """
    @import "tailwindcss" source(none);
    @source "../lib";
    """)

    File.write!(
      Path.join(dir, "lib/page.html.heex"),
      ~s(<div class="flex underline md:p-4"></div>\n)
    )

    previous = Application.get_all_env(:tailwind)

    Application.put_env(:tailwind, :version, @version)
    Application.put_env(:tailwind, :path, Application.get_env(:tailwind, :test_program))

    Application.put_env(:tailwind, :default,
      args: ~w(--input=assets/app.css --output=priv/app.css),
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

  describe "the configuration" do
    test "config_for!/1 gives the profile" do
      assert Tailwind.config_for!(:default)[:args] ==
               ~w(--input=assets/app.css --output=priv/app.css)
    end

    test "config_for!/1 raises for an unknown profile" do
      assert_raise ArgumentError, ~r/unknown tailwind profile/, fn ->
        Tailwind.config_for!(:none)
      end
    end

    test "configured_version/0 is the version of the configuration, or the latest" do
      Application.put_env(:tailwind, :version, "4.0.0")
      assert Tailwind.configured_version() == "4.0.0"
      Application.delete_env(:tailwind, :version)
      assert Tailwind.configured_version() == @version
    end

    test "bin_path/0 is the path of the configuration, or tailwind.com in _build" do
      Application.put_env(:tailwind, :path, "/x/tailwind.com")
      assert Tailwind.bin_path() == "/x/tailwind.com"
      Application.delete_env(:tailwind, :path)
      assert Path.basename(Tailwind.bin_path()) == "tailwind.com"
      assert Path.basename(Path.dirname(Tailwind.bin_path())) == "_build"
    end

    test "the Erlang module reads the same configuration", %{dir: dir} do
      assert :tailwind.configured_version() == to_charlist(@version)
      config = :tailwind.config_for(:default)
      assert config.cd == to_charlist(dir)
      assert config.args == [~c"--input=assets/app.css", ~c"--output=priv/app.css"]
      assert_raise ErlangError, fn -> :tailwind.config_for(:none) end
    end
  end

  describe "the program" do
    @describetag :program

    test "bin_version/0 is the version of Tailwind CSS" do
      assert Tailwind.bin_version() == {:ok, @version}
    end

    test "bin_version/0 is :error when the file is not there" do
      Application.put_env(:tailwind, :path, "/no/such/tailwind.com")
      assert Tailwind.bin_version() == :error
    end

    test "run/2 builds the CSS of the profile", %{dir: dir} do
      output = capture_io(fn -> assert Tailwind.run(:default, []) == 0 end)
      assert output =~ "tailwindcss v#{@version}"
      css = File.read!(Path.join(dir, "priv/app.css"))
      assert css =~ ".flex {"
      assert css =~ ".underline {"
      assert css =~ ".md\\:p-4 {"
      refute css =~ ".hidden {"
    end

    test "run/2 adds the extra arguments", %{dir: dir} do
      capture_io(fn -> assert Tailwind.run(:default, ["--minify"]) == 0 end)
      css = File.read!(Path.join(dir, "priv/app.css"))
      assert css =~ ".flex{display:flex}"
    end

    test "run/2 gives the exit status of an error", %{dir: dir} do
      File.write!(Path.join(dir, "assets/app.css"), ".a { @apply does-not-exist; }\n")
      output = capture_io(fn -> assert Tailwind.run(:default, []) == 1 end)
      assert output =~ "Cannot apply unknown utility class `does-not-exist`"
    end

    test "run/2 gives the environment of the profile to the program", %{dir: dir} do
      Application.put_env(:tailwind, :default,
        args: ~w(--input=assets/app.css --output=priv/app.css),
        cd: dir,
        env: %{"NO_COLOR" => "1", "IGNORED" => nil}
      )

      output = capture_io(fn -> assert Tailwind.run(:default, []) == 0 end)
      refute output =~ "\e["
    end

    test "run/2 raises with no arguments" do
      Application.put_env(:tailwind, :empty, args: [])
      assert_raise RuntimeError, ~r/no arguments/, fn -> Tailwind.run(:empty, []) end
    end

    test "a watch stops when its process ends", %{dir: dir} do
      parent = self()

      pid =
        spawn(fn ->
          capture_io(fn ->
            send(parent, :started)
            Tailwind.run(:default, ["--watch"])
          end)
        end)

      assert_receive :started, 5_000
      output = Path.join(dir, "priv/app.css")
      wait_until(fn -> File.exists?(output) end)

      # An edit of a source file gives a new output.
      File.write!(Path.join(dir, "lib/page.html.heex"), ~s(<div class="hidden"></div>\n))
      wait_until(fn -> File.read!(output) =~ ".hidden {" end)

      # The end of the process closes the port, and the program stops:
      # an edit gives no new output.
      Process.exit(pid, :kill)
      Process.sleep(1_500)
      File.write!(Path.join(dir, "lib/page.html.heex"), ~s(<div class="italic"></div>\n))
      Process.sleep(1_500)
      refute File.read!(output) =~ ".italic {"
    end

    test "install/1 copies the file from a URL, and makes it executable", %{dir: dir} do
      source = Application.get_env(:tailwind, :test_program)
      target = Path.join(dir, "_build/tailwind.com")
      Application.put_env(:tailwind, :path, target)

      assert Tailwind.install("file://" <> source) == :ok
      assert File.read!(target) == File.read!(source)
      assert Bitwise.band(File.stat!(target).mode, 0o111) != 0
      assert Tailwind.bin_version() == {:ok, @version}
    end

    test "install/1 checks the SHA-256 of the configuration", %{dir: dir} do
      source = Application.get_env(:tailwind, :test_program)
      target = Path.join(dir, "_build/tailwind.com")
      Application.put_env(:tailwind, :path, target)

      Application.put_env(:tailwind, :sha256, String.duplicate("0", 64))

      assert_raise RuntimeError, ~r/sha256_mismatch/, fn ->
        Tailwind.install("file://" <> source)
      end

      refute File.exists?(target)

      sha256 = :crypto.hash(:sha256, File.read!(source)) |> Base.encode16(case: :upper)
      Application.put_env(:tailwind, :sha256, sha256)
      assert Tailwind.install("file://" <> source) == :ok
    end

    test "install/1 replaces $version in the URL", %{dir: dir} do
      source = Application.get_env(:tailwind, :test_program)
      versioned = Path.join(dir, "v#{@version}")
      File.mkdir_p!(versioned)
      File.cp!(source, Path.join(versioned, "tailwind.com"))
      Application.put_env(:tailwind, :path, Path.join(dir, "_build/tailwind.com"))

      assert Tailwind.install("file://#{dir}/v$version/tailwind.com") == :ok
    end

    test "install/1 raises when the URL has no file", %{dir: dir} do
      Application.put_env(:tailwind, :path, Path.join(dir, "_build/tailwind.com"))

      assert_raise RuntimeError, ~r/could not install/, fn ->
        Tailwind.install("file:///no/such/file")
      end
    end

    test "install_and_run/2 runs the installed file" do
      capture_io(fn -> assert Tailwind.install_and_run(:default, []) == 0 end)
    end
  end

  describe "the Mix tasks" do
    @describetag :program

    test "mix tailwind runs the profile", %{dir: dir} do
      capture_io(fn -> assert Mix.Tasks.Tailwind.run(["default", "--minify"]) == :ok end)
      assert File.read!(Path.join(dir, "priv/app.css")) =~ ".flex{display:flex}"
    end

    test "mix tailwind raises for an error of the program", %{dir: dir} do
      File.write!(Path.join(dir, "assets/app.css"), ".a { @apply does-not-exist; }\n")

      capture_io(fn ->
        assert_raise Mix.Error, ~r/exited with 1/, fn -> Mix.Tasks.Tailwind.run(["default"]) end
      end)
    end

    test "mix tailwind needs a profile" do
      assert_raise Mix.Error, ~r/expects the profile/, fn -> Mix.Tasks.Tailwind.run([]) end
    end

    test "mix tailwind.install --if-missing keeps a file of the configured version" do
      # A wrong URL shows that there is no download.
      assert Mix.Tasks.Tailwind.Install.run(["--if-missing", "file:///no/such/file"]) == :ok
    end

    test "mix tailwind.install installs from the URL", %{dir: dir} do
      source = Application.get_env(:tailwind, :test_program)
      target = Path.join(dir, "_build/tailwind.com")
      Application.put_env(:tailwind, :path, target)
      assert Mix.Tasks.Tailwind.Install.run(["file://" <> source]) == :ok
      assert File.exists?(target)
    end

    test "mix tailwind.install refuses unknown arguments" do
      assert_raise Mix.Error, ~r/Invalid arguments/, fn ->
        Mix.Tasks.Tailwind.Install.run(["a", "b"])
      end
    end
  end

  defp wait_until(fun, tries \\ 100) do
    cond do
      fun.() -> :ok
      tries == 0 -> flunk("the condition did not become true")
      true -> Process.sleep(100) && wait_until(fun, tries - 1)
    end
  end
end
