// The design system that the compatibility layer of Tailwind CSS sees.
//
// The theme, the utilities and the variants are in the host. Each method
// here sends one operation to the host. The functions of plugins stay in
// JavaScript: the host gets a number for each one (see `host.ts`), and
// calls it through `entry.ts`.
import type { AstNode } from 'tailwindcss-src/ast'
import { host, register } from './host'

function createTheme() {
  return {
    get prefix(): string | null {
      return host('theme.prefix') ?? null
    },
    set prefix(value: string | null) {
      host('theme.setPrefix', value)
    },
    add(key: string, value: string, options = 0) {
      host('theme.add', [key, value, options])
    },
    get(themeKeys: string[]): string | null {
      return host('theme.get', themeKeys) ?? null
    },
    getOptions(key: string): number {
      return host('theme.getOptions', key)
    },
    hasDefault(key: string): boolean {
      return host('theme.hasDefault', key)
    },
    keysInNamespaces(themeKeys: string[]): string[] {
      return host('theme.keysInNamespaces', themeKeys)
    },
    namespace(namespace: string): Map<string | null, string> {
      return new Map(host('theme.namespace', namespace))
    },
    clearNamespace(namespace: string, options: number) {
      host('theme.clearNamespace', [namespace, options])
    },
    resolve(candidateValue: string | null, themeKeys: string[], options = 0): string | null {
      return host('theme.resolve', [candidateValue, themeKeys, options]) ?? null
    },
    resolveValue(candidateValue: string | null, themeKeys: string[]): string | null {
      return host('theme.resolveValue', [candidateValue, themeKeys]) ?? null
    },
    resolveWith(candidateValue: string, themeKeys: string[], nestedKeys: string[] = []) {
      return host('theme.resolveWith', [candidateValue, themeKeys, nestedKeys]) ?? null
    },
    markUsedVariable(themeKey: string): boolean {
      return host('theme.markUsedVariable', themeKey)
    },
    entries(): [string, { value: string; options: number }][] {
      return host('theme.entries')
    },
    addKeyframes(rule: AstNode) {
      host('theme.addKeyframes', rule)
    },
    getKeyframes(): AstNode[] {
      return host('theme.getKeyframes')
    },
  }
}

function createUtilities() {
  return {
    static(name: string, compileFn: Function) {
      host('utilities.static', { name, id: register(compileFn) })
    },
    functional(name: string, compileFn: Function, options?: { types: string[] }) {
      host('utilities.functional', { name, id: register(compileFn), types: options?.types })
    },
    has(name: string, kind: 'static' | 'functional'): boolean {
      return host('utilities.has', { name, kind })
    },
    // Suggestions are for editors. The host does not keep them.
    suggest(_name: string, _fn: Function) {},
  }
}

type VariantOptions = { compounds?: number; order?: number }

function createVariants() {
  // The function of a variant of the host. The host keeps the function
  // under `stash`: a later registration of the same name does not change
  // it.
  function hostApplyFn(stash: number) {
    return (rule: any, variant: any) => {
      let result = host('variants.apply', { stash, rule, variant })
      Object.assign(rule, result.rule)
      return result.applies ? undefined : null
    }
  }

  return {
    static(name: string, applyFn: Function, options: VariantOptions = {}) {
      host('variants.static', { name, id: register(applyFn), ...options })
    },
    functional(name: string, applyFn: Function, options: VariantOptions = {}) {
      host('variants.functional', { name, id: register(applyFn), ...options })
    },
    compound(name: string, compoundsWith: number, applyFn: Function, options: VariantOptions = {}) {
      host('variants.compound', { name, compoundsWith, id: register(applyFn), ...options })
    },
    fromAst(name: string, ast: AstNode[], _designSystem: unknown) {
      host('variants.fromAst', { name, ast })
    },
    group(fn: () => void, compareFn?: Function) {
      host('variants.groupStart', { id: compareFn ? register(compareFn) : null })
      try {
        fn()
      } finally {
        host('variants.groupEnd')
      }
    },
    has(name: string): boolean {
      return host('variants.has', name)
    },
    get(name: string) {
      let info = host('variants.get', name)
      if (!info) return undefined
      return { ...info, applyFn: hostApplyFn(info.stash) }
    },
    kind(name: string): string {
      return host('variants.kind', name)
    },
    compoundsWith(parent: string, child: any): boolean {
      return host('variants.compoundsWith', { parent, child })
    },
    suggest(_name: string, _fn: Function) {},

    // `compat/screens-config.ts` reads each variant and changes its order.
    get variants() {
      let entries: [string, { order: number }][] = host('variants.orders').map(
        ([name, order]: [string, number]) => {
          let current = order
          return [
            name,
            {
              get order() {
                return current
              },
              set order(value: number) {
                current = value
                host('variants.setOrder', { name, order: value })
              },
            },
          ]
        },
      )
      return new Map(entries)
    },

    // `compat/screens-config.ts` moves the compare functions to new orders.
    // The values of this map are the old orders: the functions stay in
    // the host.
    get compareFns(): Map<number, number> {
      return new Map(host('variants.compareFnOrders').map((order: number) => [order, order]))
    },
    set compareFns(map: Map<number, number>) {
      host('variants.moveCompareFns', Array.from(map))
    },
  }
}

export function createDesignSystem() {
  // The function of the host: theme variables only.
  let resolveThemeValue = (path: string, forceInline?: boolean): string | undefined =>
    host('resolveThemeValue', { path, forceInline: forceInline ?? true }) ?? undefined
  let hooked = false

  return {
    theme: createTheme(),
    utilities: createUtilities(),
    variants: createVariants(),
    invalidCandidates: {
      add(candidate: string) {
        host('invalidCandidates.add', candidate)
      },
    },
    get important(): boolean {
      return host('important')
    },
    set important(value: boolean) {
      host('setImportant', value)
    },
    // Upstream assigns a new function. From then on, the host calls the
    // JavaScript function for each path that is not a theme variable.
    get resolveThemeValue() {
      return resolveThemeValue
    },
    set resolveThemeValue(fn: (path: string, forceInline?: boolean) => string | undefined) {
      resolveThemeValue = fn
      if (!hooked) {
        hooked = true
        host('hookResolveThemeValue')
      }
    },
  }
}
