#!/usr/bin/env node
// The live differential fuzzer: random stylesheets and candidates go to
// the official Tailwind CSS (the npm packages of the version of UPSTREAM)
// and to this compiler, and each difference is a failure.
//
// Usage:
//   node tests/fuzz/fuzz.mjs --official DIR --builder FILE --program FILE
//        [--seed N] [--cases N] [--projects N] [--seconds N] [--out DIR]
//
//   --official DIR   a directory with node_modules/tailwindcss and
//                    node_modules/@tailwindcss/cli (tests/fuzz/install.sh)
//   --builder FILE   target/release/examples/fuzz_build
//   --program FILE   the program of this repository (target/release/tailwind)
//   --seed N         the seed of the random choices (default: the time)
//   --cases N        the number of compiler cases (default 2000)
//   --projects N     the number of command line projects (default 60)
//   --seconds N      stop the new cases after this time (default 600)
//   --out DIR        the directory of the failures (default build/fuzz)
//   --javascript P   the part of the cases with JavaScript: real plugins,
//                    random plugins and random configs (default 0.35)
//
// Two kinds of cases:
//   compiler  a stylesheet and a list of candidates: compile(css) and
//             build(candidates) of the npm package tailwindcss, against
//             examples/fuzz_build.rs, which does the same with this crate.
//   project   a directory with app.css and source files with classes:
//             the official command line program against this program,
//             with the same arguments (also --minify and --optimize). The
//             two use the scanner of Tailwind CSS.
//
// A failure goes to OUT/failures.jsonl, with a reduced case: the smallest
// part of the stylesheet and of the candidates that still differs. Add a
// reduced compiler case to tests/upstream/fuzz-found.jsonl, and the test
// tests/fuzz_found.rs keeps it fixed (see docs/TESTING.md).
import { spawnSync } from 'node:child_process'
import fs from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath, pathToFileURL } from 'node:url'
import { ARBITRARY_FORMS, EDGE_CANDIDATES, RANDOM_MODIFIERS, RANDOM_VARIANTS } from '../upstream/tools/candidate-lists.mjs'
import { officialTailwind, writeFiles } from './official.mjs'
import { REAL_CLASSES, createGenerators } from './plugins.mjs'
import { createRandom } from './random.mjs'

const here = path.dirname(fileURLToPath(import.meta.url))

function options() {
  const args = process.argv.slice(2)
  const o = { javascript: 0.35, seed: Date.now() % 2 ** 31, cases: 2000, projects: 60, seconds: 600, out: path.resolve(here, '../../build/fuzz') }
  for (let i = 0; i < args.length; i++) {
    const value = () => {
      if (i + 1 >= args.length) throw new Error(`${args[i]} needs a value`)
      return args[++i]
    }
    switch (args[i]) {
      case '--official': o.official = path.resolve(value()); break
      case '--builder': o.builder = path.resolve(value()); break
      case '--program': o.program = path.resolve(value()); break
      case '--seed': o.seed = Number(value()) % 2 ** 31; break
      case '--cases': o.cases = Number(value()); break
      case '--projects': o.projects = Number(value()); break
      case '--seconds': o.seconds = Number(value()); break
      case '--out': o.out = path.resolve(value()); break
      case '--javascript': o.javascript = Number(value()); break
      default: throw new Error(`unknown argument: ${args[i]}`)
    }
  }
  for (const name of ['official', 'builder', 'program']) {
    if (!o[name]) throw new Error(`--${name} is necessary`)
  }
  return o
}

const opts = options()
const random = createRandom(opts.seed)
const deadline = Date.now() + opts.seconds * 1000
// The compiler cases get 80 percent of the time, the projects the rest.
const compilerDeadline = Date.now() + opts.seconds * 800
const require = createRequire(path.join(opts.official, 'package.json'))
const off = await officialTailwind(opts.official)
const officialDir = off.officialDir
const version = off.version
const cli = require.resolve('@tailwindcss/cli/package.json').replace(/package\.json$/, 'dist/index.mjs')
const repository = path.resolve(here, '../..')
const interop = path.join(repository, 'tests/upstream/tools/bun-interop.mjs')
const generators = createGenerators(random)

console.log(`fuzz: Tailwind CSS ${version}, seed ${opts.seed}, ${opts.cases} compiler cases, ${opts.projects} projects`)

// The official compiler, in this process (official.mjs).
function officialResult(c) {
  return off.run(c)
}

