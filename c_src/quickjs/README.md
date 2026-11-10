# QuickJS

The JavaScript engine of Fabrice Bellard and Charlie Gordon, for `@plugin`
and `@config`.

- Version: 2026-06-04
- Commit: `e585640` of <https://github.com/bellard/quickjs>
- License: MIT (`LICENSE`, and `licenses/quickjs.txt` of this repository)

The files are copies of the files of QuickJS, with no change:

| File | What |
|---|---|
| `quickjs.c`, `quickjs.h`, `quickjs-atom.h`, `quickjs-opcode.h` | The engine. |
| `libregexp.c`, `libregexp.h`, `libregexp-opcode.h` | Regular expressions. |
| `libunicode.c`, `libunicode.h`, `libunicode-table.h` | Unicode tables. |
| `cutils.c`, `cutils.h`, `dtoa.c`, `dtoa.h`, `list.h` | Small functions. |
| `VERSION`, `LICENSE` | The version and the license. |

`quickjs-libc.c`, `intl/`, `temporal/` and the tools of QuickJS are not
here: the program gives no file access and no network access to
JavaScript, and builds without `CONFIG_TEMPORAL_INTL`.

`c_src/js.c` is the only user of these files.

## A new version

```sh
sh scripts/sync-quickjs.sh DIR_OF_QUICKJS_CLONE
```

Then update the version and the commit in this file and in `NOTICE`.
