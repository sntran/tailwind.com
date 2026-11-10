#!/usr/bin/env node
// Usage: node gen-source-maps.mjs <path of the upstream clone> <output directory>
// This script writes source-maps.jsonl.gz: one JSON line for each stylesheet.
// For each line, upstream compiles the input with the `from` option, builds each list of
// candidates, and gives the source map: the decoded map of `buildSourceMap()` and the JSON text
// of `toSourceMap(...)` of `@tailwindcss/node`.
// Each build also has `optimized` and `minified`: the `code` and the `map` of
// `optimize(css, { file, minify, map })` of `@tailwindcss/node` (Lightning CSS in two passes,
// and the replacement of `@media not (` that keeps the map correct). `file` is the `from`
// of the line, or `input.css` when the `from` is empty.
// Each line also has `locations`: the `[start, end]` of the `src` of each node of
// `parse(input, { from })`, in document order (`null` for a node with no `src`).
// A line with `error` is an input that upstream rejects: the message has the position.
//
// The inputs are:
// - a sample of the records of suite.jsonl.gz that use no JavaScript module, and all the
//   records of the upstream test file `source-maps/source-map.test.ts`,
// - the records of suite-extra.jsonl.gz (imports of more than one file, `@apply`, `@utility`),
// - the entries of full.json.gz,
// - the cases of this file, and
// - random candidates of classes.jsonl.gz with `@import "tailwindcss"`.
//
// The four stylesheets of Tailwind CSS (tests/upstream/css) are large. A content that is equal
// to one of them is `{"css": NAME}` in the record, and `"@@css:NAME@@"` in the JSON text of a map.
// The content of the generated source of a decoded map is `{"generated": true}`: it is the `css`
// of the build.
import fs from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import zlib from 'node:zlib'
import { createRandom, loadUpstream, writeGzip } from './lib.mjs'

let [clone, outDir] = process.argv.slice(2)
if (!clone || !outDir) {
  console.error('Usage: node gen-source-maps.mjs <path of the upstream clone> <output directory>')
  process.exit(2)
}
clone = path.resolve(clone)
outDir = path.resolve(outDir)

let upstream = await loadUpstream(clone)
let nodePackage = path.join(clone, 'packages/@tailwindcss-node')
let { createJiti } = createRequire(path.join(nodePackage, 'package.json'))('jiti')
let jiti = createJiti(path.join(nodePackage, 'src/'), { moduleCache: true })
let { toSourceMap } = await jiti.import(path.join(nodePackage, 'src/source-maps.ts'))
let CSS = await jiti.import(path.join(clone, 'packages/tailwindcss/src/css-parser.ts'))

const CSS_NAMES = ['index.css', 'theme.css', 'preflight.css', 'utilities.css']
let cssFiles = new Map(
  CSS_NAMES.map((name) => [fs.readFileSync(path.join(upstream.packageDir, name), 'utf8'), name]),
)

function readLines(name) {
  let text = zlib.gunzipSync(fs.readFileSync(path.join(outDir, name))).toString('utf8')
  return text.split('\n').filter((line) => line !== '').map((line) => JSON.parse(line))
}

// ---------------------------------------------------------------------------
// The cases of this file. A file `./NAME.css` has the path `BASE/NAME.css`.

const TW = '@import "tailwindcss";\n'
const THEME = '@import "tailwindcss/theme.css" layer(theme);\n'
const UTILITIES = '@tailwind utilities;\n'

