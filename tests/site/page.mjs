#!/usr/bin/env node
// The page of the site in Chromium: the directory of site/build.mjs.
//
// Usage: node tests/site/page.mjs [--site DIR] [--screenshot FILE]
//        [--cases all|N] [--versions all|A,B,...]
//
//   --site DIR          the site (default build/site)
//   --screenshot FILE   a picture of the page after the first example
//   --cases all|N       the recorded cases to open (default all)
//   --versions LIST     the versions for the mode "Rust and official"
//                       (default all)
//
// The test opens the page, each example, the recorded cases and the mode
// "Rust and official" for the versions. Each build must end with no
// error, and the console must have no error. For each example and each
// version in the mode "Rust and official", the two compilers must give
// the same output. Each recorded case that it opens must show a
// difference between its two versions.
//
// CI opens only some cases and versions: site/build.mjs already finds the
// cases that show their change, with the same host in Node.js, and
// tests/versions.sh compares the two engines for each version.
//
// It needs Playwright with Chromium.
import fs from 'node:fs'
import http from 'node:http'
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const here = path.dirname(fileURLToPath(import.meta.url))
const root = path.resolve(here, '../..')
const opts = { site: path.join(root, 'build/site'), screenshot: null, cases: 'all', versions: 'all' }
const args = process.argv.slice(2)
for (let i = 0; i < args.length; i++) {
  if (args[i] === '--site') opts.site = path.resolve(args[++i])
  else if (args[i] === '--screenshot') opts.screenshot = path.resolve(args[++i])
  else if (args[i] === '--cases') opts.cases = args[++i]
  else if (args[i] === '--versions') opts.versions = args[++i]
  else throw new Error(`unknown argument: ${args[i]}`)
}

const types = { '.html': 'text/html', '.mjs': 'text/javascript', '.css': 'text/css', '.json': 'application/json', '.wasm': 'application/wasm' }
const server = http.createServer((request, response) => {
  const name = new URL(request.url, 'http://localhost').pathname.slice(1) || 'index.html'
  const file = path.join(opts.site, name)
  if (!file.startsWith(opts.site) || !fs.existsSync(file) || fs.statSync(file).isDirectory()) {
    response.writeHead(404).end()
    return
  }
  response.writeHead(200, { 'content-type': types[path.extname(file)] ?? 'application/octet-stream' }).end(fs.readFileSync(file))
})
await new Promise((resolve) => server.listen(0, '127.0.0.1', resolve))
const base = `http://127.0.0.1:${server.address().port}/`

