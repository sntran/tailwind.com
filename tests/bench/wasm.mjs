// Run tailwind.wasm as a command line program for the benchmarks:
//
//   node tests/bench/wasm.mjs TAILWIND.WASM ARGUMENTS...
//
// The current directory is the root `/` of the WASI program. The variable
// NODE_PATH goes to the program, because a project can need it. A
// directory of NODE_PATH in the current directory changes to its path in
// the WASI program; the program cannot see a directory outside it.
import path from 'node:path'
import process from 'node:process'
import { runWasm } from '../../js/node.mjs'

const [wasm, ...args] = process.argv.slice(2)
const env = { NO_COLOR: '1' }
if (process.env.NODE_PATH) {
  const cwd = process.cwd()
  env.NODE_PATH = process.env.NODE_PATH.split(path.delimiter)
    .map((dir) => {
      const relative = path.relative(cwd, path.resolve(cwd, dir))
      if (relative.startsWith('..') || path.isAbsolute(relative)) return null
      return '/' + relative.split(path.sep).join('/')
    })
    .filter((dir) => dir !== null)
    .join(':')
}
const result = await runWasm(args, { cwd: process.cwd(), wasm, env })
process.stdout.write(result.stdout)
process.stderr.write(result.stderr)
// Not process.exit(): it can stop the process before Node.js writes all of
// a large output to a pipe.
process.exitCode = result.status
