#!/usr/bin/env node
// Usage: node gen-candidates.mjs <path of the upstream clone> <output directory>
// This script writes candidates.jsonl.gz: for a sample of the candidates of
// classes.jsonl.gz, the result of `parseCandidate` as JSON. A JavaScript
// plugin gets a candidate in this shape, so the Rust compiler must make
// the same JSON.
import fs from 'node:fs'
import path from 'node:path'
import zlib from 'node:zlib'
import { loadUpstream, writeGzip } from './lib.mjs'

// The same input as gen-classes.mjs.
const INPUT =
  '@layer theme, base, components, utilities;\n' +
  '@import "tailwindcss/theme.css" layer(theme);\n' +
  '@import "tailwindcss/utilities.css" layer(utilities);'

let [clone, outDir] = process.argv.slice(2)
if (!clone || !outDir) {
  console.error('Usage: node gen-candidates.mjs <path of the upstream clone> <output directory>')
  process.exit(2)
}
let up = await loadUpstream(clone)
let design = await up.loadDesignSystem(INPUT, { loadStylesheet: up.loadStylesheet })
let lines = zlib.gunzipSync(fs.readFileSync(path.join(outDir, 'classes.jsonl.gz'))).toString().trim().split('\n')

// Each 25th candidate, and each candidate with a variant that is not
// simple (compound, arbitrary or functional).
let out = []
for (let [index, line] of lines.entries()) {
  let { candidate } = JSON.parse(line)
  let special = /\[&|\bgroup-|\bpeer-|\bnot-|\bhas-|\bin-|\/|\bdata-|\baria-|\bsupports-|\bmin-\[|\bmax-\[|@/.test(candidate)
  if (index % 25 !== 0 && !(special && index % 5 === 0)) continue
  out.push(JSON.stringify({ candidate, parsed: Array.from(design.parseCandidate(candidate)) }) + '\n')
}
writeGzip(path.join(outDir, 'candidates.jsonl.gz'), out.join(''))
console.log(`${out.length} candidates -> candidates.jsonl.gz`)
