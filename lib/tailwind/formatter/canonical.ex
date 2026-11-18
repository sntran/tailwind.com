defmodule Tailwind.Formatter.Canonical do
  @moduledoc false
  # One run of `tailwind.com canonicalize --stream` for a profile, for all
  # the files of a run of `mix format`: the program makes its index of
  # the classes one time (about 0.6 s), and then each list takes a few
  # milliseconds. The process writes one list for each line of the
  # standard input of the program, and reads one line of the output for
  # each list. It runs the program as a NIF when the project has one, and
  # else as a port. It keeps the forms that it got, and stops when it
  # gets no request for `@idle_ms`.

  use GenServer

  @idle_ms 10_000

  @doc """
  The canonical form of each group of classes (classes with one space
  between them), from the process of the profile.
  """
  def forms(_profile, []), do: %{}

  def forms(profile, groups) do
    case GenServer.call(server(profile), {:forms, groups}, :infinity) do
      {:ok, forms} -> forms
      {:error, message} -> raise ArgumentError, message
    end
  end

  # The process of the profile: one process in the VM, for each caller.
  defp server(profile) do
    name = {:global, {__MODULE__, profile}}

    case GenServer.whereis(name) do
      pid when is_pid(pid) ->
        pid

      nil ->
        case GenServer.start(__MODULE__, profile, name: name) do
          {:ok, pid} -> pid
          {:error, {:already_started, pid}} -> pid
          {:error, reason} -> raise ArgumentError, "Tailwind.Formatter: #{inspect(reason)}"
        end
    end
  end

  @impl GenServer
  def init(profile) do
    %{args: args, cd: cd, env: env} = :tailwind.config_for(profile)
    env = Map.merge(%{~c"TAILWINDCSS_VERSION" => :tailwind.configured_version()}, env)
    css = Tailwind.Formatter.css_path(args)
    argv = [~c"canonicalize", ~c"--stream"] ++ if(css, do: [~c"--css", css], else: [])

    program =
      if :tailwind.nif() do
        {run, id} = :tailwind_nif.open(argv, %{cd: cd, env: Map.to_list(env)})
        {:nif, run, id}
      else
        unless File.exists?(:tailwind.bin_path()) do
          raise ArgumentError,
                "Tailwind.Formatter: tailwind.com is not installed. Run `mix tailwind.install`."
        end

        {exe, port_args} = :tailwind.command(argv)

        # The messages of plugins go to the standard error of the BEAM: a
        # port has one stream.
        port =
          Port.open({:spawn_executable, exe}, [
            {:args, port_args},
            {:cd, cd},
            {:env, Map.to_list(env)},
            :binary,
            :exit_status,
            :use_stdio,
            :hide
          ])

        {:port, port}
      end

    {:ok, %{profile: profile, program: program, buffer: "", errors: [], forms: %{}}, @idle_ms}
  end

  @impl GenServer
  def handle_call({:forms, groups}, _from, state) do
    new = groups |> Enum.uniq() |> Enum.reject(&Map.has_key?(state.forms, &1))

    case request(state, new) do
      {:ok, lines, state} ->
        forms = Map.merge(state.forms, Map.new(Enum.zip(new, lines)))
        state = %{state | forms: forms}
        {:reply, {:ok, Map.take(forms, groups)}, state, @idle_ms}

      {:error, message, state} ->
        {:stop, :normal, {:error, message}, state}
    end
  end

  @impl GenServer
  def handle_info(:timeout, state), do: {:stop, :normal, state}
  def handle_info(_message, state), do: {:noreply, state, @idle_ms}

  @impl GenServer
  def terminate(_reason, %{program: {:nif, run, _id}}), do: :tailwind_nif.stop(run)

  def terminate(_reason, %{program: {:port, port}}) do
    if Port.info(port), do: Port.close(port)
    :ok
  end

  # Write each group as a line, and read one line for each group.
  defp request(state, []), do: {:ok, [], state}

  defp request(state, groups) do
    data = Enum.map(groups, &[&1, ?\n])

    written =
      case state.program do
        {:nif, run, _} -> :tailwind_nif.write(run, data)
        {:port, port} -> Port.command(port, data)
      end

    if written, do: read(state, length(groups), []), else: {:error, failure(state, nil), state}
  end

  defp read(state, 0, lines), do: {:ok, Enum.reverse(lines), state}

  defp read(state, count, lines) do
    case String.split(state.buffer, "\n", parts: 2) do
      [line, rest] ->
        read(%{state | buffer: rest}, count - 1, [line | lines])

      [_] ->
        receive_output(state, count, lines)
    end
  end

  # Only the messages of the program: a request of another file waits in
  # the mailbox.
  defp receive_output(%{program: {:nif, _, id}} = state, count, lines) do
    receive do
      {:tailwind_nif, ^id, :stdout, data} -> read(%{state | buffer: state.buffer <> data}, count, lines)
      {:tailwind_nif, ^id, :stderr, data} -> read(%{state | errors: [state.errors, data]}, count, lines)
      {:tailwind_nif, ^id, :exit, status} -> {:error, failure(state, status), state}
    end
  end

  defp receive_output(%{program: {:port, port}} = state, count, lines) do
    receive do
      {^port, {:data, data}} -> read(%{state | buffer: state.buffer <> data}, count, lines)
      {^port, {:exit_status, status}} -> {:error, failure(state, status), state}
    end
  end

  defp failure(state, status) do
    messages =
      case IO.iodata_to_binary(state.errors) do
        "" -> "the exit status is #{inspect(status)}, and the messages are in the standard error"
        text -> text
      end

    "Tailwind.Formatter: the command canonicalize of the profile #{inspect(state.profile)} failed:\n" <>
      messages
  end
end
