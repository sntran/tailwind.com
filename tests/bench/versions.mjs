#!/usr/bin/env node
// The speed of tailwind.com for older versions of Tailwind CSS, with each
// of its two engines. The result is a Markdown table on standard output.
//
// Usage:
//   node tests/bench/versions.mjs [--program FILE] [--runs N]
//        [--versions LIST] [--only NAME,...]
//
//   --program FILE    tailwind.com (default build/tailwind.com)
//   --runs N          the timed runs of each cell (default 9)
//   --versions LIST   the versions, with a comma between them (default:
//                     some versions from 4.3.5 to 4.0.0)
//   --only LIST       only these workloads
//
// For each version and workload, the program builds the project with
// TAILWINDCSS_VERSION and the native engine (the Rust compiler), then
// with TAILWIND_COM_ENGINE=script (the official compiler of that version
// in QuickJS). The packages come from the file itself, not from
// TAILWIND_COM_PACKAGES. The two outputs must be the same: the table
// tells each cell where they are not.
//
// The projects are those of tests/bench/run.mjs. They need no official
// packages: tailwind.com has them.
import { spawnSync } from 'node:child_process'
import crypto from 'node:crypto'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import zlib from 'node:zlib'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.resolve(here, '../..')

// The large project is made in a second process. A parent process with
// much memory makes each start of a child process slower, and that time
// is not the time of the program.
if (process.argv[2] === '--make-large-project') {
  const dir = process.argv[3]
  fs.mkdirSync(path.join(dir, 'src'), { recursive: true })
  const lines = zlib.gunzipSync(fs.readFileSync(path.join(root, 'tests/upstream/classes.jsonl.gz'))).toString().trim().split('\n')
  const classes = lines.map((l) => JSON.parse(l)).filter((c) => c.kind === 'class').map((c) => c.candidate)
  let k = 0
  for (let f = 0; f < 1000; f++) {
    const sub = path.join(dir, 'src', `d${f % 20}`)
    fs.mkdirSync(sub, { recursive: true })
    const items = []
    for (let i = 0; i < 6; i++) {
      const cls = []
      for (let j = 0; j < 5; j++) cls.push(classes[(k = (k * 7919 + 13) % classes.length)])
      items.push(`  <div class="${cls.join(' ')}">${i}</div>`)
    }
    fs.writeFileSync(path.join(sub, `page${f}.html`), `<section>\n${items.join('\n')}\n</section>\n`)
  }
  fs.writeFileSync(path.join(dir, 'app.css'), '@import "tailwindcss" source(none);\n@source "./src";\n')
  process.exit(0)
}

function options() {
  const args = process.argv.slice(2)
  const o = {
    program: path.join(root, 'build/tailwind.com'),
    runs: 9,
    versions: ['4.3.5', '4.3.4', '4.3.0', '4.2.4', '4.2.0', '4.1.18', '4.1.0', '4.0.17', '4.0.8', '4.0.0'],
    only: null,
  }
  for (let i = 0; i < args.length; i++) {
    const value = () => args[++i]
    switch (args[i]) {
      case '--program': o.program = path.resolve(value()); break
      case '--runs': o.runs = Number(value()); break
      case '--versions': o.versions = value().split(','); break
      case '--only': o.only = value().split(','); break
      default: throw new Error(`unknown argument: ${args[i]}`)
    }
  }
  if (!fs.existsSync(o.program)) throw new Error(`${o.program} is not there: run make first`)
  return o
}

const opts = options()
const windows = process.platform === 'win32'
const command = (args) => (windows || !opts.program.endsWith('.com') ? [opts.program, ...args] : ['sh', opts.program, ...args])

const work = fs.mkdtempSync(path.join(os.tmpdir(), 'tailwind-versions-bench-'))
process.on('exit', () => fs.rmSync(work, { recursive: true, force: true }))

function copyProject(from, name) {
  const dir = path.join(work, name)
  fs.cpSync(path.join(root, from), dir, { recursive: true })
  return dir
}

function largeProject() {
  const dir = path.join(work, 'large')
  const made = spawnSync(process.execPath, [fileURLToPath(import.meta.url), '--make-large-project', dir])
  if (made.status !== 0) throw new Error(made.stderr.toString())
  return dir
}

const workloads = [
  {
    name: 'Phoenix',
    dir: () => copyProject('tests/upstream/cli/phoenix/project', 'phoenix'),
    args: ['-i', 'assets/css/app.css'],
  },
  {
    name: 'Phoenix 1.8.9 with daisyUI',
    dir: () => copyProject('tests/upstream/cli-js/phoenix-deps/project', 'phoenix-deps'),
    env: (dir) => ({ NODE_PATH: [path.join(dir, 'deps'), path.join(dir, '_build/dev')].join(path.delimiter) }),
    args: ['--input=assets/css/app.css'],
  },
  {
    name: 'large project',
    dir: largeProject,
    args: ['-i', 'app.css'],
  },
].filter((w) => !opts.only || opts.only.includes(w.name))

function once(workload, dir, version, engine) {
  const env = { ...process.env, NO_COLOR: '1', ...(workload.env ? workload.env(dir) : {}), TAILWINDCSS_VERSION: version }
  delete env.TAILWIND_COM_PACKAGES
  delete env.TAILWIND_COM_ENGINE
  if (engine === 'script') env.TAILWIND_COM_ENGINE = 'script'
  const [program, ...args] = command(workload.args)
  const start = process.hrtime.bigint()
  const result = spawnSync(program, args, { cwd: dir, env, maxBuffer: 512 * 1024 * 1024 })
  const ms = Number(process.hrtime.bigint() - start) / 1e6
  if (result.status !== 0) {
    throw new Error(`${workload.name}, ${version}, ${engine} engine: status ${result.status}\n${result.stderr.toString().slice(0, 2000)}`)
  }
  return { ms, hash: crypto.createHash('sha256').update(result.stdout).digest('hex') }
}

// One run with no time, so the files are in the cache of the system.
function measure(workload, dir, version, engine) {
  once(workload, dir, version, engine)
  const times = []
  let last
  for (let i = 0; i < opts.runs; i++) times.push((last = once(workload, dir, version, engine)).ms)
  times.sort((a, z) => a - z)
  return { median: times[Math.floor(times.length / 2)], hash: last.hash }
}

function format(ms) {
  return ms >= 1000 ? `${(ms / 1000).toFixed(2)} s` : `${Math.round(ms)} ms`
}

const dirs = workloads.map((w) => w.dir())
const rows = []
const different = []
for (const version of opts.versions) {
  const cells = []
  workloads.forEach((workload, i) => {
    const native = measure(workload, dirs[i], version, 'native')
    const script = measure(workload, dirs[i], version, 'script')
    if (native.hash !== script.hash) different.push(`${version}, ${workload.name}`)
    cells.push(format(native.median), format(script.median))
  })
  rows.push(`| ${version} | ${cells.join(' | ')} |`)
  process.stderr.write(`versions.mjs: ${version}\n`)
}

const heads = workloads.flatMap((w) => [`${w.name}, native`, `${w.name}, script`])
console.log(`| Tailwind CSS | ${heads.join(' | ')} |`)
console.log(`|---|${heads.map(() => '---:').join('|')}|`)
for (const row of rows) console.log(row)
console.log()
console.log(`${os.type()} ${os.arch()}, ${os.cpus().length} CPUs. The median of ${opts.runs} runs.`)
console.log(
  different.length === 0
    ? 'The two engines give the same output in each cell.'
    : `The two engines give a different output for: ${different.join('; ')}.`,
)
