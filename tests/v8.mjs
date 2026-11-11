#!/usr/bin/env node
// The parts of the JavaScript runtime that must do the same as V8,
// against Node.js: the sort (native/tailwind/js/sort.ts) and
// `Intl.ListFormat` (native/tailwind/js/intl.ts).
//
// Usage: node tests/v8.mjs [CASES] [--record tests/upstream/v8-sort.jsonl]
//
// The runtime has the sort algorithm of V8, because the result of a sort
// with a compare function that is not consistent depends on the
// algorithm (Tailwind CSS has such a function). This test sorts random
// arrays with the two sorts, and compares each call of the compare
// function (the two values, in sequence) and the result. The compare
// functions are consistent, not consistent, and without sense.
//
// Node.js 22.18 or later reads the TypeScript files directly.
import assert from 'node:assert'

const nativeSort = Array.prototype.sort
const nativeToSorted = Array.prototype.toSorted
const { sort, toSorted } = await import('../native/tailwind/js/sort.ts')
// The import replaced the functions of Array.prototype: put the functions
// of V8 back for the rest of this program.
Object.defineProperty(Array.prototype, 'sort', { value: nativeSort, writable: true, configurable: true })
Object.defineProperty(Array.prototype, 'toSorted', { value: nativeToSorted, writable: true, configurable: true })

let seed = 20261111
function random() {
  seed |= 0
  seed = (seed + 0x6d2b79f5) | 0
  let t = Math.imul(seed ^ (seed >>> 15), 1 | seed)
  t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t
  return ((t ^ (t >>> 14)) >>> 0) / 4294967296
}
const int = (n) => Math.floor(random() * n)

// A number from two values: the same for the same pair.
function hash(a, z, salt) {
  let h = Math.imul(a.key ^ salt, 0x9e3779b1) ^ Math.imul(z.key + salt, 0x85ebca6b)
  h ^= h >>> 13
  h = Math.imul(h, 0xc2b2ae35)
  return (h ^ (h >>> 16)) >>> 0
}

// The compare functions. Each one gets a salt.
const COMPARES = {
  consistent: () => (a, z) => a.key - z.key,
  reversed: () => (a, z) => z.key - a.key,
  // Many equal elements: the sort must be stable.
  coarse: () => (a, z) => (a.key >> 4) - (z.key >> 4),
  // As the breakpoint compare of Tailwind CSS: an element with no value
  // is "more" from both sides.
  holes: (salt) => (a, z) => {
    if ((a.key + salt) % 5 === 0) return 1
    if ((z.key + salt) % 5 === 0) return 1
    return a.key - z.key
  },
  // No sense at all: -1, 0 or 1 from the pair.
  noise: (salt) => (a, z) => (hash(a, z, salt) % 3) - 1,
  // Mostly consistent: long runs, and some wrong answers.
  mostly: (salt) => (a, z) => (hash(a, z, salt) % 11 === 0 ? z.key - a.key : a.key - z.key),
  always: () => () => 1,
  never: () => () => -1,
  zero: () => () => 0,
  nan: (salt) => (a, z) => (hash(a, z, salt) % 4 === 0 ? NaN : a.key - z.key),
  boolean: () => (a, z) => a.key > z.key,
  text: () => (a, z) => String(a.key - z.key),
}

// The arrays: random, sorted parts, a few different values, long runs
// (the merges gallop).
function values(length, kind) {
  let out = []
  for (let i = 0; i < length; i++) {
    let key
    switch (kind) {
      case 'random': key = int(1 << 20); break
      case 'few': key = int(8); break
      case 'ascending': key = i; break
      case 'descending': key = length - i; break
      case 'runs': key = (i % 97) + (Math.floor(i / 97) % 3) * 1000; break
      case 'blocks': key = Math.floor(i / 40) % 2 === 0 ? i : length - i; break
      default: key = int(length)
    }
    out.push({ key, id: i })
  }
  return out
}
const KINDS = ['random', 'few', 'ascending', 'descending', 'runs', 'blocks', 'mixed']
const LENGTHS = [0, 1, 2, 3, 5, 8, 16, 31, 32, 33, 63, 64, 65, 100, 127, 128, 129, 200, 500, 1000, 2500]

function run(sorter, input, compare) {
  let calls = []
  let array = input.slice()
  let error = null
  try {
    sorter.call(array, (a, z) => {
      calls.push(a.id, z.id)
      // A bad sort can loop: stop it.
      if (calls.length > 4000000) throw new Error('too many calls')
      return compare(a, z)
    })
  } catch (e) {
    error = String(e)
  }
  return { calls, ids: array.map((item) => item.id), error }
}

