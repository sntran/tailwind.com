// The worker of the site: it runs tailwind.wasm, so that a build does not
// stop the page.
//
// Messages to the worker:
//   { type: 'start', wasm: URL, packages: URL }
//   { type: 'build', id, markup, css, version, engine: 'native' | 'script', minify }
// Messages from the worker:
//   { type: 'ready' }
//   { type: 'result', id, status, css, messages, ms }
//   { type: 'failure', id, message }
import { Files, run } from './wasi.mjs'

let module
const files = new Files()

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

async function start(message) {
  const [compiled, packages] = await Promise.all([
    WebAssembly.compileStreaming(fetch(message.wasm)),
    fetch(message.packages).then((response) => {
      if (!response.ok) throw new Error(`${message.packages}: ${response.status}`)
      return response.arrayBuffer()
    }),
  ])
  module = compiled
  unpack(packages)
}

async function build(message) {
  // A new project for each build: the markup and the input CSS only.
  if (files.exists('/project')) files.remove('/project')
  files.write('/project/index.html', message.markup)
  files.write('/project/app.css', message.css)
  const args = ['-i', 'app.css', '-o', 'out.css']
  if (message.minify) args.push('--minify')
  const env = { TAILWIND_COM_PACKAGES: '/packages', TAILWINDCSS_VERSION: message.version, TAILWIND_COM_ENGINE: message.engine, NO_COLOR: '1' }
  const start = performance.now()
  const result = await run(module, { args, env, files, preopens: { '/': '/project', '/packages': '/packages' } })
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
