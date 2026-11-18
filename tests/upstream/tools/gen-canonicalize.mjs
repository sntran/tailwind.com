#!/usr/bin/env node
// Usage: node gen-canonicalize.mjs <upstream clone> [output file]
//
// This script records the canonical form of candidates of the official
// compiler: each call of `designSystem.canonicalizeCandidates` in the
// unit tests of `canonicalize-candidates.test.ts`, in their sequence (the
// caches of a design system change the result of later calls). It bundles
// the test file from the TypeScript source of <upstream clone> with
// esbuild, with a small replacement of vitest: the tests run, and their
// expectations do nothing. Each design system gives one record: the
// stylesheet, the stylesheets and the modules that it loaded, and its
// calls with their candidates, options and result. The Rust test
// tests/canonicalize.rs replays them
// (native/tailwind/tests/fixtures/canonicalize.json.gz).
import * as esbuild from 'esbuild'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import zlib from 'node:zlib'
import { fileURLToPath, pathToFileURL } from 'node:url'

let [clone, output] = process.argv.slice(2)
if (!clone) {
  console.error('Usage: node gen-canonicalize.mjs <upstream clone> [output file]')
  process.exit(2)
}
let here = path.dirname(fileURLToPath(import.meta.url))
output = path.resolve(output ?? path.join(here, '../../../native/tailwind/tests/fixtures/canonicalize.json.gz'))
let src = path.resolve(clone, 'packages/tailwindcss/src')

// A replacement of vitest: `describe`, `test`, `it` and `.each` register
// the tests, `expect` accepts each assertion, and `vi.spyOn` gives the
// function.
const VITEST = `
let tests = []
let names = []
function register(name, options, fn) {
  if (typeof options === 'function') fn = options
  tests.push({ name: [...names, name].join(' > '), fn })
}
function each(kind) {
  return (table) => (name, options, fn) => {
    if (typeof options === 'function') fn = options
    for (let row of table) kind(name, (...args) => fn(...(Array.isArray(row) ? row : [row]), ...args))
  }
}
export function describe(name, fn) { names.push(name); try { fn() } finally { names.pop() } }
describe.each = each(describe)
export function test(name, options, fn) { register(name, options, fn) }
test.each = each(test)
test.skip = () => {}
test.skip.each = () => () => {}
export const it = test
let anything = new Proxy(function () {}, { get: (_, key) => (key === 'then' ? undefined : anything), apply: () => anything })
export function expect() { return anything }
expect.arrayContaining = (x) => x
expect.any = (x) => x
export const vi = { spyOn: (object, key) => Object.assign(object[key], { [Symbol.dispose]() {} }), fn: (f = () => {}) => f }
globalThis.__runTests = async function run() {
  for (let { name, fn } of tests) {
    globalThis.__testName = name
    try { await fn() } catch (error) { console.error('test failed:', name, String(error).slice(0, 200)) }
  }
}
`

let work = fs.mkdtempSync(path.join(os.tmpdir(), 'gen-canonicalize-'))
fs.writeFileSync(path.join(work, 'vitest.mjs'), VITEST)
let index = path.join(src, 'index.ts')
let bundle = path.join(work, 'bundle.mjs')
await esbuild.build({
  entryPoints: [path.join(src, 'canonicalize-candidates.test.ts')],
  bundle: true,
  format: 'esm',
  platform: 'node',
  target: 'node20',
  // The test reads `../theme.css` from its directory.
  define: { __dirname: JSON.stringify(src), __filename: JSON.stringify(path.join(src, 'canonicalize-candidates.test.ts')) },
  outfile: bundle,
  logLevel: 'error',
  plugins: [
    {
      name: 'record',
      setup(build) {
        build.onResolve({ filter: /^vitest$/ }, () => ({ path: path.join(work, 'vitest.mjs') }))
        build.onResolve({ filter: /^\.$/ }, () => ({ path: 'index-wrapper', namespace: 'record' }))
        build.onLoad({ filter: /.*/, namespace: 'record' }, () => ({
          resolveDir: src,
          loader: 'ts',
          contents: `
            export * from ${JSON.stringify(index)}
            import { __unstable__loadDesignSystem as real } from ${JSON.stringify(index)}
            export function __unstable__loadDesignSystem(css, opts) { return globalThis.__record(real, css, opts) }
          `,
        }))
      },
    },
  ],
})

let records = []

// The source text of a module of a test: `plugin(handler, config)`.
function moduleSource(module) {
  if (module && typeof module.handler === 'function') {
    let config = module.config ? `, ${JSON.stringify(module.config)}` : ''
    return `module.exports = require('tailwindcss/plugin')(${module.handler.toString()}${config})`
  }
  return `module.exports = ${JSON.stringify(module)}`
}

globalThis.__record = async (real, css, opts = {}) => {
  let record = { test: globalThis.__testName, css, base: opts.base ?? '', stylesheets: [], modules: [], calls: [] }
  let wrapped = { ...opts }
  if (opts.loadStylesheet) {
    wrapped.loadStylesheet = async (id, base) => {
      let result = await opts.loadStylesheet(id, base)
      record.stylesheets.push({ id, base, result: { path: result.path, base: result.base, content: result.content } })
      return result
    }
  }
  if (opts.loadModule) {
    wrapped.loadModule = async (id, base, hint) => {
      let result = await opts.loadModule(id, base, hint)
      record.modules.push({ id, base, hint, source: moduleSource(result.module), resultBase: result.base })
      return result
    }
  }
  let designSystem = await real(css, wrapped)
  records.push(record)
  let original = designSystem.canonicalizeCandidates
  designSystem.canonicalizeCandidates = function (candidates, options) {
    let call = { candidates: [...candidates], options: options ?? null }
    try {
      call.result = original.call(this, candidates, options)
    } catch (error) {
      call.error = String(error)
      record.calls.push(call)
      throw error
    }
    record.calls.push(call)
    return call.result
  }
  return designSystem
}

await import(pathToFileURL(bundle).href)
await globalThis.__runTests()
fs.rmSync(work, { recursive: true, force: true })

records = records.filter((record) => record.calls.length > 0)
fs.writeFileSync(output, zlib.gzipSync(JSON.stringify(records), { level: 9 }))
let calls = records.reduce((sum, record) => sum + record.calls.length, 0)
console.log(`${path.basename(output)}: ${records.length} design systems, ${calls} calls`)
