#!/usr/bin/env node
// The npm package tailwind.com in Chromium: js/page.js (the default file on
// a CDN, as the Play CDN) and js/browser.mjs (the API of the condition
// "browser"). A local server gives the files at the paths of jsDelivr:
// "/npm/tailwind.com@0.0.0" is the default file, with no redirect.
//
// Usage: node tests/npm-browser.mjs [--wasm FILE] [--official FILE]
//
//   --wasm FILE      the module (default build/tailwind.wasm)
//   --official FILE  the browser build of Tailwind CSS
//                    (@tailwindcss/browser, dist/index.global.js): the
//                    page script must give the same CSS. With no file, no
//                    comparison.
//
// It needs Playwright with Chromium.
import assert from 'node:assert/strict'
import fs from 'node:fs'
import http from 'node:http'
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { wasmUrl } from '../js/browser.mjs'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.resolve(here, '..')

const opts = { wasm: path.join(root, 'build/tailwind.wasm'), official: null }
const args = process.argv.slice(2)
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--wasm') opts.wasm = path.resolve(args[++i])
  else if (args[i] === '--official') opts.official = path.resolve(args[++i])
  else throw new Error(`npm-browser.mjs: unknown argument ${args[i]}`)
}

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
const pages = {
  // The Play CDN form: a classic script and a <style type="text/tailwindcss">.
  '/page.html': `<!doctype html><html><head><meta charset="utf-8">
<script src="${prefix}"></script>
<style type="text/tailwindcss">
@plugin "@tailwindcss/typography";
@theme { --color-brand: #0d9488; }
</style></head>
<body><div id="box" class="flex p-4 bg-brand hover:underline"><article class="prose">Text</article></div></body></html>`,
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
  else if (url === `${prefix}/runtime/tailwind.wasm`) body = fs.readFileSync(opts.wasm)
  else if (url.startsWith(`${prefix}/js/`)) body = fs.readFileSync(path.join(root, 'js', path.basename(url)))
  else if (url === '/official.js' && opts.official) body = fs.readFileSync(opts.official)
  else if (url === '/compare.html') body = pages[url](new URL(request.url, 'http://localhost').searchParams.get('script'))
  else if (typeof pages[url] === 'string') body = pages[url]
  if (body === undefined) {
    response.writeHead(404).end()
    return
  }
  response.writeHead(200, { 'content-type': type ?? 'application/octet-stream' }).end(body)
})
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
const base = `http://127.0.0.1:${server.address().port}`

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

  if (opts.official) {
    await check('the page script gives the CSS of the browser build of Tailwind CSS', async () => {
      const css = []
      for (const script of [prefix, '/official.js']) {
        const tab = await browser.newPage()
        await tab.goto(`${base}/compare.html?script=${encodeURIComponent(script)}`)
        await settle(tab, () => [...document.head.querySelectorAll('style:not([type])')].at(-1)?.textContent.includes('.tab-4'))
        css.push(await pageCss(tab))
        await tab.close()
      }
      assert.equal(css[0], css[1])
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
