# The tests run the program of TAILWIND_COM: tailwind.com, or the program
# of "cargo build --release" (target/release/tailwind).
program =
  System.get_env("TAILWIND_COM") ||
    Enum.find(
      ["build/tailwind.com", "target/release/tailwind", "target/debug/tailwind"],
      &File.exists?/1
    )

if program do
  Application.put_env(:tailwind, :test_program, Path.expand(program))
  ExUnit.start()
else
  IO.puts(:stderr, "No program to test. Run `cargo build --release`, or set TAILWIND_COM.")
  ExUnit.start(exclude: [:program])
end
