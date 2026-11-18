#!/usr/bin/env node
// Usage: node gen-class-list.mjs [output file]
//
// This script records the class list of the official compiler
// (`getClassList` of the design system, for the editor tools), and the
// variants with their completions (`variants.entries()` and
// `variants.getCompletions(root)`). The canonical form of candidates uses
// them. It loads the design system of some stylesheets with the npm
// package `tailwindcss` of this directory, and the stylesheets of Tailwind
// CSS from tests/upstream/css. The Rust test tests/class_list.rs compares
// (native/tailwind/tests/fixtures/class_list.json.gz).
import fs from 'node:fs'
import path from 'node:path'
import zlib from 'node:zlib'
import os from 'node:os'
import { createRequire } from 'node:module'
import { fileURLToPath } from 'node:url'
import { __unstable__loadDesignSystem } from 'tailwindcss'

let here = path.dirname(fileURLToPath(import.meta.url))
let css = path.resolve(here, '../css')
let output = path.resolve(process.argv[2] ?? path.join(here, '../../../native/tailwind/tests/fixtures/class_list.json.gz'))

async function loadStylesheet(id) {
  let name = id === 'tailwindcss' ? 'index.css' : id.replace(/^tailwindcss\//, '').replace(/^\.\//, '')
  return { path: `/tailwindcss/${name}`, base: '/tailwindcss', content: fs.readFileSync(path.join(css, name), 'utf8') }
}

export const STYLESHEETS = [
  '@import "tailwindcss";',
  '@import "tailwindcss" prefix(tw);',
  [
    '@import "tailwindcss";',
    '@theme {',
    '  --color-brand: oklch(0.6 0.2 260);',
    '  --color-brand-dark: #123;',
    '  --spacing-card: 1rem;',
    '  --spacing-1_5: 0.375rem;',
    '  --text-huge: 5rem;',
    '  --text-huge--line-height: 1;',
    '  --breakpoint-3xl: 120rem;',
    '  --container-8xl: 96rem;',
    '  --opacity-15: 15%;',
    '  --shadow-soft: 0 1px 2px red;',
    '}',
    '@theme inline { --font-display: "Inter", sans-serif; }',
  ].join('\n'),
  '@theme { --spacing: initial; --color-red-500: red; --color-*: initial; --color-x: blue; }\n@import "tailwindcss/utilities.css";',
  [
    '@import "tailwindcss";',
    '@theme { --tab-size-github: 8; }',
    '@utility tab-* { tab-size: --value(integer, --tab-size-*, "inherit"); }',
    '@utility gap-x-sp-* { column-gap: --spacing(--value(number)); }',
    '@utility m-int-* { margin: --spacing(--value(integer)); --x: --modifier("a", "b", integer); }',
    '@utility card { padding: 1rem; }',
    '@custom-variant coarse (@media (pointer: coarse));',
    '@custom-variant theme-dark { &:where(.dark, .dark *) { @slot; } }',
  ].join('\n'),
]

// A local plugin with suggestions: `matchUtilities` with values and
// modifiers, `matchVariant` with values, and `addUtilities`.
export const PLUGIN = `const plugin = require('tailwindcss/plugin')
module.exports = plugin(function ({ matchUtilities, matchVariant, addUtilities, addComponents, theme }) {
  matchUtilities(
    { 'tab-x': (value) => ({ tabSize: value }) },
    { values: { 1: '1', 2: '2', DEFAULT: '4', large: '8', __BARE_VALUE__: (v) => v }, modifiers: { a: 'a', b: 'b' }, supportsNegativeValues: true },
  )
  matchUtilities({ 'mt-x': (value) => ({ marginTop: value }) }, { values: theme('spacing') })
  matchUtilities({ 'any-x': (value) => ({ color: value }) }, { values: { red: 'red' }, modifiers: 'any' })
  matchVariant('nth-x', (value) => \`&:nth-child(\${value})\`, { values: { 1: '1', 2: '2', DEFAULT: '3', odd: 'odd' } })
  addUtilities({ '.plain': { color: 'red' }, '.plain-2': { color: 'blue' } })
  addComponents({ '.btn': { padding: '1rem' } })
})
`

// The stylesheets with plugins, and their files.
export const PLUGIN_STYLESHEETS = [
  { css: '@import "tailwindcss";\n@plugin "./plugin.js";', files: { '/plugin.js': PLUGIN } },
  { css: '@import "tailwindcss";\n@plugin "@tailwindcss/typography";\n@plugin "@tailwindcss/forms";', files: {} },
]

let work = fs.mkdtempSync(path.join(here, '.gen-class-list-'))
let require = createRequire(path.join(here, 'gen-class-list.mjs'))
async function loadModule(id, base) {
  if (id.startsWith('.') || id.startsWith('/')) {
    let file = path.join(work, path.basename(id))
    return { path: file, base: work, module: require(file) }
  }
  return { path: require.resolve(id), base: '/', module: require(id) }
}

let inputs = STYLESHEETS.map((css) => ({ css, files: {} })).concat(PLUGIN_STYLESHEETS)

let result = []
for (let { css: input, files } of inputs) {
  for (let [name, source] of Object.entries(files)) fs.writeFileSync(path.join(work, path.basename(name)), source)
  let design = await __unstable__loadDesignSystem(input, { base: '/', loadStylesheet, loadModule })
  let classes = design.getClassList().map(([name, meta]) => [name, meta.modifiers])
  let variants = []
  for (let [root, variant] of design.variants.entries()) {
    variants.push([root, variant.kind, design.variants.getCompletions(root)])
  }
  result.push({ css: input, files, classes, variants })
}
fs.rmSync(work, { recursive: true, force: true })

fs.writeFileSync(output, zlib.gzipSync(JSON.stringify(result)))
console.log(`${path.basename(output)}: ${result.map((r) => `${r.classes.length} classes, ${r.variants.length} variants`).join('; ')}`)