// --- This compiler, through the builder ----------------------------------

function ourResults(cases) {
  const input = cases.map((c) => JSON.stringify({ css: c.css, candidates: c.candidates, base: c.base ?? '' })).join('\n') + '\n'
  const run = spawnSync(opts.builder, [path.join(officialDir)], { input, maxBuffer: 1024 * 1024 * 1024 })
  if (run.status !== 0) {
    throw new Error(`the builder failed (status ${run.status}): ${run.stderr.toString().slice(0, 2000)}`)
  }
  const lines = run.stdout.toString().split('\n').filter((l) => l !== '')
  if (lines.length !== cases.length) throw new Error(`the builder gave ${lines.length} results for ${cases.length} cases`)
  return lines.map((l) => JSON.parse(l))
}

function same(a, b) {
  return a.css === b.css && a.error === b.error && a.panic === undefined && b.panic === undefined
}

// A known fault of Tailwind CSS before 4.1.14: an `@variant` rule in the
// body of `@custom-variant` stays in the output, which is not valid CSS.
// This compiler replaces the rule in each version (docs/SWITCHES.md). A
// result of the official compiler with this fault is not a difference.
const [major, minor, patch] = version.split('.').map(Number)
const KEEPS_AT_VARIANT = major === 4 && (minor < 1 || (minor === 1 && patch < 14))
function hasKnownFault(official) {
  return KEEPS_AT_VARIANT && official.css !== undefined && /^\s*@variant /m.test(official.css)
}

// The case with no candidate that shows the known fault alone: the other
// candidates are compared again. `null`: the stylesheet shows the fault
// with no candidate (`@apply` of a custom variant).
async function withoutKnownFault(c) {
  if (hasKnownFault(await officialResult({ ...c, candidates: [] }))) return null
  const candidates = []
  for (const one of c.candidates) {
    if (!hasKnownFault(await officialResult({ ...c, candidates: [one] }))) candidates.push(one)
  }
  const rest = { ...c, candidates }
  return hasKnownFault(await officialResult(rest)) ? null : rest
}

// --- The generators ------------------------------------------------------

const design = await off.loadDesignSystem('@import "tailwindcss";')
const classList = design.getClassList().map(([name]) => name)

// The classes of the real plugins: the class list with the plugins, with
// no class of the base list.
const pluginDir = path.join(opts.official, 'work', 'plugins')
fs.mkdirSync(pluginDir, { recursive: true })
fs.copyFileSync(path.join(opts.official, 'vendor', 'daisyui.js'), path.join(pluginDir, 'daisyui.js'))
const withPlugins = await off.loadDesignSystem(
  '@import "tailwindcss";\n@plugin "@tailwindcss/typography";\n@plugin "@tailwindcss/forms";\n@plugin "@tailwindcss/aspect-ratio";\n@plugin "tailwindcss-animate";\n@plugin "./daisyui.js";',
  pluginDir,
)
const base = new Set(classList)
const pluginClasses = [...new Set([...withPlugins.getClassList().map(([name]) => name).filter((n) => !base.has(n)), ...REAL_CLASSES])]
const variantNames = []
for (const v of design.getVariants()) {
  variantNames.push(v.name)
  for (const value of v.values.slice(0, 4)) variantNames.push(v.hasDash ? `${v.name}-${value}` : `${v.name}${value}`)
}

function words(text) {
  return Array.isArray(text) ? text : text.trim().split(/\s+/)
}
const arbitrary = words(ARBITRARY_FORMS)
const variantsExtra = words(RANDOM_VARIANTS)
const modifiers = RANDOM_MODIFIERS

// Small random changes of a valid candidate: the edge of the grammar.
function mutate(c) {
  const edits = [
    () => c + random.pick(['!', '/', '-', ':', ']', ')', '[', '_']),
    () => random.pick(['!', '-', '[', '@', '*']) + c,
    () => c.replace(/-/, random.pick(['--', '_', '-[', '-('])),
    () => c.slice(0, random.int(c.length + 1)),
    () => c + '/' + random.pick(modifiers),
    () => c.replace(/\d+/, random.pick(['0', '1.25', '13', '99', '0.5', '1/2', '-1', '1e3'])),
    () => c.toUpperCase(),
    () => c + c.slice(random.int(c.length)),
  ]
  return random.pick(edits)()
}

