// The module interop of the standalone program, for Node.js.
// Usage: node --import bun-interop.mjs <the CLI> ...
//
// The standalone program of Tailwind CSS runs on Bun. Phoenix uses the standalone program.
// A CommonJS module can have the exports `__esModule` and `default` (a bundle of an ES module).
// Bun gives the `default` export of such a module as the default import. Node.js gives the full
// `module.exports` object. Thus the npm CLI on Node.js does not find the plugin function in
// such a file, for example `assets/vendor/daisyui.js` of Phoenix 1.8.
//
// The compiler calls the hook `globalThis.__tw_load` before its own import.
// This hook makes the result of Bun for such a module. All other modules load as usual.
globalThis.__tw_load = async (id) => {
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
