#!/usr/bin/env node
// Usage: node gen-variants.mjs <path of the upstream clone> [output file]
// This script writes native/tailwind/tests/fixtures/variants.json. The file
// records the behavior of `variants.ts` of the upstream clone:
// - `cases`: each `run([...candidates], css)` call of `variants.test.ts` whose
//   CSS has only `@theme` rules and `@tailwind utilities`. For each candidate:
//   the rules before the last pass of the compiler (`compileAstNodes`).
// - `slot`: results of `substituteAtSlot`.
// - `atVariant`: results of `substituteAtVariant`.
// - `fromAst`: variants from `Variants.fromAst`, and the rules of candidates.
// - `order`: a list of variants, and the list in the order of
//   `design.variants.compare`.
import fs from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { createRandom, loadUpstream } from './lib.mjs'

const DEFAULT_INPUT =
  '@layer theme, base, components, utilities;\n' +
  '@import "tailwindcss/theme.css" layer(theme);\n' +
  '@import "tailwindcss/utilities.css" layer(utilities);'

let here = path.dirname(fileURLToPath(import.meta.url))
let [clone, outFile = path.join(here, '../../../native/tailwind/tests/fixtures/variants.json')] =
  process.argv.slice(2)
if (!clone) {
  console.error('Usage: node gen-variants.mjs <path of the upstream clone> [output file]')
  process.exit(1)
}

let up = await loadUpstream(clone)
let src = path.join(path.resolve(clone), 'packages/tailwindcss/src')
let require = createRequire(path.join(path.resolve(clone), 'packages/@tailwindcss-node/package.json'))
let { createJiti } = require('jiti')
let jiti = createJiti(src + '/', { moduleCache: true })
let { toCss } = await jiti.import(path.join(src, 'ast.ts'))
let CssParser = await jiti.import(path.join(src, 'css-parser.ts'))
let { substituteAtSlot, substituteAtVariant } = await jiti.import(path.join(src, 'variants.ts'))

// A node without the source locations. A value that is not there is `null`.
function plain(node) {
  switch (node.kind) {
    case 'rule':
      return { kind: 'rule', selector: node.selector, nodes: node.nodes.map(plain) }
    case 'at-rule':
      return { kind: 'at-rule', name: node.name, params: node.params, nodes: node.nodes.map(plain) }
    case 'declaration':
      return {
        kind: 'declaration',
        property: node.property,
        value: node.value ?? null,
        important: node.important,
      }
    case 'comment':
      return { kind: 'comment', value: node.value }
    case 'context':
      return { kind: 'context', context: node.context, nodes: node.nodes.map(plain) }
    case 'at-root':
      return { kind: 'at-root', nodes: node.nodes.map(plain) }
    default:
      throw new Error(`Unknown node kind: ${node.kind}`)
  }
}

// The rules of one candidate, or the message of the error.
function compileCandidate(design, candidate) {
  try {
    let nodes = []
    for (let parsed of design.parseCandidate(candidate)) {
      for (let { node, propertySort } of design.compileAstNodes(parsed)) {
        nodes.push({
          css: toCss([node]),
          ast: plain(node),
          order: propertySort.order,
          count: propertySort.count,
        })
      }
    }
    return { candidate, nodes }
  } catch (error) {
    return { candidate, error: error.message }
  }
}

// ---------------------------------------------------------------------------
// The `run(...)` calls of variants.test.ts.

// The index after the bracket that closes the bracket at `start`.
function skipBrackets(text, start) {
  let depth = 0
  for (let i = start; i < text.length; i++) {
    let char = text[i]
    if (char === "'" || char === '"' || char === '`') {
      for (i++; text[i] !== char; i++) {
        if (text[i] === '\\') i++
      }
    } else if (char === '/' && text[i + 1] === '/') {
      while (text[i] !== '\n') i++
    } else if (char === '(' || char === '[' || char === '{') {
      depth++
    } else if (char === ')' || char === ']' || char === '}') {
      depth--
      if (depth === 0) return i + 1
    }
  }
  throw new Error('No end of the bracket')
}

