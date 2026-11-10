# QuickJS

The JavaScript engine of Fabrice Bellard and Charlie Gordon, for `@plugin`
and `@config`. It is the git submodule `c_src/quickjs`, with no change:

- Version: 2026-06-04
- Commit: `e585640` of <https://github.com/bellard/quickjs>
- License: MIT (`c_src/quickjs/LICENSE`, and `licenses/quickjs.txt` of
  this repository)

The file `UPSTREAM` has the version and the commit, and
`sh scripts/sync-upstream.sh check` compares them with the submodule.

Get the files after a clone:

```sh
git submodule update --init
```

The build uses these files of QuickJS only:

| File | What |
|---|---|
| `quickjs.c`, `quickjs.h`, `quickjs-atom.h`, `quickjs-opcode.h` | The engine. |
| `libregexp.c`, `libregexp.h`, `libregexp-opcode.h` | Regular expressions. |
| `libunicode.c`, `libunicode.h`, `libunicode-table.h` | Unicode tables. |
| `cutils.c`, `cutils.h`, `dtoa.c`, `dtoa.h`, `list.h` | Small functions. |
| `VERSION`, `LICENSE` | The version and the license. |

`quickjs-libc.c`, `intl/`, `temporal/` and the tools of QuickJS are not
in the program: the program gives JavaScript no network access and only
the file reads of its own host functions, and it builds without
`CONFIG_TEMPORAL_INTL`.

`c_src/js.c` is the only user of these files.

## A new version

```sh
sh scripts/sync-quickjs.sh COMMIT_OR_TAG
```

The script moves the submodule to that commit, and writes the version
and the commit in `UPSTREAM`. Then build, and run the tests: the plugin
cases (`CASES=tests/upstream/cli-js sh tests/cli.sh PROGRAM`) and
`cargo test`.