let userUtilities = []
let userVariants = []
let prefix = null
// The files of the JavaScript parts of the current case, and whether the
// case has a plugin.
let caseFiles = {}
let caseCopy = {}
let usesPlugins = false

function candidate() {
  const parts = []
  for (let n = random.int(4); n > 0; n--) {
    const r = random.next()
    parts.push(r < 0.5 ? random.pick(variantNames) : r < 0.85 ? random.pick(variantsExtra) : random.pick(userVariants.length ? userVariants : variantNames))
  }
  let base
  const r = random.next()
  if (r < 0.45) base = random.pick(classList)
  else if (r < 0.75) base = random.pick(arbitrary)
  else if (r < 0.85 && userUtilities.length) base = random.pick(userUtilities)
  else if (r < 0.88 && usesPlugins) base = random.pick(pluginClasses)
  else if (r < 0.92) base = random.pick(EDGE_CANDIDATES)
  else base = mutate(random.pick(classList))
  if (random.chance(0.1)) base = '-' + base
  if (random.chance(0.15)) base += '/' + random.pick(modifiers)
  if (random.chance(0.08)) base += '!'
  else if (random.chance(0.03)) base = '!' + base
  parts.push(base)
  if (prefix && random.chance(0.8)) parts.unshift(prefix)
  return parts.join(':')
}

const COLORS = ['red', '#0088cc', 'oklch(0.7 0.1 200)', 'rgb(0 128 255 / 0.5)', 'var(--x)', 'currentcolor', 'color-mix(in oklab, red 50%, blue)', 'transparent']
const LENGTHS = ['1px', '0.25rem', '2em', 'calc(1rem + 2px)', '50%', '0', '13px', 'var(--y)']

function themeBlock() {
  const kind = random.pick(['', ' inline', ' static', ' reference', ' default', ' inline reference'])
  const lines = []
  for (let n = 1 + random.int(5); n > 0; n--) {
    const r = random.next()
    if (r < 0.3) lines.push(`--color-${random.pick(['brand', 'x', 'red-500', 'accent-1', 'a\\.b'])}: ${random.pick(COLORS)};`)
    else if (r < 0.45) lines.push(`--spacing: ${random.pick(['0.25rem', '4px', '1px', 'var(--s)'])};`)
    else if (r < 0.55) lines.push(`--breakpoint-${random.pick(['xs', 'md', '3xl', 'tablet'])}: ${random.pick(['30rem', '40rem', '100px', '200em'])};`)
    else if (r < 0.65) lines.push(`--${random.pick(['color', 'spacing', 'font', 'breakpoint', 'text', 'shadow'])}-*: initial;`)
    else if (r < 0.75) lines.push(`--font-${random.pick(['display', 'sans', 'x'])}: ${random.pick(['"Inter", sans-serif', 'serif', 'var(--f)'])};`)
    else if (r < 0.85) lines.push(`--text-${random.pick(['tiny', 'sm', 'huge'])}: ${random.pick(LENGTHS)};\n    --text-${random.pick(['tiny', 'sm', 'huge'])}--line-height: ${random.pick(['1.5', '2rem', 'calc(1 / 2)'])};`)
    else if (r < 0.92) lines.push(`--animate-${random.pick(['wiggle', 'spin'])}: wiggle 1s ease-in-out infinite;\n    @keyframes wiggle { 0%, 100% { transform: rotate(-3deg); } 50% { transform: rotate(3deg); } }`)
    else lines.push(`--${random.pick(['radius', 'shadow', 'blur', 'ease'])}-${random.pick(['x', 'lg', 'soft'])}: ${random.pick(['4px', '0 1px 2px red', 'cubic-bezier(0, 0, 1, 1)', '8px'])};`)
  }
  return `@theme${kind} {\n    ${lines.join('\n    ')}\n}`
}

function utilityBlock() {
  const name = random.pick(['tab', 'scrollbar', 'x-thing', 'card', 'w', 'text-trim', 'foo'])
  if (random.chance(0.4)) {
    userUtilities.push(name)
    return `@utility ${name} {\n  ${random.pick(['color: red;', 'tab-size: 4;', '&:hover { color: blue; }', '@apply flex p-2;', 'scrollbar-width: none; &::-webkit-scrollbar { display: none; }'])}\n}`
  }
  userUtilities.push(...['1', '4', 'red', 'brand', '[3px]', '(--v)', 'sm', '2.5', '1/2'].map((v) => `${name}-${v}`))
  const value = random.pick([
    '--value(integer)', '--value(--color-*)', '--value([length])', '--value(number, [length])', "--value('auto', 'none')",
    '--value(--spacing-*, [length])', 'calc(--value(number) * 1px)', '--value(ratio)', '--value(percentage)', '--value(any)',
  ])
  const extra = random.chance(0.4) ? `\n  opacity: --modifier(integer, [percentage]);` : ''
  return `@utility ${name}-* {\n  ${random.pick(['color', 'tab-size', 'width', 'margin', '--x'])}: ${value};${extra}\n}`
}

