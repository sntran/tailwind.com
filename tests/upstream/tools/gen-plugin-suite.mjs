#!/usr/bin/env node
// Usage: node gen-plugin-suite.mjs <upstream clone> <scratch directory> [output file]
//
// This script makes tests/upstream/plugin-suite.jsonl.gz: the compile calls
// of the upstream unit tests that load a JavaScript plugin or config, with
// each module as JavaScript source text.
//
// 1. It adds the recorder to the clone (instrument.mjs), and makes
//    plugin.ts keep the arguments of `plugin.withOptions`.
// 2. It runs the unit tests of the package tailwindcss with the recorder. The
//    recorder writes each module that a test gives to `loadModule` as
//    source text (recorder.ts).
// 3. It runs each record again with the upstream compiler and the modules
//    from their source text. A record stays only when all its results are
//    the same: a module that uses variables of its test cannot be made
//    again from its text.
//
// The clone changes: use a copy when the clone must stay clean.
import { execFileSync, spawnSync } from 'node:child_process'
import fs from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { loadUpstream, writeGzip } from './lib.mjs'

let [clone, scratch, output] = process.argv.slice(2)
if (!clone || !scratch) {
  console.error('Usage: node gen-plugin-suite.mjs <upstream clone> <scratch directory> [output file]')
  process.exit(2)
}
clone = path.resolve(clone)
scratch = path.resolve(scratch)
let here = path.dirname(fileURLToPath(import.meta.url))
output = path.resolve(output ?? path.join(here, '../plugin-suite.jsonl.gz'))

// 1. The recorder.
execFileSync(process.execPath, [path.join(here, 'instrument.mjs'), clone], { stdio: 'inherit' })
fs.copyFileSync(path.join(here, 'recorder.ts'), path.join(clone, 'packages/tailwindcss/src/__oracle.ts'))

let pluginFile = path.join(clone, 'packages/tailwindcss/src/plugin.ts')
let pluginCode = fs.readFileSync(pluginFile, 'utf8')
if (!pluginCode.includes('__oracleSource')) {
  let marker = '  optionsFunction.__isOptionsFunction = true as const\n'
  if (!pluginCode.includes(marker)) {
    console.error('Cannot find `__isOptionsFunction` in plugin.ts.')
    process.exit(1)
  }
  pluginCode = pluginCode.replace(
    marker,
    marker +
      "  Object.defineProperty(optionsFunction, '__oracleSource', { value: [pluginFunction, configFunction] })\n",
  )
  fs.writeFileSync(pluginFile, pluginCode)
}

// 2. The tests.
let events = path.join(scratch, 'plugin-events')
fs.rmSync(events, { recursive: true, force: true })
let packageDir = path.join(clone, 'packages/tailwindcss')
let { createRequire } = await import('node:module')
let vitest = path.join(path.dirname(createRequire(path.join(packageDir, 'package.json')).resolve('vitest/package.json')), 'vitest.mjs')
let run = spawnSync(process.execPath, [vitest, 'run'], {
  cwd: packageDir,
  env: { ...process.env, TW_ORACLE_DIR: events, TW_ORACLE_MODULES: '1' },
  stdio: ['ignore', 'inherit', 'inherit'],
})
if (run.status !== 0) {
  console.error(`gen-plugin-suite.mjs: vitest ended with the status ${run.status}`)
  process.exit(1)
}

let merged = path.join(scratch, 'plugin-events.jsonl.gz')
execFileSync(process.execPath, [path.join(here, 'merge-suite.mjs'), events, merged], { stdio: 'inherit' })
let zlib = await import('node:zlib')
let records = zlib
  .gunzipSync(fs.readFileSync(merged))
  .toString('utf8')
  .split('\n')
  .filter(Boolean)
  .map((line) => JSON.parse(line))
  .filter((record) => record.modules > 0)

