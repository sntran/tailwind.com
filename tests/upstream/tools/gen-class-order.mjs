#!/usr/bin/env node
// Usage: node gen-class-order.mjs [output file]
//
// This script records the order of classes of the official compiler
// (`getClassOrder` of the design system, which the official Prettier plugin
// uses to sort the classes of a template). It loads the design system of
// some stylesheets with the npm package `tailwindcss` of this directory,
// and the stylesheets of Tailwind CSS from tests/upstream/css. For lists of
// classes from the class list, the variants, the arbitrary forms, the edge
// candidates and some names that are not classes, it writes the order of
// each class to native/tailwind/tests/fixtures/class_order.json. The Rust
// test tests/class_order.rs compares.
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { __unstable__loadDesignSystem } from 'tailwindcss'
import { ARBITRARY_FORMS, EDGE_CANDIDATES, RANDOM_VARIANTS } from './candidate-lists.mjs'
import { createRandom } from './lib.mjs'

let here = path.dirname(fileURLToPath(import.meta.url))
let css = path.resolve(here, '../css')
let output = path.resolve(process.argv[2] ?? path.join(here, '../../../native/tailwind/tests/fixtures/class_order.json'))

// "tailwindcss" is index.css. "tailwindcss/x.css" and "./x.css" are x.css.
async function loadStylesheet(id) {
  let name = id === 'tailwindcss' ? 'index.css' : id.replace(/^tailwindcss\//, '').replace(/^\.\//, '')
  return { path: `/tailwindcss/${name}`, base: '/tailwindcss', content: fs.readFileSync(path.join(css, name), 'utf8') }
}

let words = (text) => (Array.isArray(text) ? text : text.trim().split(/\s+/))
let arbitrary = words(ARBITRARY_FORMS)
let variants = words(RANDOM_VARIANTS)
let notClasses = ['a', 'b', 'card', 'btn-primary', 'js-toggle', 'group', 'peer', 'dark', '...', '…', 'is-active']

let stylesheets = [
  { css: '@import "tailwindcss";', prefix: '' },
  // The stylesheet of `sort.test.ts` of Tailwind CSS.
  {
    css: '@theme {\n  --spacing-1: 0.25rem;\n  --spacing-3: 0.75rem;\n  --spacing-4: 1rem;\n  --color-red-500: red;\n  --color-blue-500: blue;\n}\n',
    prefix: '',
    lists: [
      'py-3 p-1 px-3',
      'px-3 focus:hover:p-3 hover:p-1 py-3',
      'px-3 py-4! p-1',
      'py-4! px-3 p-1',
      'b p-1 a',
      'hover:b focus:p-1 a',
      'a-class px-3 p-1 b-class py-3 bg-red-500 bg-blue-500',
      'px-3 b-class p-1 py-3 bg-blue-500 a-class bg-red-500',
      '[--fg:#fff]',
      '[--bg:#111] [--bg_hover:#000] [--fg:#fff]',
    ],
  },
  {
    css: '@import "tailwindcss" prefix(tw);\n@utility tab-* {\n  tab-size: --value(integer);\n}\n@custom-variant coarse (@media (pointer: coarse));\n@layer components {\n  .card {\n    padding: 1rem;\n  }\n}\n',
    prefix: 'tw:',
    extra: ['tab-4', 'tab-8', 'coarse:flex', 'coarse:tab-2', 'card'],
  },
  { css: '@import "tailwindcss" important;\n@theme {\n  --color-brand: oklch(0.6 0.2 260);\n}\n', prefix: '', extra: ['bg-brand', 'text-brand/50', 'hover:bg-brand'] },
]

let records = []
for (let [index, stylesheet] of stylesheets.entries()) {
  let design = await __unstable__loadDesignSystem(stylesheet.css, { base: '/', loadStylesheet })
  let classList = design.getClassList().map(([name]) => name)
  let random = createRandom(1000 + index)

  let lists = (stylesheet.lists ?? []).map((list) => list.split(' '))
  for (let n = 0; n < 60; n++) {
    let length = 1 + random.int(24)
    let list = []
    for (let i = 0; i < length; i++) {
      let r = random.next()
      let candidate
      if (r < 0.45) candidate = random.pick(classList)
      else if (r < 0.65) candidate = `${random.pick(variants)}:${random.pick(classList)}`
      else if (r < 0.75) candidate = random.pick(arbitrary)
      else if (r < 0.82) candidate = random.pick(EDGE_CANDIDATES)
      else if (r < 0.88) candidate = random.pick(notClasses)
      else if (r < 0.94) candidate = `${random.pick(classList)}!`
      else if (stylesheet.extra) candidate = random.pick(stylesheet.extra)
      else candidate = random.pick(classList)
      // The prefix comes first, before the variants.
      if (stylesheet.prefix && random.chance(0.85) && !notClasses.includes(candidate)) candidate = stylesheet.prefix + candidate
      list.push(candidate)
      // Some classes two times.
      if (random.chance(0.05)) list.push(candidate)
    }
    lists.push(list.filter((candidate) => candidate.trim() !== ''))
  }

  for (let classes of lists) {
    let order = design.getClassOrder(classes).map(([name, value]) => [name, value === null ? null : Number(value)])
    records.push({ css: stylesheet.css, classes, order })
  }
}

fs.writeFileSync(output, JSON.stringify(records, null, 0) + '\n')
console.log(`gen-class-order: ${records.length} lists in ${path.relative(process.cwd(), output)}`)