let testSource = fs.readFileSync(path.join(src, 'variants.test.ts'), 'utf8')
let cases = []
let skipped = { notEvaluated: 0, options: 0, otherRules: 0 }
let testName = ''
let pattern = /^test(?:\.each\([^]*?\)\))?\((['"`])(.*?)\1|await run\(/gm
for (let match; (match = pattern.exec(testSource)); ) {
  if (match[2] !== undefined) {
    testName = match[2]
    continue
  }
  let start = match.index + match[0].length - 1
  let end = skipBrackets(testSource, start)
  let args
  try {
    args = new Function('css', `return [${testSource.slice(start + 1, end - 1)}]`)(String.raw)
  } catch {
    skipped.notEvaluated++
    continue
  }
  let [candidates, input = '@tailwind utilities;', options] = args
  if (options !== undefined) {
    skipped.options++
    continue
  }

  // Only `@theme` rules and `@tailwind utilities` are permitted.
  let onlyTheme = CssParser.parse(input).every(
    (node) =>
      node.kind === 'comment' ||
      (node.kind === 'at-rule' &&
        (node.name === '@theme' || (node.name === '@tailwind' && node.params === 'utilities'))),
  )
  if (!onlyTheme) {
    skipped.otherRules++
    continue
  }

  let design = await up.loadDesignSystem(input)
  cases.push({
    test: testName,
    input,
    prefix: design.theme.prefix ?? null,
    theme: Array.from(design.theme.values, ([key, { value, options }]) => [key, value, options]),
    candidates: candidates.map((candidate) => compileCandidate(design, candidate)),
  })
}

// ---------------------------------------------------------------------------
// substituteAtSlot

let slotInputs = [
  ['@slot;', 'color: red;'],
  ['&:hover { @slot; }', 'color: red; .a { color: blue; }'],
  ['&:hover { @slot; } &:focus { @slot; }', 'color: red;'],
  ['@media print { &:hover { @slot; } }', '@media (hover: hover) { color: red; }'],
  ['&:hover { @slot; }', ''],
  ['@keyframes spin { to { rotate: 360deg; } } &:hover { @slot; }', 'color: red;'],
  ['@property --x { syntax: "*"; inherits: false; } @slot;', 'color: red;'],
  ['.a { @keyframes k { from { @slot; } } @slot; }', 'color: red;'],
  ['@slot; @slot;', '@slot;'],
  ['@slot { color: blue; }', 'color: red;'],
  ['@media print { @slot foo; } /* c */ color: green;', '.x { .y { color: red; } }'],
  ['.a { .b { .c { @slot; @property --y { syntax: "<length>"; } } } }', 'color: red; color: blue;'],
]
let slot = slotInputs.map(([ast, nodes]) => {
  let tree = CssParser.parse(ast)
  substituteAtSlot(tree, CssParser.parse(nodes))
  return { ast, nodes, expected: tree.map(plain) }
})

// ---------------------------------------------------------------------------
// substituteAtVariant

let design = await up.loadDesignSystem(DEFAULT_INPUT, { loadStylesheet: up.loadStylesheet })

let atVariantInputs = [
  '.a { color: red; }',
  '@variant hover { color: red; }',
  '.a { @variant hover { color: red; } }',
  '.a { @variant hover:focus { color: red; } }',
  '.a { @variant hover, focus { color: red; } }',
  '.a { @variant hover:focus, dark:md , print { color: red; .b { color: blue; } } }',
  '.a { @variant dark { @variant hover { color: red; } color: blue; } }',
  '.a { @variant md { color: red; } @variant max-md { color: blue; } }',
  '.a { @variant group-hover { color: red; } }',
  '.a { @variant group-hover/name { color: red; } }',
  '.a { @variant peer-[&_p]/name { color: red; } }',
  '.a { @variant not-hover { color: red; } }',
  '.a { @variant not-dark { color: red; } }',
  '.a { @variant has-checked:in-data-active { color: red; } }',
  '.a { @variant [&_p] { color: red; } }',
  '.a { @variant [@media_print] { color: red; } }',
  '.a { @variant @sm { color: red; } }',
  '.a { @variant @max-[300px]/sidebar { color: red; } }',
  '.a { @variant before { color: red; } }',
  '.a { @variant marker { color: red; } }',
  '.a { @variant aria-[sort=ascending] { color: red; } }',
  '.a { @variant data-[state=open_i] { color: red; } }',
  '.a { @variant supports-[display:grid] { color: red; } }',
  '.a { @variant supports-[not(display:grid)or(display:flex)] { color: red; } }',
  '.a { @variant hover; }',
  '.a { @variant hover { } }',
  '.a { @variant  hover : focus  { color: red; } }',
  '@media print { .a { @variant hover { .b { @variant focus { color: red; } } } } }',
  '.a { @variant unknown { color: red; } }',
  '.a { @variant hover:unknown-thing { color: red; } }',
  '.a { @variant { color: red; } }',
  '.a { @variant hover: { color: red; } }',
  '.a { @variant hover,, focus { color: red; } }',
  '.a { @variant :hover { color: red; } }',
  '.a { @variant not-marker { color: red; } }',
  '.a { @variant not-before { color: red; } }',
  '.a { @variant group-before { color: red; } }',
  '.a { @variant max-unknown { color: red; } }',
  '.a { @variant nth-foo { color: red; } }',
  '.a { @variant [>img] { color: red; } }',
  '.a { @variant hover/foo { color: red; } }',
  '.a { @variant dark { @variant unknown { color: red; } } }',
]
let atVariant = atVariantInputs.map((input) => {
  let tree = CssParser.parse(input)
  try {
    let features = substituteAtVariant(tree, design)
    return { input, features, expected: tree.map(plain) }
  } catch (error) {
    return { input, error: error.message }
  }
})

// ---------------------------------------------------------------------------
// Variants.fromAst

let fromAstBodies = [
  '&:hover { @slot; }',
  '&:hover, &:focus { @slot; }',
  '&:hover { @slot; } &:focus { @slot; }',
  '@media (any-hover: hover) { &:hover { @slot; } }',
  '@media (any-hover: hover) { @slot; }',
  '@media foo { &:hover { @slot; } &:focus { @slot; } }',
  '&:hover { @media a { @slot; } @media b { @slot; } }',
  '&:hover { @media a { @slot; } @media b, c { @slot; } }',
  '&:hover { @media a { @slot; } @starting-style { @slot; } }',
  '@media a { @supports (b: c) { @slot; } }',
  '@media a { @slot; @supports (x: y) { @slot; } }',
  '@media a { @supports (x: y) { @slot; } @slot; }',
  '&:hover { &:focus { @slot; } }',
  '@supports (a: b) { .x & { @slot; } @media c { @slot; } }',
  '@supports (a: b) { .x & { @slot; } @media c { @slot; } .y & { @slot; } }',
  '@supports (a: b) { .x & { @slot; } @media c { @slot; } @media d { .y & { @slot; } } }',
  '@container sidebar (width > 10px) { @slot; }',
  '@container not (width > 10px) { @slot; }',
  '@container sidebar not (width > 10px) { @slot; }',
  '@container (width > 10px) { @slot; }',
  '@container sidebar { @slot; }',
  '@media not print { @slot; }',
  '@supports not (display: grid) { @slot; }',
  '@media screen, print { @slot; }',
  '@scope (.a) to (.b) { @slot; }',
  '@scope (.a) { &:hover { @slot; } }',
  '@keyframes fade { from { opacity: 0; } } &:hover { @slot; }',
  '@property --p { syntax: "*"; inherits: false; } @slot;',
  '&::before { @slot; }',
  '@starting-style { @slot; }',
  '@layer foo { @slot; }',
  '@variant hover { @slot; }',
  '@variant dark { @variant focus { &:where(.x) { @slot; } } }',
  '@variant hover, focus { @slot; }',
  '@variant unknown-variant { @slot; }',
  '@variant hover { @variant { @slot; } }',
  '&:hover { color: red; }',
  '@slot;',
  '& > * { @slot; }',
  '.a &, .b & { @media print { @slot; } }',
]
let fromAstDesign = await up.loadDesignSystem(DEFAULT_INPUT, { loadStylesheet: up.loadStylesheet })
let fromAst = fromAstBodies.map((body, idx) => {
  let name = `custom${idx}`
  fromAstDesign.variants.fromAst(name, CssParser.parse(body), fromAstDesign)
  let candidates = [
    `${name}:flex`,
    `${name}:hover:flex`,
    `hover:${name}:flex`,
    `not-${name}:flex`,
    `group-${name}:flex`,
    `group-${name}/item:flex`,
    `peer-${name}:flex`,
    `has-${name}:flex`,
    `in-${name}:flex`,
    `not-group-${name}:flex`,
    `${name}:[color:red]`,
  ]
  return {
    name,
    body,
    compounds: fromAstDesign.variants.get(name).compounds,
    candidates: candidates.map((candidate) => compileCandidate(fromAstDesign, candidate)),
  }
})

// ---------------------------------------------------------------------------
// The sort order

let orderDesign = await up.loadDesignSystem(DEFAULT_INPUT, { loadStylesheet: up.loadStylesheet })
let names = new Set()
let staticNames = []
for (let [name, info] of orderDesign.variants.entries()) {
  if (info.kind === 'static') staticNames.push(name)
}
for (let name of staticNames) names.add(name)
for (let root of ['not', 'group', 'peer', 'has', 'in']) {
  for (let name of ['hover', 'focus', 'first', 'checked', 'disabled', 'open', 'empty', 'rtl']) {
    names.add(`${root}-${name}`)
  }
  for (let name of ['dark', 'print', 'md', 'sm', 'supports-[display:grid]', 'min-[100px]', '@lg']) {
    names.add(`${root}-${name}`)
  }
  names.add(`${root}-[&_p]`)
  names.add(`${root}-[.is-open]`)
  names.add(`${root}-aria-checked`)
  names.add(`${root}-data-[state=open]`)
  names.add(`${root}-nth-3`)
  names.add(`${root}-has-checked`)
  names.add(`${root}-in-data-active`)
}
for (let root of ['group', 'peer']) {
  for (let name of ['hover', 'focus', 'first', '[&_p]', 'has-checked']) {
    names.add(`${root}-${name}/item`)
    names.add(`${root}-${name}/sidebar`)
    names.add(`${root}-${name}/[x]`)
  }
}
for (let value of ['checked', 'busy', 'hidden', 'expanded', '[sort=ascending]', '[foo]', '[current=page]']) {
  names.add(`aria-${value}`)
}
for (let value of ['active', 'open', 'foo', '[state=open]', '[state=closed]', '[foo]', '[a=b_i]', '(--x)']) {
  names.add(`data-${value}`)
}
for (let root of ['nth', 'nth-last', 'nth-of-type', 'nth-last-of-type']) {
  for (let value of ['1', '2', '3', '10', '[2n+1]', '[odd]', '[3n]']) names.add(`${root}-${value}`)
}
for (let value of ['[display:grid]', '[display]', '[not(display:grid)]', '[selector(a:hover)]', 'grid', '(--x)']) {
  names.add(`supports-${value}`)
}
let lengths = ['[100px]', '[600px]', '[10rem]', '[40rem]', '[48rem]', '[80.5rem]', '[50vw]', '[5em]']
let functions = ['[calc(100%-1rem)]', '[calc(100%-2rem)]', '[min(10px,20px)]', '[clamp(1px,2px,3px)]']
for (let root of ['min', 'max']) {
  for (let value of ['sm', 'md', 'lg', 'xl', '2xl', ...lengths, ...functions]) names.add(`${root}-${value}`)
}
for (let root of ['@', '@min-', '@max-']) {
  for (let value of ['3xs', 'xs', 'sm', 'md', 'lg', '4xl', '7xl', ...lengths, ...functions]) {
    names.add(`${root}${value}`)
  }
  names.add(`${root}sm/sidebar`)
  names.add(`${root}[123px]/main`)
}
// One variant with no value in each group: the order of two such variants
// is not defined.
names.add('min-[var(--x)]')
names.add('max-unknown')
names.add('@[var(--x)]')
names.add('@max-unknown')
names.add('aria')
names.add('data')
for (let selector of ['[&_p]', '[&>*]', '[&:hover]', '[.dark_&]', '[@media_print]', '[@supports(display:grid)]', '[&[data-x]]', '[&_Ā]', '[&_a]', '[&_Z]']) {
  names.add(selector)
}

let random = createRandom(4305)
let input = Array.from(names).filter((name) => orderDesign.parseVariant(name) !== null)
for (let i = input.length - 1; i > 0; i--) {
  let j = random.int(i + 1)
  ;[input[i], input[j]] = [input[j], input[i]]
}
let sorted = input
  .map((name) => [name, orderDesign.parseVariant(name)])
  .sort((a, z) => orderDesign.variants.compare(a[1], z[1]))
  .map(([name]) => name)
let order = { input, sorted }

// ---------------------------------------------------------------------------

fs.mkdirSync(path.dirname(outFile), { recursive: true })
fs.writeFileSync(outFile, JSON.stringify({ cases, slot, atVariant, fromAst, order }, null, 1) + '\n')
console.log(
  `variants.json: ${cases.length} cases with ${cases.reduce((n, c) => n + c.candidates.length, 0)} candidates ` +
    `(not used: ${JSON.stringify(skipped)}), ${slot.length} slot, ${atVariant.length} atVariant, ` +
    `${fromAst.length} fromAst, ${input.length} variants in the order`,
)
