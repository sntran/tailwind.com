// The page of the site: the controls, the builds in the worker, the
// output, the preview and the list of the changes of each release.
//
// Each class name in this file is written in full, so that the scanner
// of tailwind.com finds it for style.css.
import { diff } from './diff.mjs'

const $ = (id) => document.getElementById(id)
const controls = {
  example: $('example'),
  versionA: $('version-a'),
  versionALabel: $('version-a-label'),
  versionB: $('version-b'),
  versionBField: $('version-b-field'),
  minify: $('minify'),
  program: $('program'),
  markup: $('markup'),
  css: $('css'),
}
const mode = () => document.querySelector('input[name="mode"]:checked').value
const setMode = (value) => {
  document.querySelector(`input[name="mode"][value="${value}"]`).checked = true
}

const ENGINE_NAMES = { native: 'Rust compiler', script: 'official compiler' }
const TAB = 'rounded-md px-2.5 py-1 hover:bg-slate-100 dark:hover:bg-slate-800'
const TAB_SELECTED = 'rounded-md px-2.5 py-1 bg-sky-600 text-white'
const LINE_ADD = 'block bg-emerald-100 text-emerald-900 dark:bg-emerald-950 dark:text-emerald-200'
const LINE_REMOVE = 'block bg-rose-100 text-rose-900 dark:bg-rose-950 dark:text-rose-200'
const LINE_SAME = 'block text-slate-500 dark:text-slate-400'
const NOTE = 'block font-sans text-sm text-slate-600 dark:text-slate-300'

let data
let results = []
let tab = 0

// --- The worker -----------------------------------------------------------

const worker = new Worker(new URL('./worker.mjs', import.meta.url), { type: 'module' })
let ready = false
let nextId = 0
let current = 0
const pending = new Map()

worker.onmessage = ({ data: message }) => {
  if (message.type === 'ready') {
    ready = true
    build()
    return
  }
  const done = pending.get(message.id)
  if (!done) {
    // A failure of the start.
    if (message.type === 'failure') $('status').textContent = `tailwind.wasm could not start: ${message.message}`
    return
  }
  pending.delete(message.id)
  if (message.type === 'failure') done({ status: -1, css: '', messages: message.message, ms: 0 })
  else done(message)
}

function send(message) {
  const id = ++nextId
  return new Promise((resolve) => {
    pending.set(id, resolve)
    worker.postMessage({ ...message, id })
  })
}

// The builds of the selection: one, or two to compare.
function builds() {
  const a = controls.versionA.value
  if (mode() === 'two') return [{ version: a, engine: 'native' }, { version: controls.versionB.value, engine: 'native' }]
  if (mode() === 'engines') return [{ version: a, engine: 'native' }, { version: a, engine: 'script' }]
  return [{ version: a, engine: 'native' }]
}

// The selected program: the build of the site, or a release.
const program = () => data.programs.find((p) => p.wasm === controls.program.value) ?? data.programs[0]
const loaded = new Set()

async function build() {
  if (!ready) return
  const id = ++current
  const markup = controls.markup.value
  const css = controls.css.value
  const minify = controls.minify.checked
  const list = builds()
  const p = program()
  const wasm = new URL(p.wasm, location.href).href
  // The first build with a program downloads it.
  $('status').textContent = loaded.has(wasm) ? 'Building …' : `Loading the program ${p.name} (about 9 MB) …`
  const out = []
  for (const b of list) out.push({ ...b, ...(await send({ type: 'build', wasm, markup, css, minify, ...b })) })
  loaded.add(wasm)
  // A newer build started during this one: show only that one.
  if (id !== current) return
  results = out
  if (tab >= tabs().length) tab = tabs().length - 1
  render()
}

let timer
function later() {
  clearTimeout(timer)
  timer = setTimeout(build, 250)
}

// --- The output -----------------------------------------------------------

const label = (r) => (mode() === 'engines' ? ENGINE_NAMES[r.engine] : r.version)

function tabs() {
  const list = results.map((r, i) => ({ name: results.length === 1 ? 'Output' : label(r), show: () => showCss(i) }))
  if (results.length === 2) list.push({ name: 'Difference', show: showDifference })
  return list
}

function render() {
  const list = tabs()
  $('tabs').replaceChildren(
    ...list.map((t, i) => {
      const button = document.createElement('button')
      button.type = 'button'
      button.role = 'tab'
      button.textContent = t.name
      button.className = i === tab ? TAB_SELECTED : TAB
      button.setAttribute('aria-selected', String(i === tab))
      button.onclick = () => {
        tab = i
        render()
      }
      return button
    }),
  )
  list[tab]?.show()
  renderPreviews()
  renderStatus()
  saveHash()
}

// The text of a result: the CSS, or the message of a failed build.
const textOf = (r) => {
  if (r.missing) return `The program ${program().name} does not have Tailwind CSS ${r.version} with the ${ENGINE_NAMES[r.engine]}. Its version line is:\n${r.missing}`
  return r.status === 0 ? r.css : `The build failed (status ${r.status}).\n${r.messages}`
}

