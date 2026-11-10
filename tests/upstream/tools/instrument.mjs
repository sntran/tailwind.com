#!/usr/bin/env node
// Usage: node instrument.mjs <path of the upstream clone>
// This script adds the recorder to packages/tailwindcss/src/index.ts.
// You can run it again safely: it does nothing when the patch is there.
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

let clone = process.argv[2]
if (!clone) {
  console.error('Usage: node instrument.mjs <path of the upstream clone>')
  process.exit(2)
}

let here = path.dirname(fileURLToPath(import.meta.url))
let src = path.join(clone, 'packages/tailwindcss/src')
fs.copyFileSync(path.join(here, 'recorder.ts'), path.join(src, '__oracle.ts'))

let file = path.join(src, 'index.ts')
let code = fs.readFileSync(file, 'utf8')
if (code.includes('__oracle')) {
  console.log('index.ts already has the patch.')
  process.exit(0)
}

let marker = 'export async function compile(\n'
if (!code.includes(marker)) {
  console.error('Cannot find the compile function in index.ts.')
  process.exit(1)
}

code = code.replace(marker, 'async function compileOriginal(\n')
code = code.replace(
  "import { version } from '../package.json'\n",
  "import { version } from '../package.json'\nimport { wrapCompile } from './__oracle'\n",
)
code += '\nexport const compile: typeof compileOriginal = wrapCompile(compileOriginal)\n'
fs.writeFileSync(file, code)
console.log('The patch is applied.')
