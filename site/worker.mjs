// The worker of the site: it runs tailwind.wasm, so that a build does not
// stop the page.
//
// Messages to the worker:
//   { type: 'start', wasm: URL, packages: URL }
//   { type: 'build', id, wasm: URL, markup, css, version, engine: 'native' | 'script', minify }
// Messages from the worker:
//   { type: 'ready' }
//   { type: 'result', id, status, css, messages, ms, missing }
//   { type: 'failure', id, message }
//
// `wasm` of a build selects the program: the build of the site, or the
// tailwind.wasm of a release. The worker compiles each program one time,
// when a build first asks for it. `missing` is the version line of a
// program that does not have the selected version or engine: an older
// program can ignore TAILWINDCSS_VERSION and TAILWIND_COM_ENGINE, and then
// build its own version.
import { Files, run } from './wasi.mjs'

const files = new Files()
const modules = new Map()
const checked = new Map()

// packages.bin of site/build.mjs.
function unpack(buffer) {
  const view = new DataView(buffer)
  const length = view.getUint32(0, true)
  const index = JSON.parse(new TextDecoder().decode(new Uint8Array(buffer, 4, length)))
  let at = 4 + length
  for (const [name, size] of index) {
    files.write(`/packages/${name}`, new Uint8Array(buffer, at, size))
    at += size
  }
}

function program(url) {
  if (!modules.has(url)) {
    const compiled = WebAssembly.compileStreaming(fetch(url))
    // A failed download can be tried again.
    compiled.catch(() => modules.delete(url))
    modules.set(url, compiled)
  }
  return modules.get(url)
}

async function start(message) {
  const [, packages] = await Promise.all([
    program(message.wasm),
    fetch(message.packages).then((response) => {
      if (!response.ok) throw new Error(`${message.packages}: ${response.status}`)
      return response.arrayBuffer()
    }),
  ])
  unpack(packages)
}

const environment = (message) => ({
  TAILWIND_COM_PACKAGES: '/packages',
  TAILWINDCSS_VERSION: message.version,
  TAILWIND_COM_ENGINE: message.engine,
  NO_COLOR: '1',
})
const preopens = { '/': '/project', '/packages': '/packages' }

// The version line of the program for the selection, and whether it is
// the selection: "tailwind.com v0.2.0 (tailwindcss v4.1.0, script engine)".
async function check(module, message) {
  const key = `${message.wasm} ${message.version} ${message.engine}`
  if (!checked.has(key)) {
    const result = await run(module, { args: ['--version'], env: environment(message), files, preopens })
    const line = (result.stdout + result.stderr).trim()
    const found = line.match(/\(tailwindcss v([^,)]+)(, script engine)?\)/)
    const same = result.status === 0 && found?.[1] === message.version && Boolean(found[2]) === (message.engine === 'script')
    checked.set(key, { same, line })
  }
  return checked.get(key)
}

async function build(message) {
  const module = await program(message.wasm)
  // A new project for each build: the markup and the input CSS only.
  if (files.exists('/project')) files.remove('/project')
  files.write('/project/index.html', message.markup)
  files.write('/project/app.css', message.css)
  const selection = await check(module, message)
  if (!selection.same) return { status: -2, css: '', messages: '', ms: 0, missing: selection.line }
  const args = ['-i', 'app.css', '-o', 'out.css']
  if (message.minify) args.push('--minify')
  const start = performance.now()
  const result = await run(module, { args, env: environment(message), files, preopens })
  const ms = performance.now() - start
  const css = result.status === 0 && files.exists('/project/out.css') ? files.readText('/project/out.css') : ''
  // The program writes its version line and its time to stderr. The page
  // shows only the other lines: warnings and errors.
  const messages = result.stderr
    .split('\n')
    .filter((line) => line.trim() && !line.startsWith('≈ tailwindcss') && !/^Done in /.test(line))
    .join('\n')
  return { status: result.status, css, messages, ms }
}

// One message at a time, in order.
let queue = Promise.resolve()
self.onmessage = ({ data }) => {
  queue = queue.then(async () => {
    try {
      if (data.type === 'start') {
        await start(data)
        self.postMessage({ type: 'ready' })
      } else if (data.type === 'build') {
        self.postMessage({ type: 'result', id: data.id, ...(await build(data)) })
      }
    } catch (error) {
      self.postMessage({ type: 'failure', id: data.id, message: String(error?.stack ?? error) })
    }
  })
}
