// The loader of the modules of `@plugin` and `@config`.
//
// The host resolves a name to a path and reads the file. This file runs
// the modules:
//
// - A CommonJS module runs in a function with `module`, `exports`,
//   `require`, `__filename` and `__dirname`.
// - An ES module is a module of the engine. `require` of an ES module
//   gives its namespace.
// - An `import` of a CommonJS module gets a small ES module with the
//   exports of the CommonJS module (see `moduleSource`).
//
// The format of a file: `.mjs` is ESM, `.cjs` is CommonJS, `.json` is
// JSON. A `.js` file has the `type` of the nearest `package.json`. With no
// `type`, the file is CommonJS, unless it has a syntax error as CommonJS
// (`import` or `export`): then it is ESM.
//
// A TypeScript file (`.ts`, `.mts`, `.cts`) loses its types first
// (`typescript.ts`). `.mts` is ESM, `.cts` is CommonJS, and `.ts` is
// CommonJS unless it has the syntax of an ES module.
//
// The three plugins that the standalone program of Tailwind CSS has in
// its file (`@tailwindcss/forms`, `@tailwindcss/typography`,
// `@tailwindcss/aspect-ratio`) are in the program too: see `loadModule`.
import { deniedModule } from './denied'
import { host } from './host'
import assert from './node-assert'
import buffer from './node-buffer'
import fs, { promises as fsPromises } from './node-fs'
import os from './node-os'
import path, { dirname } from './node-path'
import url, { fileURLToPath, pathToFileURL, URL } from './node-url'
import util, { types as utilTypes } from './node-util'
import timers from './timers'
import { isTypeScript, stripTypes } from './typescript'

declare function __await<T>(promise: Promise<T>): T
declare function __compile(source: string, file: string): any
declare function __host(op: string, text?: string): string | undefined

type Format = 'esm' | 'cjs' | 'json' | 'unknown'
type CachedModule = { exports: any; loaded: boolean }

// The modules that are in the program. The value is `module.exports`.
// The entry of the bundle gives the modules of the package `tailwindcss`
// (`setTailwindModules`): the compatibility bundle has them from the
// source of Tailwind CSS, and the script bundle reads them from the
// package of the selected version.
const BUILTIN: Record<string, () => any> = {
  'tailwindcss/package.json': () => ({ name: 'tailwindcss', version: host('version') }),
  assert: () => assert,
  'assert/strict': () => assert.strict,
  buffer: () => buffer,
  fs: () => fs,
  'fs/promises': () => fsPromises,
  module: () => moduleModule,
  os: () => os,
  path: () => path,
  'path/posix': () => path,
  process: () => (globalThis as any).process,
  timers: () => timers,
  'timers/promises': () => timers.promises,
  url: () => url,
  util: () => util,
  'util/types': () => utilTypes,
}

// The modules of Node.js for a process or for the network: they load, and
// each of their functions throws (`denied.ts`).
for (let name of ['child_process', 'cluster', 'worker_threads']) {
  let module = deniedModule(name, 'process')
  BUILTIN[name] = () => module
}
for (let name of ['dgram', 'dns', 'http', 'http2', 'https', 'net', 'tls']) {
  let module = deniedModule(name, 'network')
  BUILTIN[name] = () => module
}

// Give the modules `tailwindcss/NAME` of the program.
export function setTailwindModules(modules: Record<string, () => any>) {
  for (let [name, load] of Object.entries(modules)) BUILTIN[name] = load
}

function builtinName(name: string): string | null {
  if (name.startsWith('node:')) name = name.slice(5)
  if (name.startsWith('tailwindcss/') && name.endsWith('.js')) name = name.slice(0, -3)
  return Object.hasOwn(BUILTIN, name) ? name : null
}

// The plugins that are in the program, as the standalone program of
// Tailwind CSS has them (`__tw_resolve` and `__tw_load` of
// `@tailwindcss/standalone`).
const EMBEDDED = ['@tailwindcss/forms', '@tailwindcss/typography', '@tailwindcss/aspect-ratio']
let embedded = new Map<string, any>()

function loadEmbedded(id: string): any {
  if (!embedded.has(id)) {
    let source = __host('asset', `plugin:${id}`)
    if (typeof source !== 'string') throw new Error(`The plugin '${id}' is not in this program.`)
    embedded.set(id, runProgramFile(source, `/$tailwindcss/${id}.js`))
  }
  return embedded.get(id)
}

let cache = new Map<string, CachedModule>()
// The `.js` files with no `type` that are ESM.
let detectedEsm = new Set<string>()

