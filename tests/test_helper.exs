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

if program do
  Application.put_env(:tailwind, :test_program, Path.expand(program))
  ExUnit.start(exclude: if(has_packages.(Path.expand(program)), do: [], else: [:packages]))
else
  IO.puts(:stderr, "No program to test. Run `cargo build --release`, or set TAILWIND_COM.")
  ExUnit.start(exclude: [:program])
end
