#!/usr/bin/env node
// Usage: node gen-optimize.mjs <path of the upstream clone> [output file]
// This script writes native/tailwind/tests/fixtures/optimize.json: the
// results of the upstream `handleNesting` and `optimizeAst` (ast.ts).
//
// The file has three lists:
// - `themes`: `{ css, prefix, entries }`. `entries` are the values of the
//   theme (`key`, `value`, `options`) after `__unstable__loadDesignSystem(css)`.
// - `nesting`: `{ css, tree }`. `tree` is `handleNesting(special(parse(css)))`.
// - `optimize`: `{ theme, used, polyfills, css, tree }`. `theme` is an index
//   in `themes`. Each case has a new design system. The script calls
//   `designSystem.trackUsedVariables` for each text of `used`, then
//   `optimizeAst(special(parse(css)), designSystem, polyfills)`.
//
// `special` changes these at-rules of the input to the nodes that CSS text
// cannot give. The Rust test has the same function.
// - `@in-variant { … }`: `context({ source: 'variant' }, …)`
// - `@x-context <words> { … }`: `context({ … }, …)`. The words are: `theme`,
//   `theme-false`, `reference`, `keyframes`, `supports-color-mix`,
//   `source-variant`, `source-user`.
// - `@x-at-root { … }`: `atRoot(…)`
// - `@x-undefined <property>;`: `decl(property, undefined)`
//
// A tree is a list of nodes. A node is one of:
// `["r", selector, nodes]`, `["a", name, params, nodes]`,
// `["d", property, value or null, important]`, `["c", value]`,
// `["x", context, nodes]`, `["t", nodes]` (an at-root).
import fs from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { createRandom } from './lib.mjs'

process.env.NODE_ENV = 'test'

let [clone, outFile] = process.argv.slice(2)
if (!clone) {
  console.error('Usage: node gen-optimize.mjs <path of the upstream clone> [output file]')
  process.exit(1)
}
clone = path.resolve(clone)
outFile ??= path.join(
  path.dirname(fileURLToPath(import.meta.url)),
  '../../../native/tailwind/tests/fixtures/optimize.json',
)

let src = path.join(clone, 'packages/tailwindcss/src')
let require = createRequire(path.join(clone, 'packages/@tailwindcss-node/package.json'))
let { createJiti } = require('jiti')
let jiti = createJiti(src + '/', { moduleCache: true })
let Ast = await jiti.import(path.join(src, 'ast.ts'))
let Parser = await jiti.import(path.join(src, 'css-parser.ts'))
let Tailwind = await jiti.import(path.join(src, 'index.ts'))

// ---------------------------------------------------------------------------
// The shared functions
// ---------------------------------------------------------------------------

function special(nodes) {
  let out = []
  for (let node of nodes) {
    if (node.kind === 'rule') {
      out.push(Ast.styleRule(node.selector, special(node.nodes)))
    } else if (node.kind === 'at-rule') {
      let nodes = special(node.nodes)
      if (node.name === '@in-variant') {
        out.push(Ast.context({ source: 'variant' }, nodes))
      } else if (node.name === '@x-context') {
        let context = {}
        for (let word of node.params.split(' ')) {
          if (word === 'theme') context.theme = true
          else if (word === 'theme-false') context.theme = false
          else if (word === 'reference') context.reference = true
          else if (word === 'keyframes') context.keyframes = true
          else if (word === 'supports-color-mix') context.supportsColorMix = true
          else if (word === 'source-variant') context.source = 'variant'
          else if (word === 'source-user') context.source = 'user'
        }
        out.push(Ast.context(context, nodes))
      } else if (node.name === '@x-at-root') {
        out.push(Ast.atRoot(nodes))
      } else if (node.name === '@x-undefined') {
        out.push(Ast.decl(node.params, undefined))
      } else {
        out.push(Ast.atRule(node.name, node.params, nodes))
      }
    } else if (node.kind === 'declaration') {
      out.push(Ast.decl(node.property, node.value, node.important))
    } else {
      out.push(Ast.comment(node.value))
    }
  }
  return out
}

function encode(nodes) {
  return nodes.map((node) => {
    switch (node.kind) {
      case 'rule':
        return ['r', node.selector, encode(node.nodes)]
      case 'at-rule':
        return ['a', node.name, node.params, encode(node.nodes)]
      case 'declaration':
        return ['d', node.property, node.value ?? null, node.important]
      case 'comment':
        return ['c', node.value]
      case 'context':
        return ['x', node.context, encode(node.nodes)]
      case 'at-root':
        return ['t', encode(node.nodes)]
      default:
        throw new Error(`Unknown kind: ${node.kind}`)
    }
  })
}

function tryParse(css) {
  try {
    return special(Parser.parse(css))
  } catch {
    return null
  }
}

// ---------------------------------------------------------------------------
// The inputs of `handleNesting`
// ---------------------------------------------------------------------------

