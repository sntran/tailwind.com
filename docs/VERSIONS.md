# The versions of Tailwind CSS

One `tailwind.com` compiles for each version of Tailwind CSS 4, from
4.0.0 to the newest version of the file `PACKAGES`. The variable
`TAILWINDCSS_VERSION` selects the version:

```sh
sh tailwind.com -i app.css -o out.css                             # 4.3.5
TAILWINDCSS_VERSION=4.1.18 sh tailwind.com -i app.css -o out.css   # 4.1.18
```

In a Phoenix project, `config :tailwind, version: "4.3.0"` does this:
the modules `Tailwind` and `tailwind` give the configured version to the
program.

## The two engines

The program has two compilers.

| | The native engine | The script engine |
|---|---|---|
| What | The Rust compiler of this repository | The official compiler of the npm package `tailwindcss` (`dist/lib.js`), with no change, in the QuickJS of the program |
| Versions | The version of the file `UPSTREAM` (4.3.5), and each older version that has all its switches: now 4.0.0 to 4.3.4 ([`SWITCHES.md`](SWITCHES.md)) | Each version whose package the program can find |
| A new version of Tailwind CSS needs | A port of the changes to Rust (`docs/SYNC.md`) | One line in the file `PACKAGES` |
| Speed, Phoenix project | 10 ms | 69 ms |
| Speed, 1000 HTML files | 224 ms | 1.31 s |

With no setting, the versions of the native engine use the native
engine, and each other version uses the script engine. Now each version
of the file `PACKAGES` is a version of the native engine: the script
engine is for a newer version that the port does not have yet, and for a
comparison. The Rust compiler
has one behavior for each release that it serves: a switch for each
change ([`SWITCHES.md`](SWITCHES.md)). For an older version than its
own, it reads `theme.css` and `preflight.css` from the package of that
version.

