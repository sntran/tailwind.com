#!/usr/bin/env node
// The npm package tailwind.com in Chromium: js/page.js (the default file on
// a CDN, as the Play CDN) and js/browser.mjs (the API of the condition
// "browser"). A local server gives the files at the paths of jsDelivr:
// "/npm/tailwind.com@0.0.0" is the default file, with no redirect.
//
// Usage: node tests/npm-browser.mjs [--wasm FILE] [--small FILE] [--official FILE] [--cases DIR]
//
//   --wasm FILE      the module (default build/tailwind.wasm)
//   --small FILE     the module with no JavaScript engine and no optimizer
//                    (default: tailwind-small.wasm next to --wasm)
//   --official FILE  the browser build of Tailwind CSS
//                    (@tailwindcss/browser, dist/index.global.js): the
//                    page script must give the same CSS. With no file, no
//                    comparison.
//   --cases DIR      the recorded cases with JavaScript (default
//                    tests/upstream/cli-js): each one must give the output
//                    of the official program with tailwind-small.wasm, its
//                    plugins in the JavaScript engine of Chromium
//                    (js/engine.mjs). The cases with --minify are not for
//                    the small program.
//
// It needs Playwright with Chromium.
import assert from 'node:assert/strict'
import fs from 'node:fs'
import http from 'node:http'
import { createRequire } from 'node:module'
import path from 'node:path'
import os from 'node:os'
import { fileURLToPath } from 'node:url'
import { wasmUrl } from '../js/browser.mjs'
import { runWasm } from '../js/node.mjs'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.resolve(here, '..')

const opts = { wasm: path.join(root, 'build/tailwind.wasm'), small: null, official: null, cases: path.join(root, 'tests/upstream/cli-js') }
const args = process.argv.slice(2)
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--wasm') opts.wasm = path.resolve(args[++i])
  else if (args[i] === '--small') opts.small = path.resolve(args[++i])
  else if (args[i] === '--official') opts.official = path.resolve(args[++i])
  else if (args[i] === '--cases') opts.cases = path.resolve(args[++i])
  else throw new Error(`npm-browser.mjs: unknown argument ${args[i]}`)
}
opts.small ??= path.join(path.dirname(opts.wasm), 'tailwind-small.wasm')

// The downloads of each program.
const downloads = { 'tailwind.wasm': 0, 'tailwind-small.wasm': 0 }

// The URL of tailwind.wasm from the URL of a file of the package.
const cases = [
  ['https://cdn.jsdelivr.net/npm/tailwind.com@0.4', 'https://cdn.jsdelivr.net/npm/tailwind.com@0.4/runtime/tailwind.wasm'],
  ['https://cdn.jsdelivr.net/npm/tailwind.com@0.4.0/+esm', 'https://cdn.jsdelivr.net/npm/tailwind.com@0.4.0/runtime/tailwind.wasm'],
  ['https://cdn.jsdelivr.net/npm/tailwind.com/js/browser.mjs', 'https://cdn.jsdelivr.net/npm/tailwind.com/runtime/tailwind.wasm'],
  ['https://esm.sh/tailwind.com@0.4.0/es2022/tailwind.com.mjs', 'https://esm.sh/tailwind.com@0.4.0/runtime/tailwind.wasm'],
  ['https://unpkg.com/tailwind.com@0.4.0/js/page.js', 'https://unpkg.com/tailwind.com@0.4.0/runtime/tailwind.wasm'],
  ['http://localhost:5173/node_modules/tailwind.com/js/browser.mjs', 'http://localhost:5173/node_modules/tailwind.com/runtime/tailwind.wasm'],
  ['https://tailwind.com/assets/index-abc.js', 'https://tailwind.com/runtime/tailwind.wasm'],
]
for (const [from, want] of cases) assert.equal(wasmUrl(from), want, from)

const prefix = '/npm/tailwind.com@0.0.0'

