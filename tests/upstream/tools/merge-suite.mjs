#!/usr/bin/env node
// Usage: node merge-suite.mjs <events directory> <output file suite.jsonl.gz>
// This script joins the event files of all workers.
// The result has one JSON line for each compile call.
import fs from 'node:fs'
import path from 'node:path'
import zlib from 'node:zlib'

let [dir, output] = process.argv.slice(2)
if (!dir || !output) {
  console.error('Usage: node merge-suite.mjs <events directory> <output file>')
  process.exit(2)
}

let records = new Map()
for (let name of fs.readdirSync(dir).sort()) {
  if (!/^events-\d+\.jsonl$/.test(name)) continue
  for (let line of fs.readFileSync(path.join(dir, name), 'utf8').split('\n')) {
    if (line === '') continue
    let event = JSON.parse(line)
    if (event.type === 'compile') {
      let { type, id, seq, ...rest } = event
      let { file, test, input, options, stylesheets, modules, moduleRequests, error, ...tail } = rest
      let record = { file, test, input, options, stylesheets, modules, moduleRequests, builds: [] }
      if (error !== undefined) record.error = error
      Object.assign(record, tail)
      records.set(id, { record, seq, order: records.size })
    } else {
      let { type, id, ...build } = event
      records.get(id).record.builds.push(build)
    }
  }
}

// Sort by test file, then keep the call order of each worker.
let list = [...records.values()].sort(
  (a, b) => String(a.record.file).localeCompare(String(b.record.file)) || a.order - b.order,
)
let text = list.map((entry) => JSON.stringify(entry.record) + '\n').join('')
fs.writeFileSync(output, zlib.gzipSync(text, { level: 9 }))
console.log(`${list.length} records -> ${output}`)
