// In place of `variants.ts`: the host replaces `@variant` rules. The other
// functions are the functions of Tailwind CSS.
//
// Only the plugin layer of Tailwind CSS 4.3.1 calls `substituteAtVariant`
// (for the styles of `addBase`). The function of Tailwind CSS needs the
// compiler, which is in the host.
import type { AstNode } from 'tailwindcss-src/ast'
import { host } from '../host'
import { usesAtRule } from './apply'

// This export of `substituteAtVariant` replaces the one of the module.
export * from 'tailwindcss-real/variants'

export function substituteAtVariant(ast: AstNode[], _designSystem: unknown): number {
  if (!usesAtRule(ast, '@variant')) return 0
  let result = host('substituteAtVariant', ast)
  ast.splice(0, ast.length, ...result.ast)
  return result.features
}