// The files of a project, as on the disk: a link of the page gives
// /assets/app.css. The test changes them.
const assets = {
  '/assets/app.css': `@import "tailwindcss";
@import "./components.css";
@import "/theme/brand.css";
@plugin "@tailwindcss/typography";
@plugin "./plugins/ring";
.after-check { color: rgb(1, 2, 3); }
`,
  // A plugin with no extension, which imports a file with a path.
  '/assets/plugins/ring.js': "const { width } = require('./width')\nmodule.exports = function ({ addUtilities }) { addUtilities({ '.ring-brand': { 'outline-width': width } }) }\n",
  '/assets/plugins/width.js': "module.exports = { width: '3px' }\n",
  // A file of a <style> with a path, from the page /docs/style.html.
  '/docs/extra.css': '.extra-check { color: rgb(7, 8, 9); }\n',
  '/assets/components.css': '.card { @apply rounded-lg p-6 shadow-md; }\n',
  '/theme/brand.css': '@theme { --color-brand: #0d9488; }\n',
}
const linkBody = '<div id="box" class="card flex bg-brand hover:underline ring-brand"><article class="prose">Text</article></div><p class="after-check" id="after">z</p>'

const pages = {
  // A file with the input CSS, as for a build on the disk. The <style>
  // after the link must come after the output in the cascade.
  '/link.html': `<!doctype html><html><head><meta charset="utf-8">
<link rel="stylesheet" type="text/tailwindcss" href="/assets/app.css">
<style>.after-check { color: rgb(4, 5, 6); }</style>
<script src="${prefix}"></script></head>
<body>${linkBody}</body></html>`,
  // The Play CDN form: a classic script and a <style type="text/tailwindcss">.
  '/page.html': `<!doctype html><html><head><meta charset="utf-8">
<script src="${prefix}"></script>
<style type="text/tailwindcss">
@plugin "@tailwindcss/typography";
@theme { --color-brand: #0d9488; }
</style></head>
<body><div id="box" class="flex p-4 bg-brand hover:underline"><article class="prose">Text</article></div>
<script>window.atParse = { visibility: getComputedStyle(document.documentElement).visibility, display: getComputedStyle(document.getElementById('box')).display }</script></body></html>`,
  // The same page with the browser build of Tailwind CSS (no plugin there).
  '/compare.html': (script) => `<!doctype html><html><head><meta charset="utf-8">
<script src="${script}"></script>
<style type="text/tailwindcss">
@theme { --color-brand: #0d9488; }
@utility tab-4 { tab-size: 4; }
.card { @apply rounded-lg p-6 shadow-md; }
</style></head>
<body><div class="flex items-center gap-4 bg-brand/50 p-4 hover:underline md:grid-cols-2 tab-4 [mask-type:luminance] dark:text-white">x</div>
<p class="text-[length:var(--size)] -mt-2 lg:hover:bg-red-500 space-y-2 *:p-1 group-hover:opacity-50 group">y</p></body></html>`,
  // A <style> with a path: from the URL of the page.
  '/docs/style.html': `<!doctype html><html><head><meta charset="utf-8">
<script src="${prefix}"></script>
<style type="text/tailwindcss">
@import "tailwindcss";
@import "./extra.css";
</style></head>
<body><p id="extra" class="extra-check mt-2">x</p></body></html>`,
  // A recorded case: window.run(args, files, input, env) with the small
  // program.
  '/cases.html': `<!doctype html><meta charset="utf-8"><script type="module">
import { runWasm } from '${prefix}/js/browser.mjs'
window.run = (args, files, input, env) => {
  for (const [name, value] of Object.entries(files)) {
    if (typeof value !== 'string') files[name] = Uint8Array.from(atob(value.base64), (c) => c.charCodeAt(0))
  }
  return runWasm(args, { files, input, env: { NO_COLOR: '1', ...env }, wasm: '${prefix}/runtime/tailwind-small.wasm' })
}
window.ready = true
</script>`,
  // The API of the condition "browser", in a module and in a Web Worker.
  '/api.html': `<!doctype html><meta charset="utf-8"><script type="module">
import { runWasm } from '${prefix}/js/browser.mjs'
const files = { 'app.css': '@import "tailwindcss";', 'index.html': '<div class="flex p-4">' }
window.main = await runWasm(['-i', 'app.css', '--minify'], { files, env: { NO_COLOR: '1' } })
const worker = new Worker(URL.createObjectURL(new Blob([\`
  import { runWasm } from '\${new URL('${prefix}/js/browser.mjs', location.href)}'
  self.onmessage = async ({ data }) => self.postMessage(await runWasm(data.args, data.options))
\`], { type: 'text/javascript' })), { type: 'module' })
worker.postMessage({ args: ['-i', '-'], options: { input: '@import "tailwindcss";', files: { 'a.html': '<p class="underline">' } } })
window.worker = await new Promise((resolve) => (worker.onmessage = ({ data }) => resolve(data)))
</script>`,
}