function showCss(i) {
  $('output').textContent = textOf(results[i])
}

function showDifference() {
  const [a, b] = results
  const output = $('output')
  const note = (text) => {
    const span = document.createElement('span')
    span.className = NOTE
    span.textContent = text
    return span
  }
  // The license line has the version, so it is always different: the
  // difference does not show it.
  const license = /^\/\*! tailwindcss v[^\n]*\*\/\n/
  const before = textOf(a).replace(license, '')
  const after = textOf(b).replace(license, '')
  if (before === after) {
    output.replaceChildren(note(`The two outputs are the same (${lines(before.split('\n').length)}${before === textOf(a) ? '' : ', with no license line'}).`))
    return
  }
  const changes = diff(before, after)
  if (!changes) {
    output.replaceChildren(note('The two outputs are too different to show as a list of changes.'))
    return
  }
  const added = changes.filter((c) => c.kind === 'add').length
  const removed = changes.filter((c) => c.kind === 'remove').length
  const list = [note(`− ${lines(removed)} only in ${label(a)}, + ${lines(added)} only in ${label(b)} (the license line is not compared)`)]
  // The lines that are the same: three around each change.
  const near = new Set()
  changes.forEach((c, i) => {
    if (c.kind !== 'same') for (let j = i - 3; j <= i + 3; j++) near.add(j)
  })
  let skipped = 0
  changes.forEach((c, i) => {
    if (c.kind === 'same' && !near.has(i)) {
      skipped++
      return
    }
    if (skipped) {
      list.push(note(`… ${lines(skipped)} that are the same`))
      skipped = 0
    }
    const span = document.createElement('span')
    span.className = c.kind === 'add' ? LINE_ADD : c.kind === 'remove' ? LINE_REMOVE : LINE_SAME
    span.textContent = `${c.kind === 'add' ? '+' : c.kind === 'remove' ? '−' : ' '} ${c.text}`
    list.push(span)
  })
  if (skipped) list.push(note(`… ${lines(skipped)} that are the same`))
  // All the lines have the width of the longest one, so that the color of
  // a line goes to its end.
  const block = document.createElement('div')
  block.className = 'w-max min-w-full'
  block.append(...list)
  output.replaceChildren(block)
}

const lines = (n) => `${n} line${n === 1 ? '' : 's'}`

function renderPreviews() {
  const markup = controls.markup.value
  $('previews').className = results.length === 2 ? 'grid h-96 gap-2 sm:grid-cols-2' : 'grid h-96 gap-2'
  $('previews').replaceChildren(
    ...results.map((r) => {
      const figure = document.createElement('figure')
      figure.className = 'grid min-h-0 grid-rows-[auto_1fr] gap-1'
      const caption = document.createElement('figcaption')
      caption.className = 'text-xs text-slate-500 dark:text-slate-400'
      caption.textContent = results.length === 2 ? label(r) : `${r.version}, ${ENGINE_NAMES[r.engine]}`
      const frame = document.createElement('iframe')
      frame.className = 'h-full w-full rounded-xl border border-slate-200 bg-white dark:border-slate-800'
      frame.title = `Preview: ${caption.textContent}`
      // No scripts in the markup.
      frame.sandbox = ''
      frame.srcdoc = `<!doctype html><html><head><meta charset="utf-8"><style>${r.css.replaceAll('</style', '<\\/style')}</style></head><body>${markup}</body></html>`
      figure.append(caption, frame)
      return figure
    }),
  )
}

function renderStatus() {
  const ms = (r) => `${Math.round(r.ms)} ms`
  const result = (r) => (r.missing ? 'not in this program' : r.status === 0 ? ms(r) : 'error')
  const name = program() === data.programs[0] ? '' : `${program().name}: `
  $('status').textContent = name + results.map((r) => `${r.version}, ${ENGINE_NAMES[r.engine]}: ${result(r)}`).join(' · ')
  const messages = results
    .filter((r) => r.messages || r.missing)
    .map((r) => (r.missing ? textOf(r) : results.length === 2 ? `${label(r)}:\n${r.messages}` : r.messages))
    .join('\n\n')
  $('messages').hidden = !messages
  $('messages').textContent = messages
}

// --- The selection --------------------------------------------------------

function updateControls() {
  const m = mode()
  controls.versionBField.hidden = m !== 'two'
  controls.versionALabel.textContent = m === 'two' ? 'First version' : 'Version'
}

function open({ markup, css, mode: m = 'one', a, b, program: p }) {
  controls.markup.value = markup
  controls.css.value = css
  setMode(m)
  if (p && data.programs.some((x) => x.wasm === p)) controls.program.value = p
  if (a) controls.versionA.value = a
  if (b) controls.versionB.value = b
  tab = m === 'one' ? 0 : 2
  updateControls()
  build()
}

