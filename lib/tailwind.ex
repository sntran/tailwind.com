defmodule Tailwind do
  @moduledoc """
  Installs and runs `tailwind.com`, the Tailwind CSS compiler in one file.

  This module has the API of the Hex package `tailwind`, so a Phoenix
  project can use it with no change of its code:

      config :tailwind,
        version: "4.3.5",
        my_app: [
          args: ~w(
            --input=assets/css/app.css
            --output=priv/static/assets/css/app.css
          ),
          cd: Path.expand("..", __DIR__)
        ]

  ## Configuration

    * `:version` - the version of Tailwind CSS. One `tailwind.com` has
      each version of Tailwind CSS 4: the version of its Rust compiler
      (`latest_version/0`) is the fastest, and each other version uses
      the official compiler of that version, in the JavaScript engine
      of the file
    * `:version_check` - `false` stops the check of the version at the
      start of the application
    * `:path` - the path of `tailwind.com` (default: `tailwind.com` in the
      priv directory of the application `tailwind`, see `install_path/0`)
    * `:url` - the URL of the download; `$program` in it is the version
      of this package, and `$version` is the version of Tailwind CSS
    * `:sha256` - the SHA-256 that the download must have
    * `:nif` - the program as a NIF (`:tailwind_nif`): `true` (the
      default) uses the NIF when it is in `priv`, `false` uses the file
      only, and a path uses the NIF at that path. With the NIF, there is
      no download, and no process of the operating system

  One file runs on Linux, macOS, Windows, FreeBSD, NetBSD and OpenBSD, on
  x86_64 and aarch64. Thus there is no `:target`.
  """

  use Application
  require Logger

  @doc false
  def start(_, _) do
    if Application.get_env(:tailwind, :version_check, true) do
      unless Application.get_env(:tailwind, :version) do
        Logger.warning("""
        The tailwind version is not configured. Set it in your config files:

            config :tailwind, :version, "#{latest_version()}"
        """)
      end
    end

    Supervisor.start_link([], strategy: :one_for_one, name: __MODULE__.Supervisor)
  end

  @doc false
  def latest_version, do: to_string(:tailwind.latest_version())

  @doc """
  Returns the configured version of Tailwind CSS.
  """
  def configured_version do
    Application.get_env(:tailwind, :version, latest_version())
  end

  @doc """
  Returns the configuration for the given profile.

  Raises if the profile does not exist.
  """
  def config_for!(profile) when is_atom(profile) do
    Application.get_env(:tailwind, profile) ||
      raise ArgumentError, """
      unknown tailwind profile. Make sure the profile is defined in your config/config.exs file, such as:

          config :tailwind,
            version: "#{latest_version()}",
            #{profile}: [
              args: ~w(
                --input=assets/css/app.css
                --output=priv/static/assets/css/app.css
              ),
              cd: Path.expand("..", __DIR__)
            ]
      """
  end

  @doc """
  Returns the path of `tailwind.com`.

  The path of the configuration (`config :tailwind, path: ...`) has
  priority. Then `tailwind.com` in the priv directory of the application
  `tailwind`, when it is there: `install/1` writes it there. The default
  is `tailwind.com` in the `_build` directory of the project, where the
  versions before 0.3.0 installed it.
  """
  def bin_path do
    priv =
      case :code.priv_dir(:tailwind) do
        {:error, _} -> nil
        dir -> Path.join(dir, "tailwind.com")
      end

    cond do
      path = Application.get_env(:tailwind, :path) ->
        Path.expand(path)

      priv && File.regular?(priv) ->
        priv

      Code.ensure_loaded?(Mix.Project) ->
        Path.join(Path.dirname(Mix.Project.build_path()), "tailwind.com")

      true ->
        Path.expand("_build/tailwind.com")
    end
  end

  @doc """
  Returns the version of Tailwind CSS that the installed file uses for
  the configured version.

  Returns `{:ok, version}`, or `:error` when the file is not there or
  does not have that version.
  """
  def bin_version do
    with_path(fn ->
      case :tailwind.bin_version() do
        {:ok, version} -> {:ok, to_string(version)}
        :error -> :error
      end
    end)
  end

  @doc """
  Runs the given profile with `args`.

  The given args are appended to the args of the profile. The output goes
  to the standard output. Returns the exit status.
  """
  def run(profile, extra_args) when is_atom(profile) and is_list(extra_args) do
    config = config_for!(profile)
    args = config[:args] || []

    if args == [] and extra_args == [] do
      raise "no arguments passed to tailwind"
    end

    # A list of paths (the NODE_PATH of a new Phoenix project) stays a
    # list: `:tailwind.config_for/1` joins it with the path separator.
    env =
      config
      |> Keyword.get(:env, %{})
      |> Enum.reject(fn {_, value} -> is_nil(value) end)
      |> Map.new(fn
        {key, value} when is_list(value) -> {to_charlist(key), value}
        {key, value} -> {to_charlist(key), to_charlist(value)}
      end)

    check_version_once()

    Application.put_env(:tailwind, :__run__,
      args: Enum.map(args ++ extra_args, &to_charlist/1),
      cd: to_charlist(config[:cd] || File.cwd!()),
      env: env
    )

    with_path(fn -> :tailwind.run(:__run__, []) end)
  end

  @doc """
  Installs, if not available, and then runs `tailwind.com`. With the
  NIF (`:nif`), there is nothing to install.

  Returns the same as `run/2`.
  """
  def install_and_run(profile, args) do
    :tailwind.nif() || File.exists?(bin_path()) || start_unique_install_worker()
    run(profile, args)
  end

  @doc """
  The default URL of the download. `$program` is the version of this
  package.
  """
  def default_base_url, do: to_string(:tailwind.default_base_url())

  @doc """
  Returns the path where `install/1` writes `tailwind.com`.

  The path of the configuration (`config :tailwind, path: ...`) has
  priority. The default is `tailwind.com` in the priv directory of the
  application `tailwind`: a release has that directory, so a program can
  run `tailwind.com` at run time (in BEAM.com, from `/zip`).
  """
  def install_path do
    case Application.get_env(:tailwind, :path) do
      nil -> to_string(:tailwind.install_path())
      path -> Path.expand(path)
    end
  end

  @doc """
  Installs `tailwind.com` with `configured_version/0`, to `install_path/0`.
  """
  def install(base_url \\ nil) do
    url = base_url || Application.get_env(:tailwind, :url, default_base_url())
    Application.put_env(:tailwind, :version, configured_version())

    with_path(install_path(), fn ->
      case :tailwind.install(to_charlist(url)) do
        :ok ->
          :ok

        {:error, reason} ->
          raise "could not install tailwind.com from #{url}: #{inspect(reason)}"
      end
    end)
  end

  # The check of the version, at the first run of the VM (not at the start
  # of the application: a program that compiles no CSS loads no NIF and
  # runs no program).
  defp check_version_once do
    key = {__MODULE__, :version_checked}

    if Application.get_env(:tailwind, :version_check, true) and
         not :persistent_term.get(key, false) do
      :persistent_term.put(key, true)
      configured_version = configured_version()

      case bin_version() do
        {:ok, ^configured_version} ->
          :ok

        {:ok, version} ->
          Logger.warning("""
          Outdated tailwind version. Expected #{configured_version}, got #{version}. \
          Please run `mix tailwind.install` or update the version in your config files.\
          """)

        :error ->
          if File.exists?(bin_path()) or :tailwind.nif() do
            Logger.warning("""
            The installed tailwind.com has no Tailwind CSS #{configured_version}. \
            Please run `mix tailwind.install` or update the version in your config files.\
            """)
          end
      end
    end
  end

  # The Erlang module reads the path from the application environment.
  defp with_path(fun), do: with_path(bin_path(), fun)

  defp with_path(path, fun) do
    previous = Application.fetch_env(:tailwind, :path)
    Application.put_env(:tailwind, :path, to_charlist(path))

    try do
      fun.()
    after
      case previous do
        {:ok, path} -> Application.put_env(:tailwind, :path, path)
        :error -> Application.delete_env(:tailwind, :path)
      end
    end
  end

  # Only one process installs the file when many profiles start together.
  defp start_unique_install_worker do
    ref =
      __MODULE__.Supervisor
      |> Supervisor.start_child(
        Supervisor.child_spec({Task, &install/0}, restart: :transient, id: __MODULE__.Installer)
      )
      |> case do
        {:ok, pid} -> pid
        {:error, {:already_started, pid}} -> pid
      end
      |> Process.monitor()

    receive do
      {:DOWN, ^ref, _, _, _} -> :ok
    end
  end
end
