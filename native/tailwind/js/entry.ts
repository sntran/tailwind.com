// The entry of the compatibility bundle (`assets/compat.js`).
//
// The bundle has the plugin layer of Tailwind CSS (`compat/` of its
// source) with no change. The parts of the compiler that have state (the
// theme, the utilities, the variants, `@apply`, the CSS functions) are in
// the host. The host calls the global functions of this file. Each one
// gets JSON text and returns JSON text or `undefined`.
import type { AstNode } from 'tailwindcss-src/ast'
import { applyCompatibilityHooks } from 'tailwindcss-src/compat/apply-compat-hooks'
import colors from 'tailwindcss-src/compat/colors'
import defaultTheme from 'tailwindcss-src/compat/default-theme'
import flattenColorPalette from 'tailwindcss-src/compat/flatten-color-palette'
import createPlugin from 'tailwindcss-src/plugin'
import { createDesignSystem } from './design-system'
import './globals'
import { callbacks, decode, encode, setTracking } from './host'
import { withCauses, withCausesAsync } from './causes'
import { loadModule, moduleSource, setTailwindModules } from './modules'
import { drainTimers, drainTimersAsync, runTimer } from './timers'

declare function __await<T>(promise: Promise<T>): T
// The engine of a browser (js/engine.mjs in the npm package): a promise
// that settles after the jobs of the promises that are pending now.
declare function __tw_yield(): Promise<void>

let designSystem = createDesignSystem()
let global = globalThis as any

// The host calls this function with `true` before `__tw_compat` when the
// compiler keeps the source locations (see `host.ts`).
global.__tw_track = (text: string) => {
  setTracking(text === 'true')
}

setTailwindModules({
  'tailwindcss/plugin': () => createPlugin,
  'tailwindcss/colors': () => colors,
  'tailwindcss/defaultTheme': () => defaultTheme,
  'tailwindcss/lib/util/flattenColorPalette': () => flattenColorPalette,
})

// Run `applyCompatibilityHooks` on the tree of the stylesheet.
global.__tw_compat = (text: string) =>
  withCauses(() => {
    let { base, ast, sources } = decode(text) as {
      base: string
      ast: AstNode[]
      sources: { base: string; pattern: string; negated: boolean }[]
    }
    let features = __await(
      applyCompatibilityHooks({ designSystem: designSystem as any, base, ast, loadModule, sources }),
    )
    // The timers that no module waited for run now, as at the end of a
    // program of Node.js.
    drainTimers(__await)
    return encode({ ast, sources, features })
  })

// `__tw_compat` for the engine of a browser, which has no `__await`: the
// program waits for the promise of this function (JS Promise
// Integration). The jobs and the timers run as with `__await`.
global.__tw_compat_async = (text: string) =>
  withCausesAsync(async () => {
    let { base, ast, sources } = decode(text) as {
      base: string
      ast: AstNode[]
      sources: { base: string; pattern: string; negated: boolean }[]
    }
    let features = await settle(
      applyCompatibilityHooks({ designSystem: designSystem as any, base, ast, loadModule, sources }),
    )
    await drainTimersAsync(settle)
    return encode({ ast, sources, features })
  })

// Wait for a promise as `__await` does: when no job is pending, the next
// timer runs.
async function settle<T>(promise: Promise<T>): Promise<T> {
  let done = false
  promise.then(
    () => (done = true),
    () => (done = true),
  )
  for (;;) {
    await __tw_yield()
    if (done) return promise
    if (!runTimer()) throw new Error('The promise cannot settle: no job and no timer is pending')
  }
}

// The compile function of a utility. `undefined`: no match. `null`: the
// candidate is not valid.
global.__tw_utility = (text: string) => {
  let { id, candidate } = decode(text)
  let result = withCauses(() => callbacks[id](candidate))
  return result === undefined ? undefined : encode(result)
}

// The function of a variant. It changes the rule in place.
global.__tw_variant = (text: string) => {
  let { id, rule, variant } = decode(text)
  let result = withCauses(() => callbacks[id](rule, variant))
  return encode({ rule, applies: result !== null })
}

// The compare function of a group of variants.
global.__tw_compare = (text: string) => {
  let { id, a, z } = decode(text)
  return encode(callbacks[id](a, z))
}

// `designSystem.resolveThemeValue`, after upstream replaced it.
global.__tw_resolve_theme_value = (text: string) => {
  let { path, forceInline } = decode(text)
  let result = designSystem.resolveThemeValue(path, forceInline)
  return result === undefined || result === null ? undefined : encode(result)
}

// The source of an ES module, for the loader of the engine.
global.__tw_module_source = (name: string) => moduleSource(name)
