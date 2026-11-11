# Roadmap

The sequence is by value for a Phoenix project.

1. **More of Node.js for JavaScript plugins.** `@plugin` and `@config`
   work with QuickJS and the plugin layer of Tailwind CSS, also with
   TypeScript modules, the three plugins of the standalone program, and
   a part of Node.js (`Buffer`, `URL`, timers, `fs`, `os`, `url`,
   `util`, `assert`, `module`). What is not there: the modules `crypto`,
   `stream`, `events` and `zlib`, the module `tailwindcss` itself for a
   config that imports a value from it (the script engine has it),
   TypeScript files with JSX or decorators, and an interrupt for a
   plugin that does not end.
2. **`tailwind.wasm` in the BEAM**: run the compiler in the WebAssembly
   runtime of BEAM.com, so a Phoenix app on Cloudflare Workers or in a
   browser can build its CSS.
3. **The next release of Tailwind CSS.** One line in `PACKAGES` gives
   the new version to the script engine (`docs/VERSIONS.md`). For the
   native engine, `tests/upstream/tools` makes the record of a new tag,
   and the differences of the record show what to port.
4. **A faster script engine.** The engine reads the text of the compiler
   at each start (about 25 ms). QuickJS can save a compiled form
   (bytecode): the build can make it for each package of the file.
5. **A package from the npm registry at install time.** For a version
   that the installed `tailwind.com` does not have, `mix
   tailwind.install` can get the package and put it in
   `TAILWIND_COM_PACKAGES`.

## Not planned

- `canonicalize`, the class list and the sort of classes for editors
  (`canonicalize-candidates.ts`, `intellisense.ts` of Tailwind CSS).
