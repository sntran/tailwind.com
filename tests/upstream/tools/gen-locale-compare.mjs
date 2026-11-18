#!/usr/bin/env node
// Usage: node gen-locale-compare.mjs [output file]
//
// This script records `a.localeCompare(b)` of Node.js for pairs of text:
// random ASCII text (letters of the two cases, digits, punctuation, spaces
// and control characters), and the properties, units and candidates that
// the canonical form of candidates compares. The Rust test
// tests/locale_compare.rs compares
// (native/tailwind/tests/fixtures/locale_compare.json.gz).
import fs from 'node:fs'
import path from 'node:path'
import zlib from 'node:zlib'
import { fileURLToPath } from 'node:url'
import { createRandom } from './lib.mjs'

let here = path.dirname(fileURLToPath(import.meta.url))
let output = path.resolve(process.argv[2] ?? path.join(here, '../../../native/tailwind/tests/fixtures/locale_compare.json.gz'))

let random = createRandom(20261118).next
let alphabet = 'aAbBzZ09_-,;:!?.\'"()[]{}@*/\\&#%`^+<=>|~$ \t\x01'
let text = (max) => {
  let length = Math.floor(random() * max)
  let s = ''
  for (let i = 0; i < length; i++) s += alphabet[Math.floor(random() * alphabet.length)]
  return s
}

let words = new Set(['', 'px', 'rem', 'em', '%', 'deg', 's', 'ms', 'vw', 'dvh', 'Q', 'q'])
let candidates = zlib
  .gunzipSync(fs.readFileSync(path.join(here, '../candidates.jsonl.gz')))
  .toString()
  .trim()
  .split('\n')
  .map((line) => JSON.parse(line).candidate)
for (let i = 0; i < candidates.length; i += 7) words.add(candidates[i])
let css = fs.readFileSync(path.join(here, '../css/utilities.css'), 'utf8') + fs.readFileSync(path.join(here, '../css/theme.css'), 'utf8')
for (let match of css.matchAll(/(--?[a-zA-Z][\w-]*|[a-z][a-z-]*)\s*:/g)) words.add(match[1])
words = [...words]

let pairs = []
for (let i = 0; i < 20000; i++) pairs.push([text(8), text(8)])
for (let i = 0; i < 20000; i++) {
  pairs.push([words[Math.floor(random() * words.length)], words[Math.floor(random() * words.length)]])
}
let result = pairs.map(([a, b]) => [a, b, Math.sign(a.localeCompare(b))])

fs.writeFileSync(output, zlib.gzipSync(JSON.stringify(result)))
console.log(`${path.basename(output)}: ${result.length} pairs, ${words.length} words`)
