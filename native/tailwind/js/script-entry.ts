// The entry of the script bundle (`assets/script.js`).
//
// The script engine runs the official compiler of Tailwind CSS: the file
// `dist/lib.js` of the npm package `tailwindcss` of one version, with no
// change. This bundle has no code of Tailwind CSS. It has the loader of
// modules and the parts of Node.js that the compatibility bundle has too,
// and the functions that the host calls. The host gives the files of the
// package (`asset` with the name `tailwindcss:FILE`).
import { withCauses } from './causes'
import './globals'
import { decode, encode, host } from './host'
import { loadModule, moduleSource, runProgramFile, setTailwindModules } from './modules'
import { drainTimers } from './timers'

declare function __await<T>(promise: Promise<T>): T
declare function __host(op: string, text?: string): string | undefined

let global = globalThis as any

// A file of the package, as text.
function packageFile(name: string): string {
  let source = __host('asset', `tailwindcss:${name}`)
  if (typeof source !== 'string') throw new Error(`The package tailwindcss has no file '${name}'.`)
  return source
}

// The CommonJS file of each subpath of the package, from `exports` of its
// `package.json`: `.` is the compiler, and `./plugin`, `./colors`,
// `./defaultTheme` and `./lib/util/flattenColorPalette` are the modules
// that a plugin or a config can load.
let targets = new Map<string, string>()
for (let [subpath, entry] of Object.entries<any>(JSON.parse(packageFile('package.json')).exports ?? {})) {
  let target = typeof entry === 'string' ? entry : (entry?.require ?? entry?.default)
  if (typeof target === 'string' && target.endsWith('.js') && target.startsWith('./')) {
    targets.set(subpath, target.slice(2))
  }
}

let loaded = new Map<string, any>()

function packageModule(subpath: string): any {
  if (!loaded.has(subpath)) {
    let file = targets.get(subpath)
    if (file === undefined) throw new Error(`The package tailwindcss has no module '${subpath}'.`)
    loaded.set(subpath, runProgramFile(packageFile(file), `/$tailwindcss/${file}`))
  }
  return loaded.get(subpath)
}

// `require('tailwindcss/NAME')` gives the module of the package. The
// name with `.js` is the same module: the loader removes the extension.
let modules: Record<string, () => any> = { tailwindcss: () => packageModule('.') }
for (let subpath of targets.keys()) {
  if (subpath !== '.' && !subpath.endsWith('.js')) modules[`tailwindcss/${subpath.slice(2)}`] = () => packageModule(subpath)
}
setTailwindModules(modules)

// `loadStylesheet` of the compiler: the host resolves and reads the file.
async function loadStylesheet(id: string, base: string) {
  return host('stylesheet.load', { id, base }) as { path: string; base: string; content: string }
}

// The compiler of the stylesheet. One engine has one stylesheet.
let compiler: any

// Compile the stylesheet. The result has what the caller needs for the
// scanner: the sources, the root and the features.
global.__tw_script_compile = (text: string) =>
  withCauses(() => {
    let { css, base, from } = decode(text) as { css: string; base: string; from?: string }
    let tailwind = packageModule('.')
    let compile = tailwind.compile ?? tailwind.default?.compile
    if (typeof compile !== 'function') throw new Error('The package tailwindcss has no function `compile`.')
    compiler = __await(compile(css, { base, from, loadModule, loadStylesheet }))
    // The timers that no module waited for run now, as at the end of a
    // program of Node.js.
    drainTimers(__await)
    // Before version 4.1, the compiler gave `globs`, with no `negated`.
    let sources: { base: string; pattern: string; negated?: boolean }[] = compiler.sources ?? compiler.globs ?? []
    return encode({
      sources: sources.map((source) => ({ base: source.base, pattern: source.pattern, negated: source.negated === true })),
      root: compiler.root ?? null,
      features: compiler.features ?? 0,
    })
  })

// Build the CSS for the candidates (a JSON list). The result is the CSS,
// as plain text.
global.__tw_script_build = (text: string) => withCauses(() => String(compiler.build(decode(text))))

// The decoded source map of the last build: `buildSourceMap()`. A mapping
// is a list: the generated line and column, then the index of the
// source (-1: none), the original line and column, and the name.
global.__tw_script_source_map = () =>
  withCauses(() => {
    if (typeof compiler.buildSourceMap !== 'function') {
      throw new Error('This version of Tailwind CSS has no source maps.')
    }
    let map = compiler.buildSourceMap()
    let index = new Map<unknown, number>(map.sources.map((source: unknown, i: number) => [source, i]))
    return JSON.stringify({
      file: map.file ?? null,
      sources: map.sources.map((source: any) => ({
        url: source.url ?? null,
        content: source.content ?? null,
        ignore: source.ignore === true,
      })),
      mappings: map.mappings.map((mapping: any) => {
        let original = mapping.originalPosition
        return [
          mapping.generatedPosition.line,
          mapping.generatedPosition.column,
          original ? (index.get(original.source) ?? -1) : -1,
          original ? original.line : 0,
          original ? original.column : 0,
          mapping.name ?? null,
        ]
      }),
    })
  })

// The source of an ES module, for the loader of the engine.
global.__tw_module_source = (name: string) => moduleSource(name)