// The function of a CommonJS module. The script has the name of the file,
// so the stack of an error has the file, and an `import()` in the module
// resolves from the file.
function compileCommonJs(source: string, file: string): Function {
  if (source.startsWith('#!')) source = '//' + source
  return __compile(`(function (exports, require, module, __filename, __dirname) {${source}\n})`, file)
}

// Run the CommonJS text of a file of the program, and give its exports.
export function runProgramFile(source: string, file: string): any {
  let module = { exports: {} as any, id: file, filename: file, loaded: false }
  compileCommonJs(source, file).call(module.exports, module.exports, createRequire(file), module, file, dirname(file))
  module.loaded = true
  return module.exports
}

function runCommonJs(file: string, fn: Function): any {
  let module = { exports: {} as any, id: file, filename: file, loaded: false }
  let record: CachedModule = { exports: module.exports, loaded: false }
  // A cycle of `require` gets the exports that are there at that time.
  cache.set(file, record)
  Object.defineProperty(record, 'exports', { get: () => module.exports, enumerable: true })
  loading.unshift(file)
  try {
    fn.call(module.exports, module.exports, createRequire(file), module, file, dirname(file))
  } catch (error) {
    cache.delete(file)
    throw error
  } finally {
    loading.shift()
  }
  module.loaded = true
  record.loaded = true
  return module.exports
}

// Add the file, and the line when the engine gives it, to a syntax error.
function locate(error: unknown, file: string): unknown {
  if (error instanceof SyntaxError && !error.message.includes(file)) {
    let position = String(error.stack ?? '').match(/:(\d+):(\d+)\)?\s*$/m)
    let inFile = String(error.stack ?? '').includes(file)
    error.message += `\n${file}${position && inFile ? `:${position[1]}:${position[2]}` : ''}`
  }
  return error
}

function importEsm(file: string): any {
  try {
    return __await(import(file))
  } catch (error) {
    throw locate(error, file)
  }
}

// The source of a file, with no types for a TypeScript file.
let stripped = new Map<string, { original: string; source: string }>()

function read(file: string): { format: Format; source: string } {
  let result: { format: Format; source: string } = host('module.read', file)
  if (isTypeScript(file)) {
    let known = stripped.get(file)
    if (known === undefined || known.original !== result.source) {
      try {
        known = { original: result.source, source: stripTypes(result.source, file) }
      } catch (error) {
        throw locate(error, file)
      }
      stripped.set(file, known)
    }
    result.source = known.source
  }
  return result
}

// Load the file `file`, as `require` does.
function load(file: string): any {
  let cached = cache.get(file)
  if (cached) return cached.exports

  let { format, source } = read(file)

  if (format === 'json') {
    let exports = JSON.parse(source)
    cache.set(file, { exports, loaded: true })
    return exports
  }

  if (format === 'unknown') {
    if (detectedEsm.has(file)) {
      format = 'esm'
    } else {
      let fn: Function
      try {
        fn = compileCommonJs(source, file)
      } catch (error) {
        if (!(error instanceof SyntaxError)) throw error
        detectedEsm.add(file)
        return finishEsm(file)
      }
      return runCommonJs(file, fn)
    }
  }

  if (format === 'esm') return finishEsm(file)
  let fn: Function
  try {
    fn = compileCommonJs(source, file)
  } catch (error) {
    throw locate(error, file)
  }
  return runCommonJs(file, fn)
}

function finishEsm(file: string): any {
  let exports = importEsm(file)
  cache.set(file, { exports, loaded: true })
  return exports
}

// The files that load now, the last one first: the `Require stack` of an
// error of Node.js.
let loading: string[] = []

export function requireModule(name: string, from: string): any {
  let builtin = builtinName(name)
  if (builtin) return BUILTIN[builtin]()
  let file: string
  try {
    file = host('module.resolve', { name, base: from, kind: 'require' })
  } catch (error: any) {
    if (String(error.message).startsWith('Cannot find module')) {
      let stack = [from, ...loading.filter((file) => file !== from)]
      error.message += `\nRequire stack:\n${stack.map((file) => `- ${file}`).join('\n')}`
      error.code = 'MODULE_NOT_FOUND'
    }
    throw error
  }
  return load(file)
}

function createRequire(file: string) {
  let require: any = (name: string) => requireModule(String(name), file)
  require.resolve = (name: string) => {
    if (builtinName(name)) return name
    try {
      return host('module.resolve', { name, base: file, kind: 'require' })
    } catch (error: any) {
      if (String(error.message).startsWith('Cannot find module')) {
        error.message += `\nRequire stack:\n- ${file}`
        error.code = 'MODULE_NOT_FOUND'
      }
      throw error
    }
  }
  require.resolve.paths = () => null
  require.cache = {}
  require.main = undefined
  return require
}

