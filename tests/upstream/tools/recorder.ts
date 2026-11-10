// This file is the recorder for the compile oracle.
// The script instrument.mjs copies it into packages/tailwindcss/src/__oracle.ts.
// The recorder is active only when the environment variable TW_ORACLE_DIR is set.
// Each worker process appends events to its own file: events-<pid>.jsonl.
// The script merge-suite.mjs joins the events into one line for each compile call.
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const DIR = process.env.TW_ORACLE_DIR
const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..')

let counter = 0
let out: string | null = null
let vitestExpect: any = null
let optimize: any = null

function emit(event: any) {
  if (out === null) {
    fs.mkdirSync(DIR!, { recursive: true })
    out = path.join(DIR!, `events-${process.pid}.jsonl`)
  }
  fs.appendFileSync(out, JSON.stringify(event) + '\n')
}

function message(err: unknown) {
  return err instanceof Error ? err.message : String(err)
}

function state() {
  try {
    let s = vitestExpect.getState()
    return {
      file: s.testPath ? path.relative(REPO, s.testPath).split(path.sep).join('/') : null,
      test: s.currentTestName ?? null,
    }
  } catch {
    return { file: null, test: null }
  }
}

// ---------------------------------------------------------------------------
// The source text of a module that a test gives to `loadModule`.

// A JavaScript expression that makes the value again. A function is its
// own source text, so a function that uses variables of the test is not
// complete: gen-plugin-suite.mjs runs the text and removes such records.
function serialize(value: any, seen: Set<any>): string {
  if (value === null) return 'null'
  switch (typeof value) {
    case 'undefined':
      return 'undefined'
    case 'string':
      return JSON.stringify(value)
    case 'number':
      return Object.is(value, -0) ? '-0' : String(value)
    case 'boolean':
      return String(value)
    case 'function': {
      // `plugin.withOptions(…)`: gen-plugin-suite.mjs makes plugin.ts keep
      // its two arguments.
      if (value.__oracleSource) {
        let [pluginFunction, configFunction] = value.__oracleSource
        return `plugin.withOptions(${serialize(pluginFunction, seen)}, ${serialize(configFunction, seen)})`
      }
      let text = functionExpression(value)
      let keys = Object.keys(value)
      if (keys.length === 0) return text
      let fields = keys.map((key) => `${JSON.stringify(key)}: ${serialize(value[key], seen)}`)
      return `Object.assign(${text}, {${fields.join(', ')}})`
    }
    case 'object': {
      if (seen.has(value)) throw new Error('the module has a cycle')
      seen.add(value)
      let text: string
      if (Array.isArray(value)) {
        text = `[${value.map((item) => serialize(item, seen)).join(', ')}]`
      } else if (value instanceof RegExp) {
        text = String(value)
      } else {
        let proto = Object.getPrototypeOf(value)
        if (proto !== Object.prototype && proto !== null) throw new Error('the module has an object of a class')
        let fields = Object.keys(value).map((key) => `${JSON.stringify(key)}: ${serialize(value[key], seen)}`)
        text = `{${fields.join(', ')}}`
      }
      seen.delete(value)
      return text
    }
    default:
      throw new Error(`the module has a ${typeof value}`)
  }
}

function parses(text: string) {
  try {
    new Function(text)
    return true
  } catch {
    return false
  }
}

function functionExpression(fn: Function): string {
  let text = Function.prototype.toString.call(fn)
  if (text.includes('[native code]')) throw new Error('the module has a native function')
  if (parses(`return (${text}\n)`)) return `(${text}\n)`
  // A method of an object literal: `name(api) { … }`.
  if (parses(`return ({ ${text}\n })`)) return `({ ${text}\n })[${JSON.stringify(fn.name)}]`
  throw new Error('the module has a function with no source text')
}

// The modules that a recorded module can use, by the end of the import
// path in the test file.
const KNOWN_IMPORTS: [RegExp, string][] = [
  [/(^|\/)plugin$/, 'tailwindcss/plugin'],
  [/(^|\/)colors$/, 'tailwindcss/colors'],
  [/(^|\/)default-theme$/, 'tailwindcss/defaultTheme'],
  [/(^|\/)flatten-color-palette$/, 'tailwindcss/lib/util/flattenColorPalette'],
]

let importCache = new Map<string, { specifier: string; names: Map<string, string> }[]>()

// The static imports of a test file, in the sequence of the file. Vitest
// names them `__vite_ssr_import_0__`, `__vite_ssr_import_1__`, … in the test code.
function importsOf(file: string | null) {
  if (!file) return []
  if (!importCache.has(file)) {
    let code = fs.readFileSync(path.join(REPO, file), 'utf8')
    let list: { specifier: string; names: Map<string, string> }[] = []
    for (let match of code.matchAll(/^import\s+(type\s+)?([^;]*?)\s+from\s+['"]([^'"]+)['"]/gm)) {
      if (match[1]) continue
      let names = new Map<string, string>()
      let clause = match[2]
      let braces = clause.match(/\{([^}]*)\}/)
      if (braces) {
        for (let part of braces[1].split(',')) {
          let [imported, local] = part.trim().split(/\s+as\s+/)
          if (imported && !imported.startsWith('type ')) names.set(imported, local ?? imported)
        }
      }
      let first = clause.replace(/\{[^}]*\}/, '').replace(/\*\s+as\s+\w+/, '').replace(/,/g, '').trim()
      if (first) names.set('default', first)
      list.push({ specifier: match[3], names })
    }
    importCache.set(file, list)
  }
  return importCache.get(file)!
}

