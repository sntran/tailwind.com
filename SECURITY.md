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
It opens no network connection.

The program runs code of the project: the JavaScript modules of the
`@plugin` and `@config` rules of the stylesheet, and the modules that
they load (also from `node_modules`), as the official program does. Run
the program only on a project that you trust, as you do with
`npm install` or `mix deps.get`.

The JavaScript runs in the engine in the file (QuickJS), with less
access than in Node.js. This is a limit of the damage, and not a
sandbox for code that you do not trust:

- It can read each file that the program can read (`fs.readFileSync`,
  `fs.readdirSync`, `require`), and its output goes into the CSS.
- It cannot write files, start a process, open a network connection or
  read the environment (`process.env` has `NODE_ENV` only).
- It can use the CPU with no end, and memory until the limit of the
  engine (1 GB). The stack of JavaScript has a limit of 1 MB.

A stylesheet with no `@plugin` and no `@config` runs no code of the
project, and the program does not start the engine. The download
is in the Erlang module only, with TLS and the certificates of the
system, and with an optional SHA-256 (`config :tailwind, sha256: "..."`).
