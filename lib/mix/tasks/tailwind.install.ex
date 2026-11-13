defmodule Mix.Tasks.Tailwind.Install do
  @moduledoc """
  Installs `tailwind.com`.

      $ mix tailwind.install
      $ mix tailwind.install --if-missing

  By default, it installs #{Tailwind.latest_version()}. Set the version in
  the configuration:

      config :tailwind, :version, "#{Tailwind.latest_version()}"

  A URL as argument is the source of the download. `$version` in it is
  the version:

      $ mix tailwind.install https://example.com/v$version/tailwind.com

  ## Options

    * `--runtime-config` - load the runtime configuration before the
      install

    * `--if-missing` - install only if the file of `Tailwind.install_path/0`
      is not there, or if it has another version

    * `--no-assets` - accepted for compatibility with the Hex package
      `tailwind`. This task never writes asset files.
  """

  @shortdoc "Installs tailwind.com"

  use Mix.Task

  @impl true
  def run(args) do
    valid_options = [runtime_config: :boolean, if_missing: :boolean, assets: :boolean]

    {opts, base_url} =
      case OptionParser.parse_head!(args, strict: valid_options) do
        {opts, []} ->
          {opts, nil}

        {opts, [base_url]} ->
          {opts, base_url}

        {_, _} ->
          Mix.raise("""
          Invalid arguments to tailwind.install, expected one of:

              mix tailwind.install
              mix tailwind.install 'https://example.com/v$version/tailwind.com'
              mix tailwind.install --runtime-config
              mix tailwind.install --if-missing
          """)
      end

    if opts[:runtime_config], do: Mix.Task.run("app.config")
    # The priv directory of the application tailwind: the install path.
    Mix.Task.run("loadpaths")

    if opts[:if_missing] && latest_version?() do
      :ok
    else
      Mix.ensure_application!(:inets)
      Mix.ensure_application!(:ssl)
      Tailwind.install(base_url)
    end
  end

  # The file of the install path (not an older `_build/tailwind.com`), or
  # the NIF, has the configured version.
  defp latest_version? do
    version = Tailwind.configured_version()

    (:tailwind.nif() or File.exists?(Tailwind.install_path())) and
      match?({:ok, ^version}, Tailwind.bin_version())
  end
end
