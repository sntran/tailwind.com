#!/usr/bin/env node
// Usage: node gen-node-api.mjs [output file]
//
// This script records the behavior of Node.js for the parts of Node.js that
// the program gives to the JavaScript of a plugin or config. It runs
// native/tailwind/tests/fixtures/node_api_cases.cjs in this Node.js (22)
// and writes the values to native/tailwind/tests/fixtures/node_api.json.
// The Rust test tests/node_api.rs runs the same file in the engine of the
// program and compares each value.
//
// It also records the file that Node.js finds for a module name (`resolution`
// in the record): `require.resolve` and `import.meta.resolve` on a tree of
// packages with `exports`, conditions in more than one sequence, patterns,
// and `imports` (`#name`). The record has the tree, so the Rust test needs
// no other file.
//
// The path of the directory of the test files is `<dir>` in the record.
import fs from 'node:fs'
import { createRequire } from 'node:module'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'

let here = path.dirname(fileURLToPath(import.meta.url))
let fixtures = path.resolve(here, '../../../native/tailwind/tests/fixtures')
let output = path.resolve(process.argv[2] ?? path.join(fixtures, 'node_api.json'))

// The files of the cases. tests/node_api.rs has the same files in memory.
let temporary = os.tmpdir()
let dir = fs.realpathSync(fs.mkdtempSync(path.join(temporary, 'node-api-')))
fs.mkdirSync(path.join(dir, 'sub/deep'), { recursive: true })
fs.writeFileSync(path.join(dir, 'text.txt'), 'Text with é, € and 𝄞.\n')
fs.writeFileSync(path.join(dir, 'bytes.bin'), Buffer.from([0x00, 0xff, 0x80, 0x41, 0xc3, 0x28, 0x0a]))
fs.writeFileSync(path.join(dir, 'sub/a.txt'), 'a')
fs.writeFileSync(path.join(dir, 'sub/b.json'), '{"list": [1, 2, 3], "name": "b"}\n')
fs.writeFileSync(path.join(dir, 'sub/deep/c.txt'), 'c')

// The environment that tests/node_api.rs gives.
process.env.NODE_API_TEST = 'value'
process.env.HOME = '/home/test'
process.env.TMPDIR = '/tmp/test/'
process.noDeprecation = true

let run = createRequire(import.meta.url)(path.join(fixtures, 'node_api_cases.cjs'))
let results = await run(dir)
fs.rmSync(dir, { recursive: true })

let failed = Object.entries(results).filter(([, value]) => value !== null && typeof value === 'object' && 'failed' in value)
for (let [name, value] of failed) console.error(`gen-node-api.mjs: ${name}: ${value.failed}`)
if (failed.length > 0) process.exit(1)

