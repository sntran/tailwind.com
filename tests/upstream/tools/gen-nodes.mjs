#!/usr/bin/env node
// Usage: node gen-nodes.mjs <path of the upstream clone> <output directory>
// This script writes nodes.jsonl.gz: for each candidate of classes.jsonl.gz,
// the rules of the candidate before the last pass of the compiler
// (`compileAstNodes`, then `toCss` of each rule). A test of one utility or
// of one variant can use these lines without the full compiler.
import fs from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import zlib from 'node:zlib'
import { loadUpstream, writeGzip } from './lib.mjs'

// The same input as gen-classes.mjs.
const INPUT =
  '@layer theme, base, components, utilities;\n' +
  '@import "tailwindcss/theme.css" layer(theme);\n' +
  '@import "tailwindcss/utilities.css" layer(utilities);'

let [clone, outDir] = process.argv.slice(2)
let up = await loadUpstream(clone)
let require = createRequire(path.join(path.resolve(clone), 'packages/@tailwindcss-node/package.json'))
let { createJiti } = require('jiti')
let jiti = createJiti(path.join(path.resolve(clone), 'packages/tailwindcss/src/'), { moduleCache: true })
let { toCss } = await jiti.import(path.join(path.resolve(clone), 'packages/tailwindcss/src/ast.ts'))

let design = await up.loadDesignSystem(INPUT, { loadStylesheet: up.loadStylesheet })
let lines = zlib.gunzipSync(fs.readFileSync(path.join(outDir, 'classes.jsonl.gz'))).toString().trim().split('\n')
let out = []
for (let line of lines) {
  let { candidate, kind } = JSON.parse(line)
  let nodes = []
  for (let parsed of design.parseCandidate(candidate)) {
    for (let { node, propertySort } of design.compileAstNodes(parsed)) {
      nodes.push({ css: toCss([node]), order: propertySort.order, count: propertySort.count })
    }
  }
  out.push(JSON.stringify({ candidate, kind, nodes }))
}
writeGzip(path.join(outDir, 'nodes.jsonl.gz'), out.join('\n') + '\n')
console.log(`nodes.jsonl.gz: ${out.length} lines`)
