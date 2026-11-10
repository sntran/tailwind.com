# Security

## Report a problem

Report a security problem in private, through a GitHub security
advisory: <https://github.com/sntran/tailwind.com/security/advisories/new>.
Do not open a public issue for it.

Give the version (`tailwind.com --version`), the system, and the steps
that show the problem.

## Versions

Only the newest release gets security fixes.

## What is in scope

- `tailwind.com`: the compiler, the scanner of source files, and the
  command line program.
- The Erlang module `tailwind` and the Elixir module `Tailwind`: the
  download of the file and the run of the file.

The program reads the files of the project and writes the output file.
It runs no code of the project: it has no JavaScript engine, so `@plugin`
and `@config` are errors. It opens no network connection. The download
is in the Erlang module only, with TLS and the certificates of the
system, and with an optional SHA-256 (`config :tailwind, sha256: "..."`).
