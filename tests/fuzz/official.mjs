// The official Tailwind CSS for the fuzzer: the npm package of DIR
// (tests/fuzz/install.sh), in this process, with the module rules of the
// standalone program (tests/upstream/tools/bun-interop.mjs):
//
// - `@tailwindcss/forms`, `@tailwindcss/typography` and
//   `@tailwindcss/aspect-ratio` are the copies of DIR, as the standalone
//   program has them in its file;
// - a CommonJS module with the exports `__esModule` and `default` gives
//   its `default` export, as Bun does.
import fs from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import { pathToFileURL } from 'node:url'

const EMBEDDED = ['@tailwindcss/forms', '@tailwindcss/typography', '@tailwindcss/aspect-ratio']

// The license comment: the official compiler prints it when NODE_ENV is not
// "test", and the builder of the fuzzer never prints it.
const BANNER = /^\/\*! tailwindcss v[^*]*\*\/\n/

export async function officialTailwind(dir) {
  const root = path.resolve(dir)
  const require = createRequire(path.join(root, 'package.json'))
  const officialDir = path.dirname(require.resolve('tailwindcss/package.json'))
  // require.resolve gives the CommonJS file: its exports are the default
  // export of the module.
  const mod = await import(pathToFileURL(require.resolve('tailwindcss')).href)
  const official = mod.compile ? mod : mod.default
  const version = JSON.parse(fs.readFileSync(path.join(officialDir, 'package.json'), 'utf8')).version

  async function loadStylesheet(id) {
    let name
    if (id === 'tailwindcss') name = 'index.css'
    else if (id.startsWith('tailwindcss/')) name = id.slice('tailwindcss/'.length)
    else if (id.startsWith('./')) name = id.slice(2)
    else throw new Error(`Could not resolve '${id}'`)
    if (!name.endsWith('.css')) name += '.css'
    const file = path.join(officialDir, name)
    if (!fs.existsSync(file)) throw new Error(`Could not resolve '${id}'`)
    return { path: file, base: officialDir, content: fs.readFileSync(file, 'utf8') }
  }

  async function loadModule(id, base) {
    let file
    if (EMBEDDED.includes(id)) {
      file = require.resolve(id)
    } else if (id.startsWith('.') || path.isAbsolute(id)) {
      file = path.resolve(base, id)
    } else {
      file = createRequire(path.join(base, 'index.js')).resolve(id)
    }
    if (!fs.existsSync(file)) throw new Error(`Can't resolve '${id}' in '${base}'`)
    let module
    if (file.endsWith('.mjs')) {
      module = (await import(pathToFileURL(file).href)).default
    } else {
      const req = createRequire(file)
      // A new read of the file for each compile: the reducer runs a case
      // more than one time, and a plugin can keep state.
      delete req.cache[file]
      module = req(file)
    }
    if (module !== null && typeof module === 'object' && module.__esModule === true && 'default' in module) {
      module = module.default
    }
    return { path: file, base: path.dirname(file), module }
  }

  // compile(css) and build(candidates). The result is { css } or { error }.
  // With `first` (a number), two builds on the one compiler, as the watch
  // does: the first `first` candidates, then the other candidates. The
  // result is the text of the second build.
  async function run({ css, candidates, base, first }) {
    // The warnings of the compiler (console.warn) and the messages of
    // plugins (console.log) are not compared.
    const { warn, log } = console
    console.warn = () => {}
    console.log = () => {}
    try {
      const compiler = await official.compile(css, { base: base ?? officialDir, loadStylesheet, loadModule })
      if (first !== undefined) compiler.build(candidates.slice(0, first))
      return { css: compiler.build(first === undefined ? candidates : candidates.slice(first)).replace(BANNER, '') }
    } catch (e) {
      return { error: e?.message ?? String(e) }
    } finally {
      console.warn = warn
      console.log = log
    }
  }

  async function loadDesignSystem(css, base) {
    const { log } = console
    console.log = () => {}
    try {
      return await official.__unstable__loadDesignSystem(css, { base: base ?? officialDir, loadStylesheet, loadModule })
    } finally {
      console.log = log
    }
  }

  return { root, officialDir, version, run, loadDesignSystem }
}

// Write the files of a case (`files`: name to text, and `copy`: name to a
// path of this repository) into the directory DIR.
export function writeFiles(dir, { files = {}, copy = {} }, repository) {
  for (const [name, text] of Object.entries(files)) {
    fs.mkdirSync(path.dirname(path.join(dir, name)), { recursive: true })
    fs.writeFileSync(path.join(dir, name), text)
  }
  for (const [name, from] of Object.entries(copy)) {
    fs.mkdirSync(path.dirname(path.join(dir, name)), { recursive: true })
    fs.copyFileSync(path.join(repository, from), path.join(dir, name))
  }
}
