# Roadmap

The sequence is by value for a Phoenix project.

1. **`@plugin` and `@config`.** They need a JavaScript engine and the
   plugin API of Tailwind CSS (`compat/plugin-api.ts`: `addBase`,
   `addUtilities`, `matchUtilities`, `addComponents`, `addVariant`,
   `matchVariant`, `theme()`). A new Phoenix project uses plugins for
   heroicons and daisyUI. The options: a small engine in the file
   (QuickJS builds with Cosmopolitan), or native ports of the common
   plugins. Until then, a project replaces a plugin with CSS
   (`@utility`, `@custom-variant`), or keeps the official program.
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