const types = { '.js': 'text/javascript', '.mjs': 'text/javascript', '.wasm': 'application/wasm', '.html': 'text/html' }
const server = http.createServer((request, response) => {
  const url = new URL(request.url, 'http://localhost').pathname
  let body
  let type = types[path.extname(url)]
  if (url === prefix) [body, type] = [fs.readFileSync(path.join(root, 'js/page.js')), types['.js']]
  else if (url === `${prefix}/runtime/tailwind.wasm`) [body, downloads['tailwind.wasm']] = [fs.readFileSync(opts.wasm), downloads['tailwind.wasm'] + 1]
  else if (url === `${prefix}/runtime/tailwind-small.wasm`) [body, downloads['tailwind-small.wasm']] = [fs.readFileSync(opts.small), downloads['tailwind-small.wasm'] + 1]
  else if (url.startsWith(`${prefix}/js/`)) body = fs.readFileSync(path.join(root, 'js', path.basename(url)))
  // The JavaScript files of the program for tailwind-small.wasm.
  else if (/^\/runtime\/(compat|typescript|plugins\/[a-z-]+)\.js$/.test(url.slice(prefix.length)) && url.startsWith(prefix)) {
    ;[body, type] = [fs.readFileSync(path.join(root, 'native/tailwind/assets', url.slice(prefix.length + '/runtime/'.length))), types['.js']]
  }
  else if (url === '/official.js' && opts.official) body = fs.readFileSync(opts.official)
  else if (url === '/compare.html') body = pages[url](new URL(request.url, 'http://localhost').searchParams.get('script'))
  else if (typeof pages[url] === 'string') body = pages[url]
  else if (assets[url] !== undefined) [body, type] = [assets[url], url.endsWith('.js') ? 'text/javascript' : 'text/css']
  // A server for a single page gives its HTML for each other path.
  else if (url.startsWith('/assets/')) [body, type] = ['<!doctype html><title>App</title>', 'text/html']
  if (body === undefined) {
    response.writeHead(404).end()
    return
  }
  response.writeHead(200, { 'content-type': type ?? 'application/octet-stream' }).end(body)
})
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
const base = `http://127.0.0.1:${server.address().port}`

function listFiles(dir, prefix = '') {
  if (!fs.existsSync(dir)) return []
  return fs.readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const name = prefix + entry.name
    return entry.isDirectory() ? listFiles(path.join(dir, entry.name), name + '/') : [name]
  })
}

const require = createRequire(import.meta.url)
const { chromium } = require('playwright')
const browser = await chromium.launch()
const messages = []
let failed = 0
async function check(name, body) {
  try {
    await body()
    console.log(`npm-browser.mjs: ${name}: pass`)
  } catch (error) {
    failed++
    console.error(`npm-browser.mjs: ${name}: ${error.stack ?? error}`)
  }
}

// The CSS of the page script, after its builds end.
const pageCss = (tab) => tab.evaluate(() => [...document.head.querySelectorAll('style:not([type])')].at(-1)?.textContent ?? '')
async function settle(tab, test) {
  await tab.waitForFunction(test, null, { timeout: 30_000 })
  await tab.waitForTimeout(200)
}