let cases = {
  'no feature: the map of the input': { input: '.a {\n  color: red;\n}\n\n/*! license */\n@media print {\n  .b { color: blue }\n}\n', builds: [[], ['flex']] },
  'no feature: an empty input': { input: '', builds: [[]] },
  'utilities only': { input: UTILITIES, builds: [['flex', 'hover:underline', 'md:p-4'], ['grid']] },
  'utilities in a layer': { input: '@layer utilities {\n  @tailwind utilities;\n}\n', builds: [['flex', 'sm:hover:underline', 'bg-red-500/50']] },
  'utilities with a source parameter': { input: '@tailwind utilities source(none);\n', builds: [['flex', 'w-1/2']] },
  'a theme': { input: '@theme {\n  --color-red: #f00;\n  --color-blue:\n    #00f;\n  --spacing: 0.25rem;\n}\n' + UTILITIES, builds: [['text-red', 'bg-blue/50', 'p-4', 'm-[3px]']] },
  'two themes': { input: '@theme {\n  --color-a: red;\n}\n.x { color: var(--color-a); }\n@theme {\n  --color-b: blue;\n  --color-a: green;\n}\n' + UTILITIES, builds: [['text-a', 'text-b']] },
  'a theme with options': { input: '@theme static {\n  --color-a: red;\n}\n@theme inline {\n  --color-b: var(--x);\n}\n@theme reference {\n  --color-c: blue;\n}\n@theme default {\n  --color-a: pink;\n  --color-d: pink;\n}\n' + UTILITIES, builds: [['text-a', 'text-b', 'text-c', 'text-d']] },
  'a theme with keyframes': { input: '@theme {\n  --animate-spin: spin 1s linear infinite;\n  --animate-none: none 1s;\n  @keyframes spin {\n    to {\n      transform: rotate(360deg);\n    }\n  }\n  @keyframes unused {\n    to { opacity: 0 }\n  }\n}\n' + UTILITIES, builds: [['animate-spin'], ['animate-none']] },
  'a theme with a prefix': { input: '@theme prefix(tw) {\n  --color-red: red;\n}\n' + UTILITIES, builds: [['tw:text-red', 'tw:hover:text-red']] },
  'apply': { input: THEME + '.btn {\n  @apply px-4 py-2   font-bold hover:underline;\n  color: red;\n}\n', builds: [[]] },
  'apply: the same candidate two times': { input: '.a {\n  @apply flex underline flex;\n}\n.b { @apply\n    flex\n    grid; }\n', builds: [[]] },
  'apply: text that is not ASCII before the rule': { input: '/* héllo wörld \u{1F600} */\n.ä\u{1F600} {\n  @apply flex content-["\u{1F600}é"] underline;\n}\n' + UTILITIES, builds: [['content-["é"]', 'flex']] },
  'apply with a utility': { input: THEME + UTILITIES + '@utility card {\n  padding: 1rem;\n  &:hover {\n    color: red;\n  }\n}\n@utility tab-* {\n  tab-size: --value(integer);\n}\n.x {\n  @apply card tab-4 md:card;\n}\n', builds: [['card', 'tab-2', 'hover:card', 'flex']] },
  'apply in a utility': { input: UTILITIES + '@utility a {\n  @apply b underline;\n}\n@utility b {\n  @apply flex;\n  color: red;\n}\n.x {\n  @apply a;\n}\n', builds: [['a', 'b', 'hover:a']] },
  'apply with important': { input: '.x {\n  @apply flex! !grid hover:underline!;\n}\n', builds: [[]] },
  'apply at the top level': { input: '@apply flex;\n' + UTILITIES, builds: [['grid']] },
  'apply: a mixin stays': { input: '.x {\n  @apply --mixin;\n}\n' + UTILITIES, builds: [['flex']] },
  'utility: static and functional': { input: UTILITIES + '@utility foo {\n  color: red;\n  @media print {\n    color: blue;\n  }\n}\n@utility bar-* {\n  width: --value(--bar-*, [length], integer);\n  height: --modifier(integer);\n}\n@theme {\n  --bar-big: 10rem;\n}\n', builds: [['foo', 'bar-big', 'bar-[3px]', 'bar-2/3', 'md:foo', 'group-hover:bar-4']] },
  'utility: the same name two times': { input: UTILITIES + '@utility foo {\n  color: red;\n}\n@utility foo {\n  background: blue;\n}\n', builds: [['foo', 'hover:foo']] },
  'utility: with a property rule': { input: UTILITIES + '@utility ring-x {\n  --tw-ring-x: 1px;\n  box-shadow: 0 0 0 var(--tw-ring-x) red;\n  @property --tw-ring-x {\n    syntax: "<length>";\n    inherits: false;\n    initial-value: 0px;\n  }\n}\n', builds: [['ring-x']] },
  'custom variants': { input: UTILITIES + '@custom-variant a (&.a);\n@custom-variant b {\n  &:where(.b) {\n    @slot;\n  }\n}\n@custom-variant c (@media print, &.c);\n@custom-variant d {\n  @media (min-width: 10px) {\n    &:hover {\n      @slot;\n    }\n  }\n  @supports (x: y) {\n    @slot;\n  }\n}\n', builds: [['a:flex', 'b:flex', 'c:flex', 'd:flex', 'a:b:c:underline', 'group-a:flex', 'not-b:flex', 'has-a:flex', 'peer-c:flex', 'in-a:flex', 'not-d:flex']] },
  'custom variants that use other variants': { input: UTILITIES + '@custom-variant a {\n  @variant b {\n    @variant hover {\n      @slot;\n    }\n  }\n}\n@custom-variant b {\n  &:where(.b) {\n    @slot;\n  }\n}\n', builds: [['a:flex', 'b:flex', 'md:a:flex']] },
  'custom variant with a scope': { input: UTILITIES + '@custom-variant scoped {\n  @scope (.x) {\n    @slot;\n  }\n}\n@custom-variant s2 (@scope (.y));\n', builds: [['scoped:flex', 's2:flex', 'scoped:hover:flex']] },
  'variant rules': { input: THEME + '.x {\n  color: red;\n  @variant hover {\n    color: blue;\n  }\n  @variant md {\n    @variant dark {\n      color: green;\n    }\n  }\n  @variant focus, active {\n    color: pink;\n  }\n}\n' + UTILITIES, builds: [['flex']] },
  'variant rules with a custom variant': { input: '@custom-variant a (&.a);\n@custom-variant b {\n  @media print {\n    @slot;\n  }\n}\n.x {\n  @variant a {\n    color: red;\n  }\n  @variant b {\n    color: blue;\n  }\n}\n', builds: [[]] },
  'reference': { input: '@reference "./a.css";\n.x {\n  @apply foo text-red;\n}\n', files: { './a.css': '@theme {\n  --color-red: red;\n}\n@utility foo {\n  color: blue;\n}\n@tailwind utilities;\n.a {\n  color: red;\n}\n' }, builds: [['foo', 'text-red']] },
  'reference to the theme of Tailwind CSS': { input: '@reference "tailwindcss";\n.x {\n  @apply text-red-500 p-4 hover:underline;\n}\n', builds: [['flex']] },
  'imports of several files': {
    input: '@import "./a.css";\n@import "./b.css" layer(b);\n@import "./c.css" print;\n@import "./d.css" layer(d) supports(display: grid) screen;\n' + UTILITIES,
    files: {
      './a.css': '@import "./a1.css";\n.a {\n  color: red;\n}\n@utility in-b {\n  color: blue;\n}\n@custom-variant in-c (&.c);\n',
      './a1.css': '.a1 {\n  color: red;\n  @apply flex;\n}\n@theme {\n  --color-a1: red;\n}\n',
      './b.css': '.b {\n  color: red;\n}\n',
      './c.css': '.c {\n  color: red;\n}\n',
      './d.css': '.d {\n  color: red;\n}\n',
    },
    builds: [['in-b', 'in-c:flex', 'text-a1', 'in-c:in-b']],
  },
  'imports: the same file two times': { input: '@import "./a.css";\n@import "./a.css" layer(x);\n', files: { './a.css': '.a {\n  color: red;\n}\n' }, builds: [[]] },
  'imports: an import in a rule': { input: '.x {\n  @import "./a.css";\n}\n@media screen {\n  @import "./b.css";\n}\n', files: { './a.css': 'color: red;\n', './b.css': '.b { color: red }\n' }, builds: [[]] },
  'imports: a remote import stays': { input: '@import url("./a.css");\n@import "https://example.com/a.css";\n.x {\n  color: red;\n}\n' + UTILITIES, builds: [['flex']] },
  'imports: utilities in an imported file': { input: '@import "./u.css";\n@import "./t.css";\n', files: { './u.css': '@layer utilities {\n  @tailwind utilities;\n}\n', './t.css': '@theme {\n  --color-x: red;\n}\n' }, builds: [['text-x', 'hover:bg-x']] },
  'imports: an empty file': { input: '@import "./e.css";\n.x { color: red }\n', files: { './e.css': '' }, builds: [[]] },
  'imports: theme options': { input: '@import "./a.css" theme(inline) prefix(tw) important;\n', files: { './a.css': '@theme {\n  --color-red: red;\n}\n@tailwind utilities;\n' }, builds: [['tw:text-red']] },
  'the stylesheets of Tailwind CSS': { input: TW, builds: [['flex', 'md:hover:underline', 'animate-spin', 'shadow-lg', 'bg-red-500/50', 'text-2xl'], ['container', 'prose', 'ring-2', 'bg-linear-to-r', 'from-red-500']] },
  'the stylesheets of Tailwind CSS with rules': { input: TW + '\n@theme {\n  --color-brand: #123456;\n  --font-display: "Satoshi", sans-serif;\n}\n\n@layer components {\n  .btn {\n    @apply rounded-lg bg-brand px-4 py-2 font-display text-white hover:bg-brand/80;\n  }\n}\n\n@utility content-auto {\n  content-visibility: auto;\n}\n\n@custom-variant theme-midnight (&:where([data-theme="midnight"] *));\n', builds: [['btn', 'content-auto', 'theme-midnight:flex', 'text-brand', 'dark:md:content-auto']] },
  'the theme and the utilities of Tailwind CSS': { input: '@layer theme, base, components, utilities;\n' + THEME + '@import "tailwindcss/utilities.css" layer(utilities);\n', builds: [['flex', 'p-4', 'text-red-500', 'shadow-md', 'translate-x-4', 'rotate-45', 'blur-sm', 'backdrop-blur', 'ring', 'divide-x', 'space-y-2', 'border', 'outline', 'transition', 'mask-b-from-80%']] },
  'property fallbacks': { input: UTILITIES, builds: [['translate-x-4', 'shadow', 'ring', 'border-2', 'content-["a"]', 'before:flex', 'bg-linear-to-r']] },
  'property fallbacks with a license comment and a charset': { input: '@charset "UTF-8";\n/*! a license */\n@import url("https://example.com/x.css");\n' + UTILITIES, builds: [['translate-x-4', 'shadow']] },
  'color-mix fallbacks': { input: '@theme {\n  --color-red: #f00;\n}\n.x {\n  color: color-mix(in oklab, var(--color-red) 50%, transparent);\n  background: --alpha(var(--color-red) / 50%);\n}\n' + UTILITIES, builds: [['bg-red/50', 'text-[red]/20', 'border-current/10']] },
  'no polyfills': { input: '@theme {\n  --color-red: #f00;\n}\n' + UTILITIES, polyfills: 0, builds: [['bg-red/50', 'translate-x-4']] },
  'only the property polyfill': { input: '@theme {\n  --color-red: #f00;\n}\n' + UTILITIES, polyfills: 1, builds: [['bg-red/50', 'translate-x-4']] },
  'only the color-mix polyfill': { input: '@theme {\n  --color-red: #f00;\n}\n' + UTILITIES, polyfills: 2, builds: [['bg-red/50', 'translate-x-4']] },
  'nesting': { input: '.a {\n  color: red;\n  .b & {\n    color: blue;\n  }\n  @media print {\n    color: green;\n    &:hover {\n      color: pink;\n    }\n  }\n  & {\n    color: black;\n  }\n}\n@media screen {\n  @media (min-width: 1px) {\n    .c {\n      color: red;\n    }\n  }\n  .d {\n    color: red;\n  }\n}\n' + UTILITIES, builds: [['flex']] },
  'nesting: rules with the same selector': { input: '.a {\n  color: red;\n}\n.a {\n  background: blue;\n}\n@media print {\n  .a {\n    color: red;\n  }\n}\n@media print {\n  .a {\n    color: blue;\n  }\n}\n@layer x;\n@layer x;\n' + UTILITIES, builds: [['flex']] },
  'nesting: at-rules that stay': { input: '.a {\n  @font-face {\n    font-family: x;\n  }\n  @starting-style {\n    opacity: 0;\n  }\n  @container (width > 1px) {\n    color: red;\n  }\n}\n@scope (.x) to (.y) {\n  .a {\n    color: red;\n    @scope (.z) {\n      color: blue;\n    }\n  }\n}\n' + UTILITIES, builds: [['flex', 'starting:opacity-0', '@md:flex']] },
  'compound variants': { input: UTILITIES, builds: [['group-hover:flex', 'peer-focus:flex', 'not-hover:flex', 'has-checked:flex', 'in-focus:flex', 'group-has-[a]:flex', 'not-md:flex', 'not-supports-[display:grid]:flex', 'group-data-[x=y]/name:flex', '*:flex', '**:flex', 'before:after:flex', 'hover:focus:active:flex']] },
  'media variants': { input: '@theme {\n  --breakpoint-sm: 40rem;\n  --breakpoint-md: 48rem;\n  --container-sm: 24rem;\n}\n' + UTILITIES, builds: [['sm:flex', 'max-md:flex', 'min-[100px]:flex', '@sm:flex', '@max-sm:flex', 'supports-[display:grid]:flex', 'dark:flex', 'print:flex', 'motion-safe:flex', 'starting:flex', 'sm:max-md:dark:hover:flex']] },
  'arbitrary properties and values': { input: UTILITIES, builds: [['[color:red]', '[--x:1px]', 'hover:[color:red]', 'w-[calc(100%-1rem)]', 'bg-[url(/a.png)]', '[&>*]:flex', '[@media(print)]:flex', '!flex', 'flex!']] },
  'important': { input: '@import "./a.css" important;\n', files: { './a.css': '@tailwind utilities;\n.x {\n  color: red;\n}\n' }, builds: [['flex', 'hover:underline']] },
  'comments': { input: '/*! first license */\n/* a comment */\n.a {\n  /*! in a rule */\n  color: red; /* after */\n}\n/*! last license */\n' + UTILITIES, builds: [['flex']] },
  'a byte order mark and CRLF': { input: '\uFEFF.a {\r\n  color: red;\r\n  background:\r\n    blue;\r\n}\r\n@tailwind utilities;\r\n', builds: [['flex']] },
  'text that is not ASCII': { input: '.é\u{1F600} {\n  content: "\u{1F600}é";\n  --x: \u{1F600};\n}\n/*! \u{1F600} */\n@theme {\n  --color-é: red;\n}\n.z { color: red }\n' + UTILITIES, builds: [['text-é', 'content-["\u{1F600}"]', 'flex']] },
  'text that is not ASCII in an imported file': { input: '@import "./ü.css";\n' + UTILITIES, files: { './ü.css': '/* \u{1F600} */ .ü { color: red }\n@utility e-u {\n  content: "é";\n}\n' }, builds: [['flex']] },
  'escapes at the start of a node': { input: '\\31 0 { color: red }\n.a { \\63olor: red; "x": y }\n' + UTILITIES, builds: [['flex']] },
  'declarations at the top level': { input: 'color: red;\n--x: { a: b };\n@layer a, b;\n.x { color: red }\n@unknown foo', builds: [[]] },
  'a declaration with no semicolon at the end': { input: UTILITIES + '.a { color: red }\n.b { }\ncolor: blue', builds: [['flex']] },
  'multi-line values': { input: '.a {\n  grid-template-areas:\n    "a b"\n    "c d";\n  --x:\n    1\n    2;\n  font-family:\n    a,\n\n    b;\n}\n@media\n  screen\n  and (min-width: 1px)\n{\n  .b\n  .c,\n  .d\n  {\n    color: red\n  }\n}\n' + UTILITIES, builds: [['flex']] },
  'functions': { input: '@theme {\n  --color-red: #f00;\n  --spacing: 0.25rem;\n  --breakpoint-md: 48rem;\n}\n.a {\n  color: theme(--color-red);\n  margin: --spacing(4);\n  background: --alpha(theme(--color-red) / 50%);\n}\n@media (min-width: theme(--breakpoint-md)) {\n  .b {\n    color: --theme(--color-red);\n  }\n}\n' + UTILITIES, builds: [['flex']] },
  'media with theme options': { input: '@media theme(inline) {\n  @theme {\n    --color-a: red;\n  }\n}\n@media print and important {\n  @tailwind utilities;\n  .x { color: red }\n}\n', builds: [['text-a', 'flex']] },
  'source rules': { input: '@source "./a";\n@source inline("flex hover:underline p-{1..2}");\n@source not inline("p-1");\n' + UTILITIES, builds: [[], ['grid']] },
  'a variable candidate': { input: '@theme {\n  --color-a: red;\n  --color-b: blue;\n}\n' + UTILITIES, builds: [[], ['--color-a'], ['flex'], ['--color-b', 'flex']] },
  'a second build with no change': { input: UTILITIES, builds: [['flex'], ['flex'], ['nope'], ['grid'], []] },
  'from with a directory': { from: '/project/src/styles/app.css', input: '@import "./a.css";\n.x {\n  @apply flex;\n}\n' + UTILITIES, files: { './a.css': '.a {\n  color: red;\n}\n' }, builds: [['grid']] },
  'an empty from': { from: '', input: '@import "./a.css";\n.x {\n  color: red;\n}\n' + UTILITIES, files: { './a.css': '.a {\n  color: red;\n}\n' }, builds: [['grid']] },
  'legacy utilities and the default theme': { input: THEME + UTILITIES + '.a {\n  color: theme(colors.red.500);\n}\n', builds: [['max-w-screen-md', 'flex-shrink', 'start-4']] },
  'container': { input: '@theme {\n  --breakpoint-sm: 40rem;\n  --breakpoint-md: 48rem;\n}\n' + UTILITIES, builds: [['container', 'md:container']] },
  'optimizer: media rules with not': { input: '@theme {\n  --breakpoint-sm: 40rem;\n  --breakpoint-md: 48rem;\n}\n' + UTILITIES + '@media not (width >= 48rem) {\n  .é\u{1F600} {\n    content: "\u{1F600}";\n  }\n}\n@media not (hover: hover) {\n  .a { color: red }\n}\n@media not print {\n  .b { color: red }\n}\n', builds: [['not-sm:flex', 'not-md:hover:underline', 'not-print:hidden', 'max-md:not-sm:p-4', 'not-hover:flex', 'not-dark:grid', 'not-motion-safe:flex', 'content-["\u{1F600}"]', 'not-supports-[display:grid]:flex']] },
  'optimizer: media rules with not at the start and at the end': { input: '@media not (width >= 48rem) {\n  .a { color: red }\n}\n.b { color: blue }\n@media not (width < 10px) {\n  .c { color: red }\n}', builds: [[]] },
  'optimizer: media rules with not in imported files': { input: '@import "./a.css";\n@import "./b.css" layer(b);\n@import "tailwindcss/theme.css" layer(theme);\n' + UTILITIES, files: { './a.css': '@media not (width >= 48rem) {\n  .a {\n    color: red;\n    &:hover { color: blue }\n  }\n}\n', './b.css': '.b {\n  @media not (min-width: 10px) {\n    color: red;\n  }\n  @apply not-md:flex;\n}\n' }, builds: [['not-md:flex', 'not-lg:p-4', 'not-sm:not-hover:underline']] },
  'optimizer: nesting and equal rules': { input: '.a {\n  color: red;\n  .b {\n    color: blue;\n  }\n}\n.a {\n  background: red;\n}\n.c { color: red }\n.d { color: red }\n@media (width >= 10px) {\n  .a { color: red }\n}\n@media (width >= 10px) {\n  .e { color: red }\n}\n' + UTILITIES, builds: [['flex', 'md:flex', 'hover:flex']] },
  'keyframes of a utility': { input: UTILITIES + '@utility wiggle {\n  animation: wiggle 1s;\n  @keyframes wiggle {\n    to {\n      rotate: 3deg;\n    }\n  }\n}\n@keyframes top {\n  from {\n    opacity: 0;\n  }\n}\n', builds: [['wiggle', 'hover:wiggle']] },
}

