#!/usr/bin/env node
// Usage: node gen-class-sort.mjs <directory> [output file]
//
// This script records the sort of classes of the official Prettier plugin
// (`prettier-plugin-tailwindcss`). <directory> must have the packages
// `prettier@3`, `prettier-plugin-tailwindcss@0.8.1` and `tailwindcss` of the
// version of UPSTREAM in its node_modules:
//
//   npm install --prefix DIR prettier@3 prettier-plugin-tailwindcss@0.8.1 tailwindcss@4.3.5
//
// It writes three kinds of records to
// native/tailwind/tests/fixtures/class_sort.json, for the stylesheet
// `@import "tailwindcss";`:
//
// - `attributes`: `sortClassAttributes` of the sorter API (`sortClasses`
//   with its default options), with the options `preserveWhitespace` and
//   `preserveDuplicates` of the sorter, or with none.
// - `lists`: `sortClassLists` of the sorter API.
// - `templates`: the fixed parts of a template literal in `className={…}`
//   after Prettier with the plugin (`sortTemplateLiteral`).
//
// The Rust test tests/class_order.rs compares.
import fs from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { createRandom } from './lib.mjs'

let [directory, output] = process.argv.slice(2)
if (!directory) {
  console.error('Usage: node gen-class-sort.mjs <directory> [output file]')
  process.exit(2)
}
directory = path.resolve(directory)
let here = path.dirname(fileURLToPath(import.meta.url))
output = path.resolve(output ?? path.join(here, '../../../native/tailwind/tests/fixtures/class_sort.json'))

let require = createRequire(path.join(directory, 'package.json'))
let prettierModule = await import(pathToFileURL(require.resolve('prettier')).href)
let prettier = prettierModule.format ? prettierModule : prettierModule.default
let { createSorter } = await import(pathToFileURL(require.resolve('prettier-plugin-tailwindcss/sorter')).href)
let plugin = require.resolve('prettier-plugin-tailwindcss')

fs.writeFileSync(path.join(directory, 'app.css'), '@import "tailwindcss";\n')
let sorters = {}
for (let [name, options] of Object.entries({ default: {}, preserveWhitespace: { preserveWhitespace: true }, preserveDuplicates: { preserveDuplicates: true } })) {
  sorters[name] = await createSorter({ base: directory, stylesheetPath: 'app.css', ...options })
}

let tailwindModule = await import(pathToFileURL(require.resolve('tailwindcss')).href)
// The CommonJS entry gives its functions as `default` to `import`.
let tailwind = tailwindModule.__unstable__loadDesignSystem ? tailwindModule : tailwindModule.default
let design = await tailwind.__unstable__loadDesignSystem('@import "tailwindcss";', {
  base: directory,
  loadStylesheet: async (id) => {
    let file = require.resolve(id === 'tailwindcss' ? 'tailwindcss/index.css' : id)
    return { path: file, base: path.dirname(file), content: fs.readFileSync(file, 'utf8') }
  },
})
let classList = design.getClassList().map(([name]) => name)
let random = createRandom(7)
let others = ['card', 'js-toggle', 'a', 'b', '...', '…', 'is-active', 'group', 'peer']
let variants = ['hover', 'focus', 'md', 'lg', 'dark', 'group-hover', 'peer-checked', 'data-[open]', 'max-sm', 'first']
let spaces = [' ', ' ', ' ', '  ', '\n  ', '\t', ' \n', '\r\n ', '\f']

function randomClass() {
  let r = random.next()
  if (r < 0.6) return random.pick(classList)
  if (r < 0.8) return `${random.pick(variants)}:${random.pick(classList)}`
  if (r < 0.9) return random.pick(others)
  return `${random.pick(classList)}!`
}

function randomList() {
  let list = []
  let length = 1 + random.int(12)
  for (let i = 0; i < length; i++) {
    let name = randomClass()
    list.push(name)
    if (random.chance(0.15)) list.push(random.chance(0.5) ? name : random.pick(list))
  }
  return list
}

function randomAttribute() {
  let list = randomList()
  let text = random.chance(0.2) ? random.pick(spaces) : ''
  for (let [i, name] of list.entries()) text += (i > 0 ? random.pick(spaces) : '') + name
  if (random.chance(0.2)) text += random.pick(spaces)
  return text
}

let attributes = [
  '',
  ' ',
  '\n\t ',
  'py-3 p-1 px-3',
  '  py-3 p-1  px-3 foo p-1 foo ... hover:p-1 ',
  'p-1 ... px-3 … py-3',
  '... ... p-1',
  'p-1 p-1 p-1 foo foo',
  'flex\n  items-center\n  p-4',
  '{{ x }} p-1 flex',
  'b p-1 a',
  'hover:b focus:p-1 a',
]
for (let i = 0; i < 150; i++) attributes.push(randomAttribute())

let records = { attributes: [], lists: [], templates: [] }
for (let [name, sorter] of Object.entries(sorters)) {
  let sorted = sorter.sortClassAttributes(attributes)
  for (let [i, input] of attributes.entries()) records.attributes.push({ options: name, input, output: sorted[i] })
}

let lists = []
for (let i = 0; i < 60; i++) lists.push(randomList())
for (let name of ['default', 'preserveDuplicates']) {
  let sorted = sorters[name].sortClassLists(lists)
  for (let [i, input] of lists.entries()) records.lists.push({ options: name, input, output: sorted[i] })
}

// The template literals: `className={`q0${a}q1${b}q2`}`.
function quasi() {
  let r = random.next()
  if (r < 0.15) return ''
  if (r < 0.3) return ' '
  let text = random.chance(0.5) ? ' ' : ''
  let list = randomList().filter((name) => !name.includes('`') && !name.includes('$') && !name.includes('\\'))
  text += list.join(' ')
  if (random.chance(0.5)) text += ' '
  return text
}
let expressions = ['a', 'b', 'c']
let sources = []
for (let i = 0; i < 80; i++) {
  let count = 1 + random.int(3)
  let parts = []
  for (let k = 0; k <= count; k++) parts.push(quasi())
  sources.push(parts)
}
for (let parts of sources) {
  let literal = parts.map((part, k) => part + (k < parts.length - 1 ? '${' + expressions[k] + '}' : '')).join('')
  let code = `export const x = <div className={\`${literal}\`} />\n`
  let formatted = await prettier.format(code, {
    parser: 'babel',
    plugins: [plugin],
    tailwindStylesheet: path.join(directory, 'app.css'),
    filepath: path.join(directory, 'x.jsx'),
    printWidth: 100000,
  })
  let inner = formatted.slice(formatted.indexOf('`') + 1, formatted.lastIndexOf('`'))
  let out = inner.split(/\$\{[abc]\}/)
  records.templates.push({ input: parts, output: out })
}

fs.writeFileSync(output, JSON.stringify(records) + '\n')
console.log(
  `gen-class-sort: ${records.attributes.length} attributes, ${records.lists.length} lists, ${records.templates.length} templates in ${path.relative(process.cwd(), output)}`,
)