const require = createRequire(import.meta.url)
const { chromium } = require('playwright')
const browser = await chromium.launch()
const problems = []
const log = (text) => console.log(`page.mjs: ${text}`)
try {
  const page = await browser.newPage({ viewport: { width: 1400, height: 1000 } })
  // The init script below also goes into the preview frames, and their
  // sandbox blocks it: that message is of the test, not of the page.
  // A preview frame can ask for files that the markup or the CSS of a case
  // names (url(x.png)): such a failed load is not a fault of the page.
  const own = new Set(fs.readdirSync(opts.site).map((name) => `${base}${name}`).concat(base))
  page.on('console', (m) => {
    if (m.type() !== 'error' || m.text().includes("'about:srcdoc'")) return
    const url = m.location().url
    if (/^Failed to load resource|^Access to image/.test(m.text()) && !own.has(url)) return
    problems.push(`console: ${m.text()} (${url})`)
  })
  page.on('pageerror', (e) => problems.push(`page: ${e}`))
  // The count of the results that the page shows.
  await page.addInitScript(() => {
    window.addEventListener('DOMContentLoaded', () => {
      const status = document.getElementById('status')
      window.shown = 0
      new MutationObserver(() => {
        if (!status.textContent.startsWith('Building') && !status.textContent.startsWith('Loading')) window.shown++
      }).observe(status, { childList: true, characterData: true, subtree: true })
    })
  })
  const start = Date.now()
  await page.goto(base)
  await page.waitForFunction(() => window.shown > 0, null, { timeout: 60000 })
  log(`the first result after ${Date.now() - start} ms`)

  // Do an action, then wait for its result.
  async function after(action) {
    const count = await page.evaluate(() => window.shown)
    await action()
    await page.waitForFunction((n) => window.shown > n, count, { timeout: 60000 })
    return page.evaluate(() => ({
      status: document.getElementById('status').textContent,
      messages: document.getElementById('messages').hidden ? '' : document.getElementById('messages').textContent,
      output: document.querySelector('#output span')?.textContent ?? document.getElementById('output').textContent,
      tabs: [...document.querySelectorAll('#tabs button')].map((b) => b.textContent),
    }))
  }

  const status = await page.evaluate(() => document.getElementById('status').textContent)
  if (/error/.test(status)) problems.push(`the first example: ${status}`)
  if (opts.screenshot) {
    await page.screenshot({ path: opts.screenshot, fullPage: true })
    log(`wrote ${opts.screenshot}`)
  }

  // Each example.
  const data = await page.evaluate(() => fetch('data.json').then((r) => r.json()))
  for (const [i, example] of data.examples.entries()) {
    const result = await after(() => page.selectOption('#example', String(i)))
    if (/error/.test(result.status)) problems.push(`example "${example.name}": ${result.status}\n${result.messages}`)
    if (example.mode === 'engines' && !result.output.startsWith('The two outputs are the same')) {
      problems.push(`example "${example.name}": the Rust compiler and the official compiler give different output`)
    }
    log(`example "${example.name}": ${result.status}${result.tabs.includes('Difference') ? `; ${result.output.split('\n')[0]}` : ''}`)
  }

  // The mode "Rust and official" for each version, with the first example.
  await after(() => page.selectOption('#example', '0'))
  await after(() => page.check('input[name="mode"][value="engines"]'))
  const versions = opts.versions === 'all' ? data.versions : opts.versions.split(',')
  for (const version of versions) {
    if (!data.versions.includes(version)) throw new Error(`${version} is not a version of the site`)
  }
  let different = []
  for (const version of versions) {
    const result = await after(() => page.selectOption('#version-a', version))
    if (/error/.test(result.status)) problems.push(`Rust and official, ${version}: ${result.status}`)
    if (!result.output.startsWith('The two outputs are the same')) different.push(version)
  }
  if (different.length) problems.push(`Rust and official: different output for ${different.join(', ')}`)
  log(`Rust and official, the first example, ${versions.length} versions: ${different.length} different`)

  // The programs of the releases (site/releases.mjs). Each one builds the
  // first example, or says that it does not have the selection: an older
  // program can ignore TAILWINDCSS_VERSION, and the page must not show
  // its output as the output of that version.
  await after(() => page.selectOption('#example', '0'))
  for (const program of data.programs.slice(1)) {
    for (const version of [data.upstream, '4.0.0']) {
      await after(() => page.selectOption('#version-a', version))
      const result = await after(() => page.selectOption('#program', program.wasm))
      if (/error/.test(result.status)) problems.push(`${program.name}, ${version}: ${result.status}\n${result.messages}`)
      log(`${program.name}, ${version}: ${result.status}`)
      await after(() => page.selectOption('#program', data.programs[0].wasm))
    }
  }
  // v0.1.0 has only 4.3.5, and builds it for each value of the variable.
  if (data.programs.some((p) => p.name === 'v0.1.0')) {
    await after(() => page.selectOption('#version-a', '4.0.0'))
    const result = await after(() => page.selectOption('#program', 'releases/v0.1.0/tailwind.wasm'))
    if (!/not in this program/.test(result.status)) problems.push(`v0.1.0 as 4.0.0: ${result.status}: the page must say that the program does not have 4.0.0`)
    await after(() => page.selectOption('#program', data.programs[0].wasm))
    await after(() => page.selectOption('#version-a', data.upstream))
  }

  // The recorded cases.
  const cases = await page.$$('#releases button')
  const count = opts.cases === 'all' ? cases.length : Math.min(cases.length, Number(opts.cases))
  const same = []
  for (let i = 0; i < count; i++) {
    const button = (await page.$$('#releases button'))[i]
    const name = await button.textContent()
    const result = await after(async () => {
      await button.evaluate((b) => b.closest('details').setAttribute('open', ''))
      await button.click()
    })
    if (/error/.test(result.status) && !result.messages) problems.push(`${name}: ${result.status}`)
    if (result.output.startsWith('The two outputs are the same')) same.push(`${name} (${result.status})`)
  }
  // The page lists only the cases that show their change (site/build.mjs).
  for (const name of same) problems.push(`no difference: ${name}`)
  log(`${count} recorded cases: ${count - same.length} show a difference`)
} finally {
  await browser.close()
  server.close()
}
for (const p of problems) console.error(`page.mjs: ${p}`)
if (problems.length) process.exit(1)
log('pass')
