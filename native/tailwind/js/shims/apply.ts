// In place of `apply.ts`: the host does `@apply`.
import type { AstNode } from 'tailwindcss-src/ast'
import { host } from '../host'

export function substituteAtApply(ast: AstNode[], _designSystem: unknown): number {
  // Most trees of plugins have no `@apply`.
  if (!usesAtRule(ast, '@apply')) return 0
  let result = host('substituteAtApply', ast)
  ast.splice(0, ast.length, ...result.ast)
  return result.features
}

export function usesAtRule(ast: AstNode[], name: string): boolean {
  for (let node of ast) {
    if (node.kind === 'at-rule' && node.name === name) return true
    if ('nodes' in node && node.nodes && usesAtRule(node.nodes, name)) return true
  }
  return false
}
