#!/usr/bin/env node
// Usage: node gen-classes.mjs <path of the upstream clone> <output directory>
// This script writes classes.jsonl.gz and classes-all.json.gz.
import path from 'node:path'
import { ARBITRARY_FORMS, EDGE_CANDIDATES, RANDOM_MODIFIERS, RANDOM_VARIANTS } from './candidate-lists.mjs'
import { createRandom, loadUpstream, writeGzip } from './lib.mjs'

export const SEED = 20240405
export const RANDOM_COUNT = 6000
export const INPUT =
  '@layer theme, base, components, utilities;\n' +
  '@import "tailwindcss/theme.css" layer(theme);\n' +
  '@import "tailwindcss/utilities.css" layer(utilities);'

let [clone, outDir] = process.argv.slice(2)
if (!clone || !outDir) {
  console.error('Usage: node gen-classes.mjs <path of the upstream clone> <output directory>')
  process.exit(2)
}

let up = await loadUpstream(clone)
let options = { loadStylesheet: up.loadStylesheet }
let design = await up.loadDesignSystem(INPUT, options)
let classList = design.getClassList()
let variants = design.getVariants()

// Pick at most `count` items, spread out over the list. The first and the last item are always in.
function spread(list, count) {
  if (list.length <= count) return [...list]
  let result = []
  for (let i = 0; i < count; i++) result.push(list[Math.round((i * (list.length - 1)) / (count - 1))])
  return result
}

let seen = new Set()
let candidates = []
function add(kind, candidate) {
  if (seen.has(candidate)) return false
  seen.add(candidate)
  candidates.push({ kind, candidate })
  return true
}

// (a) The class list, and each class with some of its modifiers.
for (let [name, meta] of classList) {
  add('class', name)
  for (let modifier of spread(meta.modifiers, 3)) add('class', `${name}/${modifier}`)
}

// (b) The variants.
function variantName(entry, value) {
  if (value === undefined) return entry.name
  return entry.hasDash ? `${entry.name}-${value}` : `${entry.name}${value}`
}
for (let entry of variants) {
  add('variant', `${entry.name}:flex`)
  for (let value of spread(entry.values, 5)) add('variant', `${variantName(entry, value)}:flex`)
}
let inner = ['hover', 'focus', '[&_p]', 'aria-checked', 'data-[state=open]']
for (let compound of ['group', 'peer', 'not', 'has', 'in']) {
  for (let item of inner) {
    add('variant', `${compound}-${item}:flex`)
    if (compound === 'group' || compound === 'peer') add('variant', `${compound}-${item}/name:flex`)
  }
}
for (let item of ['group-hover/name', 'peer-checked/name', 'group-has-[a]/item', 'group-not-hover', 'peer-not-focus/x', 'not-has-[a]', 'has-not-[a]', 'in-not-hover', 'group-in-[.x]', 'not-group-hover', 'not-peer-checked/name', 'has-group-hover']) {
  add('variant', `${item}:flex`)
}

// (c) Random candidates from a seeded generator.
let random = createRandom(SEED)
let classNames = classList.map(([name]) => name)
let variantPool = [...RANDOM_VARIANTS]
for (let entry of variants) {
  variantPool.push(entry.name)
  for (let value of spread(entry.values, 5)) variantPool.push(variantName(entry, value))
}
let randomTotal = 0
while (randomTotal < RANDOM_COUNT) {
  let roll = random.next()
  let depth = roll < 0.4 ? 0 : roll < 0.75 ? 1 : roll < 0.93 ? 2 : 3
  let parts = []
  for (let i = 0; i < depth; i++) parts.push(random.pick(variantPool))

  let base = random.chance(0.55) ? random.pick(classNames) : random.pick(ARBITRARY_FORMS)
  if (random.chance(0.08)) base = `-${base}`
  if (!base.includes('/') && random.chance(0.15)) base = `${base}/${random.pick(RANDOM_MODIFIERS)}`
  let flag = random.next()
  if (flag < 0.15) base = `${base}!`
  else if (flag < 0.19) base = `!${base}`

  parts.push(base)
  if (add('random', parts.join(':'))) randomTotal++
}

// (d) Hand-made invalid candidates and edge cases.
for (let candidate of EDGE_CANDIDATES) add('edge', candidate)

console.log(`${classList.length} classes, ${variants.length} variants, ${candidates.length} candidates`)

// One fresh compiler for each candidate.
let lines = []
let started = Date.now()
for (let [index, entry] of candidates.entries()) {
  let compiler = await up.compile(INPUT, options)
  let record = { candidate: entry.candidate, kind: entry.kind }
  try {
    record.css = compiler.build([entry.candidate])
  } catch (err) {
    record.error = err instanceof Error ? err.message : String(err)
  }
  lines.push(JSON.stringify(record))
  if ((index + 1) % 10000 === 0) {
    console.log(`${index + 1} / ${candidates.length} (${Math.round((Date.now() - started) / 1000)} s)`)
  }
}
writeGzip(path.join(outDir, 'classes.jsonl.gz'), lines.join('\n') + '\n')
console.log(`classes.jsonl.gz: ${lines.length} records`)

// One compiler for all the candidates in one build call.
let all = candidates.map((entry) => entry.candidate)
let compiler = await up.compile(INPUT, options)
let css = compiler.build(all)
let result = { input: INPUT, candidates: all, css }
try {
  result.optimized = up.optimize(css).code
  result.minified = up.optimize(css, { minify: true }).code
} catch (err) {
  result.optimizeError = err instanceof Error ? err.message : String(err)
}
writeGzip(path.join(outDir, 'classes-all.json.gz'), JSON.stringify(result))
console.log(`classes-all.json.gz: ${all.length} candidates, ${css.length} characters of CSS`)
