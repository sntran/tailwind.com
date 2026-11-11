#!/usr/bin/env node
// Record the official output of each case of tests/upstream/fuzz-found.jsonl
// again, with the official packages of DIR (tests/fuzz/install.sh).
//
// Usage: node tests/fuzz/refresh.mjs DIR [FILE]
//
// A new version of Tailwind CSS can change the official output of a case:
// scripts/sync-upstream.sh runs this after the install of the new version.
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { officialTailwind, writeFiles } from './official.mjs'

const here = path.dirname(fileURLToPath(import.meta.url))
const [dir, file = path.join(here, '../upstream/fuzz-found.jsonl')] = process.argv.slice(2)
if (!dir) {
  console.error('Usage: node tests/fuzz/refresh.mjs DIR [FILE]')
  process.exit(2)
}
const off = await officialTailwind(dir)
const repository = path.resolve(here, '../..')
const lines = fs.readFileSync(file, 'utf8').split('\n').filter((l) => l.trim() !== '')
const out = []
for (const [i, line] of lines.entries()) {
  const c = JSON.parse(line)
  // The files of the case, in a directory next to the official packages.
  const base = path.join(off.root, 'work', `refresh-${i}`)
  fs.rmSync(base, { recursive: true, force: true })
  fs.mkdirSync(base, { recursive: true })
  writeFiles(base, c, repository)
  const official = await off.run({ css: c.css, candidates: c.candidates, base })
  fs.rmSync(base, { recursive: true, force: true })
  out.push(JSON.stringify({ ...c, version: off.version, official }))
}
fs.writeFileSync(file, out.join('\n') + '\n')
console.log(`refresh.mjs: ${out.length} cases of ${file}, Tailwind CSS ${off.version}`)
