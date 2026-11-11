// TypeScript modules: the types go, and the rest is JavaScript. There is
// no type check, as with jiti in the official program.
//
// The transformer is a second JavaScript file of the program (the
// TypeScript transform of Sucrase: `assets/typescript.js`). The engine
// evaluates it at the first TypeScript file only.

declare function __host(op: string, text?: string): string | undefined

type Strip = (source: string, file: string) => string

let strip: Strip | null = null

export function isTypeScript(file: string): boolean {
  return /\.[cm]?ts$/.test(file)
}

export function stripTypes(source: string, file: string): string {
  if (strip === null) {
    let asset = __host('asset', 'typescript')
    if (typeof asset !== 'string') throw new Error(`Cannot load '${file}': this program has no TypeScript transformer.`)
    strip = (0, eval)(asset) as Strip
  }
  return strip(source, file)
}
