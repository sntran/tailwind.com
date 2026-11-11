// The module rules of the standalone program, for Node.js.
// Usage: node --import bun-interop.mjs <the CLI> ...
//
// The standalone program of Tailwind CSS runs on Bun. Phoenix uses the standalone program.
// This file gives the npm CLI on Node.js the two rules of the standalone program that change
// the output. The compiler calls the hooks `globalThis.__tw_resolve` and `globalThis.__tw_load`
// before its own work (`compile.ts` of `@tailwindcss/node`).
//
// 1. A CommonJS module can have the exports `__esModule` and `default` (a bundle of an ES module).
//    Bun gives the `default` export of such a module as the default import. Node.js gives the full
//    `module.exports` object. Thus the npm CLI on Node.js does not find the plugin function in
//    such a file, for example `assets/vendor/daisyui.js` of Phoenix 1.8.
//
// 2. The standalone program has three plugins in its file: `@tailwindcss/forms`,
//    `@tailwindcss/typography` and `@tailwindcss/aspect-ratio`
//    (`packages/@tailwindcss-standalone/src/index.ts`). For these three names, `__tw_resolve`
//    gives the name itself, and `__tw_load` gives the copy of the program: a copy in the
//    `node_modules` directory of the project is not used. Here the copy of the program is the
//    package next to the CLI (the scratch directory of `run-cli.mjs`).
import { createRequire } from 'node:module'

const EMBEDDED = ['@tailwindcss/forms', '@tailwindcss/typography', '@tailwindcss/aspect-ratio']
// The packages next to the CLI (`process.argv[1]` is the file of the CLI).
const requireEmbedded = createRequire(process.argv[1])

globalThis.__tw_resolve = (id) => (EMBEDDED.includes(id) ? id : false)

globalThis.__tw_load = async (id) => {
  for (let name of EMBEDDED) {
    if (id.endsWith(name)) return requireEmbedded(name)
  }
  let namespace
  try {
    namespace = await import(id)
  } catch {
    return undefined
  }
  let exports = namespace.default
  if (exports !== null && typeof exports === 'object' && exports.__esModule === true && 'default' in exports) {
    return exports
  }
  return undefined
}
