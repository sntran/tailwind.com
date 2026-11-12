#!/usr/bin/env node
// Benchmarks of tailwind.com against the official programs of Tailwind CSS.
// The result is a Markdown table on standard output.
//
// Usage:
//   node tests/bench/run.mjs --official DIR [--program FILE] [--native FILE]
//        [--wasm FILE] [--standalone FILE] [--runs N] [--only NAME,...]
//
//   --official DIR     the official npm packages (tests/fuzz/install.sh DIR)
//   --program FILE     tailwind.com (default build/tailwind.com, when it is there)
//   --native FILE      the program for this computer (target/release/tailwind)
//   --wasm FILE        tailwind.wasm, run in Node.js with the API of js/node.mjs
//   --standalone FILE  the official standalone program (tests/standalone.sh --download)
//   --script           also tailwind.com with its script engine (it needs the
//                      package of the version of UPSTREAM: tailwind.com has it)
//   --runs N           the timed runs of each workload (default 7)
//   --only LIST        only these workloads
//
// Each workload runs one time with no time, then N times. The table has the
// median, and the fastest time in parentheses. The times are for programs on
// the same machine at the same time: CI machines are shared, so a
// difference of less than about 10% does not tell much.
//
// Each output is also compared with the output of the official npm CLI: a
// column with a different output has the mark "≠" in its cell.
import { spawn, spawnSync } from 'node:child_process'
import fs from 'node:fs'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import zlib from 'node:zlib'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.resolve(here, '../..')

function options() {
  const args = process.argv.slice(2)
  const o = { runs: 7, only: null }
  for (let i = 0; i < args.length; i++) {
    const value = () => args[++i]
    switch (args[i]) {
      case '--official': o.official = path.resolve(value()); break
      case '--program': o.program = path.resolve(value()); break
      case '--native': o.native = path.resolve(value()); break
      case '--wasm': o.wasm = path.resolve(value()); break
      case '--standalone': o.standalone = path.resolve(value()); break
      case '--script': o.script = true; break
      case '--runs': o.runs = Number(value()); break
      case '--only': o.only = value().split(','); break
      default: throw new Error(`unknown argument: ${args[i]}`)
    }
  }
  if (!o.official) throw new Error('--official is necessary')
  if (!o.program && fs.existsSync(path.join(root, 'build/tailwind.com'))) o.program = path.join(root, 'build/tailwind.com')
  return o
}

const opts = options()
const cli = path.join(opts.official, 'node_modules/@tailwindcss/cli/dist/index.mjs')
const interop = pathToFileURL(path.join(root, 'tests/upstream/tools/bun-interop.mjs')).href
const windows = process.platform === 'win32'

// The programs: a name, and the command for the arguments of a run.
const programs = []
if (opts.program) {
  programs.push({
    name: 'tailwind.com',
    command: (args) => (windows || !opts.program.endsWith('.com') ? [opts.program, ...args] : ['sh', opts.program, ...args]),
  })
}
// The same file with its second engine: the official compiler of the
// package of this version, in the JavaScript engine of the file
// (docs/VERSIONS.md). A version that is newer than the port has this
// speed. tests/bench/versions.mjs measures the older versions.
if (opts.program && opts.script) {
  programs.push({
    name: 'tailwind.com, script engine',
    command: programs[0].command,
    env: { TAILWIND_COM_ENGINE: 'script' },
  })
}
if (opts.native) programs.push({ name: 'native', command: (args) => [opts.native, ...args] })
if (opts.wasm) {
  const runner = path.join(here, 'wasm.mjs')
  programs.push({ name: 'tailwind.wasm', command: (args) => [process.execPath, '--no-warnings', runner, opts.wasm, ...args], wasm: true })
}
// The official programs. The npm CLI gets the module rules of the standalone
// program (as the records of tests/upstream).
programs.push({ name: 'official npm CLI', command: (args) => [process.execPath, '--import', interop, cli, ...args], official: true })
if (opts.standalone) programs.push({ name: 'official standalone', command: (args) => [opts.standalone, ...args] })

// --- The projects --------------------------------------------------------

// The projects are next to node_modules of the official packages: the
// official programs resolve `@import "tailwindcss"` and the plugins from
// there.
const work = fs.mkdtempSync(path.join(opts.official, 'bench-'))
process.on('exit', () => fs.rmSync(work, { recursive: true, force: true }))

function copyProject(from, name) {
  const dir = path.join(work, name)
  fs.cpSync(path.join(root, from), dir, { recursive: true })
  return dir
}

// A large project: 1000 files with 30 classes each, from the record of the
// classes of Tailwind CSS (the same each time).
function largeProject() {
  const dir = path.join(work, 'large')
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
  return dir
}

