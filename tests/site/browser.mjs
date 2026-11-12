#!/usr/bin/env node
// tailwind.wasm in Chromium, with the WASI host of the site
// (site/wasi.mjs) and its file system in memory. Each scenario of
// tests/site/scenarios.mjs runs in the browser and in Node.js (node:wasi
// on a real directory), and the two results must be the same.
//
// Usage: node tests/site/browser.mjs [--wasm FILE] [--packages DIR] [--runs N]
//
//   --wasm FILE      the module (default build/tailwind.wasm)
//   --packages DIR   the packages of Tailwind CSS in the form of the ZIP
//                    directory of tailwind.com (default
//                    build/ape/zip/tailwindcss, which `make` writes)
//   --runs N         the timed runs of each scenario in the browser (default 5)
//
// It needs Playwright with Chromium.
import fs from 'node:fs'
import http from 'node:http'
import { createRequire } from 'node:module'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { WASI } from 'node:wasi'
import { project, scenarios } from './scenarios.mjs'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.resolve(here, '../..')

const opts = { wasm: path.join(root, 'build/tailwind.wasm'), packages: path.join(root, 'build/ape/zip/tailwindcss'), runs: 5 }
const args = process.argv.slice(2)
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--wasm') opts.wasm = path.resolve(args[++i])
  else if (args[i] === '--packages') opts.packages = path.resolve(args[++i])
  else if (args[i] === '--runs') opts.runs = Number(args[++i])
  else throw new Error(`unknown argument: ${args[i]}`)
}

function walk(dir, base = '') {
  const out = []
  for (const e of fs.readdirSync(dir, { withFileTypes: true })) {
    const rel = base ? `${base}/${e.name}` : e.name
    if (e.isDirectory()) out.push(...walk(path.join(dir, e.name), rel))
    else out.push(rel)
  }
  return out
}

// The time of a build in a message is not the same in two runs.
const clean = (text) => text.replace(/in \d+(\.\d+)?m?s/g, 'in T')

// --- Node.js, on a real directory ----------------------------------------

const real = fs.mkdtempSync(path.join(os.tmpdir(), 'tailwind-site-'))
process.on('exit', () => fs.rmSync(real, { recursive: true, force: true }))
for (const [name, text] of Object.entries(project)) {
  fs.mkdirSync(path.dirname(path.join(real, name)), { recursive: true })
  fs.writeFileSync(path.join(real, name), text)
}
fs.cpSync(opts.packages, path.join(real, 'packages'), { recursive: true })
const module = await WebAssembly.compile(fs.readFileSync(opts.wasm))

function expected(scenario) {
  const io = fs.mkdtempSync(path.join(real, 'io-'))
  const out = fs.openSync(path.join(io, 'out'), 'w')
  const err = fs.openSync(path.join(io, 'err'), 'w')
  const wasi = new WASI({
    version: 'preview1',
    args: ['tailwind', ...scenario.args],
    env: { PWD: '/', ...scenario.env },
    preopens: { '/': path.join(real, 'app'), '/packages': path.join(real, 'packages') },
    returnOnExit: true,
    stdout: out,
    stderr: err,
  })
  const status = wasi.start(new WebAssembly.Instance(module, wasi.getImportObject()))
  fs.closeSync(out)
  fs.closeSync(err)
  const stdout = scenario.output ? fs.readFileSync(path.join(real, scenario.output), 'utf8') : fs.readFileSync(path.join(io, 'out'), 'utf8')
  return { status, stdout, stderr: clean(fs.readFileSync(path.join(io, 'err'), 'utf8')) }
}

// --- The browser ----------------------------------------------------------

// The packages as one JSON file: { path: base64 }.
const packages = {}
for (const rel of walk(opts.packages)) packages[rel] = fs.readFileSync(path.join(opts.packages, rel)).toString('base64')

