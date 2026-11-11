// In place of `css-functions.ts`: the host replaces `theme(…)` and the
// other functions.
import type { AstNode } from 'tailwindcss-src/ast'
import { host } from '../host'

export function substituteFunctions(ast: AstNode[], _designSystem: unknown): number {
  let result = host('substituteFunctions', ast)
  ast.splice(0, ast.length, ...result.ast)
  return result.features
}
