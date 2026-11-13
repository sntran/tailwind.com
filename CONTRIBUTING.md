# Contributing to tailwind.com

Thank you for your help. Bug reports, fixes, tests and documentation are
all welcome.

## Before you start

- For a difference from the output of Tailwind CSS, open an issue with
  the smallest input CSS and class list that shows it, the output of
  `tailwind.com`, and the output of the official `tailwindcss` program
  of the same version (`tailwind.com --version` gives the versions).
- For a new feature, read [`docs/ROADMAP.md`](docs/ROADMAP.md) first, and
  open an issue before a large change.
- For a security problem, see [`SECURITY.md`](SECURITY.md). Do not open
  a public issue.

## Build and test

See [`docs/BUILDING.md`](docs/BUILDING.md) and
[`docs/TESTING.md`](docs/TESTING.md). In short:

```sh
cargo test             # the compiler, against the record of Tailwind CSS
cargo build --release  # target/release/tailwind, for this computer
make                   # build/tailwind.com, for all the systems
make test              # all the tests: Rust, the program, TLC, Erlang and Elixir
```

CI runs the tests for each push to `main`, `feat/**`, `fix/**` and
`ci/**`, and for each pull request from a fork. A pull request from one
of these branches of this repository runs no job: the push of the branch
already tested the same commit. The fuzzer runs only when a push changes
a file that it tests (`docs/TESTING.md`).

For tests on a system that you do not have (Windows, macOS, Linux on
arm64, or FreeBSD, NetBSD and OpenBSD in a VM), use SSH to a runner of
CI. You need `ssh`, and `cargo` or `cloudflared` for the bridge to the
tunnel:

1. Make a key, one time:
   `sh scripts/remote_ssh_client.sh keygen ~/.ssh/tailwind_com_ci`.
2. Start the workflow "Remote SSH" (`.github/workflows/remote-ssh.yml`)
   by hand, with the runner, the VM, the line of
   `~/.ssh/tailwind_com_ci.pub`, and a branch `debug/...`. On a work
   branch, a push that changes the file `.github/remote-ssh.request`
   starts it too. The file has one `NAME=VALUE` for each line: `os`,
   `vm`, `key`, `branch`, `minutes`.
3. When the branch exists, run commands and copy files:

```sh
scripts/remote_ssh_client.sh debug/ssh ~/.ssh/tailwind_com_ci exec 'uname -a'
scripts/remote_ssh_client.sh debug/ssh ~/.ssh/tailwind_com_ci put build/tailwind.com tailwind.com
scripts/remote_ssh_client.sh debug/ssh ~/.ssh/tailwind_com_ci get out.log out.log
scripts/remote_ssh_client.sh debug/ssh ~/.ssh/tailwind_com_ci shell
```

The server is the OpenSSH server of the runner, as the user of the job,
behind a quick tunnel of Cloudflare. With a VM, the session is on the
Linux runner, and `ssh freebsd CMD` (or `netbsd`, `openbsd`) runs a
command in the VM. A session starts in the work directory of the job.
`touch "$REMOTE_SSH_STOP"` ends the job, and so does the limit of
minutes. The job deletes the branch at its end.

## Rules for a change

- The output of Tailwind CSS is the specification. A change of the
  output must keep `cargo test` at zero differences from the record in
  `tests/upstream`.
- Each file of `native/tailwind/src` follows one file of
  `packages/tailwindcss/src` of Tailwind CSS. Keep this relation: a new
  release of Tailwind CSS then has one clear place for each change.
- The plugin layer for `@plugin` and `@config` is the JavaScript of
  Tailwind CSS, and not a port. Do not edit the files of
  `native/tailwind/assets` (`compat.js`, `typescript.js`, `plugins/`)
  or `native/tailwind/c_src/quickjs`: scripts make the files, and the directory is a
  copy (see [`docs/BUILDING.md`](docs/BUILDING.md)).
- A part of Node.js in `native/tailwind/js` follows Node.js 22: add a
  case to `native/tailwind/tests/fixtures/node_api_cases.cjs` and
  record it with Node.js for each change.
- Each change has tests of its behavior, also for errors and limits, not
  only for the normal case.
- Keep each change small, and give it one purpose.
- Update the docs of the behavior that you change.

## The language of the docs

The docs, the code comments, the log text and the commit messages use
[ASD-STE100 Simplified Technical English](https://www.asd-ste100.org/):

- Write short sentences: 20 words or fewer for an instruction, 25 words
  or fewer for a description.
- Use the active voice and the present tense.
- Use one word for one meaning, and the same word in all the files.
- Give one instruction in each sentence, and start it with the verb.
- Do not use the "-ing" form of a verb, except in a technical name.

The messages of the program that come from Tailwind CSS (the errors of
the compiler, the help text) stay as Tailwind CSS has them.

## License

By a contribution, you agree that it is licensed under the Apache
License 2.0 (see [`LICENSE`](LICENSE)).