// 3. The check. The recorder is off: TW_ORACLE_DIR is not set here.
delete process.env.TW_ORACLE_DIR
let upstream = await loadUpstream(clone)
let jitiRequire = createRequire(path.join(clone, 'packages/@tailwindcss-node/package.json'))
let jiti = jitiRequire('jiti').createJiti(path.join(packageDir, 'src/'), { moduleCache: true })
let builtin = {
  'tailwindcss/plugin': (await jiti.import(path.join(packageDir, 'src/plugin.ts'))).default,
  'tailwindcss/colors': (await jiti.import(path.join(packageDir, 'src/compat/colors.ts'))).default,
  'tailwindcss/defaultTheme': (await jiti.import(path.join(packageDir, 'src/compat/default-theme.ts'))).default,
  'tailwindcss/lib/util/flattenColorPalette': (
    await jiti.import(path.join(packageDir, 'src/compat/flatten-color-palette.ts'))
  ).default,
}

function evaluate(source) {
  let module = { exports: {} }
  let require = (name) => {
    if (!(name in builtin)) throw new Error(`Cannot find module '${name}'`)
    return builtin[name]
  }
  new Function('require', 'module', 'exports', source)(require, module, module.exports)
  return module.exports
}

function message(error) {
  return error instanceof Error ? error.message : String(error)
}

// Run a record again. The result is null when all is the same, or the
// first difference.
async function check(record) {
  let warn = console.warn
  console.warn = () => {}
  try {
    let requests = record.moduleRequests.map((request) => ({ ...request }))
    let compiler
    try {
      compiler = await upstream.compile(record.input, {
        base: record.options.base ?? undefined,
        polyfills: record.options.polyfills ?? undefined,
        async loadStylesheet(id, base) {
          let call = record.stylesheets.find((sheet) => sheet.id === id && sheet.base === base)
          if (!call?.result) throw new Error(call?.error ?? `no stylesheet ${id}`)
          return call.result
        },
        async loadModule(id, base, hint) {
          let index = requests.findIndex((request) => request.id === id && request.base === base && !request.used)
          if (index === -1) throw new Error(`no module ${id}`)
          let request = requests[index]
          request.used = true
          if (request.error !== undefined) throw new Error(request.error)
          return { path: '', base: request.resultBase, module: evaluate(request.source) }
        },
      })
    } catch (error) {
      return record.error === message(error) ? null : `compile: ${message(error)}`
    }
    if (record.error !== undefined) return 'compile gave no error'
    if (compiler.features !== record.features) return 'the features are different'
    if (JSON.stringify(compiler.sources) !== JSON.stringify(record.sources)) return 'the sources are different'
    for (let build of record.builds) {
      let css
      try {
        css = compiler.build(build.candidates)
      } catch (error) {
        if (build.error === message(error)) continue
        return `build: ${message(error)}`
      }
      if (build.error !== undefined) return 'build gave no error'
      if (css !== build.css) return 'the CSS of a build is different'
    }
    return null
  } finally {
    console.warn = warn
  }
}

let kept = []
let skipped = new Map()
function skip(record, reason) {
  let key = reason.replace(/[`'"].*$/, '').slice(0, 60)
  skipped.set(key, (skipped.get(key) ?? 0) + 1)
  if (process.env.VERBOSE) console.error(`skip: ${record.file} > ${record.test}: ${reason}`)
}

for (let record of records) {
  let broken = record.moduleRequests.find((request) => request.source === undefined && request.error === undefined)
  if (broken) {
    skip(record, `not serializable: ${broken.unserializable}`)
    continue
  }
  let difference = await check(record)
  if (difference !== null) {
    skip(record, `not the same from source text: ${difference}`)
    continue
  }
  let { moduleRequests, modules, ...rest } = record
  kept.push({
    ...rest,
    modules: moduleRequests.map(({ id, base, hint, source, resultBase, error }) => ({ id, base, hint, source, resultBase, error })),
    builds: record.builds.map(({ candidates, css, error }) => ({ candidates, css, error })),
  })
}

writeGzip(output, kept.map((record) => JSON.stringify(record) + '\n').join(''))
console.log(`${records.length} records with modules, ${kept.length} kept -> ${output}`)
for (let [reason, count] of [...skipped].sort((a, z) => z[1] - a[1])) {
  console.log(`  skipped ${count}: ${reason}`)
}