function variantBlock() {
  const name = random.pick(['theme-x', 'hocus', 'pointer-fine', 'supports-x', 'phx-click-loading'])
  userVariants.push(name)
  return random.pick([
    `@custom-variant ${name} (&:hover, &:focus);`,
    `@custom-variant ${name} (&:where([data-theme=dark], [data-theme=dark] *));`,
    `@custom-variant ${name} (@media (pointer: fine));`,
    `@custom-variant ${name} {\n  &:hover {\n    @slot;\n  }\n  @media (any-hover: none) {\n    &:active { @slot; }\n  }\n}`,
    `@custom-variant ${name} (.phx-click-loading&, .phx-click-loading &);`,
    `@custom-variant ${name} {\n  @variant hover { @slot; }\n}`,
  ])
}

function ruleBlock() {
  const sel = random.pick(['.a', '.btn', 'h1', '.card > p', ':root', '.x:hover'])
  const body = []
  for (let n = 1 + random.int(3); n > 0; n--) {
    body.push(
      random.pick([
        `@apply ${candidate()} ${random.chance(0.5) ? random.pick(classList) : ''};`,
        `color: theme(colors.red.500);`,
        `color: --theme(--color-red-500 / 50%);`,
        `margin: --spacing(${random.pick(['4', '2.5', '-1', '0'])});`,
        `background: --alpha(var(--color-blue-500) / 25%);`,
        `padding: theme(spacing.4, 1rem);`,
        `& .b { color: red; }`,
        `@variant ${random.pick(['hover', 'md', 'dark', 'focus'])} { color: blue; }`,
        `@media (width >= theme(--breakpoint-md)) { color: green; }`,
      ]),
    )
  }
  const layer = random.pick(['', '', 'components', 'base', 'utilities'])
  const rule = `${sel} {\n  ${body.join('\n  ')}\n}`
  return layer ? `@layer ${layer} {\n${rule}\n}` : rule
}

// A stylesheet as a list of parts, so the reducer can remove them one at
// a time.
function stylesheet() {
  userUtilities = []
  userVariants = []
  prefix = null
  caseFiles = {}
  caseCopy = {}
  usesPlugins = false
  const parts = []
  const r = random.next()
  if (r < 0.6) parts.push('@import "tailwindcss";')
  else if (r < 0.75) parts.push('@import "tailwindcss" important;')
  else if (r < 0.85) {
    prefix = 'tw'
    parts.push('@import "tailwindcss" prefix(tw);')
  } else {
    parts.push('@layer theme, base, components, utilities;')
    parts.push('@import "tailwindcss/theme.css" layer(theme);')
    if (random.chance(0.5)) parts.push('@import "tailwindcss/preflight.css" layer(base);')
    parts.push('@import "tailwindcss/utilities.css" layer(utilities);')
  }
  for (let n = random.int(6); n > 0; n--) {
    const g = random.next()
    parts.push(g < 0.3 ? themeBlock() : g < 0.5 ? utilityBlock() : g < 0.7 ? variantBlock() : g < 0.95 ? ruleBlock() : `@source inline("${random.pick(['p-{1,2,4}', 'flex', '{hover:,}underline', 'bg-red-{50,{100..900..200}}'])}");`)
  }
  // JavaScript: real plugins, random plugins, and random configs.
  if (random.chance(opts.javascript)) {
    usesPlugins = true
    for (let n = 1 + random.int(3); n > 0; n--) {
      const g = random.next()
      const part = g < 0.4 ? generators.real() : g < 0.8 ? generators.plugin() : generators.config()
      parts.push(part.line)
      Object.assign(caseFiles, part.files)
      Object.assign(caseCopy, part.copy)
      userUtilities.push(...part.utilities)
      userVariants.push(...part.variants)
    }
  }
  return parts
}

