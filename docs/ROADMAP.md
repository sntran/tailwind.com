# Roadmap

The sequence is by value for a Phoenix project.

1. **More of Node.js for JavaScript plugins.** `@plugin` and `@config`
   work with QuickJS and the plugin layer of Tailwind CSS. What is not
   there: TypeScript modules (`tailwind.config.ts`; the official program
   uses jiti for them), the modules `os`, `crypto` and `url`, `Buffer`,
   and the three plugins that the standalone program of Tailwind CSS
   has in its file (`@tailwindcss/forms`, `@tailwindcss/typography`,
   `@tailwindcss/aspect-ratio`): here they load from `node_modules`
   only. An interrupt for a plugin that does not end is also not there.
2. **File events for the watch on Windows** (`ReadDirectoryChangesW`).
   The watch polls there.
3. **`tailwind.wasm` in the BEAM**: run the compiler in the WebAssembly
   runtime of BEAM.com, so a Phoenix app on Cloudflare Workers or in a
   browser can build its CSS.
4. **The next release of Tailwind CSS.** `tests/upstream/tools` makes
   the record of a new tag. The differences of the record show what to
   port.
5. **The parts for editors**: `canonicalize`, the class list, the sort
   of classes (`getClassOrder`).
6. **FreeBSD, NetBSD and OpenBSD in CI**, as in BEAM.com.
