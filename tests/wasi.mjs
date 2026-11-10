// Run the WASI program in Node.js on the recorded cases of the official
// program of Tailwind CSS (tests/upstream/cli and tests/upstream/cli-map),
// and compare the output.
//
// Usage: node tests/wasi.mjs build/tailwind.wasm [CASE...]
//
// Each case runs in a child process, because a WASI instance runs one
// time. The project is a copy outside this repository, as in tests/cli.sh.
//
// The cases of cli-map are for --map: see tests/cli.sh. The project is
// the root directory of the WASI program, so the path /PROJECT/app.css of
// a record is /app.css here.
import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))

if (process.argv[2] === '--child') {
  // The child: run the program with the project as the root directory.
  const [wasm, project, ...args] = process.argv.slice(3)
  const { WASI } = await import('node:wasi')
  const wasi = new WASI({
    version: 'preview1',
    args: ['tailwind', ...args],
    env: { NO_COLOR: '1', PWD: '/' },
    preopens: { '/': project },
    returnOnExit: true,
    stdin: process.stdin.fd,
  })
  const module = await WebAssembly.compile(fs.readFileSync(wasm))
  const instance = await WebAssembly.instantiate(module, wasi.getImportObject())
  process.exit(wasi.start(instance))
}

const wasm = path.resolve(process.argv[2])
const caseDirs = [path.join(here, 'upstream/cli'), path.join(here, 'upstream/cli-map')]
const names =
  process.argv.length > 3 ? process.argv.slice(3) : caseDirs.flatMap((cases) => fs.readdirSync(cases).sort())
const work = fs.mkdtempSync(path.join(os.tmpdir(), 'tailwind-wasi-'))

// A record with the root directory at the place of /PROJECT, also in a
// source map that is in a comment as Base64 text.
function atRoot(buffer) {
  const replace = (text) => text.replaceAll('/PROJECT/', '/').replaceAll('PROJECT/', '')
  const text = replace(buffer.toString('latin1')).replace(
    /(\/\*# sourceMappingURL=data:application\/json;base64,)([A-Za-z0-9+\/=]+)( \*\/)/,
    (_, start, data, end) =>
      start + Buffer.from(replace(Buffer.from(data, 'base64').toString('latin1')), 'latin1').toString('base64') + end,
  )
  return Buffer.from(text, 'latin1')
}

function listFiles(dir, prefix = '') {
  if (!fs.existsSync(dir)) return []
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const name = prefix + entry.name
    return entry.isDirectory() ? listFiles(path.join(dir, entry.name), name + '/') : [name]
  })
}

let failed = 0
for (const name of names) {
  const dir = caseDirs.map((cases) => path.join(cases, name)).find((dir) => fs.existsSync(path.join(dir, 'args.txt')))
  if (!dir) continue
  const stdin = path.join(dir, 'stdin.txt')
  const project = path.join(work, name)
  fs.cpSync(path.join(dir, 'project'), project, { recursive: true })
  const args = fs.readFileSync(path.join(dir, 'args.txt'), 'utf8').split('\n').filter((line) => line !== '')
  const result = spawnSync(
    process.execPath,
    ['--no-warnings', fileURLToPath(import.meta.url), '--child', wasm, project, ...args],
    { maxBuffer: 256 * 1024 * 1024, input: fs.existsSync(stdin) ? fs.readFileSync(stdin) : '' },
  )
  const expectedStatus = Number(fs.readFileSync(path.join(dir, 'exit.txt'), 'utf8'))
  const expected = atRoot(fs.readFileSync(path.join(dir, 'stdout.css')))
  // Each file that the official program made must be the same.
  const different = listFiles(path.join(dir, 'files')).filter((file) => {
    const made = path.join(project, file)
    return !fs.existsSync(made) || !fs.readFileSync(made).equals(atRoot(fs.readFileSync(path.join(dir, 'files', file))))
  })
  // For an error with a position: the message of the official program.
  const message = fs.existsSync(path.join(dir, 'stderr.txt'))
    ? atRoot(fs.readFileSync(path.join(dir, 'stderr.txt'))).toString().split('\n').find((line) => line.includes('│ '))
    : undefined
  const sameMessage = !message || result.stderr.toString().includes(message.replace(/\x1b\[[0-9;]*m/g, ''))
  if (result.status === expectedStatus && result.stdout.equals(expected) && different.length === 0 && sameMessage) {
    console.log(`wasi.mjs: ${name}: pass`)
  } else {
    failed++
    console.error(`wasi.mjs: ${name}: exit status ${result.status} (expected ${expectedStatus}), output ${result.stdout.equals(expected) ? 'same' : 'different'}${different.length > 0 ? `, different files: ${different.join(' ')}` : ''}${sameMessage ? '' : ', another error message'}`)
    console.error(result.stderr.toString().split('\n').slice(0, 8).join('\n'))
  }
}
console.log(`wasi.mjs: ${failed} failed`)
process.exit(failed === 0 ? 0 : 1)