function compilerCase() {
  const parts = stylesheet()
  const candidates = []
  for (let n = random.int(40); n > 0; n--) candidates.push(candidate())
  return { parts, css: parts.join('\n'), candidates, files: caseFiles, copy: caseCopy }
}

// --- The reducer ---------------------------------------------------------

async function differs(c) {
  const [ours] = ourResults([c])
  const theirs = await officialResult(c)
  return !same(ours, theirs) && !hasKnownFault(theirs)
}

// The smallest part of the case that still differs: first the
// candidates, then the parts of the stylesheet, one at a time.
async function reduce(c) {
  let best = { parts: [...c.parts], candidates: [...c.candidates], files: c.files, copy: c.copy, base: c.base }
  const make = (b) => ({ ...b, css: b.parts.join('\n') })
  if (best.candidates.length > 0 && (await differs(make({ ...best, candidates: [] })))) {
    best.candidates = []
  } else {
    // A candidate that differs alone is the best result.
    for (const one of best.candidates) {
      if (await differs(make({ ...best, candidates: [one] }))) {
        best.candidates = [one]
        break
      }
    }
    for (let i = best.candidates.length - 1; i >= 0 && best.candidates.length > 1; i--) {
      const fewer = best.candidates.filter((_, j) => j !== i)
      if (await differs(make({ ...best, candidates: fewer }))) best.candidates = fewer
    }
  }
  for (let i = best.parts.length - 1; i >= 0; i--) {
    const fewer = best.parts.filter((_, j) => j !== i)
    if (fewer.length > 0 && (await differs(make({ ...best, parts: fewer })))) best.parts = fewer
  }
  return make(best)
}

// The files that the stylesheet of a reduced case names.
function usedFiles(c) {
  return Object.fromEntries(Object.entries(c.files ?? {}).filter(([name]) => c.css.includes(`./${name}`)))
}
function usedCopy(c) {
  return Object.fromEntries(Object.entries(c.copy ?? {}).filter(([name]) => c.css.includes(`./${name}`)))
}

// --- The project cases ---------------------------------------------------

function projectCase() {
  const parts = stylesheet()
  // Next to node_modules of the official packages: the official program
  // resolves @import "tailwindcss" from there.
  fs.mkdirSync(path.join(opts.official, 'work'), { recursive: true })
  const dir = fs.mkdtempSync(path.join(opts.official, 'work', 'p-'))
  fs.mkdirSync(path.join(dir, 'src', 'deep'), { recursive: true })
  const classes = () => Array.from({ length: 5 + random.int(30) }, candidate).join(' ')
  fs.writeFileSync(path.join(dir, 'app.css'), parts.join('\n') + '\n')
  writeFiles(dir, { files: caseFiles, copy: caseCopy }, repository)
  const jsFiles = { ...caseFiles }
  fs.writeFileSync(path.join(dir, 'src', 'index.html'), `<div class="${classes()}"></div>\n<p class='${classes()}'>x</p>\n`)
  fs.writeFileSync(path.join(dir, 'src', 'deep', 'page.html.heex'), `<.button class={["${classes()}", @active && "${classes()}"]} />\n`)
  fs.writeFileSync(path.join(dir, 'src', 'app.js'), `const a = "${classes()}"; export default \`${classes()}\`\n`)
  const args = ['-i', 'app.css', ...random.pick([[], [], ['--minify'], ['--optimize']])]
  return { dir, args, jsFiles, copy: { ...caseCopy } }
}

function runProgram(command, args, cwd) {
  const r = spawnSync(command[0], [...command.slice(1), ...args, ...(command.extra ?? [])], {
    cwd,
    env: { ...process.env, NO_COLOR: '1', NODE_ENV: '' },
    maxBuffer: 512 * 1024 * 1024,
  })
  return { status: r.status, stdout: r.stdout.toString(), stderr: r.stderr.toString().slice(0, 4000) }
}

function ourCommand() {
  return opts.program.endsWith('.com') && process.platform !== 'win32' ? ['sh', opts.program] : [opts.program]
}

// --- The run -------------------------------------------------------------

fs.mkdirSync(opts.out, { recursive: true })
const failuresFile = path.join(opts.out, 'failures.jsonl')
fs.writeFileSync(failuresFile, '')
let failures = 0
let compared = 0
// What the cases did, to show that they test more than errors.
const stats = { compilerErrors: 0, compilerCss: 0, cssBytes: 0, projectErrors: 0, projectCss: 0, javascript: 0, knownFaults: 0, knownFaultsOnly: 0 }