// The state of the page in the address: a link opens the same state.
function saveHash() {
  const state = { m: controls.markup.value, c: controls.css.value, o: mode(), a: controls.versionA.value, b: controls.versionB.value, x: controls.minify.checked ? 1 : 0 }
  // The build of the site is the default: a link has a release only when
  // the viewer selected one.
  if (program() !== data.programs[0]) state.p = program().wasm
  const bytes = new TextEncoder().encode(JSON.stringify(state))
  let binary = ''
  for (let i = 0; i < bytes.length; i += 8192) binary += String.fromCharCode(...bytes.subarray(i, i + 8192))
  const text = btoa(binary).replaceAll('+', '-').replaceAll('/', '_').replace(/=+$/, '')
  history.replaceState(null, '', `#${text}`)
}

function readHash() {
  if (location.hash.length < 2) return null
  try {
    const text = atob(location.hash.slice(1).replaceAll('-', '+').replaceAll('_', '/'))
    const state = JSON.parse(new TextDecoder().decode(Uint8Array.from(text, (c) => c.charCodeAt(0))))
    controls.minify.checked = Boolean(state.x)
    return { markup: state.m, css: state.c, mode: state.o, a: state.a, b: state.b, program: state.p }
  } catch {
    return null
  }
}

// --- The changes of each release ------------------------------------------

function renderReleases() {
  $('releases').replaceChildren(
    ...data.releases.map((release) => {
      const details = document.createElement('details')
      details.className = 'rounded-xl border border-slate-200 bg-white p-3 dark:border-slate-800 dark:bg-slate-900'
      const summary = document.createElement('summary')
      summary.className = 'cursor-pointer font-medium'
      const count = (n, word) => `${n} ${word}${n === 1 ? '' : 's'}`
      const shown = release.cases.filter((c) => c.shows)
      summary.textContent = `${release.version}: ${count(release.switches.length, 'switch')}, ${count(shown.length, 'recorded case')}`
      const list = document.createElement('ul')
      list.className = 'mt-3 grid gap-1.5 text-sm'
      for (const s of release.switches) {
        const item = document.createElement('li')
        const name = document.createElement('code')
        name.className = 'font-mono text-xs text-sky-700 dark:text-sky-300'
        name.textContent = s.name
        name.title = s.doc
        item.append(name, `: ${s.what}`)
        list.append(item)
      }
      const cases = document.createElement('div')
      cases.className = 'mt-3 flex flex-wrap gap-2'
      for (const c of shown) {
        const button = document.createElement('button')
        button.type = 'button'
        button.className = 'rounded-md border border-slate-300 px-2.5 py-1 text-left text-sm hover:bg-slate-100 dark:border-slate-700 dark:hover:bg-slate-800'
        button.textContent = `Open: ${c.name}`
        button.onclick = () => {
          open({ markup: c.markup, css: c.css, mode: 'two', a: c.before, b: c.version })
          controls.example.value = ''
          window.scrollTo({ top: 0, behavior: 'smooth' })
        }
        cases.append(button)
      }
      details.append(summary, list, cases)
      const hidden = release.cases.length - shown.length
      if (hidden) {
        const note = document.createElement('p')
        note.className = 'mt-2 text-xs text-slate-500 dark:text-slate-400'
        note.textContent = `${count(hidden, 'more case')} of this release show the change only with the compiler API: a build of this page gives the same output in the two versions.`
        details.append(note)
      }
      return details
    }),
  )
}

// --- The start ------------------------------------------------------------

async function main() {
  data = await (await fetch('data.json')).json()
  for (const select of [controls.versionA, controls.versionB]) {
    select.replaceChildren(...data.versions.map((v) => new Option(v === data.upstream ? `${v} (newest)` : v, v)))
  }
  controls.example.replaceChildren(new Option('(your own)', ''), ...data.examples.map((e, i) => new Option(e.name, String(i))))
  controls.program.replaceChildren(...data.programs.map((p, i) => new Option(i === 0 ? `${p.name}, this site` : `${p.name}, a release`, p.wasm)))
  renderReleases()

  for (const input of [controls.markup, controls.css]) {
    input.addEventListener('input', () => {
      controls.example.value = ''
      later()
    })
  }
  for (const input of [controls.versionA, controls.versionB, controls.minify, controls.program]) input.addEventListener('change', build)
  for (const radio of document.querySelectorAll('input[name="mode"]')) {
    radio.addEventListener('change', () => {
      tab = mode() === 'one' ? 0 : 2
      updateControls()
      build()
    })
  }
  controls.example.addEventListener('change', () => {
    const example = data.examples[Number(controls.example.value)]
    if (example) open(example)
  })
  $('copy-link').addEventListener('click', async () => {
    saveHash()
    await navigator.clipboard?.writeText(location.href)
    $('copy-link').textContent = 'The link is copied'
    setTimeout(() => ($('copy-link').textContent = 'Copy a link to this page'), 2000)
  })

  const state = readHash()
  if (state) {
    controls.example.value = ''
    open(state)
  } else {
    controls.example.value = '0'
    open(data.examples[0])
  }
  worker.postMessage({ type: 'start', wasm: new URL('tailwind.wasm', location.href).href, packages: new URL('packages.bin', location.href).href })
}

main().catch((error) => {
  $('status').textContent = `The page could not start: ${error.message}`
})
