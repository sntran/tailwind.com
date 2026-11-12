#!/usr/bin/env node
// Build the site of tailwind.com: a static directory for GitHub Pages.
//
// Usage: node site/build.mjs --wasm FILE [--packages DIR] [--out DIR]
//
//   --wasm FILE      tailwind.wasm, built with WASI_SDK_PATH (make wasm), so
//                    that it has the JavaScript engine for @plugin
//   --packages DIR   the packages in the form of the ZIP directory of
//                    tailwind.com (default: build/site-packages, made with
//                    scripts/packages.sh and tools/twpatch, as
//                    scripts/build.sh does)
//   --out DIR        the site (default build/site)
//
// tailwind.wasm also builds the CSS of the page, in Node.js, with the WASI
// host of the page (wasi.mjs).
//
// The site has:
//   index.html, app.mjs, worker.mjs, wasi.mjs, diff.mjs  the page
//   style.css        the CSS of the page, built with tailwind.wasm
//   tailwind.wasm    the program
//   packages.bin     the packages of each version of Tailwind CSS
//   data.json        the versions, the examples, the switches of each
//                    release (native/tailwind/src/switches.rs) and the
//                    recorded cases (tests/upstream/switches.jsonl)
import { execFileSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { examples } from './examples.mjs'
import { Files, run } from './wasi.mjs'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.resolve(here, '..')

const opts = { out: path.join(root, 'build/site') }
const args = process.argv.slice(2)
for (let i = 0; i < args.length; i++) {
  const value = () => path.resolve(args[++i])
  switch (args[i]) {
    case '--wasm': opts.wasm = value(); break
    case '--packages': opts.packages = value(); break
    case '--out': opts.out = value(); break
    default: throw new Error(`unknown argument: ${args[i]}`)
  }
}
if (!opts.wasm) throw new Error('Usage: node site/build.mjs --wasm FILE [--packages DIR] [--out DIR]')

const log = (text) => console.error(`site/build.mjs: ${text}`)
const compare = (a, b) => {
  const x = a.split('.').map(Number)
  const y = b.split('.').map(Number)
  return x[0] - y[0] || x[1] - y[1] || x[2] - y[2]
}

// The versions of the file PACKAGES, the newest first.
const versions = fs
  .readFileSync(path.join(root, 'PACKAGES'), 'utf8')
  .split('\n')
  .filter((line) => line.trim() && !line.startsWith('#'))
  .map((line) => line.split(' ')[0])
  .sort((a, b) => compare(b, a))
const upstream = fs.readFileSync(path.join(root, 'UPSTREAM'), 'utf8').match(/^TAILWINDCSS_VERSION=(.*)$/m)[1]

// --- The packages ---------------------------------------------------------

let store = opts.packages
if (!store) {
  const downloads = path.join(root, 'build/packages')
  execFileSync('sh', [path.join(root, 'scripts/packages.sh'), downloads, ...versions], { stdio: 'inherit' })
  execFileSync('cargo', ['build', '-q', '--release', '--manifest-path', path.join(root, 'tools/twpatch/Cargo.toml')], { stdio: 'inherit' })
  store = path.join(root, 'build/site-packages')
  fs.rmSync(store, { recursive: true, force: true })
  const oldestFirst = [...versions].sort(compare)
  execFileSync(path.join(root, 'tools/twpatch/target/release/twpatch'), ['pack', downloads, store, ...oldestFirst], { stdio: 'inherit' })
}

// packages.bin: a 4-byte length (little endian), the JSON index
// [[path, size], ...], then the files, one after the other.
function pack(dir) {
  const list = []
  const walk = (at, base) => {
    for (const e of fs.readdirSync(at, { withFileTypes: true }).sort((a, b) => (a.name < b.name ? -1 : 1))) {
      const rel = base ? `${base}/${e.name}` : e.name
      if (e.isDirectory()) walk(path.join(at, e.name), rel)
      else list.push([rel, fs.readFileSync(path.join(at, e.name))])
    }
  }
  walk(dir, '')
  const index = Buffer.from(JSON.stringify(list.map(([name, data]) => [name, data.length])))
  const head = Buffer.alloc(4)
  head.writeUInt32LE(index.length)
  return Buffer.concat([head, index, ...list.map(([, data]) => data)])
}

// --- The switches and the recorded cases ----------------------------------

function switches() {
  const text = fs.readFileSync(path.join(root, 'native/tailwind/src/switches.rs'), 'utf8')
  const pattern = /((?:[ \t]*\/\/\/.*\n)*)[ \t]*pub const (\w+): Switch =\s*Switch::since\((\d+), (\d+), (\d+), "((?:[^"\\]|\\.)*)"\);/g
  const list = []
  for (const m of text.matchAll(pattern)) {
    const doc = m[1]
      .split('\n')
      .map((line) => line.replace(/^\s*\/\/\/ ?/, ''))
      .join(' ')
      .replace(/\s+/g, ' ')
      .trim()
    list.push({ name: m[2], version: `${m[3]}.${m[4]}.${m[5]}`, what: JSON.parse(`"${m[6]}"`), doc })
  }
  return list
}

