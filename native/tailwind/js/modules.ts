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
import colors from 'tailwindcss-src/compat/colors'
import defaultTheme from 'tailwindcss-src/compat/default-theme'
import flattenColorPalette from 'tailwindcss-src/compat/flatten-color-palette'
import createPlugin from 'tailwindcss-src/plugin'
import { host } from './host'
import fs from './node-fs'
import path, { dirname } from './node-path'
import util from './node-util'

declare function __await<T>(promise: Promise<T>): T

type Format = 'esm' | 'cjs' | 'json' | 'unknown'
type CachedModule = { exports: any; loaded: boolean }

// The modules that are in the program. The value is `module.exports`.
const BUILTIN: Record<string, () => any> = {
  'tailwindcss/plugin': () => createPlugin,
  'tailwindcss/colors': () => colors,
  'tailwindcss/defaultTheme': () => defaultTheme,
  'tailwindcss/lib/util/flattenColorPalette': () => flattenColorPalette,
  'tailwindcss/package.json': () => ({ name: 'tailwindcss', version: host('version') }),
  fs: () => fs,
  path: () => path,
  util: () => util,
}

function builtinName(name: string): string | null {
  if (name.startsWith('node:')) name = name.slice(5)
  if (name.startsWith('tailwindcss/') && name.endsWith('.js')) name = name.slice(0, -3)
  return Object.hasOwn(BUILTIN, name) ? name : null
}

let cache = new Map<string, CachedModule>()
// The `.js` files with no `type` that are ESM.
let detectedEsm = new Set<string>()

function compileCommonJs(source: string): Function {
  if (source.startsWith('#!')) source = '//' + source
  return new Function('exports', 'require', 'module', '__filename', '__dirname', source)
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

function read(file: string): { format: Format; source: string } {
  return host('module.read', file)
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
        fn = compileCommonJs(source)
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
    fn = compileCommonJs(source)
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
  require.resolve = (name: string) =>
    builtinName(name) ?? host('module.resolve', { name, base: file, kind: 'require' })
  require.cache = {}
  return require
}

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
      compileCommonJs(source)
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

// `loadModule` of the compiler: the module of a `@plugin` or `@config`.
export async function loadModule(id: string, base: string, _hint: 'plugin' | 'config') {
  let builtin = builtinName(id)
  if (builtin) return { path: id, base, module: BUILTIN[builtin]() }

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