try {
  await check('the page script, as the Play CDN', async () => {
    const tab = await browser.newPage()
    tab.on('console', (m) => messages.push(`page.html: ${m.text()}`))
    tab.on('pageerror', (e) => messages.push(`page.html: ${e}`))
    const builds = []
    await tab.exposeFunction('build', (status) => builds.push(status))
    await tab.addInitScript(() => document.addEventListener('tailwind.com:build', (e) => window.build(e.detail.status)))
    await tab.goto(`${base}/page.html`)
    await settle(tab, () => getComputedStyle(document.getElementById('box')).display === 'flex')
    // The first visit: the page is hidden until the first CSS.
    assert.deepEqual(await tab.evaluate(() => window.atParse), { visibility: 'hidden', display: 'block' })
    assert.equal(await tab.evaluate(() => getComputedStyle(document.documentElement).visibility), 'visible')
    // The next visit: the CSS of the last build styles the page at once.
    await tab.reload()
    assert.deepEqual(await tab.evaluate(() => window.atParse), { visibility: 'visible', display: 'flex' })
    await settle(tab, () => getComputedStyle(document.getElementById('box')).display === 'flex')
    const css = await pageCss(tab)
    for (const rule of ['.bg-brand {', '--color-brand: #0d9488;', '.prose {', '.hover\\:underline:hover {']) assert.ok(css.includes(rule), rule)
    assert.equal(await tab.evaluate(() => getComputedStyle(document.getElementById('box')).paddingTop), '16px')

    // A new element with a new class.
    await tab.evaluate(() => document.body.insertAdjacentHTML('beforeend', '<p id="new" class="mt-8">New</p>'))
    await settle(tab, () => getComputedStyle(document.getElementById('new')).marginTop === '32px')
    // A new class of an element. A class that goes stays in the CSS.
    await tab.evaluate(() => (document.getElementById('box').className = 'grid'))
    await settle(tab, () => getComputedStyle(document.getElementById('box')).display === 'grid')
    assert.ok((await pageCss(tab)).includes('.bg-brand {'))
    // A change of the stylesheet.
    await tab.evaluate(() => (document.querySelector('style[type="text/tailwindcss"]').textContent += '\n@theme { --color-brand: #ff0000; }'))
    await settle(tab, () => [...document.head.querySelectorAll('style:not([type])')].at(-1).textContent.includes('--color-brand: #ff0000;'))
    // A change that adds no class does not build.
    const count = builds.length
    await tab.evaluate(() => document.body.insertAdjacentHTML('beforeend', '<p class="mt-8">Again</p>'))
    await tab.waitForTimeout(300)
    assert.equal(builds.length, count)
    assert.ok(builds.every((status) => status === 0), String(builds))
    await tab.close()
  })

  await check('the page script with a <link> to a file: the CSS of a build on the disk', async () => {
    const tab = await browser.newPage()
    tab.on('console', (m) => messages.push(`link.html: ${m.text()}`))
    tab.on('pageerror', (e) => messages.push(`link.html: ${e}`))
    await tab.goto(`${base}/link.html`)
    await settle(tab, () => getComputedStyle(document.getElementById('box')).display === 'flex')
    // The browser does not apply the file itself.
    assert.equal(await tab.evaluate(() => document.querySelector('link').sheet), null)
    // The output has the place of the link: the <style> after it wins.
    assert.equal(await tab.evaluate(() => document.querySelector('link').nextElementSibling.textContent.includes('.card {')), true)
    assert.equal(await tab.evaluate(() => getComputedStyle(document.getElementById('after')).color), 'rgb(4, 5, 6)')
    assert.equal(await tab.evaluate(() => getComputedStyle(document.getElementById('box')).paddingTop), '24px')
    assert.equal(await tab.evaluate(() => getComputedStyle(document.getElementById('box')).outlineWidth), '3px')

    // The same files and classes on the disk, with the program.
    const css = await tab.evaluate(() => document.querySelector('link').nextElementSibling.textContent)
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'npm-browser-'))
    try {
      for (const [name, text] of Object.entries(assets)) {
        fs.mkdirSync(path.join(dir, path.dirname(name)), { recursive: true })
        fs.writeFileSync(path.join(dir, name), text)
      }
      fs.writeFileSync(path.join(dir, 'assets/index.html'), linkBody)
      const local = await runWasm(['-i', 'assets/app.css'], { cwd: dir, wasm: opts.wasm, env: { NO_COLOR: '1' } })
      assert.equal(local.status, 0, local.stderr)
      assert.equal(css, local.stdout.replace(/\n$/, ''))
    } finally {
      fs.rmSync(dir, { recursive: true, force: true })
    }

    // A file changes on the server, with no change of the page.
    assets['/theme/brand.css'] = '@theme { --color-brand: #ff0000; }\n'
    await settle(tab, () => getComputedStyle(document.getElementById('box')).backgroundColor === 'rgb(255, 0, 0)')
    // A live reload: a change of href.
    assets['/theme/brand.css'] = '@theme { --color-brand: #00ff00; }\n'
    await tab.evaluate(() => (document.querySelector('link').href = '/assets/app.css?v=2'))
    await settle(tab, () => getComputedStyle(document.getElementById('box')).backgroundColor === 'rgb(0, 255, 0)')
    // The plugins ran in the JavaScript engine of the browser: no full
    // program, and the realm of each build is gone.
    assert.equal(downloads['tailwind.wasm'], 0)
    assert.equal(await tab.evaluate(() => document.querySelectorAll('iframe[data-tailwind-com]').length), 0)
    await tab.close()
  })

  await check('the page script with a path in a <style>: from the URL of the page', async () => {
    const tab = await browser.newPage()
    tab.on('pageerror', (e) => messages.push(`style.html: ${e}`))
    await tab.goto(`${base}/docs/style.html`)
    await settle(tab, () => getComputedStyle(document.getElementById('extra')).marginTop === '8px')
    assert.equal(await tab.evaluate(() => getComputedStyle(document.getElementById('extra')).color), 'rgb(7, 8, 9)')
    await tab.close()
  })

  await check('the API of the condition "browser", in a module and in a Web Worker', async () => {
    const tab = await browser.newPage()
    tab.on('pageerror', (e) => messages.push(`api.html: ${e}`))
    await tab.goto(`${base}/api.html`)
    await tab.waitForFunction(() => window.main && window.worker, null, { timeout: 30_000 })
    const { main, worker } = await tab.evaluate(() => ({ main: window.main, worker: window.worker }))
    assert.equal(main.status, 0, main.stderr)
    assert.ok(main.stdout.includes('.flex{display:flex}'), main.stdout.slice(-300))
    assert.ok(main.stdout.includes('.p-4{padding:calc(var(--spacing) * 4)}'), main.stdout.slice(-300))
    assert.equal(worker.status, 0, worker.stderr)
    assert.ok(worker.stdout.includes('.underline {'), worker.stdout.slice(-300))
    await tab.close()
  })

  await check('the recorded cases with JavaScript: plugins in the JavaScript engine of the browser', async () => {
    const tab = await browser.newPage()
    tab.on('pageerror', (e) => messages.push(`cases.html: ${e}`))
    await tab.goto(`${base}/cases.html`)
    await tab.waitForFunction(() => window.ready === true)
    const failures = []
    let count = 0
    for (const name of fs.readdirSync(opts.cases).sort()) {
      const dir = path.join(opts.cases, name)
      if (!fs.existsSync(path.join(dir, 'args.txt'))) continue
      const args = fs.readFileSync(path.join(dir, 'args.txt'), 'utf8').split('\n').filter((line) => line !== '')
      if (args.some((arg) => arg === '--minify' || arg === '-m' || arg === '--optimize' || arg.startsWith('--map'))) continue
      // The project: text as text, other bytes as Base64.
      const shared = path.join(dir, 'project.txt')
      const source = fs.existsSync(shared) ? path.join(opts.cases, fs.readFileSync(shared, 'utf8').trim(), 'project') : path.join(dir, 'project')
      const files = {}
      for (const file of listFiles(source)) {
        const bytes = fs.readFileSync(path.join(source, file))
        try {
          files[file] = new TextDecoder('utf-8', { fatal: true }).decode(bytes)
        } catch {
          files[file] = { base64: bytes.toString('base64') }
        }
      }
      const env = {}
      if (fs.existsSync(path.join(dir, 'env.txt'))) {
        for (const line of fs.readFileSync(path.join(dir, 'env.txt'), 'utf8').split('\n')) {
          const index = line.indexOf('=')
          if (index > 0) env[line.slice(0, index)] = line.slice(index + 1).trim().replaceAll('<project>/', '/').replaceAll('<project>', '/')
        }
      }
      const stdin = path.join(dir, 'stdin.txt')
      const input = fs.existsSync(stdin) ? fs.readFileSync(stdin, 'utf8') : ''
      const result = await tab.evaluate(([args, files, input, env]) => window.run(args, files, input, env), [args, files, input, env])
      count++
      const status = Number(fs.readFileSync(path.join(dir, 'exit.txt'), 'utf8'))
      const expected = fs.readFileSync(path.join(dir, 'stdout.css'), 'utf8').replaceAll('/PROJECT/', '/').replaceAll('PROJECT/', '')
      // The lines of the error output of the official program, as in
      // tests/wasi.mjs.
      const contains = path.join(dir, 'stderr-contains.txt')
      let lines = []
      if (fs.existsSync(contains)) lines = fs.readFileSync(contains, 'utf8').split('\n')
      else if (fs.existsSync(path.join(dir, 'stderr.txt'))) {
        lines = fs.readFileSync(path.join(dir, 'stderr.txt'), 'utf8').replace(/\x1b\[[0-9;]*m/g, '').split('\n').filter((line) => line.startsWith('│'))
      }
      const missing = lines.map((line) => line.replaceAll('<project>/', '/').replaceAll('<project>', '/')).filter((line) => line !== '' && !result.stderr.includes(line))
      if (result.status !== status || result.stdout !== expected || missing.length > 0) {
        failures.push(`${name}: status ${result.status} (${status}), output ${result.stdout === expected ? 'same' : 'different'}${missing.length ? `, no "${missing[0]}"` : ''}\n${result.stderr.split('\n').slice(0, 6).join('\n')}`)
      }
    }
    await tab.close()
    assert.ok(count > 90, `${count} cases`)
    assert.equal(failures.length, 0, `${failures.length} of ${count} cases failed:\n${failures.join('\n')}`)
    console.log(`npm-browser.mjs: ${count} recorded cases with JavaScript: the same output`)
  })

  if (opts.official) {
    await check('the page script gives the CSS of the browser build of Tailwind CSS', async () => {
      const css = []
      const before = { ...downloads }
      for (const script of [prefix, '/official.js']) {
        const tab = await browser.newPage()
        await tab.goto(`${base}/compare.html?script=${encodeURIComponent(script)}`)
        await settle(tab, () => [...document.head.querySelectorAll('style:not([type])')].at(-1)?.textContent.includes('.tab-4'))
        css.push(await pageCss(tab))
        await tab.close()
      }
      assert.equal(css[0], css[1])
      // A page with no @plugin gets only the small program.
      assert.equal(downloads['tailwind.wasm'], before['tailwind.wasm'])
      assert.ok(downloads['tailwind-small.wasm'] > before['tailwind-small.wasm'])
    })
  }
} finally {
  await browser.close()
  server.close()
}
for (const m of messages) console.error(`npm-browser.mjs: console: ${m}`)
if (failed > 0) {
  console.error(`npm-browser.mjs: ${failed} failed`)
  process.exit(1)
}
console.log('npm-browser.mjs: all pass')