function record(entry) {
  failures++
  fs.appendFileSync(failuresFile, JSON.stringify({ seed: opts.seed, version, ...entry }) + '\n')
}

// The compiler cases, in batches for the builder.
const BATCH = 200
for (let done = 0; done < opts.cases && Date.now() < compilerDeadline; done += BATCH) {
  const cases = Array.from({ length: Math.min(BATCH, opts.cases - done) }, compilerCase)
  // The files of the JavaScript parts of the batch: the names are new for
  // each part, so the cases of a batch share one directory.
  const batchDir = path.join(opts.official, 'work', `batch-${opts.seed}-${done}`)
  fs.rmSync(batchDir, { recursive: true, force: true })
  fs.mkdirSync(batchDir, { recursive: true })
  for (const c of cases) {
    writeFiles(batchDir, c, repository)
    c.base = batchDir
  }
  const ours = ourResults(cases)
  for (let i = 0; i < cases.length; i++) {
    const theirs = await officialResult(cases[i])
    compared++
    if (cases[i].css.includes('@plugin') || cases[i].css.includes('@config')) stats.javascript++
    if (theirs.error !== undefined) stats.compilerErrors++
    else {
      stats.compilerCss++
      stats.cssBytes += theirs.css.length
    }
    if (same(ours[i], theirs)) continue
    if (hasKnownFault(theirs)) {
      stats.knownFaults++
      const rest = await withoutKnownFault(cases[i])
      if (rest === null) stats.knownFaultsOnly++
      if (rest === null || !(await differs(rest))) continue
      cases[i] = rest
    }
    const reduced = await reduce(cases[i])
    const [reducedOurs] = ourResults([reduced])
    const reducedTheirs = await officialResult(reduced)
    console.log(`fuzz: a difference in compiler case ${done + i}:\n${reduced.css}\n  candidates: ${JSON.stringify(reduced.candidates)}`)
    record({
      kind: 'compiler',
      case: { css: cases[i].css, candidates: cases[i].candidates, files: cases[i].files, copy: cases[i].copy },
      reduced: { css: reduced.css, candidates: reduced.candidates, files: usedFiles(reduced), copy: usedCopy(reduced), official: reducedTheirs, ours: reducedOurs },
    })
  }
}

// The project cases.
// The module rules of the standalone program (bun-interop.mjs).
const officialCommand = [process.execPath, '--import', pathToFileURL(interop).href, cli]
for (let n = 0; n < opts.projects && Date.now() < deadline; n++) {
  const { dir, args, jsFiles, copy } = projectCase()
  const theirs = runProgram(officialCommand, args, dir)
  const ours = runProgram(ourCommand(), args, dir)
  compared++
  if (theirs.status === 0) stats.projectCss++
  else stats.projectErrors++
  if (theirs.status === ours.status && theirs.stdout === ours.stdout) {
    fs.rmSync(dir, { recursive: true, force: true })
    continue
  }
  const files = {}
  for (const f of ['app.css', 'src/index.html', 'src/deep/page.html.heex', 'src/app.js']) files[f] = fs.readFileSync(path.join(dir, f), 'utf8')
  console.log(`fuzz: a difference in project ${n} (${args.join(' ')}): exit ${theirs.status} and ${ours.status}`)
  record({ kind: 'project', args, files: { ...files, ...jsFiles }, copy, official: theirs, ours })
  fs.rmSync(dir, { recursive: true, force: true })
}

console.log(
  `fuzz: compiler cases with CSS ${stats.compilerCss} (${(stats.cssBytes / 1e6).toFixed(1)} MB), with an error ${stats.compilerErrors}, with JavaScript ${stats.javascript}; projects with CSS ${stats.projectCss}, with an error ${stats.projectErrors}`,
)
if (stats.knownFaults > 0) {
  console.log(
    `fuzz: ${stats.knownFaults} cases with a known fault of Tailwind CSS ${version} (an \`@variant\` rule in the output): ${stats.knownFaults - stats.knownFaultsOnly} compared again with no candidate that shows it, ${stats.knownFaultsOnly} not compared (the stylesheet shows it)`,
  )
}
console.log(`fuzz: ${compared} cases compared, ${failures} different (seed ${opts.seed})`)
if (failures > 0) {
  console.log(`fuzz: the failures are in ${failuresFile}`)
  process.exit(1)
}
