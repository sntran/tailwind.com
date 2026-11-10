defmodule Mix.Tasks.Tailwind do
  @moduledoc """
  Invokes `tailwind.com` with the given args.

  Usage:

      $ mix tailwind TASK_OPTIONS PROFILE TAILWIND_ARGS

  Example:

      $ mix tailwind default --minify

  If `tailwind.com` is not installed, the task downloads it first.

  The task does not start the application of the project. Use
  `--runtime-config` to load `config/runtime.exs` before the run.

  ## Options

    * `--runtime-config` - load the runtime configuration before the run
  """

  @shortdoc "Invokes tailwind.com with the profile and args"

  use Mix.Task

  @impl true
  def run(args) do
    switches = [runtime_config: :boolean]
    {opts, remaining_args} = OptionParser.parse_head!(args, switches: switches)

    Mix.ensure_application!(:inets)
    Mix.ensure_application!(:ssl)

    if opts[:runtime_config] do
      Mix.Task.run("app.config")
    else
      Mix.Task.run("loadpaths")
      Application.ensure_all_started(:tailwind)
    end

    Mix.Task.reenable("tailwind")
    install_and_run(remaining_args)
  end

  defp install_and_run([profile | args] = all) do
    case Tailwind.install_and_run(String.to_atom(profile), args) do
      0 -> :ok
      status -> Mix.raise("`mix tailwind #{Enum.join(all, " ")}` exited with #{status}")
    end
  end

  defp install_and_run([]) do
    Mix.raise("`mix tailwind` expects the profile as argument")
  end
end