// ---------------------------------------------------------------------------

function ref(content, css) {
  if (content === null || content === undefined) return null
  if (cssFiles.has(content)) return { css: cssFiles.get(content) }
  if (css !== undefined && content === css) return { generated: true }
  return content
}

function decoded(map, css) {
  let index = new Map(map.sources.map((source, i) => [source, i]))
  return {
    file: map.file,
    sources: map.sources.map((source) => ({ url: source.url, content: ref(source.content, css) })),
    mappings: map.mappings.map((mapping) => {
      let original = mapping.originalPosition
      return [
        mapping.generatedPosition.line,
        mapping.generatedPosition.column,
        original ? index.get(original.source) : -1,
        original ? original.line : 0,
        original ? original.column : 0,
      ]
    }),
  }
}

function raw(map) {
  return placeholders(toSourceMap(map).raw)
}

function placeholders(text) {
  for (let [content, name] of cssFiles) {
    text = text.replaceAll(JSON.stringify(content), JSON.stringify(`@@css:${name}@@`))
  }
  return text
}

// The `src` of each node of a tree, in document order.
function locations(ast, out = []) {
  for (let node of ast) {
    out.push(node.src ? [node.src[1], node.src[2]] : null)
    if (node.src && (node.dst?.[0] !== node.src[0] || node.dst[1] !== node.src[1] || node.dst[2] !== node.src[2])) {
      throw new Error('the parser gave a dst that is not the src')
    }
    if (node.nodes) locations(node.nodes, out)
  }
  return out
}