const workloads = [
  {
    name: 'start: --help',
    what: 'the start of the program, with no build',
    dir: () => work,
    args: ['--help'],
    compare: false,
  },
  {
    name: 'preflight only',
    what: '`@import "tailwindcss" source(none)`: the theme and preflight, no classes',
    dir: () => {
      const dir = path.join(work, 'empty')
      fs.mkdirSync(dir, { recursive: true })
      fs.writeFileSync(path.join(dir, 'app.css'), '@import "tailwindcss" source(none);\n')
      return dir
    },
    args: ['-i', 'app.css'],
  },
  {
    name: 'Phoenix',
    what: 'the Phoenix project of `tests/upstream/cli/phoenix` (no JavaScript)',
    dir: () => copyProject('tests/upstream/cli/phoenix/project', 'phoenix'),
    args: ['-i', 'assets/css/app.css'],
  },
  {
    name: 'Phoenix, --minify',
    what: 'the same, with `--minify` (Lightning CSS)',
    dir: () => path.join(work, 'phoenix'),
    args: ['-i', 'assets/css/app.css', '--minify'],
  },
  {
    name: 'Phoenix 1.8.9 with daisyUI',
    what: '`mix phx.new` of Phoenix 1.8.9: daisyUI and heroicons as JavaScript plugins',
    dir: () => copyProject('tests/upstream/cli-js/phoenix-deps/project', 'phoenix-deps'),
    env: (dir) => ({ NODE_PATH: [path.join(dir, 'deps'), path.join(dir, '_build/dev')].join(path.delimiter) }),
    args: ['--input=assets/css/app.css'],
  },
  {
    name: 'typography and forms',
    what: '`@tailwindcss/typography` and `@tailwindcss/forms`, from `tests/upstream/cli-js`',
    dir: () => {
      const dir = path.join(work, 'plugins')
      fs.mkdirSync(dir, { recursive: true })
      fs.writeFileSync(
        path.join(dir, 'app.css'),
        '@import "tailwindcss" source(none);\n@source "./index.html";\n@plugin "@tailwindcss/typography";\n@plugin "@tailwindcss/forms";\n',
      )
      fs.writeFileSync(
        path.join(dir, 'index.html'),
        '<article class="prose prose-lg prose-slate dark:prose-invert"><input class="form-input rounded-md"></article>\n',
      )
      return dir
    },
    args: ['-i', 'app.css'],
  },
  {
    name: 'large project',
    what: '1000 HTML files with 30 classes each (about 5600 different classes)',
    dir: largeProject,
    args: ['-i', 'app.css'],
  },
  {
    name: 'large project, --minify',
    what: 'the same, with `--minify`',
    dir: () => path.join(work, 'large'),
    args: ['-i', 'app.css', '--minify'],
  },
]

// --- The measures --------------------------------------------------------

function run(program, workload, dir) {
  const [command, ...args] = program.command(workload.args)
  const start = process.hrtime.bigint()
  const result = spawnSync(command, args, {
    cwd: dir,
    env: { ...process.env, NO_COLOR: '1', ...(workload.env ? workload.env(dir) : {}), ...(program.env ?? {}) },
    maxBuffer: 512 * 1024 * 1024,
  })
  const ms = Number(process.hrtime.bigint() - start) / 1e6
  return { ms, status: result.status, stdout: result.stdout?.toString() ?? '', stderr: result.stderr?.toString() ?? '' }
}

function median(values) {
  const sorted = [...values].sort((a, z) => a - z)
  return sorted[Math.floor(sorted.length / 2)]
}

function format(ms) {
  return ms >= 1000 ? `${(ms / 1000).toFixed(2)} s` : `${Math.round(ms)} ms`
}

// The time from an edit of a source file to a new output, in --watch. The
// median of 5 edits.
async function watchLatency(program, dir) {
  const [command, ...args] = program.command(['-i', 'app.css', '-o', 'out.css', '--watch'])
  const child = spawn(command, args, { cwd: dir, env: { ...process.env, NO_COLOR: '1', ...(program.env ?? {}) }, stdio: ['pipe', 'ignore', 'ignore'] })
  const out = path.join(dir, 'out.css')
  const read = () => {
    try {
      return fs.readFileSync(out, 'utf8')
    } catch {
      return ''
    }
  }
  const until = async (test, limit = 20000) => {
    const end = Date.now() + limit
    while (Date.now() < end) {
      if (test()) return true
      await new Promise((r) => setTimeout(r, 2))
    }
    return false
  }
  const times = []
  try {
    if (!(await until(() => read().includes('.flex')))) return null
    await new Promise((r) => setTimeout(r, 300))
    for (let i = 0; i < 5; i++) {
      const cls = `mt-[${101 + i}px]`
      const start = process.hrtime.bigint()
      fs.writeFileSync(path.join(dir, 'src', 'edit.html'), `<p class="flex ${cls}"></p>\n`)
      const marker = `mt-\\[${101 + i}px\\]`
      if (!(await until(() => read().includes(marker)))) return null
      times.push(Number(process.hrtime.bigint() - start) / 1e6)
      await new Promise((r) => setTimeout(r, 300))
    }
  } finally {
    child.stdin.end()
    child.kill()
  }
  return median(times)
}

function sizeOf(file) {
  try {
    return fs.statSync(file).size
  } catch {
    return null
  }
}

function dirSize(dir) {
  let total = 0
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const p = path.join(dir, entry.name)
    if (entry.isDirectory()) total += dirSize(p)
    else if (entry.isFile()) total += fs.statSync(p).size
  }
  return total
}