// The resolver: a tree of packages, and the questions.
let json = (value) => JSON.stringify(value)
let tree = {
  'package.json': json({
    name: 'proj',
    imports: {
      '#config': './config/main.js',
      '#lib/*': './lib/*.js',
      '#dep': 'dep-main',
      '#dep/*': 'dep-patterns/features/*',
      '#cond': { import: './cond/esm.mjs', require: './cond/cjs.cjs' },
      '#default-first': { default: './cond/default.js', import: './cond/esm.mjs', require: './cond/cjs.cjs' },
      '#list': ['not-relative-and-not-there', './config/main.js'],
    },
  }),
  'app.js': '',
  'sub/mod.js': '',
  'sub/package.json': json({ name: 'sub-scope', imports: { '#config': './own.js' } }),
  'sub/own.js': '',
  'config/main.js': '',
  'lib/a.js': '',
  'lib/deep/b.js': '',
  'cond/esm.mjs': '',
  'cond/cjs.cjs': '',
  'cond/default.js': '',
  'node_modules/dep-main/package.json': json({ main: './lib/main.js' }),
  'node_modules/dep-main/lib/main.js': '',
  'node_modules/dep-index/index.js': '',
  'node_modules/require-first/package.json': json({ exports: { require: './r.cjs', import: './i.mjs', default: './d.js' } }),
  'node_modules/import-first/package.json': json({ exports: { import: './i.mjs', require: './r.cjs', default: './d.js' } }),
  'node_modules/default-first/package.json': json({ exports: { default: './d.js', import: './i.mjs', require: './r.cjs' } }),
  'node_modules/node-first/package.json': json({ exports: { node: './n.js', import: './i.mjs', require: './r.cjs' } }),
  'node_modules/node-last/package.json': json({ exports: { import: './i.mjs', require: './r.cjs', node: './n.js' } }),
  'node_modules/nested/package.json': json({ exports: { '.': { node: { import: './i.mjs', require: './r.cjs' }, default: './d.js' } } }),
  'node_modules/nested-miss/package.json': json({ exports: { '.': { browser: { default: './b.js' }, import: { browser: './b.js' }, default: './d.js' } } }),
  'node_modules/module-sync/package.json': json({ exports: { 'module-sync': './m.mjs', import: './i.mjs', require: './r.cjs' } }),
  'node_modules/browser/package.json': json({ exports: { browser: './b.js', worker: './b.js', default: './d.js' } }),
  'node_modules/only-import/package.json': json({ exports: { import: './i.mjs' } }),
  'node_modules/only-require/package.json': json({ exports: { require: './r.cjs' } }),
  'node_modules/dep-patterns/package.json': json({
    exports: {
      '.': './d.js',
      './features/*': './src/features/*.js',
      './features/private/*': null,
      './exact': { import: './i.mjs', require: './r.cjs' },
      './ext/*.js': './src/ext/*.js',
      './package.json': './package.json',
    },
  }),
  'node_modules/dep-patterns/src/features/a.js': '',
  'node_modules/dep-patterns/src/features/private/p.js': '',
  'node_modules/dep-patterns/src/features/deep/c.js': '',
  'node_modules/dep-patterns/src/ext/e.js': '',
  'node_modules/dep-patterns/hidden.js': '',
  'node_modules/list/package.json': json({ exports: ['./d.js', './i.mjs'] }),
  'node_modules/@scope/pkg/package.json': json({ exports: { '.': './d.js', './sub': { require: './r.cjs', default: './d.js' } } }),
}
for (let name of ['require-first', 'import-first', 'default-first', 'node-first', 'node-last', 'nested', 'nested-miss', 'module-sync', 'browser', 'only-import', 'only-require', 'dep-patterns', 'list', '@scope/pkg']) {
  for (let file of ['d.js', 'i.mjs', 'r.cjs', 'n.js', 'b.js', 'm.mjs']) tree[`node_modules/${name}/${file}`] = ''
}
let names = [
  './sub/mod.js', './config/main.js',
  'dep-main', 'dep-index', 'require-first', 'import-first', 'default-first', 'node-first', 'node-last', 'nested', 'nested-miss',
  'module-sync', 'browser', 'only-import', 'only-require', 'list', '@scope/pkg', '@scope/pkg/sub', '@scope/pkg/other',
  'dep-patterns', 'dep-patterns/features/a', 'dep-patterns/features/deep/c', 'dep-patterns/features/private/p', 'dep-patterns/exact',
  'dep-patterns/ext/e.js', 'dep-patterns/hidden.js', 'dep-patterns/package.json', 'not-installed',
  '#config', '#lib/a', '#lib/deep/b', '#lib/missing', '#dep', '#dep/a', '#cond', '#default-first', '#list', '#missing', '#', '#/config',
]
let root = fs.realpathSync(fs.mkdtempSync(path.join(temporary, 'node-resolve-')))
for (let [file, content] of Object.entries(tree)) {
  fs.mkdirSync(path.dirname(path.join(root, file)), { recursive: true })
  fs.writeFileSync(path.join(root, file), content)
}
let questions = []
for (let from of ['app.js', 'sub/mod.js']) {
  // `import.meta.resolve` needs a module in the directory of the question.
  let helper = path.join(root, path.dirname(from), '__resolve.mjs')
  fs.writeFileSync(helper, 'export default (name) => import.meta.resolve(name)\n')
  let importResolve = (await import(pathToFileURL(helper).href)).default
  let requireResolve = createRequire(path.join(root, from)).resolve
  for (let name of names) {
    let viaRequire = null
    let viaImport = null
    try {
      viaRequire = requireResolve(name)
    } catch {}
    try {
      viaImport = fileURLToPath(importResolve(name))
      // `import.meta.resolve` does not look for the file.
      if (!fs.existsSync(viaImport)) viaImport = null
    } catch {}
    questions.push({ name, from: `<dir>/${from}`, require: viaRequire, import: viaImport })
  }
  fs.rmSync(helper)
}
fs.rmSync(root, { recursive: true })
let resolution = JSON.parse(JSON.stringify({ files: tree, questions }).split(root).join('<dir>'))

let text = JSON.stringify({ node: process.version, results, resolution }, null, 1).split(dir).join('<dir>')
fs.writeFileSync(output, text + '\n')
console.error(`gen-node-api.mjs: wrote ${output} (${Object.keys(results).length} cases, ${questions.length} questions for the resolver, Node.js ${process.version})`)
