// This file is the recorder for the compile oracle.
// The script instrument.mjs copies it into packages/tailwindcss/src/__oracle.ts.
// The recorder is active only when the environment variable TW_ORACLE_DIR is set.
// Each worker process appends events to its own file: events-<pid>.jsonl.
// The script merge-suite.mjs joins the events into one line for each compile call.
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const DIR = process.env.TW_ORACLE_DIR
const REPO = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../../..')

let counter = 0
let out: string | null = null
let vitestExpect: any = null
let optimize: any = null

function emit(event: any) {
  if (out === null) {
    fs.mkdirSync(DIR!, { recursive: true })
    out = path.join(DIR!, `events-${process.pid}.jsonl`)
  }
  fs.appendFileSync(out, JSON.stringify(event) + '\n')
}

function message(err: unknown) {
  return err instanceof Error ? err.message : String(err)
}

function state() {
  try {
    let s = vitestExpect.getState()
    return {
      file: s.testPath ? path.relative(REPO, s.testPath).split(path.sep).join('/') : null,
      test: s.currentTestName ?? null,
    }
  } catch {
    return { file: null, test: null }
  }
}

export function wrapCompile<T extends (css: string, opts?: any) => Promise<any>>(original: T): T {
  if (!DIR) return original

  return (async (css: string, opts: any = {}) => {
    vitestExpect ??= (await import('vitest')).expect
    optimize ??= (await import('../../@tailwindcss-node/src/optimize')).optimize

    let id = `${process.pid}-${counter++}`
    let stylesheets: any[] = []
    let modules: any[] = []
    let wrapped: any = { ...opts }

    if (opts.loadStylesheet) {
      wrapped.loadStylesheet = async (sheetId: string, base: string) => {
        let entry: any = { id: sheetId, base }
        stylesheets.push(entry)
        try {
          let result = await opts.loadStylesheet(sheetId, base)
          entry.result = { path: result.path, base: result.base, content: result.content }
          return result
        } catch (err) {
          entry.error = message(err)
          throw err
        }
      }
    }

    if (opts.loadModule) {
      wrapped.loadModule = async (moduleId: string, base: string, hint: string) => {
        modules.push({ id: moduleId, base, hint })
        return opts.loadModule(moduleId, base, hint)
      }
    }

    let head = {
      type: 'compile',
      id,
      seq: Date.now(),
      ...state(),
      input: css,
      options: {
        base: opts.base ?? null,
        from: opts.from ?? null,
        polyfills: opts.polyfills ?? null,
        hasLoadModule: typeof opts.loadModule === 'function',
        hasLoadStylesheet: typeof opts.loadStylesheet === 'function',
      },
    }

    let compiler: any
    try {
      compiler = await original(css, wrapped)
    } catch (err) {
      emit({
        ...head,
        stylesheets,
        modules: modules.length,
        moduleRequests: modules,
        error: message(err),
      })
      throw err
    }

    // Release the loader callbacks. An upstream test checks that the compiler does not keep them.
    opts = null
    wrapped = null

    emit({
      ...head,
      stylesheets,
      modules: modules.length,
      moduleRequests: modules,
      sources: compiler.sources,
      root: compiler.root,
      features: compiler.features,
    })

    return {
      ...compiler,
      build(candidates: string[]) {
        let event: any = { type: 'build', id, candidates: [...candidates] }
        let result: string
        try {
          result = compiler.build(candidates)
        } catch (err) {
          event.error = message(err)
          emit(event)
          throw err
        }
        event.css = result
        try {
          event.optimized = optimize(result).code
          event.minified = optimize(result, { minify: true }).code
        } catch (err) {
          event.optimizeError = message(err)
        }
        emit(event)
        return result
      },
    }
  }) as T
}
