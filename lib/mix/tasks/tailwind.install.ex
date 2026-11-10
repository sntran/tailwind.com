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

    * `--if-missing` - install only if the file is not there, or if it
      has another version

    * `--no-assets` - accepted for compatibility with the Hex package
      `tailwind`. This task never writes asset files.
  """

  @shortdoc "Installs tailwind.com"
  @compile {:no_warn_undefined, Mix}

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

    if opts[:if_missing] && latest_version?() do
      :ok
    else
      if function_exported?(Mix, :ensure_application!, 1) do
        Mix.ensure_application!(:inets)
        Mix.ensure_application!(:ssl)
      end

      Mix.Task.run("loadpaths")
      Tailwind.install(base_url)
    end
  end

  defp latest_version? do
    version = Tailwind.configured_version()
    match?({:ok, ^version}, Tailwind.bin_version())
  end
end
