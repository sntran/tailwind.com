// The entry of `assets/typescript.js`: the TypeScript transform of
// Sucrase. The value of the script is the function that removes the types
// of a module. The imports and exports stay as they are, and an import
// that is only a type goes, as with the TypeScript compiler.
//
// Sucrase removes a `namespace` with its content. A namespace can have
// values, so a step before Sucrase changes each namespace to the function
// that the TypeScript compiler makes for it.
import { transform } from 'sucrase'
import { parse } from 'sucrase/dist/esm/parser/index'
import { ContextualKeyword } from 'sucrase/dist/esm/parser/tokenizer/keywords'
import { TokenType as tt } from 'sucrase/dist/esm/parser/tokenizer/types'

type Token = { type: number; contextualKeyword: number; start: number; end: number }
type Edit = { start: number; end: number; text: string }

function isNamespaceKeyword(token: Token | undefined): boolean {
  return (
    token !== undefined &&
    token.type === tt.name &&
    (token.contextualKeyword === ContextualKeyword._namespace || token.contextualKeyword === ContextualKeyword._module)
  )
}

// `namespace Name { export const a = 1 }` becomes
// `var Name; (function (Name) { const a = 1 ;Name.a = a;})(Name || (Name = {}));`.
// The exported names go to the object at the end of the body. The line
// numbers stay the same.
export function rewriteNamespaces(source: string): string {
  if (!/\b(?:namespace|module)\s+[A-Za-z_$]/.test(source)) return source
  let tokens: Token[] = parse(source, false, true, false).tokens
  let text = (token: Token) => source.slice(token.start, token.end)
  let edits: Edit[] = []

  // The names that have a binding already: `class Name`, `function Name`,
  // `enum Name`, or a namespace before this one.
  let bound = new Set<string>()
  for (let i = 0; i + 1 < tokens.length; i++) {
    let token = tokens[i]
    let isEnum = token.type === tt._enum || (token.type === tt.name && token.contextualKeyword === ContextualKeyword._enum)
    if ((token.type === tt._class || token.type === tt._function || isEnum) && tokens[i + 1].type === tt.name) {
      bound.add(text(tokens[i + 1]))
    }
  }

  for (let i = 0; i + 2 < tokens.length; i++) {
    if (!isNamespaceKeyword(tokens[i]) || tokens[i + 1].type !== tt.name || tokens[i + 2].type !== tt.braceL) continue
    let before = tokens[i - 1]
    if (before && text(before) === 'declare') continue
    if (before && before.type === tt.dot) continue
    let name = text(tokens[i + 1])

    // The end of the body, and the exports of the body.
    let depth = 0
    let exported: string[] = []
    let end = -1
    for (let j = i + 2; j < tokens.length; j++) {
      let type = tokens[j].type
      if (type === tt.braceL || type === tt.dollarBraceL) depth++
      else if (type === tt.braceR && --depth === 0) {
        end = j
        break
      } else if (type === tt._export && depth === 1) {
        let k = j + 1
        let isName = (word: string) => text(tokens[k]) === word
        let ambient = false
        while (k < tokens.length && (isName('declare') || isName('abstract') || isName('async'))) {
          if (isName('declare')) ambient = true
          k++
        }
        let next = tokens[k]
        if (ambient || isName('type') || isName('interface')) continue
        // The word `export` goes: the body is the body of a function.
        edits.push({ start: tokens[j].start, end: tokens[j].end, text: ' '.repeat(tokens[j].end - tokens[j].start) })
        let value = next.type === tt._const || next.type === tt._let || next.type === tt._var
        if (next.type === tt._const && tokens[k + 1].type === tt._enum) k++
        else if (next.type === tt._function && tokens[k + 1].type === tt.star) k++
        let named =
          value || next.type === tt._function || next.type === tt._class || next.type === tt._enum || isNamespaceKeyword(next) || isName('enum')
        if (named && tokens[k + 1].type === tt.name) exported.push(text(tokens[k + 1]))
      }
    }
    if (end === -1) continue

    let declaration = bound.has(name) ? '' : `var ${name}; `
    bound.add(name)
    edits.push({ start: tokens[i].start, end: tokens[i + 2].end, text: `${declaration}(function (${name}) {` })
    let assignments = exported.map((item) => `${name}.${item} = ${item};`).join('')
    edits.push({ start: tokens[end].start, end: tokens[end].end, text: `;${assignments}})(${name} || (${name} = {}));` })
  }

  edits.sort((a, z) => a.start - z.start)
  let result = ''
  let position = 0
  for (let edit of edits) {
    result += source.slice(position, edit.start) + edit.text
    position = edit.end
  }
  return result + source.slice(position)
}

export default function stripTypes(source: string, file: string): string {
  try {
    return transform(rewriteNamespaces(source), { transforms: ['typescript'], disableESTransforms: true, filePath: file }).code
  } catch (error: any) {
    // The parser gives an error with `loc`: the message of a syntax error
    // has the file, the line and the column.
    let syntax: any = new SyntaxError(String(error?.message ?? error).replace(/^Error transforming [^:]*: /, ''))
    if (error?.loc) syntax.message += `\n${file}:${error.loc.line}:${error.loc.column}`
    throw syntax
  }
}