const cases = Number(/^\d+$/.test(process.argv[2] ?? '') ? process.argv[2] : 4000)
let done = 0
let callsTotal = 0
const names = Object.keys(COMPARES)
for (let i = 0; i < cases; i++) {
  let length = i < LENGTHS.length * 4 ? LENGTHS[i % LENGTHS.length] : random() < 0.7 ? int(70) : int(1500)
  let kind = KINDS[int(KINDS.length)]
  let name = names[i % names.length]
  let input = values(length, kind)
  let salt = int(1 << 30)
  let expected = run(nativeSort, input, COMPARES[name](salt))
  let actual = run(sort, input, COMPARES[name](salt))
  let label = `case ${i}: ${length} values (${kind}), compare "${name}"`
  assert.deepStrictEqual(actual.error, expected.error, `${label}: the error`)
  assert.strictEqual(actual.calls.length, expected.calls.length, `${label}: the number of calls`)
  assert.deepStrictEqual(actual.calls, expected.calls, `${label}: the calls of the compare function`)
  assert.deepStrictEqual(actual.ids, expected.ids, `${label}: the result`)
  callsTotal += expected.calls.length / 2
  done++
}

// With no compare function, `undefined` values, holes, and `toSorted`.
for (let i = 0; i < 300; i++) {
  let length = int(120)
  let make = () => {
    let saved = seed
    let array = []
    for (let k = 0; k < length; k++) {
      let r = int(10)
      if (r === 0) array.push(undefined)
      else if (r === 1) array.length++
      else if (r === 2) array.push(String(int(50)))
      else array.push(int(2000) - 1000)
    }
    return [array, saved]
  }
  let [a] = make()
  let b = a.slice()
  // slice keeps the holes.
  assert.deepStrictEqual(sort.call(b), nativeSort.call(a), `no compare function, case ${i}`)
  let c = a.slice()
  assert.deepStrictEqual(toSorted.call(c, (x, y) => (x < y ? -1 : x > y ? 1 : 0)), nativeToSorted.call(a, (x, y) => (x < y ? -1 : x > y ? 1 : 0)), `toSorted, case ${i}`)
  done++
}
assert.throws(() => sort.call([2, 1], 1), TypeError)
assert.deepStrictEqual(sort.call({ length: 3, 0: 'c', 1: 'a', 2: 'b' }), { length: 3, 0: 'a', 1: 'b', 2: 'c' })

console.log(`v8.mjs: sort: ${done} cases, ${callsTotal} calls of the compare function: the same as V8 (Node.js ${process.version})`)

// With --record FILE: sorts of V8 for the Rust form of the algorithm
// (native/tailwind/src/utils/v8_sort.rs, tests/v8_sort.rs). Each line has
// the name of a compare function, its salt, the keys of the values, and
// the sequence of the values after the sort of V8.
const recordAt = process.argv.indexOf('--record')
if (recordAt !== -1) {
  const fs = await import('node:fs')
  const recorded = ['consistent', 'reversed', 'coarse', 'holes', 'noise', 'mostly', 'always', 'never', 'zero']
  const lines = []
  seed = 4711
  for (let i = 0; i < 180; i++) {
    let length = i < 40 ? [0, 1, 2, 3, 7, 31, 32, 33, 63, 64, 65, 127, 128, 129, 200, 257, 400, 640, 1000, 1500][i % 20] : random() < 0.75 ? int(90) : int(700)
    let kind = KINDS[int(KINDS.length)]
    let name = recorded[i % recorded.length]
    let input = values(length, kind)
    let salt = int(1 << 30)
    let result = run(nativeSort, input, COMPARES[name](salt))
    lines.push(JSON.stringify({ compare: name, salt, keys: input.map((item) => item.key), sorted: result.ids, calls: result.calls.length / 2 }))
  }
  fs.writeFileSync(process.argv[recordAt + 1], lines.join('\n') + '\n')
  console.log(`v8.mjs: recorded ${lines.length} sorts of V8 in ${process.argv[recordAt + 1]}`)
}

// `Intl.ListFormat`, for English.
const NativeIntl = globalThis.Intl
const { Intl: OurIntl } = await import('../native/tailwind/js/intl.ts')
assert.strictEqual(globalThis.Intl, NativeIntl)
let lists = 0
for (let type of ['conjunction', 'disjunction', 'unit', undefined]) {
  for (let style of ['long', 'short', 'narrow', undefined]) {
    for (let length = 0; length <= 6; length++) {
      let list = Array.from({ length }, (_, i) => ['`has-group`', 'in-not', 'a b', '', 'é', 'x,y'][i])
      let expected = new NativeIntl.ListFormat('en', { style, type })
      let actual = new OurIntl.ListFormat('en', { style, type })
      assert.strictEqual(actual.format(list), expected.format(list), `Intl.ListFormat ${type} ${style} ${length}`)
      assert.deepStrictEqual(actual.formatToParts(list), expected.formatToParts(list))
      assert.deepStrictEqual(actual.resolvedOptions(), expected.resolvedOptions())
      lists++
    }
  }
}
assert.strictEqual(new OurIntl.ListFormat().format(new Set(['a', 'b'])), 'a and b')
assert.throws(() => new OurIntl.ListFormat('en', { type: 'x' }), RangeError)
assert.throws(() => new OurIntl.ListFormat('en').format([1]), TypeError)
console.log(`v8.mjs: Intl.ListFormat: ${lists} lists: the same as V8`)
