#!/usr/bin/env node
// Usage: node gen-optimize-warnings.mjs <scratch directory of run-cli.mjs> [output file]
//
// This script records the warnings of the optimizer of the official program
// (`optimize` of `@tailwindcss/node`: Lightning CSS with error recovery).
// For each input it writes the code and the exact text that the official
// program gives to `console.warn`, with its color codes, to
// native/tailwind/tests/fixtures/optimize_warnings.json. The Rust test
// tests/optimize_warnings.rs compares.
import fs from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

let [scratch, output] = process.argv.slice(2)
if (!scratch) {
  console.error('Usage: node gen-optimize-warnings.mjs <scratch directory of run-cli.mjs> [output file]')
  process.exit(2)
}
let here = path.dirname(fileURLToPath(import.meta.url))
output = path.resolve(output ?? path.join(here, '../../../native/tailwind/tests/fixtures/optimize_warnings.json'))

// The official program does not print the warnings with NODE_ENV=test.
process.env.NODE_ENV = ''
let cli = createRequire(path.join(path.resolve(scratch), 'node_modules/@tailwindcss/cli/package.json'))
let { optimize } = await import(pathToFileURL(cli.resolve('@tailwindcss/node')).href)

let inputs = [
  // No warning.
  '.a {\n  color: red;\n}\n',
  // One warning: an unknown pseudo-class.
  '.a {\n  color: red;\n}\n.b:not-a-class {\n  color: blue;\n}\n.c {\n  color: green;\n}\n',
  // The first line and the last line of the input.
  '.a:first-warning { color: red }',
  '.a { color: red }\n.b { color: blue }\n.c::last-warning(x) { color: green }',
  // More than one warning: each one has a number.
  '.a:one {\n  color: red;\n}\n\n\n.b::two(x) {\n  color: blue;\n}\n@media (width >= 40rem) {\n  .c:three .d {\n    color: green;\n  }\n}\n',
  // The pseudo-classes of Vue and Angular give no warning.
  '.a :deep(.b) { color: red }\n.c:slotted(p) { color: red }\n:global(.d) { color: red }\n.e::ng-deep .f { color: red }\n:host-context(.g) .h { color: red }\n',
  // One of them with another warning: the count is the count after the filter.
  '.a :deep(.b) { color: red }\n.c:unknown-thing { color: red }\n.d:global(.e) { color: red }\n',
  // Text that is not ASCII before the place of the warning.
  '.é { color: red }\n.ü .日本:unknown-here { color: red }\n',
  // A warning in a nested rule, with indentation.
  '@layer components {\n  .card {\n    color: red;\n    &:not-known {\n      color: blue;\n    }\n  }\n}\n',
  // Other errors that the parser recovers from.
  '.a { color: red }\n@media (min-width: ) { .b { color: blue } }\n.c { color: green }\n',
  '.a { color: red }\n.b { & > { color: blue } }\n.c { color: green }\n',
  '.a { color: red }\n..b { color: blue }\n.c { color: green }\n',
  '@import "x.css";\n.a { color: red }\n@import "late.css";\n',
]

let records = []
let warn = console.warn
for (let input of inputs) {
  for (let minify of [false, true]) {
    let warnings = []
    console.warn = (...args) => warnings.push(args.join(' '))
    let result
    try {
      result = { code: optimize(input, { minify }).code }
    } catch (error) {
      result = { error: String(error.message ?? error) }
    }
    console.warn = warn
    records.push({ input, minify, ...result, warnings })
  }
}
fs.writeFileSync(output, JSON.stringify(records, null, 1) + '\n')
console.error(`gen-optimize-warnings.mjs: wrote ${output} (${records.length} records, ${records.filter((record) => record.warnings.length > 0).length} with a warning)`)