// The module `module` of Node.js: `createRequire` for a file or a `file:` URL.
const moduleModule: any = function Module() {}
Object.assign(moduleModule, {
  createRequire(from: unknown) {
    let file = from instanceof URL || (typeof from === 'string' && from.startsWith('file:')) ? fileURLToPath(from) : from
    if (typeof file !== 'string' || !file.startsWith('/')) {
      let error: any = new TypeError(
        `The argument 'filename' must be a file URL object, file URL string, or absolute path string. Received ${typeof from === 'string' ? `'${from}'` : String(from)}`,
      )
      error.code = 'ERR_INVALID_ARG_VALUE'
      throw error
    }
    return createRequire(file)
  },
  isBuiltin: (name: string) => builtinName(String(name)) !== null && !String(name).startsWith('tailwindcss/'),
})
Object.defineProperty(moduleModule, 'builtinModules', {
  get: () => Object.keys(BUILTIN).filter((name) => !name.startsWith('tailwindcss/')),
  enumerable: true,
})
moduleModule.default = moduleModule

const IDENTIFIER = /^[A-Za-z_$][A-Za-z0-9_$]*$/
const RESERVED = new Set(
  'default break case catch class const continue debugger delete do else enum export extends false finally for function if import in instanceof new null return super switch this throw true try typeof var void while with yield let static implements interface package private protected public await'.split(
    ' ',
  ),
)

// An ES module that has the exports of a CommonJS value.
function facade(expression: string, exports: any): string {
  let names =
    exports !== null && (typeof exports === 'object' || typeof exports === 'function')
      ? Object.keys(exports).filter((name) => IDENTIFIER.test(name) && !RESERVED.has(name))
      : []
  let source = `const m = ${expression};\nexport default m;\n`
  if (names.length > 0) source += `export const { ${names.join(', ')} } = m;\n`
  return source
}

// The source of the ES module `name`, for the loader of the engine.
export function moduleSource(name: string): string {
  let builtin = builtinName(name)
  if (builtin) {
    return facade(`globalThis.__tw_builtin(${JSON.stringify(builtin)})`, BUILTIN[builtin]())
  }

  let { format, source } = read(name)
  if (format === 'esm' || detectedEsm.has(name)) return source
  if (format === 'json') return `export default ${source}\n`
  if (format === 'unknown' && !cache.has(name)) {
    try {
      compileCommonJs(source, name)
    } catch (error) {
      if (!(error instanceof SyntaxError)) throw error
      detectedEsm.add(name)
      return source
    }
  }
  return facade(`globalThis.__tw_load(${JSON.stringify(name)})`, load(name))
}

;(globalThis as any).__tw_builtin = (name: string) => BUILTIN[name]()
;(globalThis as any).__tw_load = load
// For the host: the name of a module of the program, or nothing.
;(globalThis as any).__tw_is_builtin = (name: string) => (builtinName(name) === null ? undefined : name)
// For `import.meta.url` (`js.c`).
;(globalThis as any).__tw_file_url = (file: string) => pathToFileURL(file).href

// `loadModule` of the compiler: the module of a `@plugin` or `@config`.
export async function loadModule(id: string, base: string, _hint: 'plugin' | 'config') {
  let builtin = builtinName(id)
  if (builtin) return { path: id, base, module: BUILTIN[builtin]() }

  // The standalone program gives its own copy of these plugins, also when
  // the project has one in `node_modules`. The path is the name, as there.
  if (EMBEDDED.includes(id)) {
    let exports = loadEmbedded(id)
    return { path: id, base: dirname(id), module: exports.default ?? exports }
  }

  let resolved: { path: string; base: string } = host('module.resolve', { name: id, base, kind: 'entry' })
  let file = resolved.path
  let exports = load(file)
  let module = exports
  if (exports !== null && typeof exports === 'object') {
    if (isNamespace(exports)) {
      // An ES module: its default export, as `@tailwindcss/node`.
      module = exports.default ?? exports
    } else if (exports.__esModule === true && 'default' in exports) {
      // A CommonJS file that is a bundle of an ES module. The standalone
      // program of Tailwind CSS (Bun) gives its `default`.
      module = exports.default ?? exports
    }
  }
  return { path: file, base: resolved.base, module }
}

function isNamespace(value: object): boolean {
  return Object.prototype.toString.call(value) === '[object Module]'
}