const escape = (text) => text.replaceAll('&', '&amp;').replaceAll('"', '&quot;').replaceAll('<', '&lt;')

function cases() {
  const lines = fs.readFileSync(path.join(root, 'tests/upstream/switches.jsonl'), 'utf8').split('\n').filter(Boolean)
  return lines.map((line) => {
    const c = JSON.parse(line)
    const version = c.name.split(' ')[0]
    const text = c.name.slice(version.length + 1)
    // The version before the change: the newest version of PACKAGES that
    // is older than the change.
    const before = versions.find((v) => compare(v, version) < 0)
    // The page has the markup and the CSS in text areas: one element for
    // each candidate, and a text area has no CRLF.
    const markup = (c.candidates ?? []).map((candidate) => `<div class="${escape(candidate)}"></div>`).join('\n') + '\n'
    return { version, name: text, before, markup, css: c.css.replaceAll('\r\n', '\n').replaceAll('\r', '\n') }
  })
}

// Each case on the path of the page: the program builds the markup and the
// CSS as the version before the change and as the version of the change.
// A case of the compiler API (a list of candidates in a given sequence, a
// CRLF) can give the same output on this path: the page does not list it.
async function mark(list) {
  const license = /^\/\*! tailwindcss v[^\n]*\*\/\n/
  const output = async (c, version) => {
    if (files.exists('/project')) files.remove('/project')
    files.write('/project/index.html', c.markup)
    files.write('/project/app.css', c.css)
    const env = { TAILWIND_COM_PACKAGES: '/packages', TAILWINDCSS_VERSION: version, NO_COLOR: '1' }
    const r = await run(module, { args: ['-i', 'app.css'], env, files, preopens: { '/': '/project', '/packages': '/packages' } })
    return r.status === 0 ? r.stdout.replace(license, '') : r.stderr.split('\n').filter((l) => !l.startsWith('≈ tailwindcss')).join('\n')
  }
  for (const c of list) c.shows = (await output(c, c.before)) !== (await output(c, c.version))
}

async function releases() {
  const all = switches()
  const recorded = cases()
  await mark(recorded)
  const list = new Map()
  for (const s of all) {
    if (!list.has(s.version)) list.set(s.version, { version: s.version, switches: [], cases: [] })
    list.get(s.version).switches.push(s)
  }
  for (const c of recorded) {
    if (!list.has(c.version)) list.set(c.version, { version: c.version, switches: [], cases: [] })
    list.get(c.version).cases.push(c)
  }
  return [...list.values()].sort((a, b) => compare(b.version, a.version))
}

// --- The site -------------------------------------------------------------

// tailwind.wasm in Node.js, with the packages in /packages.
const module = await WebAssembly.compile(fs.readFileSync(opts.wasm))
const files = new Files()
const walkStore = (at, base) => {
  for (const e of fs.readdirSync(at, { withFileTypes: true })) {
    const rel = base ? `${base}/${e.name}` : e.name
    if (e.isDirectory()) walkStore(path.join(at, e.name), rel)
    else files.write(`/packages/${rel}`, fs.readFileSync(path.join(at, e.name)))
  }
}
walkStore(store, '')

fs.rmSync(opts.out, { recursive: true, force: true })
fs.mkdirSync(opts.out, { recursive: true })
for (const name of ['index.html', 'app.mjs', 'worker.mjs', 'wasi.mjs', 'diff.mjs']) fs.copyFileSync(path.join(here, name), path.join(opts.out, name))
fs.copyFileSync(opts.wasm, path.join(opts.out, 'tailwind.wasm'))
fs.writeFileSync(path.join(opts.out, 'packages.bin'), pack(store))
const data = { upstream, versions, examples, releases: await releases() }
fs.writeFileSync(path.join(opts.out, 'data.json'), JSON.stringify(data))

// The CSS of the page, with tailwind.wasm.
for (const name of ['index.html', 'app.mjs', 'style.css']) files.write(`/site/${name}`, fs.readFileSync(path.join(here, name)))
const css = await run(module, { args: ['-i', 'style.css', '-o', 'out.css', '--minify'], env: { NO_COLOR: '1' }, files, preopens: { '/': '/site' } })
if (css.status !== 0) throw new Error(`the CSS of the page: ${css.stderr}`)
fs.writeFileSync(path.join(opts.out, 'style.css'), files.read('/site/out.css'))

const size = (name) => `${(fs.statSync(path.join(opts.out, name)).size / 1e6).toFixed(2)} MB`
const count = data.releases.reduce((n, r) => n + r.switches.length, 0)
const all = data.releases.flatMap((r) => r.cases)
log(`wrote ${opts.out}: tailwind.wasm ${size('tailwind.wasm')}, packages.bin ${size('packages.bin')} (${versions.length} versions), ${count} switches, ${all.filter((c) => c.shows).length} of ${all.length} cases on the page`)
