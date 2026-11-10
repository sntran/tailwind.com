#!/usr/bin/env node
// Usage: node gen-css-utility.mjs <path of the upstream clone> <output file>
// This script writes the fixture of the `@utility` tests (css_utility.json).
//
// A case has a CSS input (`@theme` rules and `@utility` rules) and a list of
// candidates. For each candidate the fixture has the rules before the last
// pass of the compiler: `parseCandidate`, `compileAstNodes`, then `toCss` of
// each rule.
//
// The cases come from two sources:
// 1. Each template literal with `@utility` in `utilities.test.ts` and
//    `index.test.ts` of the clone.
// 2. The list EXTRA_CASES of this file.
//
// The script adds generated candidates for each utility name of a case.
// A candidate is not in the fixture when it has a variant, or when a
// utility of Tailwind CSS itself gives rules for it. In a case, `rules` has
// the candidates with rules (the CSS, the property order and the count of
// declarations of each rule) and `none` has the candidates with no rule,
// with a space between them. A case is not in the
// fixture when a `@utility` rule has `@apply` or `@variant` in it.
import fs from 'node:fs'
import { createRequire } from 'node:module'
import path from 'node:path'
import { loadUpstream } from './lib.mjs'

let [clone, outFile] = process.argv.slice(2)
if (!clone || !outFile) {
  console.error('Usage: node gen-css-utility.mjs <path of the upstream clone> <output file>')
  process.exit(1)
}
clone = path.resolve(clone)
let up = await loadUpstream(clone)
let require = createRequire(path.join(clone, 'packages/@tailwindcss-node/package.json'))
let { createJiti } = require('jiti')
let src = path.join(clone, 'packages/tailwindcss/src/')
let jiti = createJiti(src, { moduleCache: true })
let { toCss } = await jiti.import(path.join(src, 'ast.ts'))
let { parse } = await jiti.import(path.join(src, 'css-parser.ts'))
let { unescape } = await jiti.import(path.join(src, 'utils/escape.ts'))
let { createCssUtility, isValidStaticUtilityName, isValidFunctionalUtilityName } = await jiti.import(
  path.join(src, 'utilities.ts'),
)

// Upstream gives warnings for some cases. They are not a part of the fixture.
console.warn = () => {}

// ---------------------------------------------------------------------------
// Names for the two name checks.

const NAMES = [
  'foo', 'foo-123', 'foo-2.5', '-foo', 'foo-bar', 'foo_bar', 'foo-50%', 'foo-1/2', 'foo-sm/8',
  'foo-4/snug', 'foo_', 'foo/bar', 'Foo', '-Foo', 'foo-', 'foo-1/', 'foo-p%', 'foo.bar', 'foo-1..5',
  'foo..bar', 'foo/bar/baz', 'foo-*', 'foo--*', '-foo-*', 'foo-bar-*', 'foo_bar-*', 'Foo-*', '-Foo-*',
  'foo!-*', 'foo-[…]', '', '-', '--', '-*', '--*', '*', 'a', 'a-*', 'z9', '9z', '-9', 'foo*', 'foo-**',
  'foo-*-*', 'my-*-utility', 'foo-*-bar', 'foo-1.', 'foo-1.5', 'foo-.5', 'foo.5', 'foo1.5', 'foo-1.5.5',
  'foo-1.a', 'foo-a.1', 'foo%', 'foo-5%', 'foo-5%%', 'foo-5%5', 'foo-%', 'foo5%', 'foo/', '/foo', 'foo//bar',
  'foo/bar/', 'foo/1.5', 'foo/50%', 'foo-1/2/3', 'foo bar', 'foo\tbar', 'foo:bar', 'foo[bar]', 'foo(bar)',
  'foo#bar', 'foo@bar', 'foo!', '!foo', 'foo+bar', 'foo,bar', 'foo~bar', 'fooé', 'éfoo', '💨', 'foo-💨',
  'foo-💨-*', 'fooBar', 'foo-Bar', 'fOO', 'foo__bar', 'foo--bar', 'foo---', '-foo-', '--foo', '-foo--*',
  '_foo', 'foo-_', 'foo-1_000', 'ui/button', 'ui/button/sm', 'push-1/2', 'push-50%', 'foo-1/.5', 'foo-1./2',
  'foo.', 'foo-1.%', 'foo-1%.', 'foo/-', 'foo/_', 'foo-/bar', 'foo-*x', 'x-*-', 'foo-\\*', 'foo.-*', 'foo/1-*',
  'foo%-*', 'tab-size-*', 'tab-size-[…]-*', ' foo', 'foo ', 'foo-* ', '-a-b-c-*', 'a1-*', '1a-*', '-1-*',
]

