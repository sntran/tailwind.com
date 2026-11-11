#!/usr/bin/env node
// The sort of the JavaScript runtime (native/tailwind/js/sort.ts) against
// the sort of V8, in Node.js.
//
// Usage: node tests/sort.mjs [CASES]
//
// The runtime has the sort algorithm of V8, because the result of a sort
// with a compare function that is not consistent depends on the
// algorithm (Tailwind CSS has such a function). This test sorts random
// arrays with the two sorts, and compares each call of the compare
// function (the two values, in sequence) and the result. The compare
// functions are consistent, not consistent, and without sense.
//
// Node.js 22.18 or later reads the TypeScript file directly.
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

const cases = Number(process.argv[2] ?? 4000)
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

console.log(`sort.mjs: ${done} cases, ${callsTotal} calls of the compare function: the same as V8 (Node.js ${process.version})`)