function message(err) {
  return err instanceof Error ? err.message : String(err)
}

// Run one stylesheet. `load` gives the result of a loader call, or throws.
async function run({ name, input, from, base, polyfills, builds }, load) {
  let stylesheets = []
  async function loadStylesheet(id, sheetBase) {
    let entry = { id, base: sheetBase }
    stylesheets.push(entry)
    try {
      let result = await load(id, sheetBase)
      entry.result = { path: result.path, base: result.base, content: ref(result.content) }
      return result
    } catch (err) {
      entry.error = message(err)
      throw err
    }
  }

  let options = { from, base, loadStylesheet }
  if (polyfills !== null && polyfills !== undefined) options.polyfills = polyfills

  let record = { name, input, from, base, polyfills: polyfills ?? null, stylesheets, builds: [] }
  let compiler
  try {
    record.locations = locations(CSS.parse(input, { from }))
    compiler = await upstream.compile(input, options)
  } catch (err) {
    record.error = message(err)
    return record
  }

  for (let candidates of builds) {
    let css = compiler.build(candidates)
    let map = compiler.buildSourceMap()
    let build = { candidates, css, map: decoded(map, css), raw: raw(map) }
    for (let [key, minify] of [['optimized', false], ['minified', true]]) {
      try {
        let result = upstream.optimize(css, { file: from || 'input.css', minify, map: toSourceMap(map).raw })
        build[key] = { code: result.code, map: placeholders(result.map) }
      } catch (err) {
        build[key] = { error: message(err) }
      }
    }
    record.builds.push(build)
  }
  return record
}

