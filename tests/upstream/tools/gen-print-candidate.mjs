#!/usr/bin/env node
// Usage: node gen-print-candidate.mjs [output file]
//
// This script records the print functions of the official compiler:
// `printCandidate` of the design system for each reading of each candidate
// of candidates.jsonl.gz, and `printVariant` for each variant of
// RANDOM_VARIANTS and of the candidates. It uses two stylesheets: with no
// prefix and with the prefix `tw`. The Rust test tests/print_candidate.rs
// compares (native/tailwind/tests/fixtures/print_candidate.json.gz).
import fs from 'node:fs'
import path from 'node:path'
import zlib from 'node:zlib'
import { fileURLToPath } from 'node:url'
import { __unstable__loadDesignSystem } from 'tailwindcss'
import { RANDOM_VARIANTS } from './candidate-lists.mjs'

let here = path.dirname(fileURLToPath(import.meta.url))
let css = path.resolve(here, '../css')
let output = path.resolve(process.argv[2] ?? path.join(here, '../../../native/tailwind/tests/fixtures/print_candidate.json.gz'))

async function loadStylesheet(id) {
  let name = id === 'tailwindcss' ? 'index.css' : id.replace(/^tailwindcss\//, '').replace(/^\.\//, '')
  return { path: `/tailwindcss/${name}`, base: '/tailwindcss', content: fs.readFileSync(path.join(css, name), 'utf8') }
}

let candidates = zlib
  .gunzipSync(fs.readFileSync(path.join(here, '../candidates.jsonl.gz')))
  .toString()
  .trim()
  .split('\n')
  .map((line) => JSON.parse(line).candidate)

let result = []
for (let [input, prefix] of [
  ['@import "tailwindcss";', ''],
  ['@import "tailwindcss" prefix(tw);', 'tw:'],
]) {
  let design = await __unstable__loadDesignSystem(input, { base: '/', loadStylesheet })
  let printed = []
  let variants = new Set(RANDOM_VARIANTS)
  for (let candidate of candidates) {
    let readings = design.parseCandidate(prefix + candidate)
    if (readings.length === 0) continue
    printed.push([prefix + candidate, readings.map((reading) => design.printCandidate(reading))])
    for (let reading of readings) {
      for (let variant of reading.variants) variants.add(design.printVariant(variant))
    }
  }
  let variantList = []
  for (let variant of variants) {
    let parsed = design.parseVariant(variant)
    if (parsed !== null) variantList.push([variant, design.printVariant(parsed)])
  }
  result.push({ css: input, candidates: printed, variants: variantList })
}

fs.writeFileSync(output, zlib.gzipSync(JSON.stringify(result)))
console.log(`${path.basename(output)}: ${result.map((r) => `${r.candidates.length} candidates, ${r.variants.length} variants`).join('; ')}`)