// Each css`…` text of a test file, without the texts that have `${…}`.
function cssLiterals(file) {
  let text = fs.readFileSync(path.join(src, file), 'utf8')
  let out = []
  for (let match of text.matchAll(/css`([^`]*)`/g)) {
    if (match[1].includes('${')) continue
    out.push(match[1])
  }
  return out
}

// The rows of the `it.each` tables of ast.test.ts: `['root', 'nested', 'expected'],`
function tableRows() {
  let text = fs.readFileSync(path.join(src, 'ast.test.ts'), 'utf8')
  let out = []
  for (let match of text.matchAll(/^\s*\['([^']*)', '([^']*)', '[^']*'\],/gm)) {
    out.push(`${match[1]} {\n  ${match[2]} {\n    --x: 0;\n  }\n}\n`)
  }
  return out
}

const EXTRA_NESTING = [
  // Upstream loses the second declaration here.
  '@media a { color: red } @media a { color: blue; background: x }',
  '@page { margin: 1cm } @page { margin: 2cm; size: A4 }',
  '@media a { color: red; .x { color: blue } color: green; }',
  '@layer a { @media b { color: red } } @layer a { @media b { .x { color: blue } margin: 0 } }',
  '.a { color: red } .a { color: red; margin: 0 } .a { margin: 0 }',
  '.a { & { color: red } & { color: red; color: blue } }',
  '.a { &:hover { @media x { color: red; color: red; } } }',
  '.a { .b { --x: 1 } } .a { .b { --x: 1; --y: 2 } .b { --y: 2 } }',
  '@media a { .x { --a: 1 } } @media a { .x { --a: 1; --b: 2 } @media c { .x { --a: 1 } } .x { --b: 2 } }',
  '@layer a; @layer a; @layer b; @layer a;',
  '@charset "UTF-8"; @charset "UTF-8"; @import "a.css"; @import "a.css";',
  // Two lists with a declaration two times: one list is in the other list.
  '@media a { color: red; color: red; margin: 0; } @media a { .x { --a: 1; --a: 1 } } @media a { .x { --b: 1; --b: 1 } }',
  '@layer l { @media a { --p: 1; --p: 1; } } @layer l { @media a { .x { --a: 1; --a: 1; --a: 2 } .y { --a: 1; --a: 1 } } }',
  '.a { @layer x; @layer x; color: red; }',
  '.a { @layer x; } .a { @layer x; }',
  '.a { .b { @layer x; @apply flex; @unknown; @media print; } }',
  '.a::before { > * { color: red; @layer x; @media print { color: blue; .y { --z: 1 } :is(&, .f) { --w: 1 } } @font-face { a: b } } }',
  '.a::before { > * { :is(&, .f) { color: red } @scope (&) { .x { --a: 1 } } @scope (:is(&, .q)) { .x { --a: 2 } } } }',
  '.a { @scope { color: red; .b { --x: 1 } margin: 0 } }',
  '.a { @in-variant { @scope (.f) to (.t) { color: red; @x-context source-user { margin: 0; .q { --x: 1 } } } } }',
  '.a { @in-variant { @x-context source-user { @scope (.f) to (.t) { color: red; } } } }',
  '.a { @in-variant { @scope (& .f) to (& .t) { color: red; & > p { --x: 1 } } } }',
  '@scope (& > .x) to (&) { & { --a: 1 } .y & { --b: 2 } }',
  '@scope (.a, .b) { color: red }',
  '.p, .q { @scope (.a, &.b) to (.c, & .d) { color: red; .z { --x: 1 } } }',
  '.p { @scope to (.end) { color: red } }',
  '.p { @scope ( .a , .b ) to ( .c ) { --x: 1 } }',
  '.p { color: red; @scope (.a) { --x: 1 } margin: 0 }',
  '.p { .q { color: red; .r { @scope (.a) { --x: 1 } } } }',
  '.p { @media print { @scope (.a) { .b { --x: 1 } @media screen { .c { --y: 1 } } } } }',
  '.p { @scope (.a) { @scope (.b) { .c { --x: 1 } } } }',
  '.p { @scope; @media print; @supports (a: b); @container; @starting-style; @page; @view-transition; }',
  '& { color: red } &:hover { --x: 1 } .a & { --y: 1 } [x="&"] { --z: 1 } \\& { --w: 1 }',
  '.a { [x="&"] { --z: 1 } \\& { --w: 1 } .b\\&c & { --v: 1 } [y="a,b"], .c { --u: 1 } }',
  '.a, .b { & + & { --x: 1 } && { --y: 1 } :not(&) { --z: 1 } :is(&) & { --w: 1 } }',
  '* { &:hover { --x: 1 } &.a { --y: 1 } .b& { --z: 1 } & > & { --w: 1 } }',
  '*.foo { &:hover { --x: 1 } :not(&):focus { --y: 1 } }',
  '.a { *|div& { --x: 1 } *:hover { --y: 1 } *& { --z: 1 } **& { --w: 1 } }',
  '.a > .b { .c &:focus { --x: 1 } &:focus .c { --y: 1 } .c&.d { --z: 1 } :is(.c &.d) { --w: 1 } }',
  '.a::before { &:hover { --x: 1 } & > * { --y: 1 } & { --z: 1 } }',
  '.a { &::BEFORE > * { --x: 1 } &:Before > * { --y: 1 } &::-webkit-scrollbar > * { --z: 1 } &::ng-deep > * { --w: 1 } &::part(foo) > * { --v: 1 } &::Part(foo) + p { --u: 1 } }',
  '.a { :is(&::before > *, .b) { --x: 1 } :where(&::after + *) { --y: 1 } :not(&::after + *) { --z: 1 } :is() { --w: 1 } }',
  '.a { > * { --x: 1 } + .b, ~ .c { --y: 1 } }',
  '.a { @x-context source-user { color: red; } @x-at-root { .b { --x: 1 } } }',
  '.a { color: red; @x-context source-user { .b { --x: 1 } } @x-at-root { .c { --y: 1 } } }',
  '@x-at-root { .a { .b { --x: 1 } } color: red }',
  '.a { @keyframes x { from { --a: 1 } to { --a: 2 } } @font-face { font-family: x } @property --p { syntax: "*"; inherits: false } }',
  '.a { @media x { @keyframes k { from { --a: 1 } } } }',
  '@media x { @media y { .a { --x: 1 } } } @media x { @media y { .a { --y: 1 } } @media z { .a { --z: 1 } } }',
  '@media x { .a { --x: 1 } } @media x { .b { --x: 1 } } @supports (a: b) { @media x { .b { --y: 1 } } }',
  '.a { --x: 1; --x: 1 !important; --x: 1; --x: 2; --x: 1 !important; }',
  '.a { @x-undefined color; @x-undefined color; --x: undefined; @x-undefined --x; }',
  '.a { & { @x-undefined color; } & { @x-undefined color; color: undefined; } }',
  '.a { /*! one */ .b { /*! two */ } /*! three */ }',
  '/*! top */ .a { /*! in */ --x: 1 }',
  '.a::before { /*! dead? */ > p { /*! dead */ --x: 1 } }',
]

// A generator of random nested CSS.
const TOP_SELECTORS = [
  '.a', '.b', '.a, .b', '.a > .b', 'div', '*', '.a::before', '.a:hover', '& .top', '&', ':is(.a, .b)',
  '.a .b, .c', '[x="&"]', '::selection', '.a::after, .b', '#id', '*.u', 'ul li', '.a + .b', ':root, :host',
  '.x::-webkit-scrollbar', '.x::ng-deep', 'a:not(.b)', '.s\\:hover\\:x:hover', '[data-a="1,2"], .l',
]
const NESTED_SELECTORS = [
  '&', '&:hover', '& > *', '& .x', '.x &', '.x > &', '&.y', '.y&', 'div&', '&div', '*&', '&*', '& + &', '&&',
  ':is(&, .z)', ':where(&) > p', ':not(&)', ':has(> &)', ':is(.q, &:hover) span', '.p, &.q',
  '& > *, .fallback &', '[data-a="&"] &', '&[data-b="x,y"]', '> .child', '+ .sib', '&::before', '&::after > span',
  '&::selection .x', '*|div&', '*:hover', ':is(&::before > *)', '.n::before:hover', '\\&', '.x\\&y', ':scope > &',
  '.x', '.x, .y', '.x .y &:focus', '&:focus .x', '.m&.n', '&:not(.a, .b)', ':where(&, .w)::before',
  '&::placeholder', '&::file-selector-button:hover', '&::marker ~ li', '.k &::before', '&:is(.a, .b) > &',
  ':is(&::after > *, &.ok)', '& &', '&:has(&)', '[a="]"]&', '&, &', '& ,.c', '&::slotted(span) > b',
  '&::view-transition-old(root) + p', '&:first-line > i', '&:FIRST-LETTER > i', '&::unknown > i',
]
const BLOCK_AT_RULES = [
  '@media (min-width: 1px)', '@media print', '@supports (display: grid)',
  '@supports (color: color-mix(in lab, red, red))', '@container (width > 1px)', '@container card (width > 1px)',
  '@layer base', '@layer utilities', '@layer', '@scope (.from) to (.to)', '@scope (.from)',
  '@scope (& > .s) to (& .l)', '@scope', '@scope to (.end)', '@scope (.a, .b)', '@scope (&)',
  '@scope (:is(&, .alt))', '@scope (.s) to (&)', '@starting-style', '@page :first', '@view-transition',
  '@unknown foo', '@in-variant', '@x-context source-user', '@x-context source-variant', '@x-at-root',
  '@media (min-width: 1px)', '@media print', '@layer base', '@supports (display: grid)',
]
const LEAF_AT_RULES = [
  ['@keyframes spin', ['from { --k: 1; }', 'to { --k: 2; }', '50% { color: color-mix(in oklab, red, blue); }']],
  ['@font-face', ['font-family: "X";', 'src: url(x.woff2);']],
  ['@property --p', ['syntax: "*";', 'inherits: false;', 'initial-value: 0;']],
  ['@property --q', ['syntax: "<color>";', 'inherits: true;', 'initial-value: red;']],
  ['@property --r', ['syntax: "*";', 'inherits: false;']],
]
const STATEMENTS = [
  '@layer foo;', '@layer foo, bar;', '@charset "UTF-8";', '@import "x.css";', '@apply flex;', '@media print;',
  '@unknown;', '@namespace svg url(http://www.w3.org/2000/svg);', '@custom-media --m (color);', '@scope;',
  '@supports (a: b);', '@container;', '@layer;',
]
const DECLARATIONS = [
  'color: red;', 'color: blue;', '--x: 1;', '--x: 2;', 'margin: 0;', 'color: red !important;', '--y: { a: b };',
  'background: url(a.png);', 'color: red;', '--x: 1;',
]
const OPTIMIZE_DECLARATIONS = [
  'color: var(--color-red-500);', 'background: var(--color-primary);', 'fill: var(--color-secondary, blue);',
  'color: color-mix(in oklab, var(--color-red-500) 50%, transparent);',
  'color: color-mix(in oklab, currentcolor 50%, transparent);',
  'color: color-mix(in oklch, var(--unknown) 50%, var(--color-blue-500));',
  'border-color: color-mix(in srgb, var(--color-primary), var(--color-blue-500) 20%);',
  'color: color-mix(var(--color-red-500) 50%, blue);',
  '--tw-sort: zzz;', 'animation: spin 1s linear infinite;', 'animation: var(--animate-ping);',
  'animation: none;', '--color-red-500: #ef4444;', '--color-extra: var(--color-red-500);',
  '--animate-spin: spin 1s linear infinite;', '--initial: initial;', '--color-primary: var(--color-red-500);',
  'padding: calc(var(--spacing) * 4);', '--tw-shadow: 0 0 color-mix(in oklab, var(--color-current) 50%, transparent);',
  '--color-mixed: color-mix(in lab, var(--color-red-500) 50%, var(--color-blue-500));',
]
const OPTIMIZE_AT_RULES = [
  '@x-context theme', '@x-context theme', '@x-context reference', '@x-context theme reference',
  '@x-context theme-false', '@x-context keyframes', '@x-context supports-color-mix', '@x-at-root', '@x-at-root',
  '@layer theme', '@layer properties',
]
const OPTIMIZE_LEAF_AT_RULES = [
  ['@keyframes ping', ['75%, 100% { transform: scale(2); opacity: 0; }']],
  ['@keyframes pulse', ['50% { opacity: 0.5; color: color-mix(in oklab, var(--color-red-500) 50%, transparent); }']],
  ['@property --tw-a', ['syntax: "*";', 'inherits: false;']],
  ['@property --tw-b', ['syntax: "<length>";', 'inherits: true;', 'initial-value: 0px;']],
]

function randomCss(random, optimize) {
  let lines = []
  let budget = 30

  function block(depth, inRule) {
    let pad = '  '.repeat(depth)
    let count = random.int(4) + (depth === 0 ? 1 : 0)
    for (let i = 0; i < count && budget > 0; i++) {
      budget--
      let roll = random.next()
      if (optimize && random.chance(0.3)) {
        if (roll < 0.45 && depth > 0) {
          lines.push(pad + random.pick(OPTIMIZE_DECLARATIONS))
        } else if (roll < 0.85 && depth < 5) {
          lines.push(`${pad}${random.pick(OPTIMIZE_AT_RULES)} {`)
          block(depth + 1, inRule)
          lines.push(`${pad}}`)
        } else {
          let [prelude, body] = random.pick(OPTIMIZE_LEAF_AT_RULES)
          lines.push(`${pad}${prelude} {`)
          for (let line of body) if (random.chance(0.9)) lines.push(`${pad}  ${line}`)
          lines.push(`${pad}}`)
        }
      } else if (roll < 0.3 && (depth > 0 || random.chance(0.1))) {
        lines.push(pad + random.pick(DECLARATIONS))
      } else if (roll < 0.33) {
        lines.push(`${pad}/*! note ${random.int(3)} */`)
      } else if (roll < 0.65 && depth < 5) {
        let selector = inRule || random.chance(0.15) ? random.pick(NESTED_SELECTORS) : random.pick(TOP_SELECTORS)
        if (random.chance(0.15)) selector += ', ' + random.pick(inRule ? NESTED_SELECTORS : TOP_SELECTORS)
        lines.push(`${pad}${selector} {`)
        block(depth + 1, true)
        lines.push(`${pad}}`)
      } else if (roll < 0.87 && depth < 5) {
        lines.push(`${pad}${random.pick(BLOCK_AT_RULES)} {`)
        block(depth + 1, inRule)
        lines.push(`${pad}}`)
      } else if (roll < 0.93) {
        let [prelude, body] = random.pick(LEAF_AT_RULES)
        lines.push(`${pad}${prelude} {`)
        for (let line of body) if (random.chance(0.8)) lines.push(`${pad}  ${line}`)
        lines.push(`${pad}}`)
      } else {
        lines.push(pad + random.pick(STATEMENTS))
      }
    }
  }

  block(0, false)
  return lines.join('\n') + '\n'
}

// ---------------------------------------------------------------------------
// The inputs of `optimizeAst`
// ---------------------------------------------------------------------------

const THEME = `@theme {
  --color-red-500: #ef4444;
  --color-blue-500: oklch(0.6 0.2 250);
  --color-primary: var(--color-red-500);
  --color-secondary: var(--color-primary);
  --color-current: currentcolor;
  --color-current-upper: CurrentColor;
  --color-loop-a: var(--color-loop-b);
  --color-loop-b: var(--color-loop-a);
  --color-fallback: var(--undefined-thing, red);
  --color-empty: ;
  --color-two: var(--color-red-500) var(--color-blue-500);
  --spacing: 0.25rem;
  --font-sans: ui-sans-serif, system-ui;
  --animate-spin: spin 1s linear infinite;
  --animate-ping: ping 1s cubic-bezier(0, 0, 0.2, 1) infinite;
  --animate-both: spin 1s,pulse 2s;
  @keyframes spin { to { transform: rotate(360deg); } }
  @keyframes ping { 75%, 100% { transform: scale(2); opacity: 0; } }
  @keyframes pulse { 50% { opacity: 0.5; } }
}
@theme static {
  --color-static: #000;
  --animate-static: wiggle 1s;
}
@theme inline {
  --color-inline: var(--color-red-500);
}
@theme reference {
  --color-ref: #111;
}
`
const THEME_PREFIX = `@theme prefix(tw) {
  --color-red-500: #ef4444;
  --color-blue-500: oklch(0.6 0.2 250);
  --color-primary: var(--tw-color-red-500);
  --animate-spin: spin 1s linear infinite;
  --animate-ping: ping 1s linear infinite;
  @keyframes spin { to { transform: rotate(360deg); } }
}
@theme prefix(tw) static {
  --color-static: #000;
}
`
const THEME_EMPTY = ''

// The theme rule as the compiler makes it: the variables in a theme context,
// and each `@keyframes` rule in an at-root.
const THEME_RULE = `@layer theme {
  @x-context theme {
    :root, :host {
      --color-red-500: #ef4444;
      --color-blue-500: oklch(0.6 0.2 250);
      --color-primary: var(--color-red-500);
      --color-secondary: var(--color-primary);
      --color-current: currentcolor;
      --color-loop-a: var(--color-loop-b);
      --color-loop-b: var(--color-loop-a);
      --color-fallback: var(--undefined-thing, red);
      --spacing: 0.25rem;
      --font-sans: ui-sans-serif, system-ui;
      --animate-spin: spin 1s linear infinite;
      --animate-ping: ping 1s cubic-bezier(0, 0, 0.2, 1) infinite;
      --animate-both: spin 1s,pulse 2s;
      --color-static: #000;
      --animate-static: wiggle 1s;
      --not-in-theme: 1;
      --gone: initial;
    }
  }
}
@x-context theme { @x-at-root { @keyframes spin { to { transform: rotate(360deg); } } } }
@x-context theme { @x-at-root { @keyframes ping { 75%, 100% { transform: scale(2); opacity: 0; } } } }
@x-context theme { @x-at-root { @keyframes pulse { 50% { opacity: 0.5; } } } }
@x-context theme { @x-at-root { @keyframes wiggle { 50% { rotate: 3deg; } } } }
`
const THEME_RULE_PREFIX = `@layer theme {
  @x-context theme {
    :root, :host {
      --tw-color-red-500: #ef4444;
      --tw-color-blue-500: oklch(0.6 0.2 250);
      --tw-color-primary: var(--tw-color-red-500);
      --tw-animate-spin: spin 1s linear infinite;
      --tw-animate-ping: ping 1s linear infinite;
      --tw-color-static: #000;
      --color-red-500: no-prefix;
    }
  }
}
@x-context theme { @x-at-root { @keyframes spin { to { transform: rotate(360deg); } } } }
@x-context theme { @x-at-root { @keyframes ping { to { opacity: 0; } } } }
`

// Each case: [theme, css, used]. Each case runs with each value of `Polyfills`.
const OPTIMIZE_CASES = [
  // The theme variables.
  [THEME, THEME_RULE, []],
  [THEME, THEME_RULE + '.a { color: var(--color-red-500); }', []],
  [THEME, THEME_RULE + '.a { color: var(--color-secondary); }', []],
  [THEME, THEME_RULE + '.a { color: var(--color-loop-a); }', []],
  [THEME, THEME_RULE + '.a { margin: calc(var(--spacing) * 2); font-family: var(--font-sans, var(--color-blue-500)); }', []],
  [THEME, THEME_RULE, ['var(--color-primary)']],
  [THEME, THEME_RULE, ['var(--color-blue-500)', 'var(--animate-both)']],
  [THEME, THEME_RULE, ['var(--animate-spin)', 'calc(var(--spacing) * var(--color-red-500))']],
  [THEME, THEME_RULE + '.a { animation: var(--animate-ping); }', []],
  [THEME, THEME_RULE + '.a { animation: ping 1s, pulse 2s; }', []],
  [THEME, THEME_RULE + '.a { animation:   spin\t1s ,wiggle; }', []],
  [THEME, THEME_RULE + '.a { --my-animation: spin 1s; animation-name: ping; }', []],
  [THEME_PREFIX, THEME_RULE_PREFIX, []],
  [THEME_PREFIX, THEME_RULE_PREFIX + '.a { color: var(--tw-color-primary); }', []],
  [THEME_PREFIX, THEME_RULE_PREFIX + '.a { color: var(--color-red-500); animation: var(--tw-animate-spin); }', []],
  [THEME_PREFIX, THEME_RULE_PREFIX, ['var(--tw-animate-ping)', 'var(--tw-color-red-500)']],
  [THEME_PREFIX, THEME_RULE_PREFIX + '.a { color: color-mix(in oklab, var(--tw-color-red-500) 50%, transparent); background: color-mix(in oklab, var(--color-red-500) 50%, transparent); }', []],
  // Empty parents after the removal of the variables.
  [THEME, '@layer theme { @x-context theme { :root { --color-red-500: red; } } .other { color: blue; } } .after { color: red; }', []],
  [THEME, '@layer a { @layer b { @x-context theme { :root { --color-red-500: red; } :host { --color-blue-500: blue; } } } .keep { --x: 1; } }', []],
  [THEME, '@layer a { .wrap { @x-context theme { :root { --color-red-500: red; } } color: red; } }', []],
  [THEME, '@layer a { @x-context source-user { @x-context theme { :root { --color-red-500: red; } } } }', []],
  [THEME, '@media print { @x-context theme { :root { --color-red-500: red; } } }', []],
  [THEME, '@x-context theme { :root { --color-red-500: red; } --color-blue-500: blue; }', []],
  [THEME, '@x-context theme { --color-red-500: red; --color-blue-500: blue; }', ['var(--color-blue-500)']],
  [THEME, '@x-context theme { @layer theme { --color-red-500: red; } @media print { --color-blue-500: blue; } }', []],
  [THEME, '@x-context theme { :root { --color-red-500: red; color: var(--color-blue-500); } :host { --color-blue-500: blue; } }', []],
  [THEME, '@x-context theme { :root { --color-red-500: red; /*! keep */ } }', []],
  [THEME, '@x-context theme { @x-at-root { --color-red-500: red; :root { --color-blue-500: blue; } } }', []],
  [THEME, '@x-context theme { :root { --a: var(--b); --b: var(--c); --c: var(--a); --d: var(--color-red-500); --e: var(--d) var(--f); --f: 1; } } .x { color: var(--e); }', []],
  [THEME, '@x-context theme { :root { --color-red-500: var(--p) var(--q); --p: var(--z); --q: var(--z); --z: 1; --color-blue-500: var(--color-red-500); } }', []],
  [THEME, '@x-context theme { :root { --color-red-500: var(--spacing); --spacing: var(--color-red-500); --font-sans: var(--color-red-500); } }', []],
  [THEME, '@x-context theme { :root { --color-red-500: initial; --color-blue-500: initial; } } @x-context theme { :host { --spacing: initial; color: initial; } }', []],
  [THEME, '@x-context theme { @keyframes spin { from { --color-red-500: red; } } :root { --color-red-500: red; } }', []],
  // The keyframes.
  [THEME, '@x-context theme { @x-at-root { @keyframes spin { to { rotate: 1turn; } } } } .last { --x: 1; } @x-at-root { .root { --y: 1; } }', []],
  [THEME, '@x-context theme { @keyframes spin { to { rotate: 1turn; } } } @x-at-root { .one { --y: 1; } .two { --z: 1; } }', []],
  [THEME, '@x-context theme { @keyframes a { } @keyframes b { } } @x-at-root { .one { --y: 1; } .two { --z: 1; } .three { --z: 1; } }', []],
  [THEME, '@x-context theme { @x-at-root { @keyframes spin { to { rotate: 1turn; } } @keyframes spin { to { rotate: 2turn; } } @keyframes other { to { rotate: 2turn; } } } } .a { animation: spin; }', []],
  [THEME, '@x-context theme { @x-at-root { .wrap { @keyframes spin { to { rotate: 1turn; } } } .end { --x: 1; } } }', []],
  [THEME, '@x-at-root { @keyframes spin { to { rotate: 1turn; } } } @keyframes ping { to { rotate: 1turn; } } @x-context theme { @x-at-root { @keyframes { to { rotate: 1turn; } } } }', []],
  [THEME, '@x-context theme { @x-at-root { @keyframes spin { to { rotate: 1turn; } } } :root { --animate-spin: spin 1s; --animate-x: ping 1s; } @x-at-root { @keyframes ping { to { scale: 2; } } } }', ['var(--animate-spin)']],
  [THEME, '@x-context theme { @x-at-root { --animate-spin: spin 1s; @keyframes spin { to { rotate: 1turn; } } } }', ['var(--animate-spin)']],
  // `@property`.
  [THEME, '@property --tw-a { syntax: "*"; inherits: false; } @property --tw-b { syntax: "<length>"; inherits: true; initial-value: 0px; } @property --tw-a { syntax: "*"; inherits: true; }', []],
  [THEME, '/*! license */ @charset "UTF-8"; @import "a.css"; @layer x; .a { --x: 1; } @property --tw-a { syntax: "*"; inherits: false; initial-value: 1; initial-value: 2; }', []],
  [THEME, '/*! license */ @import "a.css"; @x-at-root { @property --tw-c { inherits: true; inherits: false; inherits: true; } }', []],
  [THEME, '.a { @property --tw-nested { syntax: "*"; inherits: false; } --x: 1; } @x-context theme { @property --tw-d { syntax: "*"; inherits: TRUE; initial-value: var(--color-red-500); } }', []],
  [THEME, '@property --tw-e { @x-undefined initial-value; inherits: false; --tw-sort: 1; } @property --tw-f { initial-value: 5; @x-undefined initial-value; inherits: true } @property --tw-g { }', []],
  [THEME, '@property --tw-a { syntax: "<color>"; inherits: false; initial-value: color-mix(in oklab, var(--color-red-500) 50%, transparent); } .a { --x: 1; }', []],
  [THEME, '@layer properties { .x { --y: 1; } } @property --tw-a { inherits: false; } @layer properties;', []],
  // `color-mix(…)`.
  [THEME, '.a { color: color-mix(in oklab, var(--color-red-500) 50%, transparent); }', []],
  [THEME, '.a { color: color-mix(in oklch, var(--color-red-500) 50%, var(--color-blue-500)); background: color-mix(in lab, var(--color-primary), red); fill: color-mix(in lch, var(--color-secondary) 10%, red); stroke: color-mix(in srgb, var(--color-red-500), red); outline-color: color-mix(in hsl, var(--color-red-500), red); }', []],
  [THEME, '.a { color: color-mix(in oklab, currentcolor 50%, transparent); background: color-mix(in oklab, CURRENTCOLOR 50%, transparent); fill: color-mix(in oklab, var(--color-current) 50%, transparent); stroke: color-mix(in oklab, var(--color-current-upper), red); }', []],
  [THEME, '.a { color: color-mix(in oklab, var(--unknown) 50%, transparent); background: color-mix(in oklab, var(--color-loop-a) 50%, transparent); fill: color-mix(in oklab, var(--color-fallback), red); stroke: color-mix(in oklab, var(--color-empty), red); }', []],
  [THEME, '.a { color: color-mix(var(--color-red-500) 50%, blue); background: color-mix(red, var(--color-red-500)); fill: color-mix(IN oklab, var(--color-red-500), blue); stroke: color-mix(  in\toklab, var(--color-red-500), blue); outline-color: color-mix(inoklab, var(--color-red-500), blue); }', []],
  [THEME, '.a { --a: color-mix(/* c, 1 */ in oklab, var(--color-red-500) 50%, transparent); --b: color-mix(in /* c */ oklab, var(--color-red-500) /* x */ 50%, transparent /* y */); --c: color-mix(/* open in oklab, var(--color-red-500), red); --d: color-mix(/**/in/**/oklab, var(--color-red-500), red); --e: color-mix(/*/ in oklab, var(--color-red-500), red); }', []],
  [THEME, '.a { @supports (color: color-mix(in lab, red, red)) { color: color-mix(in oklab, var(--color-red-500) 50%, transparent); } @supports (display: grid) { color: color-mix(in oklab, var(--color-red-500) 50%, transparent); } }', []],
  [THEME, '@keyframes k { from { color: color-mix(in oklab, var(--color-red-500) 50%, transparent); } } .a { @media print { color: color-mix(in oklab, var(--color-red-500) 50%, transparent) !important; } }', []],
  [THEME, 'color: color-mix(in oklab, var(--color-red-500) 50%, transparent); .a { --x: 1; } @x-at-root { color: color-mix(in oklab, var(--color-red-500) 50%, transparent); .b { color: color-mix(in oklab, var(--color-red-500) 50%, transparent); } }', []],
  [THEME, '.a { box-shadow: 0 0 color-mix(in oklab, var(--color-red-500) 50%, transparent), 0 0 color-mix(in oklab, currentcolor 50%, transparent), 0 0 color-mix(in oklch, var(--color-blue-500), red); }', []],
  [THEME, '.a { color: color-mix(in oklab, color-mix(in oklch, var(--color-red-500), var(--color-blue-500)) 50%, transparent); background: color-mix(in oklab, color-mix(in oklab, currentcolor, red) 50%, var(--color-red-500)); fill: color-mix(in oklab, color-mix(var(--color-red-500), red), blue); }', []],
  [THEME, '.a { color: color-mix(in oklab, red 50%, blue); background: color-mix(in oklab, var(--color-red-500, var(--color-blue-500)) 50%, blue); fill: color-mix(in oklab, var(--unknown, var(--color-blue-500)) 50%, blue); stroke: color-mix(in oklab, var() 50%, blue); outline-color: color-mix(in oklab, var(  --color-red-500  ), red); }', []],
  [THEME, '.a { color: color-mix(in oklab, currentcolor); background: color-mix(in oklab currentcolor); fill: color-mix(in oklab, var(--unknown)); stroke: color-mix(in oklab,, var(--unknown)); outline-color: color-mix(in oklab, var(--unknown) 50%,); border-color: color-mix(in oklab , currentcolor , red); }', []],
  [THEME, '.a { color: color-mix(in oklab, var(--color-inline), red); background: color-mix(in oklab, var(--color-ref), red); fill: color-mix(in oklab, var(--color-static), red); stroke: color-mix(in oklab, var(--color-two), red); outline-color: color-mix(in oklab, var(--color-secondary), var(--color-secondary)); }', []],
  [THEME, '.a { color: color-mix(in oklab, var(--color-red-500) 50%, transparent); color: color-mix(in oklab, var(--color-red-500) 50%, transparent); .b { color: color-mix(in oklab, var(--color-blue-500) 50%, transparent); } margin: 0; }', []],
  [THEME, '@x-context theme { :root { --color-mixed: color-mix(in oklab, var(--color-red-500) 50%, transparent); --color-red-500: color-mix(in oklab, var(--color-blue-500) 50%, transparent); } }', ['var(--color-red-500)']],
  [THEME, '@x-context supports-color-mix { .a { color: color-mix(in oklab, var(--color-red-500), red); } } @x-context keyframes { .b { color: color-mix(in oklab, var(--color-red-500), red); } } @supports (x: color-mix()) { @x-context theme-false { .c { color: color-mix(in oklab, var(--color-red-500), red); } } }', []],
  [THEME, '.a { @x-context source-variant { color: color-mix(in oklab, var(--color-red-500), red); @scope (.x) { color: color-mix(in oklab, var(--color-blue-500), red); } } }', []],
  [THEME_EMPTY, '.a { color: color-mix(in oklab, var(--color-red-500) 50%, transparent); background: color-mix(in oklab, currentColor 50%, transparent); fill: color-mix(in oklab, red, blue); }', []],
  // Reference contexts, at-roots, `--tw-sort`, empty nodes.
  [THEME, '@x-context reference { .a { color: red; } @property --tw-ref { inherits: false; } } .b { @x-context reference { color: var(--color-red-500); } --x: 1; } ' + THEME_RULE, []],
  [THEME, '.a { --tw-sort: 1; color: red; @x-undefined margin; } .b { --tw-sort: 2; } .c { @x-undefined color; } .d { .e { --tw-sort: 3; } }', []],
  [THEME, '.a { @x-at-root { .b { --x: 1; } @media print { .c { --y: 1; } } } --z: 1; } @x-at-root { @x-at-root { .d { --w: 1; } } .e { --v: 1; } } .f { --u: 1; }', []],
  [THEME, '.a { } @media print { } @layer x { } @layer; @charset "UTF-8"; @custom-media --m (color); @namespace svg url(x); @import "a.css"; @apply; @supports (a: b) { .x { } } @container { } @unknown { } @font-face { } @layer y { .z { } }', []],
  [THEME, '.a { @apply flex; } .b { @layer q; } .c { @media print; } @x-context source-user { } @x-context source-user { .d { } } @x-context source-variant { .e { --x: 1; } }', []],
  [THEME, '.a { @x-context source-variant { @scope (.from) to (.to) { color: red; } } } .b { @x-context source-user { @scope (.from) { color: red; } } }', []],
  [THEME, '.a { @x-context theme { --color-red-500: red; .b { --color-blue-500: blue; } } color: red; } .c { @x-context theme { --color-red-500: red; } }', []],
]

// ---------------------------------------------------------------------------
// The records
// ---------------------------------------------------------------------------

let nestingInputs = [
  ...cssLiterals('ast.test.ts'),
  ...tableRows(),
  ...cssLiterals('index.test.ts'),
  ...EXTRA_NESTING,
]
{
  let random = createRandom(4350)
  for (let i = 0; i < 500; i++) nestingInputs.push(randomCss(random, false))
}

let nesting = []
let seen = new Set()
for (let css of nestingInputs) {
  if (seen.has(css)) continue
  seen.add(css)
  let ast = tryParse(css)
  if (ast === null) continue
  nesting.push({ css, tree: encode(Ast.handleNesting(ast)) })
}

let themes = []
let themeIndexes = new Map()
async function themeIndex(theme) {
  if (!themeIndexes.has(theme)) {
    let designSystem = await Tailwind.__unstable__loadDesignSystem(theme)
    let entries = []
    for (let [key, value] of designSystem.theme.values) entries.push([key, value.value, value.options])
    themeIndexes.set(theme, themes.length)
    themes.push({ css: theme, prefix: designSystem.theme.prefix ?? null, entries })
  }
  return themeIndexes.get(theme)
}

let optimize = []
async function record(theme, css, used, polyfills) {
  let ast = tryParse(css)
  if (ast === null) throw new Error(`The input has a syntax error: ${css}`)
  // Each case has a new design system: `optimizeAst` marks variables as used.
  let designSystem = await Tailwind.__unstable__loadDesignSystem(theme)
  for (let raw of used) designSystem.trackUsedVariables(raw)
  let tree = encode(Ast.optimizeAst(ast, designSystem, polyfills))
  optimize.push({ theme: await themeIndex(theme), used, polyfills, css, tree })
}

for (let [theme, css, used] of OPTIMIZE_CASES) {
  for (let polyfills of [0, 1, 2, 3]) await record(theme, css, used, polyfills)
}
// The inputs of ast.test.ts and the extra inputs, with no theme.
for (let css of [...cssLiterals('ast.test.ts'), ...EXTRA_NESTING]) {
  if (tryParse(css) === null) continue
  await record(THEME_EMPTY, css, [], 3)
}
{
  let random = createRandom(9120)
  const USED = ['var(--color-primary)', 'var(--animate-spin)', 'var(--color-blue-500)', 'var(--spacing)']
  for (let i = 0; i < 400; i++) {
    let css = randomCss(random, true)
    let used = USED.filter(() => random.chance(0.2))
    await record(THEME, css, used, random.int(4))
  }
}

fs.mkdirSync(path.dirname(outFile), { recursive: true })
fs.writeFileSync(outFile, JSON.stringify({ themes, nesting, optimize }) + '\n')
console.log(`optimize.json: ${nesting.length} nesting cases, ${optimize.length} optimize cases`)