let records = []

// 1. The records of the suite.
function replayLoader(calls) {
  return async (id, base) => {
    let call = calls.find((call) => call.id === id && call.base === base)
    if (!call || !call.result) throw new Error(call?.error ?? `no stylesheet ${id}`)
    // Many upstream tests give an empty path. A path gives the nodes of the file a source.
    return { ...call.result, path: call.result.path || `${base}/${id}`.replace('/./', '/') }
  }
}

// SOURCE_MAPS_STEP=1 uses each record of the suite: a larger file for a full check.
const STEP = Number(process.env.SOURCE_MAPS_STEP ?? 13)
let suite = readLines('suite.jsonl.gz').filter((record) => record.modules === 0 && !record.error)
let step = 0
for (let record of suite) {
  let isSourceMapTest = record.file.endsWith('source-maps/source-map.test.ts')
  if (record.builds.length === 0 || record.builds.some((build) => build.error)) continue
  if (!isSourceMapTest && step++ % STEP !== 0) continue
  records.push(
    await run(
      {
        name: `suite: ${record.file.replace('packages/tailwindcss/src/', '')}: ${record.test}`,
        input: record.input,
        from: record.options.from ?? 'input.css',
        base: record.options.base ?? '',
        polyfills: record.options.polyfills,
        builds: record.builds.map((build) => build.candidates),
      },
      replayLoader(record.stylesheets),
    ),
  )
}