// ---------------------------------------------------------------------------
// The cases of the upstream tests.

function evalTemplate(body) {
  if (body.includes('${')) return null
  return new Function('return `' + body + '`')()
}

function extractCases(file) {
  let source = fs.readFileSync(path.join(src, file), 'utf8')

  // The start of each test.
  let starts = []
  for (let match of source.matchAll(/^[ \t]*(?:test|it)(?:\.\w+)?\(/gm)) starts.push(match.index)

  // Each template literal. The tests do not nest them.
  let templates = []
  for (let i = 0; i < source.length; i++) {
    let c = source[i]
    if (c === '/' && source[i + 1] === '/') {
      while (i < source.length && source[i] !== '\n') i++
    } else if (c === "'" || c === '"') {
      let j = i + 1
      while (j < source.length && source[j] !== c && source[j] !== '\n') j += source[j] === '\\' ? 2 : 1
      i = j
    } else if (c === '`') {
      let j = i + 1
      while (j < source.length && source[j] !== '`') j += source[j] === '\\' ? 2 : 1
      templates.push({ start: i, end: j + 1 })
      i = j
    }
  }

  let cases = []
  let counter = new Map()
  for (let template of templates) {
    let css = evalTemplate(source.slice(template.start + 1, template.end - 1))
    if (css === null || !css.includes('@utility')) continue

    let blockStart = starts.filter((s) => s < template.start).pop() ?? 0
    let blockEnd = starts.find((s) => s > template.start) ?? source.length

    // The text of the test, without its template literals.
    let text = ''
    let pos = blockStart
    for (let other of templates) {
      if (other.end <= blockStart || other.start >= blockEnd) continue
      text += source.slice(pos, other.start)
      pos = other.end
    }
    text += source.slice(pos, blockEnd)

    let title = /(?:test|it)(?:\.\w+)?\(\s*(['"])((?:\\.|(?!\1).)*)\1/.exec(text)?.[2] ?? ''
    let candidates = []
    for (let match of text.matchAll(/'((?:\\.|[^'\\\n])*)'|"((?:\\.|[^"\\\n])*)"/g)) {
      let value = new Function('return ' + match[0])()
      if (value === '' || /\s/.test(value)) continue
      candidates.push(value)
    }

    let name = `${file}: ${title}`
    let count = (counter.get(name) ?? 0) + 1
    counter.set(name, count)
    cases.push({ name: count > 1 ? `${name} (${count})` : name, css, candidates })
  }
  return cases
}

// ---------------------------------------------------------------------------
// More cases.

const THEME = `
@theme {
  --spacing: 0.25rem;
  --color-red-500: #ef4444;
  --color-blue: blue;
  --tab-size-github: 8;
  --tab-size-deep-nested: 16;
  --text-sm: 0.875rem;
  --text-sm--line-height: 1.25rem;
  --text-lg: 1.125rem;
  --text-lg--line-height: 1.75rem;
  --text-lg--letter-spacing: 0.01em;
  --leading-tight: 1.25;
  --leading-6: 1.5rem;
  --opacity-half: 0.5;
  --value-2\\.5: two-and-a-half;
  --value-1\\/2: one-half;
  --empty-none: ;
}
@theme inline {
  --inline-a: 1px;
  --inline-a--extra: 2px;
}
`

let extra = (name, body, candidates = []) => ({ name: `extra: ${name}`, css: THEME + body, candidates })

const EXTRA_CASES = [
  extra('static', '@utility foo { color: red; }', ['foo', 'foo-bar', 'foo-1', 'foo/50', 'foo-[1]', 'bar']),
  extra('static with nested rules', '@utility foo { color: red; &:hover { color: blue; } @media (min-width: 1px) { color: green; .x & { y: z } } }'),
  extra('static with custom properties', '@utility foo { --a: { b: c }; --d: 1; --e: --value(integer); }'),
  extra('static keeps value functions', '@utility foo { a: --value(integer); b: --modifier(integer); }'),
  extra('static with important declaration', '@utility foo { a: b !important; c: d; }'),
  extra('static with comment and at-rules', '@utility foo { /* text */ a: b; @supports (a: b) { c: d; } @starting-style { e: f; } }'),
  extra('static with a dot', '@utility foo-1\\.5 { a: b; }', ['foo-1.5', 'foo-1', 'foo-1.50']),
  extra('static with a percentage', '@utility foo-50\\% { a: b; }', ['foo-50%', 'foo-50']),
  extra('static with a slash', '@utility foo\\/bar { a: b; } @utility foo-1\\/2 { c: d; }', ['foo/bar', 'foo-1/2', 'foo', 'foo-1']),
  extra('static with upper case and underscore', '@utility fooBar_baz { a: b; }', ['fooBar_baz', 'foobar_baz']),
  extra('static negative', '@utility -foo { a: b; } @utility -foo-bar { c: d; }', ['-foo', '-foo-bar', 'foo', 'foo-bar']),
  extra('static defined two times', '@utility foo { a: b; } @utility foo { c: d; }'),
  extra('static and functional with the same root', '@utility foo { a: b; } @utility foo-* { c: --value(integer); } @utility foo-1 { e: f; }'),
  extra('static with theme function', '@utility foo { a: theme(--color-blue); b: --theme(--text-sm); c: --spacing(4); d: --alpha(red / 50%); }'),
  extra('static with a ratio name', '@utility aspect-foo-16\\/9 { aspect-ratio: 16 / 9; }', ['aspect-foo-16/9']),

  extra('invalid: upper case', '@utility Foo { a: b; }'),
  extra('invalid: ends with dash', '@utility foo- { a: b; }'),
  extra('invalid: star in the middle', '@utility foo-*-bar { a: b; }'),
  extra('invalid: star with no dash', '@utility foo* { a: b; }'),
  extra('invalid: two slashes', '@utility a\\/b\\/c { a: b; }'),
  extra('invalid: dot', '@utility foo\\.bar { a: b; }'),
  extra('invalid: percentage', '@utility foo-p\\% { a: b; }'),
  extra('invalid: functional with special character', '@utility foo\\!-* { a: --value(integer); }'),
  extra('invalid: one valid and one invalid', '@utility foo { a: b; } @utility Bar-* { a: --value(integer); }'),
  extra('invalid: empty body', '@utility foo { }'),
  extra('invalid: functional upper case root', '@utility -Foo-* { a: --value(integer); }'),

  extra('no value function', '@utility foo-* { a: b; }'),
  extra('only a modifier function', '@utility foo-* { a: --modifier(integer); }'),
  extra('integer', '@utility foo-* { a: --value(integer); }'),
  extra('number', '@utility foo-* { a: --value(number); }'),
  extra('percentage', '@utility foo-* { a: --value(percentage); }'),
  extra('ratio', '@utility foo-* { a: --value(ratio); }'),
  extra('ratio and integer in one function', '@utility foo-* { a: --value(ratio, integer); }'),
  extra('integer and ratio in one function', '@utility foo-* { a: --value(integer, ratio); }'),
  extra('ratio and number in two declarations', '@utility foo-* { a: --value(number); b: --value(ratio); c: d; }'),
  extra('ratio and theme in two declarations', '@utility foo-* { a: --value(--value-*); b: --value(ratio); }'),
  extra('ratio in nested rule', '@utility foo-* { a: --value(integer); &:hover { b: --value(ratio); c: --value(integer); } }'),
  extra('ratio with modifier function', '@utility foo-* { a: --value(ratio); b: --modifier(integer); }'),
  extra('ratio and integer with modifier function', '@utility foo-* { a: --value(ratio); a: --value(integer); b: --modifier(integer); }'),
  extra('ratio and integer in one declaration', '@utility foo-* { a: --value(ratio) --value(integer); }'),
  extra('unsupported bare types', '@utility foo-* { a: --value(color); b: --value(length); c: --value(integer); d: --value(foo, bar, integer); }', ['foo-red', 'foo-1px', 'foo-#fff', 'foo-foo', 'foo-bar']),
  extra('all bare types', '@utility foo-* { a: --value(integer, number, percentage, ratio); }'),
  extra('arbitrary any', '@utility foo-* { a: --value([*]); }'),
  extra('arbitrary any with space', '@utility foo-* { a: --value([ * ]); }'),
  extra('arbitrary any escaped', '@utility foo-* { a: --value([\\*]); }'),
  extra('arbitrary integer', '@utility foo-* { a: --value([integer]); }'),
  extra('arbitrary length', '@utility foo-* { a: --value([length]); }'),
  extra('arbitrary color', '@utility foo-* { a: --value([color]); }'),
  extra('arbitrary percentage and number', '@utility foo-* { a: --value([percentage], [number]); }'),
  extra('arbitrary ratio', '@utility foo-* { a: --value([ratio]); }'),
  extra('arbitrary url image position', '@utility foo-* { a: --value([url]); b: --value([image]); c: --value([position]); d: --value([bg-size]); }'),
  extra('arbitrary line-width angle vector', '@utility foo-* { a: --value([line-width]); b: --value([angle]); c: --value([vector]); }'),
  extra('arbitrary family and sizes', '@utility foo-* { a: --value([family-name]); b: --value([generic-name]); c: --value([absolute-size]); d: --value([relative-size]); }'),
  extra('arbitrary unknown type', '@utility foo-* { a: --value([foo]); b: --value([]); }', ['foo-[foo:1]', 'foo-[foo]', 'foo-[:1]']),
  extra('arbitrary several types', '@utility foo-* { a: --value([integer], [length], [color]); }'),
  extra('arbitrary types in several declarations', '@utility foo-* { a: --value([integer]); b: --value([length]); c: --value([color]); d: --value([*]); }'),
  extra('literal', "@utility foo-* { a: --value('bar', 'baz'); }", ['foo-bar', 'foo-baz', 'foo-qux']),
  extra('literal double quotes', '@utility foo-* { a: --value("bar", "baz-qux"); }', ['foo-bar', 'foo-baz-qux', 'foo-baz']),
  extra('literal with other types', "@utility foo-* { a: --value('auto', integer, [length], --tab-size-*); }", ['foo-auto']),
  extra('literal number', "@utility foo-* { a: --value('1', '2.5', '50%'); }"),
  extra('literal empty', "@utility foo-* { a: --value('', integer); }"),
  extra('theme key', '@utility foo-* { a: --value(--tab-size-*); }'),
  extra('theme key with no star', '@utility foo-* { a: --value(--tab-size); }'),
  extra('theme key with space', '@utility foo-* { a: --value(--tab-size- *); }'),
  extra('theme key escaped', '@utility foo-* { a: --value(--tab-size-\\*); }'),
  extra('theme key two stars', '@utility foo-* { a: --value(--tab-size-*-*); }'),
  extra('theme key of colors', '@utility foo-* { a: --value(--color-*); }'),
  extra('theme key several', '@utility foo-* { a: --value(--color-*, --tab-size-*, --text-*); }'),
  extra('theme key unknown', '@utility foo-* { a: --value(--unknown-*); }'),
  extra('theme key inline', '@utility foo-* { a: --value(--inline-*); b: --value(--inline-*--extra); }'),
  extra('theme key with empty value', '@utility foo-* { a: --value(--empty-*); }'),
  extra('theme key with dot and slash', '@utility foo-* { a: --value(--value-*); }'),
  extra('theme key spacing namespace', '@utility foo-* { a: --value(--spacing-*); b: --value(--spacing); }'),
  extra('theme key leading', '@utility foo-* { a: --value(--leading-*, integer); }'),
  extra('sub namespace', '@utility foo-* { a: --value(--text-*); b: --value(--text-*--line-height); c: --value(--text-*--letter-spacing); }'),
  extra('sub namespace with spaces', '@utility foo-* { b: --value(--text --line-height); c: --value(--text- * --letter-spacing); d: --value(--text-\\* --line-height); }'),
  extra('sub namespace only', '@utility foo-* { b: --value(--text-*--line-height); }'),
  extra('sub namespace unknown', '@utility foo-* { b: --value(--text-*--unknown, --text-*--line-height); }'),
  extra('sub namespace three keys', '@utility foo-* { b: --value(--text --line-height --letter-spacing); }'),
  extra('sub namespace with newline', '@utility foo-* { b: --value(--text\n--line-height); c: --value(--text-*\n  --line-height); }'),
  extra('default', '@utility foo-* { a: --value(--default(1px), integer); }'),
  extra('default only', '@utility foo-* { a: --value(--default(1px)); }'),
  extra('default as it is', '@utility foo-* { a: --value(--default( --foo --bar , \\* ), integer); }'),
  extra('default with nested functions', '@utility foo-* { a: --value(--default(calc(1px + var(--x, 2px))), integer); }'),
  extra('default last', '@utility foo-* { a: --value(integer, --default(none)); }'),
  extra('default two times', '@utility foo-* { a: --value(--default(a), --default(b)); }'),
  extra('default in two declarations', '@utility foo-* { a: --value(--default(a)); b: --value(integer); }'),
  extra('default in modifier', '@utility foo-* { a: --value(integer); b: --modifier(--default(none), integer); }'),
  extra('default in modifier and value', '@utility foo-* { a: --value(--default(a), integer); b: --modifier(--default(b), integer); }'),
  extra('default empty', '@utility foo-* { a: --value(--default(), integer); }'),
  extra('modifier integer', '@utility foo-* { a: --value(integer); b: --modifier(integer); }'),
  extra('modifier number', '@utility foo-* { a: --value(integer); b: --modifier(number); }'),
  extra('modifier percentage', '@utility foo-* { a: --value(integer); b: --modifier(percentage); }'),
  extra('modifier ratio', '@utility foo-* { a: --value(integer); b: --modifier(ratio); }'),
  extra('modifier arbitrary', '@utility foo-* { a: --value(integer); b: --modifier([*]); }'),
  extra('modifier arbitrary length', '@utility foo-* { a: --value(integer); b: --modifier([length]); }'),
  extra('modifier literal', "@utility foo-* { a: --value(integer); b: --modifier('bar', 'baz'); }", ['foo-1/bar', 'foo-1/baz', 'foo-1/qux']),
  extra('modifier theme', '@utility foo-* { a: --value(integer); b: --modifier(--leading-*); }', ['foo-1/tight', 'foo-1/6', 'foo-1/7']),
  extra('modifier theme and types', '@utility foo-* { a: --value(--text-*); b: --modifier(--leading-*, integer, [*]); }', ['foo-sm/tight', 'foo-sm/6', 'foo-lg/7', 'foo-sm/[1.5]']),
  extra('modifier sub namespace', '@utility foo-* { a: --value(integer); b: --modifier(--text-*--line-height); }', ['foo-1/sm', 'foo-1/lg', 'foo-1/xl']),
  extra('modifier in the same declaration', '@utility foo-* { a: --value(integer) / --modifier(integer); }'),
  extra('modifier in the same declaration with fallback', '@utility foo-* { a: --value(integer) / --modifier(integer); a: --value(integer); }'),
  extra('modifier before value', '@utility foo-* { a: --modifier(integer); b: --value(integer); }'),
  extra('modifier in a nested rule', '@utility foo-* { a: --value(integer); & > * { b: --modifier(integer); } }'),
  extra('modifier two declarations', '@utility foo-* { a: --value(integer); b: --modifier(integer); c: --modifier([*]); }'),
  extra('value function two times in one declaration', '@utility foo-* { a: --value(integer) --value(integer); }'),
  extra('value functions of different types in one declaration', '@utility foo-* { a: --value(integer) --value([length]); }'),
  extra('value functions with theme and bare in one declaration', '@utility foo-* { a: --value(--tab-size-*) --value(--tab-size-*, integer); }'),
  extra('value function in calc', '@utility foo-* { a: calc(--value(integer) * 1px); b: calc(--value(number) * 1%); }'),
  extra('value function in nested functions', '@utility foo-* { a: min(max(calc(--value(integer) * 1px), 2px), var(--x, --value(integer))); }'),
  extra('value function in spacing', '@utility foo-* { a: --spacing(--value(integer)); b: --spacing(--value(number)); }'),
  extra('value and modifier function in spacing', '@utility foo-* { a: --spacing(--value(number)); b: --spacing(--modifier(integer)); }'),
  extra('value function in var fallback', '@utility foo-* { a: var(--x, --value(integer)); }'),
  extra('value function in custom property', '@utility foo-* { --x: --value(integer); a: var(--x); }'),
  extra('value function with important', '@utility foo-* { a: --value(integer) !important; b: c; }'),
  extra('value function with text around', '@utility foo-* { a: 1px solid --value(--color-*) ; b: "--value(integer)" --value(integer); }'),
  extra('value function with upper case', '@utility foo-* { a: --VALUE(integer); b: --value(integer); c: --Value(integer); }'),
  extra('value function with no arguments', '@utility foo-* { a: --value(); b: --value(integer); }'),
  extra('value function with spaces in arguments', '@utility foo-* { a: --value(  integer ,   [ length ] ,  --tab-size - * ); }'),
  extra('value function with trailing comma', '@utility foo-* { a: --value(integer,); b: --value(,integer); }'),
  extra('value function with nested function argument', '@utility foo-* { a: --value(calc(1px), integer); }'),
  extra('value function in a default', '@utility foo-* { a: --value(--default(--value(integer)), [*]); }'),
  extra('declaration with no function stays', '@utility foo-* { a: b; c: --value(integer); d: var(--e); }'),
  extra('declaration that does not resolve goes out', '@utility foo-* { a: --value(integer); b: --value([length]); c: --value(--tab-size-*); d: --value("x"); }', ['foo-x']),
  extra('declaration with one function that fails', '@utility foo-* { a: --value(integer) --value([length]); b: --value(integer) --modifier(integer); c: --value(integer); }'),
  extra('nested rules', '@utility foo-* { a: --value(integer); &:hover { b: --value(integer); c: --value([length]); } @media (min-width: 1px) { d: --value(integer); .x & { e: --value(--tab-size-*); } } }'),
  extra('nested rule becomes empty', '@utility foo-* { a: --value(integer); &:hover { b: --value([length]); } }'),
  extra('only nested rules', '@utility foo-* { & > * { a: --value(integer); } }'),
  extra('nested at-rules with value in params', '@utility foo-* { @media (min-width: --value(integer)) { a: --value(integer); } }'),
  extra('comment in functional', '@utility foo-* { /* text */ a: --value(integer); }'),
  extra('negative root', '@utility -foo-* { a: calc(--value(integer) * -1px); }', ['-foo-1', 'foo-1', '--foo-1']),
  extra('negative and positive root', '@utility foo-* { a: --value(integer); } @utility -foo-* { a: calc(--value(integer) * -1); }'),
  extra('root with double dash', '@utility foo--* { a: --value(integer, "bar"); }', ['foo--1', 'foo--bar', 'foo-1', 'foo---1']),
  extra('root with underscore and digits', '@utility foo_1-bar2-* { a: --value(integer); }', ['foo_1-bar2-3', 'foo_1-bar2']),
  extra('root with upper case', '@utility fooBar-* { a: --value(integer); }', ['fooBar-1', 'foobar-1']),
  extra('root with dashes', '@utility foo-bar-baz-* { a: --value(integer, --tab-size-*); }', ['foo-bar-baz-1', 'foo-bar-baz-github', 'foo-bar-1']),
  extra('two roots that overlap', '@utility foo-* { a: --value("bar-1", integer); } @utility foo-bar-* { b: --value(integer); }', ['foo-bar-1', 'foo-1', 'foo-bar-2']),
  extra('functional defined two times', '@utility foo-* { a: --value(integer); } @utility foo-* { b: --value([length]); } @utility foo-* { c: --value(integer); }'),
  extra('functional and static override', '@utility foo-* { a: --value(integer); } @utility foo-2 { b: c; }'),
  extra('tab size full', "@utility tabs-* { tab-size: --value(--tab-size-*, integer, [integer], 'auto'); }", ['tabs-github', 'tabs-deep-nested', 'tabs-auto', 'tabs-4', 'tabs-[8]', 'tabs-[integer:var(--x)]', 'tabs-(integer:--x)', 'tabs-(--x)']),
  extra('opacity like', '@utility op-* { opacity: calc(--value(integer) * 1%); opacity: --value(--opacity-*, [percentage], [number]); }', ['op-50', 'op-half', 'op-[50%]', 'op-[0.5]', 'op-[var(--x)]']),
  extra('aspect like', "@utility asp-* { aspect-ratio: --value(ratio, [ratio], 'auto'); }", ['asp-16/9', 'asp-[4/3]', 'asp-auto', 'asp-16', 'asp-0/0', 'asp-1/0', 'asp-01/2', 'asp-1.5/2', 'asp-[16_/_9]']),
  extra('text like with modifier', '@utility txt-* { font-size: --value(--text-*, [length]); line-height: --value(--text-*--line-height); line-height: --modifier(--leading-*, number, [*]); }', ['txt-sm', 'txt-lg', 'txt-sm/tight', 'txt-sm/6', 'txt-sm/1.5', 'txt-sm/[20px]', 'txt-[12px]', 'txt-[12px]/tight', 'txt-xl']),
  extra('color like with alpha', '@utility clr-* { color: --alpha(--value(--color-*, [color]) / --modifier(integer, --default(100))%); }', ['clr-red-500', 'clr-blue', 'clr-blue/50', 'clr-[#fff]', 'clr-[#fff]/20']),
  extra('margin like with spacing', '@utility mrg-* { margin: --spacing(--value(number)); margin: --value([length], [percentage]); } @utility -mrg-* { margin: --spacing(--value(number) * -1); margin: calc(--value([length], [percentage]) * -1); }', ['mrg-4', 'mrg-2.5', 'mrg-2.3', 'mrg-[4px]', '-mrg-4', '-mrg-[4px]', 'mrg-[10%]']),
  extra('arbitrary value with theme function', '@utility foo-* { a: --value([*]); }', ['foo-[theme(--color-blue)]', 'foo-[--spacing(4)]', 'foo-[--theme(--text-sm)]']),
  extra('arbitrary value with spaces and quotes', '@utility foo-* { a: --value([*]); }', ['foo-[a_b]', "foo-['a_b']", 'foo-[a\\_b]', 'foo-[url(a_b)]', 'foo-[1px,2px]', 'foo-[calc(1px+2px)]']),
  extra('value with separators', '@utility foo-* { a: --value([*]), --value([*]); b: --value([*])/--value([*]); }', ['foo-[a,b]', 'foo-[a/b]']),
].map((c) => ({ ...c, extra: true }))

// The same case with the `important` flag of the design system.
const IMPORTANT_CASES = [
  extra('important design system: static', '@utility foo { a: b; &:hover { c: d; } }'),
  extra('important design system: functional', '@utility foo-* { a: --value(integer); b: --modifier(integer); & > * { c: --value(integer); } }'),
  extra('important design system: declaration is important', '@utility foo-* { a: --value(integer) !important; }'),
].map((c) => ({ ...c, extra: true, important: true }))

// ---------------------------------------------------------------------------
// Candidates.

const VALUES = [
  '1', '0', '2', '4', '12', '123', '01', '1.5', '2.5', '0.25', '2.3', '1.50', '.5', '1e3', '50%', '12.5%', '0%', '050%',
  '1/2', '16/9', '2.5/3', '1/2.5', '0/1', '1/0', 'a/b', 'bar', 'baz', 'github', 'deep-nested', 'red-500', 'blue', 'sm',
  'lg', 'tight', 'half', 'none', 'a', 'auto', 'px', '1px', '#fff',
  '[1]', '[12px]', '[1.5]', '[50%]', '[#0088cc]', '[red]', '[16/9]', '[var(--x)]', '[calc(1px*2)]', '[url(a.png)]',
  '[integer:1]', '[integer:var(--x)]', '[length:var(--x)]', '[color:var(--x)]', '[number:1.5]', '[percentage:var(--x)]',
  '[foo:bar]', '[length:1]', '[1px_2px]', '[thin]', '[45deg]', '[1_2_3]', '[top_left]', '[serif]', '[large]', '[larger]',
  '["a"]', '[linear-gradient(red,blue)]', '(--x)', '(integer:--x)', '(length:--x)', '(color:--x)', '(--x,1px)', '[]', '()',
]
const MODIFIERS = [
  '1', '2', '50', '1.5', '2.3', '50%', 'bar', 'baz', 'tight', '6', 'sm', '[1]', '[12px]', '[var(--x)]', '(--x)', '[]',
  '1/2',
]

function generatedCandidates(root, themeKeys, literals) {
  let out = [root, `${root}-`, `${root}!`, `!${root}`, `-${root}`, `${root}/1`, `${root}/[1px]`, `${root}/bar`]
  let values = [...VALUES, ...literals]
  for (let key of themeKeys) {
    // `--tab-size-github` gives `size-github` and `github`.
    let parts = key.slice(2).split('-')
    for (let i = 1; i < parts.length; i++) values.push(parts.slice(i).join('-'))
  }
  values = [...new Set(values)]
  for (let value of values) out.push(`${root}-${value}`)
  let some = ['1', '2.5', 'bar', 'sm', '[12px]', ...literals.slice(0, 2)]
  for (let value of new Set(some)) {
    for (let modifier of MODIFIERS) out.push(`${root}-${value}/${modifier}`)
  }
  for (let value of ['1', 'bar', '[12px]', '1/2', '1/bar']) {
    out.push(`${root}-${value}!`, `!${root}-${value}`, `-${root}-${value}`)
  }
  return out
}

// ---------------------------------------------------------------------------
// The record of one case.

async function load(input) {
  return up.loadDesignSystem(input, { loadStylesheet: up.loadStylesheet })
}

function hasRule(nodes, names) {
  for (let node of nodes) {
    if (node.kind === 'at-rule' && names.includes(node.name)) return true
    if (node.nodes && hasRule(node.nodes, names)) return true
  }
  return false
}


async function record(testCase) {
  let ast
  try {
    ast = parse(testCase.css)
  } catch {
    return null
  }

  // Only the `@theme` rules and the `@utility` rules of the top level.
  let themes = ast.filter((node) => node.kind === 'at-rule' && node.name === '@theme')
  let utilities = ast.filter((node) => node.kind === 'at-rule' && node.name === '@utility')
  if (utilities.length === 0) return null
  if (hasRule(utilities, ['@apply', '@variant', '@slot'])) return { skipped: true }

  let themeCss = toCss(themes)
  let utilityCss = toCss(utilities)
  let input = themeCss + utilityCss
  let valid = utilities.map((node) => createCssUtility(node) !== null)

  let result = { name: testCase.name, input, important: testCase.important === true, valid, error: null, rules: {}, none: [] }

  let design
  let baseline
  try {
    design = await load(input + '@tailwind utilities;')
    baseline = await load(themeCss + '@tailwind utilities;')
  } catch (error) {
    result.error = error.message
    result.none = ''
    return result
  }
  if (result.important) {
    design.important = true
    baseline.important = true
  }

  let themeKeys = design.theme.entries ? [...design.theme.entries()].map(([key]) => key) : []
  let literals = [...utilityCss.matchAll(/'([^'"\s]+)'|"([^'"\s]+)"/g)].map((m) => m[1] ?? m[2])

  let candidates = [...testCase.candidates]
  for (let node of utilities) {
    let name = unescape(node.params)
    if (name.endsWith('-*')) {
      candidates.push(...generatedCandidates(name.slice(0, -2), themeKeys, literals))
    } else {
      candidates.push(name, `${name}!`, `!${name}`, `-${name}`, `${name}-1`, `${name}/1`, `${name}-[1px]`, `${name}/[1px]`)
    }
  }

  for (let candidate of new Set(candidates)) {
    let parsed = design.parseCandidate(candidate)
    if (parsed.some((p) => p.variants.length > 0)) continue
    if (candidate.includes(':') && parsed.length === 0 && !candidate.includes('[') && !candidate.includes('(')) continue

    // A utility of Tailwind CSS itself gives rules: the Rust test cannot
    // replay the candidate.
    let builtin = false
    for (let p of baseline.parseCandidate(candidate)) {
      if (baseline.compileAstNodes(p).length > 0) builtin = true
    }
    if (builtin) continue

    let nodes = []
    for (let p of parsed) {
      for (let { node, propertySort } of design.compileAstNodes(p)) {
        nodes.push([toCss([node]), propertySort.order, propertySort.count])
      }
    }
    if (/\s/.test(candidate)) continue
    if (nodes.length > 0) result.rules[candidate] = nodes
    else result.none.push(candidate)
  }
  result.none = result.none.join(' ')
  return result
}

// ---------------------------------------------------------------------------

let cases = [...extractCases('utilities.test.ts'), ...extractCases('index.test.ts'), ...EXTRA_CASES, ...IMPORTANT_CASES]

let out = []
let skipped = []
for (let testCase of cases) {
  let result = await record(testCase)
  if (result === null) continue
  if (result.skipped) {
    skipped.push(testCase.name)
    continue
  }
  out.push(result)
}

let names = NAMES.map((name) => ({
  name,
  static: isValidStaticUtilityName(name),
  functional: isValidFunctionalUtilityName(name),
}))

fs.mkdirSync(path.dirname(path.resolve(outFile)), { recursive: true })
// One line for each case.
let text =
  '{\n"names": ' +
  JSON.stringify(names) +
  ',\n"skipped": ' +
  JSON.stringify(skipped) +
  ',\n"cases": [\n' +
  out.map((c) => JSON.stringify(c)).join(',\n') +
  '\n]\n}\n'
fs.writeFileSync(outFile, text)

let withRules = out.reduce((sum, c) => sum + Object.keys(c.rules).length, 0)
let candidates = withRules + out.reduce((sum, c) => sum + (c.none === '' ? 0 : c.none.split(' ').length), 0)
console.log(
  `${path.basename(outFile)}: ${names.length} names, ${out.length} cases, ${candidates} candidates ` +
    `(${withRules} with rules), ${out.filter((c) => c.error).length} cases with an error, ${skipped.length} cases skipped`,
)