The plugin layer of the Rust compiler (for `@plugin` and `@config`) is
the JavaScript of Tailwind CSS. The program has the plugin layer of each
version that the Rust compiler serves, made from the source of that
version with no change ([`SWITCHES.md`](SWITCHES.md), "The plugin
layer"). So a stylesheet with plugins uses the Rust compiler too: a new
Phoenix project (Tailwind CSS 4.3.3, with daisyUI) builds in 120 ms, and
in 200 ms with the script engine. `TAILWIND_COM_ENGINE` can
be `native` or `script`: `script` with the version of the native engine
runs the official compiler of that version too.

`tailwind.com --version` shows the selection:

```
$ TAILWINDCSS_VERSION=4.1.0 sh tailwind.com --version
tailwind.com v0.1.0 (tailwindcss v4.1.0)
$ TAILWIND_COM_ENGINE=script TAILWINDCSS_VERSION=4.1.0 sh tailwind.com --version
tailwind.com v0.1.0 (tailwindcss v4.1.0, script engine)
```

Why this works: the compiler of Tailwind CSS 4 is one JavaScript file
with no dependency. The parts of the official program that are native
code are the scanner of the source files and Lightning CSS. The program
has both in Rust already. So a version of Tailwind CSS is a small set of
files, and not a new program:

- `dist/lib.js`: the compiler;
- `dist/plugin.js`, `dist/colors.js`, `dist/default-theme.js` and
  `dist/flatten-color-palette.js`: the modules that a plugin or a config
  loads (`tailwindcss/plugin`, ...);
- `theme.css`, `preflight.css` and `utilities.css`;
- `package.json` and `LICENSE`.

`index.css` is the file of the repository of Tailwind CSS, as in the
standalone program. It is the same file in each version 4.

## Where the packages are

An APE file is also a ZIP file. The build puts the packages of the
versions of `PACKAGES` into the ZIP directory `tailwindcss/` of
`tailwind.com`, and the program reads them there (`/zip/tailwindcss/`).

```sh
unzip -l tailwind.com 'tailwindcss/*'
```

The compilers of two versions next to each other are almost the same
text. So the file does not have each package in full:

- The newest version is a directory with its files
  (`tailwindcss/4.3.5/dist/lib.js`, ...).
- Each older version is one patch file (`tailwindcss/4.3.4.patch`): the
  differences of its files from the next newer version. To read a file
  of 4.3.3, the program reads the file of 4.3.5, and applies the patch
  of 4.3.4 and then the patch of 4.3.3.

A patch has, for each file, the bytes that are new and the places of
the bytes that are a copy from the other version
(`native/tailwind/src/patch.rs`). A patch file is 0.2 KB to 57 KB
before deflate, and about 6 KB with deflate. The 48 versions from 4.0.0
to 4.3.5 are 0.37 MB of the file. As 48 full packages they were 4.1 MB.
The chain of patches costs no time that a measure shows: the oldest
version (47 patches) starts as fast from the patches as from a full
package.

The tool `tools/twpatch` makes the patch files in the build. It reads
each version back from the patches and compares it with the package,
so a patch that gives another file stops the build. `twpatch unpack
STORE VERSION OUT` writes the full files of one version.

The program first looks in the directory of the variable
`TAILWIND_COM_PACKAGES`. That directory has, for each version, a
directory with the files of the package (`4.2.4/package.json`,
`4.2.4/dist/lib.js`, ...) or a patch file. Use it for a version that the
file does not have, for a program with no ZIP directory
(`tailwind.wasm`, a program of `cargo build`), or to try a changed
package:

```sh
sh scripts/packages.sh build/packages 4.2.4
TAILWIND_COM_PACKAGES=build/packages TAILWINDCSS_VERSION=4.2.4 target/release/tailwind -i app.css
```

`scripts/packages.sh` downloads a package from the npm registry and
checks its SHA-256 against the file `PACKAGES`.

## A new version of Tailwind CSS

```sh
sh scripts/packages.sh --record 4.3.6
```

This command writes the version and the SHA-256 of its package into
`PACKAGES`. The next build of `tailwind.com` has the version. No code
changes. The script engine then compiles for 4.3.6, and the native
engine stays at its version until the port (`docs/SYNC.md`).

## What is the same as the official program, and what is not

With the script engine, the compiler is the official compiler. For each
version, these are the same as the official program of that version:

- the CSS of a build with no optimizer;
- the plugin API, `@plugin` and `@config`, and the modules
  `tailwindcss/plugin`, `tailwindcss/colors`, `tailwindcss/defaultTheme`
  and `tailwindcss/lib/util/flattenColorPalette`;
- the error messages of the compiler.

These parts are the code of this program, for each version:

- **The command line program** (the options, the messages, the watch).
- **The scanner of source files**: the `tailwindcss-oxide` crate of the
  version of `UPSTREAM`. An older version of Tailwind CSS has an older
  scanner, which can find other candidates in a source file in rare
  cases.
- **The optimizer** (`--minify`, `--optimize`): the Lightning CSS of the
  version of `UPSTREAM`. An older release of Tailwind CSS has an older
  Lightning CSS, so the optimized text can be different in small
  details: for example `var(--tw-blur, )` in place of
  `var(--tw-blur,  )`.
- **Source maps** (`--map`): the compiler of Tailwind CSS has source maps
  from version 4.1.6. For an earlier version, `--map` is an error.

Tailwind CSS 3 is another program (a PostCSS plugin with many
dependencies). It does not work, and the program says so.

## How the script engine stays the same as V8

The official programs run the compiler in V8 (Node.js) or JavaScriptCore
(Bun), and this program runs it in QuickJS. JavaScript is a standard, but
two things needed work:

- **`Array.prototype.sort`.** Tailwind CSS sorts variants with a compare
  function that is not consistent: for two variants with no breakpoint
  value, `compare(a, z)` and `compare(z, a)` are both 1. Then the result
  depends on the sort algorithm. The runtime has the algorithm of V8
  (`native/tailwind/js/sort.ts`).
- **`Intl.ListFormat`.** Tailwind CSS uses it for one error message, and
  QuickJS has no `Intl`. The runtime has it for English
  (`native/tailwind/js/intl.ts`).

`String.prototype.localeCompare` of QuickJS compares code units, and
V8 uses the rules of a locale. Tailwind CSS uses it only in its
`canonicalize` code, which a build does not call.

The tests:

- `tests/v8.mjs` compares the sort with Node.js: each call of the
  compare function, for 4,000 random arrays and 12 compare functions.
- With the package of 4.3.5, the script engine gives the recorded output
  of the official program for the 150 projects of `tests/upstream`, and
  the same 24 MB of CSS as the native engine for 75,082 candidates.
- The fuzzer (`FUZZ_ENGINE=script`) compares the script engine with the
  official compiler in Node.js, for the same version, on random
  stylesheets, classes, plugins and configs. CI does this for 4.0.17,
  4.1.18, 4.2.4, 4.3.0 and 4.3.5.
- `tests/versions.sh` builds a project with a plugin with each version of
  `PACKAGES`, on each system of CI.