// 2. The records of suite-extra.
for (let record of readLines('suite-extra.jsonl.gz')) {
  if (record.modules !== 0 || record.error || record.builds.length === 0) continue
  if (record.builds.some((build) => build.error)) continue
  records.push(
    await run(
      {
        name: `extra: ${record.test}`,
        input: record.input,
        from: '/root/input.css',
        base: record.options.base ?? '',
        polyfills: record.options.polyfills,
        builds: record.builds.map((build) => build.candidates),
      },
      replayLoader(record.stylesheets),
    ),
  )
}

// The loader of the cases: the stylesheets of Tailwind CSS, and the files of the case.
function caseLoader(files = {}) {
  return async (id, base) => {
    if (id in files) {
      let name = id.replace(/^\.\//, '')
      return { path: `${base}/${name}`, base: `${base}/${name.replace(/\.css$/, '')}`, content: files[id] }
    }
    if (id === 'tailwindcss' || id.startsWith('tailwindcss/') || CSS_NAMES.includes(id.replace(/^\.\//, ''))) {
      return upstream.loadStylesheet(id, base)
    }
    throw new Error(`no stylesheet ${id}`)
  }
}

// 3. The entries of full.json.gz.
let full = JSON.parse(zlib.gunzipSync(fs.readFileSync(path.join(outDir, 'full.json.gz'))).toString('utf8'))
for (let [index, entry] of full.entries()) {
  records.push(
    await run(
      { name: `full: ${index}`, input: entry.input, from: 'input.css', base: '/root', builds: [entry.candidates] },
      caseLoader(),
    ),
  )
}

// 4. The cases of this file.
for (let [name, spec] of Object.entries(cases)) {
  records.push(
    await run(
      {
        name: `case: ${name}`,
        input: spec.input,
        from: spec.from ?? 'input.css',
        base: '/root',
        polyfills: spec.polyfills,
        builds: spec.builds,
      },
      caseLoader(spec.files),
    ),
  )
}

// 5. Random candidates.
let random = createRandom(20260405)
let classes = readLines('classes.jsonl.gz').map((record) => record.candidate)
const RANDOM_INPUTS = [
  TW,
  '@layer theme, base, components, utilities;\n' + THEME + '@import "tailwindcss/utilities.css" layer(utilities);\n',
  THEME + UTILITIES,
  '@import "tailwindcss/theme.css" theme(reference);\n' + UTILITIES,
  THEME + '@import "tailwindcss/utilities.css" layer(utilities) important;\n',
]
for (let i = 0; i < 30; i++) {
  let builds = []
  for (let b = 0; b < 1 + random.int(2); b++) {
    let candidates = []
    for (let c = 0; c < 5 + random.int(60); c++) candidates.push(random.pick(classes))
    builds.push(candidates)
  }
  records.push(
    await run(
      { name: `random: ${i}`, input: RANDOM_INPUTS[i % RANDOM_INPUTS.length], from: 'input.css', base: '/root', builds },
      caseLoader(),
    ),
  )
}

// 6. Inputs with an error.
const ERRORS = {
  'a closing bracket with no rule': '.foo {\n  color: red;\n}\n\n.bar\n  color: blue;\n}\n',
  'a rule with no end': '.foo {\n  color: red;\n}\n\n  .bar {\n  color: blue;\n',
  'an at-rule with no end': '.a { color: red }\n@media (width > 1px) {\n  .b {\n    color: red;\n  }\n',
  'a string with no end': '.foo {\n  content: "Hello world!\n  font-weight: bold;\n}\n',
  'a string with no end and a semicolon': '.foo {\n  content: "Hello world!;\n  font-weight: bold;\n}\n',
  'a string with no end in a custom property': '.foo {\n  --bar: \'Hello world!\n}\n',
  'a custom property with no value': '--foo',
  'a custom property with no value in a rule': '.foo { --bar }',
  'a custom property with no value after a rule': '.a {\n  color: red;\n}\n.foo {\n  color: blue;\n    --bar }',
  'a declaration with no value': '.foo { bar }',
  'a declaration with no value and a semicolon': '.foo {\n  color: red;\n  bar;\n}',
  'a closing parenthesis with no start': '.foo {\n  color: rgb(1 2 3));\n}',
  'after text that is not ASCII': '/* h\u00e9llo \u{1F600} */\n.\u00e9\u{1F600} { content: "\u{1F600}"; \u{1F600} }',
  'text that is not ASCII in the line of the error': '.a { --\u00e9: "\u{1F600}"; \u{1F600}\u00e9: \u{1F600}; b\u{1F600}r }',
  'after a byte order mark': '\uFEFF.foo { bar }',
  'with CRLF': '.foo {\r\n  color: red;\r\n}\r\n\r\n.bar\r\n  color: blue;\r\n}\r\n',
  'a string with no end and CRLF': '.foo {\r\n  content: "Hello world!\r\n  font-weight: bold;\r\n}\r\n',
  'a rule with no end after an escape': '\\31 0 {\n  color: red;\n}\n\\32 0 {\n  color: red',
  'a rule with no end after a string': '.a { color: red }\n"x" y {\n  color: red',
  'a rule with no end after a parenthesis': '.a { color: red }\n   (x) y {\n  color: red',
  'an error in an imported file': { input: '@import "./a.css";\n', files: { './a.css': '.a {\n  color: red;\n}\n.b { bar }\n' } },
  'an error in a file that an imported file imports': {
    input: '.x { color: red }\n@import "./a.css";\n',
    files: { './a.css': '@import "./b.css";\n', './b.css': '\n\n   .b {\n' },
  },
  'an empty from': { from: '', input: '.foo { bar }' },
}
for (let [name, spec] of Object.entries(ERRORS)) {
  if (typeof spec === 'string') spec = { input: spec }
  let record = await run(
    { name: `error: ${name}`, input: spec.input, from: spec.from ?? 'input.css', base: '/root', builds: [] },
    caseLoader(spec.files),
  )
  if (!record.error) throw new Error(`no error: ${name}`)
  let { error, ...rest } = record
  records.push({ ...rest, expectedError: error })
}

let failed = records.filter((record) => record.error)
for (let record of failed) console.error(`error: ${record.name}: ${record.error}`)
if (failed.length > 0) process.exit(1)

let out = process.env.SOURCE_MAPS_OUT ?? path.join(outDir, 'source-maps.jsonl.gz')
writeGzip(
  out,
  records.map(({ expectedError, ...record }) => JSON.stringify(expectedError ? { ...record, error: expectedError } : record)).join('\n') + '\n',
)
let builds = records.reduce((sum, record) => sum + record.builds.length, 0)
let mappings = records.reduce(
  (sum, record) => sum + record.builds.reduce((sum, build) => sum + build.map.mappings.length, 0),
  0,
)
console.log(`${out}: ${records.length} records, ${builds} builds, ${mappings} mappings, ${fs.statSync(out).size} bytes`)
