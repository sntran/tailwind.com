#!/usr/bin/env node
// Usage: node gen-fold.mjs <upstream clone> [output file]
//
// This script records three functions of the canonical form of candidates
// of the official compiler: `constantFoldDeclaration` (with no `rem` and
// with `rem: 16`, and with `normalizeUnit` false), `canonicalizeCalcExpressions`
// and `expandDeclaration`. The npm package does not export them, so the
// script bundles them from the TypeScript source of <upstream clone> with
// esbuild. The values are the values of the test tables of the three
// modules, the arbitrary values of candidates.jsonl.gz, and random
// expressions. The Rust test tests/fold.rs compares
// (native/tailwind/tests/fixtures/fold.json.gz).
import * as esbuild from 'esbuild'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import zlib from 'node:zlib'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { createRandom } from './lib.mjs'

let [clone, output] = process.argv.slice(2)
if (!clone) {
  console.error('Usage: node gen-fold.mjs <upstream clone> [output file]')
  process.exit(2)
}
let here = path.dirname(fileURLToPath(import.meta.url))
output = path.resolve(output ?? path.join(here, '../../../native/tailwind/tests/fixtures/fold.json.gz'))
let src = path.resolve(clone, 'packages/tailwindcss/src')

let work = fs.mkdtempSync(path.join(os.tmpdir(), 'gen-fold-'))
let entry = path.join(work, 'entry.ts')
fs.writeFileSync(
  entry,
  [
    `export { constantFoldDeclaration } from ${JSON.stringify(path.join(src, 'constant-fold-declaration.ts'))}`,
    `export { canonicalizeCalcExpressions } from ${JSON.stringify(path.join(src, 'canonicalize-calc-expressions.ts'))}`,
    `export { expandDeclaration } from ${JSON.stringify(path.join(src, 'expand-declaration.ts'))}`,
    `export { decl } from ${JSON.stringify(path.join(src, 'ast.ts'))}`,
  ].join('\n'),
)
let bundle = path.join(work, 'bundle.mjs')
await esbuild.build({ entryPoints: [entry], bundle: true, format: 'esm', platform: 'node', outfile: bundle, logLevel: 'error' })
let { constantFoldDeclaration, canonicalizeCalcExpressions, expandDeclaration, decl } = await import(pathToFileURL(bundle).href)
fs.rmSync(work, { recursive: true, force: true })

// The string values of the test tables.
let values = new Set()
for (let file of ['constant-fold-declaration.test.ts', 'canonicalize-calc-expressions.test.ts']) {
  let text = fs.readFileSync(path.join(src, file), 'utf8')
  for (let match of text.matchAll(/'((?:calc|[-+0-9(]|var)[^']*)'/g)) values.add(match[1])
}

// The arbitrary values of the candidates, with `_` as a space.
let candidates = zlib
  .gunzipSync(fs.readFileSync(path.join(here, '../candidates.jsonl.gz')))
  .toString()
  .trim()
  .split('\n')
  .map((line) => JSON.parse(line).candidate)
for (let candidate of candidates) {
  for (let match of candidate.matchAll(/\[([^\]]+)\]/g)) values.add(match[1].replaceAll('_', ' '))
}

// Random expressions.
let random = createRandom(20261118)
let numbers = ['0', '-0', '+0', '0.0', '1', '-1', '2', '3', '3.5', '0.25', '.5', '-0.0', '100', '29', '16', '1e3', '0.1', '0.2', '7']
let units = ['', '', 'px', 'rem', 'em', '%', 'in', 'cm', 'mm', 'Q', 'q', 'pc', 'pt', 'deg', 'rad', 'grad', 'turn', 'ms', 's', 'kHz', 'hz', 'vw', 'dvh', 'fr', 'ch']
let operators = ['*', '+', '-', '/']
let unknowns = ['var(--foo)', 'var(--spacing)', 'var(--spacing, 0.25rem)', '100vw', 'theme(spacing.4)', 'min(1px,2px)']
function operand(depth) {
  let r = random.next()
  if (depth > 0 && r < 0.25) return expression(depth - 1)
  if (r < 0.4) return random.pick(unknowns)
  return random.pick(numbers) + random.pick(units)
}
function expression(depth) {
  let name = random.chance(0.8) ? 'calc' : ''
  return `${name}(${operand(depth)} ${random.pick(operators)} ${operand(depth)})`
}
for (let i = 0; i < 6000; i++) values.add(random.chance(0.15) ? operand(0) : expression(2))

let fold = []
for (let value of values) {
  fold.push([
    value,
    constantFoldDeclaration(value),
    constantFoldDeclaration(value, 16),
    constantFoldDeclaration(value, null, false),
    canonicalizeCalcExpressions(value),
  ])
}

// The declarations to expand: each property, with 0 to 5 parts.
let properties = [
  'inset', 'margin', 'padding', 'scroll-margin', 'scroll-padding', 'border-width', 'border-style', 'border-color', 'gap', 'overflow',
  'overscroll-behavior', 'inset-block', 'inset-inline', 'margin-block', 'margin-inline', 'padding-block', 'padding-inline',
  'scroll-margin-block', 'scroll-margin-inline', 'scroll-padding-block', 'scroll-padding-inline', 'border-block',
  'border-block-color', 'border-block-style', 'border-block-width', 'border-inline', 'border-inline-color',
  'border-inline-style', 'border-inline-width', 'margin-top', 'color', 'border', 'flex',
]
let parts = ['10px', '20px', 'calc(1px + 2px)', 'auto', 'var(--x, 1px 2px)', '0']
let expand = []
for (let property of properties) {
  for (let count = 0; count <= 5; count++) {
    let value = Array.from({ length: count }, (_, i) => parts[(i + count) % parts.length]).join(' ')
    for (let important of [false, true]) {
      for (let features of [1, 3]) {
        let result = expandDeclaration(decl(property, value, important), features)
        expand.push([property, value, important, features, result ? result.map((d) => [d.property, d.value, d.important]) : null])
      }
    }
  }
}

fs.writeFileSync(output, zlib.gzipSync(JSON.stringify({ fold, expand })))
console.log(`${path.basename(output)}: ${fold.length} values, ${expand.length} declarations`)
