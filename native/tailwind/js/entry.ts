// The entry of the compatibility bundle (`assets/compat.js`).
//
// The bundle has the plugin layer of Tailwind CSS (`compat/` of its
// source) with no change. The parts of the compiler that have state (the
// theme, the utilities, the variants, `@apply`, the CSS functions) are in
// the host. The host calls the global functions of this file. Each one
// gets JSON text and returns JSON text or `undefined`.
import type { AstNode } from 'tailwindcss-src/ast'
import { applyCompatibilityHooks } from 'tailwindcss-src/compat/apply-compat-hooks'
import { createDesignSystem } from './design-system'
import './globals'
import { callbacks } from './host'
import { loadModule, moduleSource } from './modules'

declare function __await<T>(promise: Promise<T>): T

let designSystem = createDesignSystem()
let global = globalThis as any

// Run `applyCompatibilityHooks` on the tree of the stylesheet.
global.__tw_compat = (text: string) => {
  let { base, ast, sources } = JSON.parse(text) as {
    base: string
    ast: AstNode[]
    sources: { base: string; pattern: string; negated: boolean }[]
  }
  let features = __await(
    applyCompatibilityHooks({ designSystem: designSystem as any, base, ast, loadModule, sources }),
  )
  return JSON.stringify({ ast, sources, features })
}

// The compile function of a utility. `undefined`: no match. `null`: the
// candidate is not valid.
global.__tw_utility = (text: string) => {
  let { id, candidate } = JSON.parse(text)
  let result = callbacks[id](candidate)
  return result === undefined ? undefined : JSON.stringify(result)
}

// The function of a variant. It changes the rule in place.
global.__tw_variant = (text: string) => {
  let { id, rule, variant } = JSON.parse(text)
  let result = callbacks[id](rule, variant)
  return JSON.stringify({ rule, applies: result !== null })
}

// The compare function of a group of variants.
global.__tw_compare = (text: string) => {
  let { id, a, z } = JSON.parse(text)
  return JSON.stringify(callbacks[id](a, z))
}

// `designSystem.resolveThemeValue`, after upstream replaced it.
global.__tw_resolve_theme_value = (text: string) => {
  let { path, forceInline } = JSON.parse(text)
  let result = designSystem.resolveThemeValue(path, forceInline)
  return result === undefined || result === null ? undefined : JSON.stringify(result)
}

// The source of an ES module, for the loader of the engine.
global.__tw_module_source = (name: string) => moduleSource(name)