const page = `<!doctype html><meta charset="utf-8"><title>tailwind.wasm</title>
<script type="module">
import { Files, run } from '/site/wasi.mjs'
import { project, scenarios } from '/tests/site/scenarios.mjs'
window.test = async (runs) => {
  const times = {}
  let start = performance.now()
  const module = await WebAssembly.compileStreaming(fetch('/tailwind.wasm'))
  times.compile = performance.now() - start
  start = performance.now()
  const files = new Files()
  for (const [name, text] of Object.entries(project)) files.write('/' + name, text)
  const packages = await (await fetch('/packages.json')).json()
  for (const [name, data] of Object.entries(packages)) files.write('/packages/' + name, Uint8Array.from(atob(data), (c) => c.charCodeAt(0)))
  times.packages = performance.now() - start
  const results = []
  for (const scenario of scenarios) {
    const all = []
    let result
    for (let i = 0; i <= runs; i++) {
      start = performance.now()
      result = await run(module, { args: scenario.args, env: scenario.env, files, preopens: { '/': '/app', '/packages': '/packages' } })
      all.push(performance.now() - start)
      if (scenario.output) result.stdout = files.readText('/' + scenario.output)
    }
    const first = all.shift()
    all.sort((a, z) => a - z)
    results.push({ name: scenario.name, ...result, first, median: all[Math.floor(all.length / 2)] })
  }
  return { times, results }
}
window.ready = true
</script>`

const types = { '.mjs': 'text/javascript', '.wasm': 'application/wasm', '.json': 'application/json', '.html': 'text/html' }
const server = http.createServer((request, response) => {
  const url = new URL(request.url, 'http://localhost').pathname
  let body
  if (url === '/') body = page
  else if (url === '/tailwind.wasm') body = fs.readFileSync(opts.wasm)
  else if (url === '/packages.json') body = JSON.stringify(packages)
  else if (url === '/site/wasi.mjs' || url === '/tests/site/scenarios.mjs') body = fs.readFileSync(path.join(root, url))
  if (body === undefined) {
    response.writeHead(404).end()
    return
  }
  response.writeHead(200, { 'content-type': types[path.extname(url)] ?? 'text/html' }).end(body)
})
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
const base = `http://127.0.0.1:${server.address().port}/`

const require = createRequire(import.meta.url)
const { chromium } = require('playwright')
const browser = await chromium.launch()
let failed = 0
try {
  const tab = await browser.newPage()
  const messages = []
  tab.on('console', (m) => messages.push(m.text()))
  tab.on('pageerror', (e) => messages.push(String(e)))
  await tab.goto(base)
  await tab.waitForFunction(() => window.ready === true)
  const { times, results } = await tab.evaluate((runs) => window.test(runs), opts.runs)
  const version = browser.version()
  const format = (ms) => `${Math.round(ms)} ms`
  console.log(`Chromium ${version}: compile ${format(times.compile)}, packages ${format(times.packages)}`)
  console.log('| Scenario | Same as node:wasi | First run | Median of the next runs |')
  console.log('|---|---|---:|---:|')
  for (const [i, r] of results.entries()) {
    const want = expected(scenarios[i])
    const same = r.status === want.status && r.stdout === want.stdout && clean(r.stderr) === want.stderr
    if (!same) {
      failed++
      console.error(`browser.mjs: ${r.name}: status ${r.status} and ${want.status}, ${r.stdout.length} and ${want.stdout.length} bytes\n${r.stderr.slice(0, 600)}`)
    }
    console.log(`| ${r.name} | ${same ? 'yes' : 'NO'} | ${format(r.first)} | ${format(r.median)} |`)
  }
  for (const m of messages) console.error(`browser.mjs: console: ${m}`)
} finally {
  await browser.close()
  server.close()
}
if (failed > 0) {
  console.error(`browser.mjs: ${failed} scenarios are different from node:wasi`)
  process.exit(1)
}
console.log(`browser.mjs: ${scenarios.length} scenarios, the same as node:wasi`)
