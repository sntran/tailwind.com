import plugin from 'tailwindcss/plugin'
import type { PluginAPI } from 'tailwindcss/plugin'
import { rules, Level, type Rules } from './lib/rules.js'

type Api = Pick<PluginAPI, 'addUtilities' | 'addVariant' | 'matchUtilities'>

namespace Depth {
  export const values: Record<string, string> = { 1: '10', 2: '20' }
  export function rule(value: string) {
    return { zIndex: value }
  }
}

function assertRules(value: unknown): asserts value is Rules {
  if (typeof value !== 'object' || value === null) throw new TypeError('not rules')
}

export default plugin(({ addUtilities, addVariant, matchUtilities }: Api) => {
  let all: unknown = rules
  assertRules(all)
  addUtilities(all)
  addVariant('hocus', ['&:hover', '&:focus'])
  matchUtilities({ depth: Depth.rule }, { values: Depth.values })
  addUtilities({ '.ts-two': { opacity: String(Level.Half / 100) } })
})
