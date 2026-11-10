// Shared helpers for the generator scripts.
// The scripts load the TypeScript sources of the upstream clone with jiti.
// The clone must have its dependencies: run "pnpm install --ignore-scripts" in it first.
import fs from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import zlib from 'node:zlib'

// NODE_ENV=test stops the license banner comment at the top of the output.
// The upstream unit tests run with the same value.
process.env.NODE_ENV = 'test'

export async function loadUpstream(clone) {
  clone = path.resolve(clone)
  let nodePackage = path.join(clone, 'packages/@tailwindcss-node')
  let require = createRequire(path.join(nodePackage, 'package.json'))
  let { createJiti } = require('jiti')
  let jiti = createJiti(path.join(clone, 'packages/tailwindcss/src/'), { moduleCache: true })
  let tailwind = await jiti.import(path.join(clone, 'packages/tailwindcss/src/index.ts'))
  let { optimize } = await jiti.import(path.join(nodePackage, 'src/optimize.ts'))
  let packageDir = path.join(clone, 'packages/tailwindcss')

  // The loader gives virtual paths, so that the fixtures do not contain machine paths.
  // "tailwindcss" maps to index.css. "tailwindcss/x.css" and "./x.css" map to x.css.
  let cache = new Map()
  async function loadStylesheet(id, base) {
    let name = id === 'tailwindcss' ? 'index.css' : id.replace(/^tailwindcss\//, '').replace(/^\.\//, '')
    if (!cache.has(name)) cache.set(name, fs.readFileSync(path.join(packageDir, name), 'utf8'))
    return { path: `/tailwindcss/${name}`, base: '/tailwindcss', content: cache.get(name) }
  }

  return {
    compile: tailwind.compile,
    loadDesignSystem: tailwind.__unstable__loadDesignSystem,
    optimize,
    loadStylesheet,
    packageDir,
  }
}

export function writeGzip(file, text) {
  fs.writeFileSync(file, zlib.gzipSync(text, { level: 9 }))
}

// A small seeded random number generator (mulberry32).
export function createRandom(seed) {
  let a = seed >>> 0
  function next() {
    a = (a + 0x6d2b79f5) >>> 0
    let t = a
    t = Math.imul(t ^ (t >>> 15), t | 1)
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61)
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296
  }
  return {
    next,
    int: (n) => Math.floor(next() * n),
    pick: (list) => list[Math.floor(next() * list.length)],
    chance: (p) => next() < p,
  }
}
