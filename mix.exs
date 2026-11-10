defmodule TailwindCom.MixProject do
  use Mix.Project

  # tailwind.com as one Mix project:
  #
  # - native/tailwind/: the compiler, in Rust;
  # - src/: the Erlang module tailwind, which installs and runs the file;
  # - lib/: the Elixir module Tailwind and the Mix tasks, with the API of
  #   the Hex package tailwind;
  # - tests/: the tests of the Erlang and Elixir code, with ExUnit.
  #
  # The application has the name tailwind, so a Phoenix project can use
  # this project in place of the Hex package tailwind.
  @version "0.1.0"

  # The code uses Erlang/OTP 29 and Elixir 1.20: the versions of BEAM.com.
  if String.to_integer(System.otp_release()) < 29 do
    Mix.raise("tailwind.com needs Erlang/OTP 29 or later, not #{System.otp_release()}")
  end

  def project do
    [
      app: :tailwind,
      version: @version,
      elixir: "~> 1.20",
      erlc_paths: ["src"],
      test_paths: ["tests"],
      test_pattern: "*_test.exs",
      deps: [],
      description: "Tailwind CSS in one file: tailwind.com for Erlang and Elixir",
      package: [
        licenses: ["Apache-2.0"],
        links: %{"GitHub" => "https://github.com/sntran/tailwind.com"},
        files: ~w(lib src mix.exs rebar.config README.md LICENSE NOTICE)
      ]
    ]
  end

  def application do
    [
      extra_applications: [:logger, :inets, :ssl, :public_key, :crypto],
      mod: {Tailwind, []},
      env: [default: []]
    ]
  end
end
