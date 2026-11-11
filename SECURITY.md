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
sandbox for code that you do not trust. The rules:

- **Read-only access to files.** The JavaScript can read each file that
  the program can read (`fs.readFileSync`, `fs.readdirSync`,
  `fs.promises`, `require`, `import`), and its output goes into the CSS.
  Each function of `fs` that writes, removes or opens a file for a
  write (`fs.writeFileSync`, `fs.mkdirSync`, `fs.rmSync`,
  `fs.createWriteStream`, ...) throws the error `fs.NAME is not
  permitted: the JavaScript of a plugin or config has read-only access
  to files.`
- **No network access.** `fetch`, `XMLHttpRequest`, `WebSocket`, and
  each function of the modules `http`, `https`, `http2`, `net`, `tls`,
  `dgram` and `dns` throw the error `NAME is not permitted: the
  JavaScript of a plugin or config has no network access.`
- **No new process.** Each function of `child_process`,
  `worker_threads` and `cluster`, and `process.exit`, `process.kill`
  and `process.chdir` throw the error `NAME is not permitted: the
  JavaScript of a plugin or config cannot start a process.`
- **It reads the environment.** `process.env` has the environment
  variables of the program, as in Node.js, and `process.argv` has its
  arguments. A value that the JavaScript reads can go into the CSS.
  Do not run the program with a secret in its environment on a project
  that you do not trust.
- **No clock for timers.** `setTimeout` runs its function after the
  current job, with no delay. A plugin cannot make the program wait.
- It can use the CPU with no end, and memory until the limit of the
  engine (1 GB). The stack of JavaScript has a limit of 1 MB.

The error of a rule has the code `ERR_ACCESS_DENIED`. The engine has no
function for a write, a connection or a process: the rules are not a
filter in front of such a function.

The file has JavaScript of other projects, which runs in the same
engine: the plugin layer of Tailwind CSS, the plugins
`@tailwindcss/forms`, `@tailwindcss/typography` and
`@tailwindcss/aspect-ratio`, and the TypeScript transform of Sucrase
(only for a TypeScript module). [`NOTICE`](NOTICE) names their versions.

A stylesheet with no `@plugin` and no `@config` runs no code of the
project, and the program does not start the engine. The download
is in the Erlang module only, with TLS and the certificates of the
system, and with an optional SHA-256 (`config :tailwind, sha256: "..."`).
