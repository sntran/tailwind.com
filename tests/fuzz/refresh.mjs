#!/usr/bin/env node
// Record the official output of each case of tests/upstream/fuzz-found.jsonl
// again, with the official packages of DIR (tests/fuzz/install.sh).
//
// Usage: node tests/fuzz/refresh.mjs DIR [FILE]
//
// A new version of Tailwind CSS can change the official output of a case:
// scripts/sync-upstream.sh runs this after the install of the new version.
import fs from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const [dir, file = path.join(here, '../upstream/fuzz-found.jsonl')] = process.argv.slice(2)
if (!dir) {
  console.error('Usage: node tests/fuzz/refresh.mjs DIR [FILE]')
  process.exit(2)
}
const require = createRequire(path.join(path.resolve(dir), 'package.json'))
const officialDir = path.dirname(require.resolve('tailwindcss/package.json'))
const mod = await import(pathToFileURL(require.resolve('tailwindcss')).href)
const official = mod.compile ? mod : mod.default
const version = JSON.parse(fs.readFileSync(path.join(officialDir, 'package.json'), 'utf8')).version

async function loadStylesheet(id) {
  let name = id === 'tailwindcss' ? 'index.css' : id.replace(/^tailwindcss\//, '').replace(/^\.\//, '')
  if (!name.endsWith('.css')) name += '.css'
  const p = path.join(officialDir, name)
  if (!fs.existsSync(p)) throw new Error(`Could not resolve '${id}'`)
  return { path: p, base: officialDir, content: fs.readFileSync(p, 'utf8') }
}

const BANNER = /^\/\*! tailwindcss v[^*]*\*\/\n/
const lines = fs.readFileSync(file, 'utf8').split('\n').filter((l) => l.trim() !== '')
const out = []
const warn = console.warn
console.warn = () => {}
for (const line of lines) {
  const c = JSON.parse(line)
  let result
  try {
    const compiler = await official.compile(c.css, { base: officialDir, loadStylesheet })
    result = { css: compiler.build(c.candidates).replace(BANNER, '') }
  } catch (e) {
    result = { error: e?.message ?? String(e) }
  }
  out.push(JSON.stringify({ ...c, version, official: result }))
}
console.warn = warn
fs.writeFileSync(file, out.join('\n') + '\n')
console.log(`refresh.mjs: ${out.length} cases of ${file}, Tailwind CSS ${version}`)