function moduleSource(module: any, file: string | null): string {
  let expression = serialize(module, new Set())
  let imports = importsOf(file)

  // The names of the imports, as the test file has them.
  expression = expression.replace(
    /\(0,\s*__vi(?:te_ssr)?_import_(\d+)__\.(\w+)\)|__vi(?:te_ssr)?_import_(\d+)__\.(\w+)/g,
    (text, index1, name1, index2, name2) => {
      let entry = imports[Number(index1 ?? index2)]
      return entry?.names.get(name1 ?? name2) ?? text
    },
  )

  let prelude = [
    `const plugin = require('tailwindcss/plugin')`,
    // The helpers of the test files.
    `const css = String.raw, html = String.raw`,
    // The checks of a test in a plugin function do nothing here.
    `const __chain = new Proxy(function () {}, { get: () => __chain, apply: () => __chain })`,
    `const expect = () => __chain`,
    `const vi = { fn: (implementation) => implementation ?? (() => {}) }`,
  ]
  for (let entry of imports) {
    let known = KNOWN_IMPORTS.find(([pattern]) => pattern.test(entry.specifier))
    let local = entry.names.get('default')
    if (known && local && local !== 'plugin') prelude.push(`const ${local} = require('${known[1]}')`)
  }
  return `${prelude.join('\n')}\nmodule.exports = ${expression}\n`
}

export function wrapCompile<T extends (css: string, opts?: any) => Promise<any>>(original: T): T {
  if (!DIR) return original

  return (async (css: string, opts: any = {}) => {
    vitestExpect ??= (await import('vitest')).expect
    optimize ??= (await import('../../@tailwindcss-node/src/optimize')).optimize

    let id = `${process.pid}-${counter++}`
    let stylesheets: any[] = []
    let modules: any[] = []
    let wrapped: any = { ...opts }

    if (opts.loadStylesheet) {
      wrapped.loadStylesheet = async (sheetId: string, base: string) => {
        let entry: any = { id: sheetId, base }
        stylesheets.push(entry)
        try {
          let result = await opts.loadStylesheet(sheetId, base)
          entry.result = { path: result.path, base: result.base, content: result.content }
          return result
        } catch (err) {
          entry.error = message(err)
          throw err
        }
      }
    }

    if (opts.loadModule) {
      wrapped.loadModule = async (moduleId: string, base: string, hint: string) => {
        let entry: any = { id: moduleId, base, hint }
        modules.push(entry)
        if (!process.env.TW_ORACLE_MODULES) return opts.loadModule(moduleId, base, hint)

        // For the plugin suite (gen-plugin-suite.mjs): the module as
        // JavaScript source text.
        let result: any
        try {
          result = await opts.loadModule(moduleId, base, hint)
        } catch (err) {
          entry.error = message(err)
          throw err
        }
        entry.resultBase = result.base
        try {
          entry.source = moduleSource(result.module, state().file)
        } catch (err) {
          entry.unserializable = message(err)
        }
        return result
      }
    }

    let head = {
      type: 'compile',
      id,
      seq: Date.now(),
      ...state(),
      input: css,
      options: {
        base: opts.base ?? null,
        from: opts.from ?? null,
        polyfills: opts.polyfills ?? null,
        hasLoadModule: typeof opts.loadModule === 'function',
        hasLoadStylesheet: typeof opts.loadStylesheet === 'function',
      },
    }

    let compiler: any
    try {
      compiler = await original(css, wrapped)
    } catch (err) {
      emit({
        ...head,
        stylesheets,
        modules: modules.length,
        moduleRequests: modules,
        error: message(err),
      })
      throw err
    }

    // Release the loader callbacks. An upstream test checks that the compiler does not keep them.
    opts = null
    wrapped = null

    emit({
      ...head,
      stylesheets,
      modules: modules.length,
      moduleRequests: modules,
      sources: compiler.sources,
      root: compiler.root,
      features: compiler.features,
    })

    return {
      ...compiler,
      build(candidates: string[]) {
        let event: any = { type: 'build', id, candidates: [...candidates] }
        let result: string
        try {
          result = compiler.build(candidates)
        } catch (err) {
          event.error = message(err)
          emit(event)
          throw err
        }
        event.css = result
        try {
          event.optimized = optimize(result).code
          event.minified = optimize(result, { minify: true }).code
        } catch (err) {
          event.optimizeError = message(err)
        }
        emit(event)
        return result
      },
    }
  }) as T
}
