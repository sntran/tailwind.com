#!/usr/bin/env node
// Compare the builder with the official compiler on given cases.
//
// Usage:
//   node tests/fuzz/compare.mjs --official DIR --builder FILE [CASES.jsonl]
//
//   --official DIR   the official npm packages of one version
//                    (sh tests/fuzz/install.sh DIR VERSION)
//   --builder FILE   target/release/examples/fuzz_build
//   CASES.jsonl      one case for each line: {"css": "...", "candidates": [...]}
//                    (default: the standard input). A line can have "name",
//                    and "first" (a number): two builds on one compiler,
//                    the first `first` candidates and then the other ones.
//   --record         write the official result of each case into the line of
//                    CASES.jsonl, as results[VERSION]. The file
//                    tests/upstream/switches.jsonl has such lines: the Rust
//                    test tests/switches.rs replays them with no Node.js.
//
// The Rust compiler of the builder compiles as the version of the package
// in DIR. So this is the tool for a switch (native/tailwind/src/
// switches.rs): write the case of a change, and compare with the official
// packages of the version before the change and of the version with it.
// The exit status is 1 when a case is different.
import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import process from 'node:process'
import { officialTailwind } from './official.mjs'

const args = process.argv.slice(2)
let officialDir, builder, file
let record = false
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--official') officialDir = args[++i]
  else if (args[i] === '--builder') builder = args[++i]
  else if (args[i] === '--record') record = true
  else file = args[i]
}
if (!officialDir || !builder) {
  console.error('Usage: node tests/fuzz/compare.mjs --official DIR --builder FILE [CASES.jsonl]')
  process.exit(2)
}

const cases = fs
  .readFileSync(file ?? 0, 'utf8')
  .split('\n')
  .filter((line) => line.trim() !== '' && !line.startsWith('//'))
  .map((line) => JSON.parse(line))
const official = await officialTailwind(officialDir)

const input = cases.map((c) => JSON.stringify({ css: c.css, candidates: c.candidates ?? [], base: official.officialDir, first: c.first })).join('\n') + '\n'
const run = spawnSync(path.resolve(builder), [official.officialDir], { input, maxBuffer: 1024 * 1024 * 1024 })
if (run.status !== 0) {
  console.error(`the builder failed (status ${run.status}): ${run.stderr.toString().slice(0, 2000)}`)
  process.exit(2)
}
const ours = run.stdout.toString().trim().split('\n').map((line) => JSON.parse(line))

let different = 0
for (let [index, c] of cases.entries()) {
  const expected = await official.run({ css: c.css, candidates: c.candidates ?? [], first: c.first })
  if (record) c.results = { ...(c.results ?? {}), [official.version]: expected }
  const actual = ours[index]
  const name = c.name ?? `case ${index + 1}`
  if (JSON.stringify(expected) === JSON.stringify(actual)) {
    console.log(`same       ${name}`)
    continue
  }
  different++
  console.log(`DIFFERENT  ${name}`)
  const show = (side, result) => {
    const text = result.css ?? `error: ${result.error ?? result.panic}`
    const other = (side === 'official' ? actual : expected).css ?? ''
    // Only the lines that the other side does not have.
    const lines = text.split('\n').filter((line) => !other.split('\n').includes(line))
    for (const line of lines.slice(0, 12)) console.log(`    ${side}: ${line}`)
    if (lines.length > 12) console.log(`    ${side}: ... ${lines.length - 12} more lines`)
  }
  show('official', expected)
  show('ours    ', actual)
}
if (record && file) {
  // The versions of the results in sequence, so that the file is stable.
  const order = (a, z) => a.localeCompare(z, undefined, { numeric: true })
  for (const c of cases) c.results = Object.fromEntries(Object.entries(c.results).sort(([a], [z]) => order(a, z)))
  fs.writeFileSync(file, cases.map((c) => JSON.stringify(c)).join('\n') + '\n')
}
console.log(`compare: Tailwind CSS ${official.version}: ${cases.length} cases, ${different} different`)
process.exit(different === 0 ? 0 : 1)