// --- The run -------------------------------------------------------------

const rows = []
for (const workload of workloads) {
  if (opts.only && !opts.only.includes(workload.name)) continue
  const dir = workload.dir()
  const cells = []
  let reference = null
  for (const program of programs) {
    if (program.wasm && workload.name.startsWith('start')) {
      cells.push('')
      continue
    }
    run(program, workload, dir)
    const times = []
    let last
    for (let i = 0; i < opts.runs; i++) {
      last = run(program, workload, dir)
      times.push(last.ms)
    }
    let cell = `${format(median(times))} (${format(Math.min(...times))})`
    if (last.status !== 0 && !workload.name.startsWith('start')) cell = `error ${last.status}`
    if (program.official && !program.name.includes('standalone')) reference = last.stdout
    cells.push({ cell, stdout: last.stdout })
  }
  // The mark for an output that is not the output of the npm CLI.
  const row = cells.map((c) => {
    if (typeof c === 'string') return c
    if (workload.compare === false || reference === null || c.stdout === reference) return c.cell
    return `${c.cell} ≠`
  })
  rows.push([workload.name, ...row])
}

// The watch.
if (!opts.only || opts.only.includes('watch')) {
  const cells = []
  for (const program of programs) {
    if (program.wasm) {
      cells.push('')
      continue
    }
    const dir = path.join(work, `watch-${cells.length}`)
    fs.mkdirSync(path.join(dir, 'src'), { recursive: true })
    fs.writeFileSync(path.join(dir, 'app.css'), '@import "tailwindcss" source(none);\n@source "./src";\n')
    fs.writeFileSync(path.join(dir, 'src', 'edit.html'), '<p class="flex"></p>\n')
    const ms = await watchLatency(program, dir)
    cells.push(ms === null ? 'no new output' : format(ms))
  }
  rows.push(['watch: an edit to a new output', ...cells])
}

// The peak memory of a build (the largest resident set), with
// getrusage of Python, where it is there (not on Windows).
function peakMemory(program, workload, dir) {
  const [command, ...args] = program.command(workload.args)
  const script =
    'import resource, subprocess, sys\n' +
    'subprocess.run(sys.argv[1:], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)\n' +
    'r = resource.getrusage(resource.RUSAGE_CHILDREN).ru_maxrss\n' +
    'print(r if sys.platform == "darwin" else r * 1024)\n'
  const result = spawnSync('python3', ['-I', '-c', script, command, ...args], {
    cwd: dir,
    env: { ...process.env, NO_COLOR: '1', ...(workload.env ? workload.env(dir) : {}), ...(program.env ?? {}) },
  })
  const bytes = Number(result.stdout?.toString().trim())
  return result.status === 0 && bytes > 0 ? bytes : null
}

if (!windows && spawnSync('python3', ['-c', 'import resource']).status === 0) {
  for (const name of ['Phoenix 1.8.9 with daisyUI', 'large project']) {
    const workload = workloads.find((w) => w.name === name)
    if (opts.only && !opts.only.includes(name)) continue
    const dir = workload.name === 'large project' ? path.join(work, 'large') : path.join(work, 'phoenix-deps')
    if (!fs.existsSync(dir)) continue
    const cells = programs.map((program) => {
      const bytes = peakMemory(program, workload, dir)
      return bytes === null ? '' : `${Math.round(bytes / 1048576)} MB`
    })
    rows.push([`peak memory: ${name}`, ...cells])
  }
}

// The sizes.
const mb = (bytes) => (bytes === null ? '' : `${(bytes / 1048576).toFixed(1)} MB`)
const sizes = programs.map((p) => {
  if (p.name.startsWith('tailwind.com')) return mb(sizeOf(opts.program))
  if (p.name === 'native') return mb(sizeOf(opts.native))
  if (p.wasm) return mb(sizeOf(opts.wasm))
  if (p.name === 'official standalone') return mb(sizeOf(opts.standalone))
  // The npm CLI: its packages, with no Node.js.
  return `${mb(dirSize(path.join(opts.official, 'node_modules')))} + Node.js`
})
rows.push(['size on disk', ...sizes])

console.log(`| Workload | ${programs.map((p) => p.name).join(' | ')} |`)
console.log(`|---|${programs.map(() => '---:').join('|')}|`)
for (const row of rows) console.log(`| ${row.join(' | ')} |`)
console.log()
console.log(`${os.type()} ${os.release()} ${os.arch()}, ${os.cpus().length} CPUs, Node.js ${process.version}. The median of ${opts.runs} runs, and the fastest in parentheses.`)
console.log()
for (const w of workloads) if (!opts.only || opts.only.includes(w.name)) console.log(`- **${w.name}**: ${w.what}.`)
console.log('- **watch: an edit to a new output**: `--watch`, then the time from the write of a source file to the new class in the output (the median of 5 edits).')
console.log('- **peak memory**: the largest resident set of the process tree of one build (for `tailwind.wasm` and the npm CLI, Node.js too).')
console.log('- **size on disk**: the program. For the npm CLI, its `node_modules` (with the plugins of the benchmarks), and Node.js is more.')
