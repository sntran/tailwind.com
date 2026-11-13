# The tests run the program of TAILWIND_COM: tailwind.com, or the program
# of "cargo build --release" (target/release/tailwind).
program =
  System.get_env("TAILWIND_COM") ||
    Enum.find(
      ["build/tailwind.com", "target/release/tailwind", "target/debug/tailwind"],
      &File.exists?/1
    )

# tailwind.com has the packages of the other versions of Tailwind CSS in
# its file. A program of `cargo build` has none: the tests with the tag
# `:packages` then do not run.
has_packages = fn program ->
  {command, args} =
    if String.ends_with?(program, ".com") and not match?({:win32, _}, :os.type()),
      do: {"sh", [program, "--version"]},
      else: {program, ["--version"]}

  match?(
    {_, 0},
    System.cmd(command, args, env: [{"TAILWINDCSS_VERSION", "4.1.0"}], stderr_to_stdout: true)
  )
end

# TAILWIND_NIF: the path of the NIF (target/release/libtailwind_nif, with
# no extension). The tests then run the program as a NIF, and the tests
# with the tag `:file` (the tests of the file only) do not run.
nif = System.get_env("TAILWIND_NIF")

if nif do
  :ok = :tailwind_nif.load(Path.expand(nif))
  Application.put_env(:tailwind, :nif, Path.expand(nif))
else
  Application.put_env(:tailwind, :nif, false)
end

nif_has_packages = fn ->
  match?(
    {0, _},
    :tailwind_nif.run(
      ["--version"],
      %{env: [{"TAILWINDCSS_VERSION", "4.1.0"}]},
      fn _, _, acc -> acc end,
      :ok
    )
  )
end

exclude = if nif, do: [:file], else: [:nif]

cond do
  nif ->
    if program, do: Application.put_env(:tailwind, :test_program, Path.expand(program))
    ExUnit.start(exclude: exclude ++ if(nif_has_packages.(), do: [], else: [:packages]))

  program ->
    Application.put_env(:tailwind, :test_program, Path.expand(program))

    ExUnit.start(
      exclude: exclude ++ if(has_packages.(Path.expand(program)), do: [], else: [:packages])
    )

  true ->
    IO.puts(:stderr, "No program to test. Run `cargo build --release`, or set TAILWIND_COM.")
    ExUnit.start(exclude: exclude ++ [:program])
end
