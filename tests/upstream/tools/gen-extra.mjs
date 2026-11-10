#!/usr/bin/env node
// Usage: node gen-extra.mjs <path of the upstream clone> <output directory>
// This script writes suite-extra.jsonl.gz: records in the format of suite.jsonl.gz for cases
// that the upstream unit tests do not have. The cases are for the parts of the compiler
// that have a different structure in the Rust port: the sequence of the imports, `@apply`
// with `@utility`, the compatibility layer for `theme(a.b.c)`, and the legacy utilities.
import path from 'node:path'
import { loadUpstream, writeGzip } from './lib.mjs'

let [clone, outDir] = process.argv.slice(2)
if (!clone || !outDir) {
  console.error('Usage: node gen-extra.mjs <path of the upstream clone> <output directory>')
  process.exit(2)
}

let upstream = await loadUpstream(clone)

const THEME = '@import "tailwindcss/theme.css" theme(reference);\n'
const UTILITIES = '@tailwind utilities;\n'

function themeCase(paths) {
  return {
    input: THEME + '.a {\n' + paths.map((p, i) => `  --v${i}: theme(${p});\n`).join('') + '}\n',
  }
}

let cases = {
  // The sequence of the loader calls, and the wrappers of the imports.
  'imports: nested imports load level by level': {
    input: '@import "./a.css";\n@import "./b.css" layer(b);\n@import "./c.css" print;\n',
    files: {
      './a.css': '@import "./a1.css";\n@import "./a2.css" supports(display: grid);\n.a { color: red }\n',
      './b.css': '@import "./b1.css" layer;\n.b { color: red }\n',
      './c.css': '.c { color: red }\n',
      './a1.css': '@import "./a11.css";\n.a1 { color: red }\n',
      './a2.css': '.a2 { color: red }\n',
      './b1.css': '.b1 { color: red }\n',
      './a11.css': '.a11 { color: red }\n',
    },
  },
  'imports: an import in a rule and in a media rule': {
    input: '.x { @import "./a.css"; }\n@media screen { @import "./b.css"; }\n',
    files: { './a.css': 'color: red;\n', './b.css': '.b { color: red }\n' },
  },
  'imports: the same file two times': {
    input: '@import "./a.css";\n@import "./a.css" layer(x);\n',
    files: { './a.css': '.a { color: red }\n' },
  },
  'imports: url and remote imports stay': {
    input: '@import url("./a.css");\n@import "https://example.com/a.css";\n@import "./a.css" layer(a) layer(b);\n.x { color: red }\n@tailwind utilities;\n',
    builds: [['flex']],
  },
  'imports: layer after supports': {
    input: '@import "./a.css" supports(display: grid) layer(x);\n',
    files: { './a.css': '.a { color: red }\n' },
  },
  'imports: a missing file': {
    input: '@import "./a.css";\n@import "./missing.css";\n',
    files: { './a.css': '@import "./missing-2.css";\n' },
  },
  'imports: a circular import': {
    input: '@import "./a.css";\n',
    files: { './a.css': '@import "./a.css";\n' },
  },
  'imports: reference with utilities and theme': {
    input: '@reference "./a.css";\n.x { @apply foo text-red; }\n@tailwind utilities;\n',
    files: {
      './a.css': '@theme { --color-red: red; }\n@utility foo { color: blue; }\n@tailwind utilities;\n.a { color: red }\n',
    },
    builds: [['foo', 'text-red']],
  },
  'imports: source and theme options': {
    input: '@import "./a.css" source("../src") theme(inline) prefix(tw) important;\n',
    files: { './a.css': '@theme { --color-red: red; }\n@tailwind utilities;\n' },
    builds: [['tw:text-red', 'text-red']],
  },

  // `@apply`.
  'apply: utilities that apply utilities, in the reverse sequence': {
    input: THEME + UTILITIES + '@utility a { @apply b underline; }\n@utility b { @apply c; color: red; }\n@utility c { @apply flex; }\n.x { @apply a; }\n',
    builds: [['a', 'b', 'c']],
  },
  'apply: a functional utility that applies a utility': {
    input: THEME + UTILITIES + '@utility tab-* { tab-size: --value(integer); @apply b; margin: --spacing(--value(integer)); }\n@utility b { @apply flex; }\n.x { @apply tab-4 tab-1 tab-0; }\n',
    builds: [['tab-4', 'tab-1', 'tab-0']],
  },
  'apply: a nested rule in a utility': {
    input: THEME + UTILITIES + '@utility a { color: red; &:hover { @apply underline; } @media print { @apply hidden; } }\n.x { @apply a; }\n',
    builds: [['a', 'hover:a']],
  },
  'apply: at the top level': {
    input: THEME + '@apply flex;\n@utility a { @apply underline; }\n.x { @apply a; }\n',
  },
  'apply: two definitions of one utility': {
    input: THEME + UTILITIES + '@utility a { @apply flex; }\n@utility a { color: red; }\n@utility a { @apply underline; }\n.x { @apply a; }\n',
    builds: [['a']],
  },
  'apply: variants and important': {
    input: THEME + '.x { @apply hover:underline! md:focus:flex !block dark:text-red-500/50; .y & { @apply p-4 -m-2; } }\n',
  },
  'apply: declarations stay in their sequence': {
    input: THEME + '.x { color: red; @apply flex underline; color: blue; @apply hidden; }\n',
  },
  'apply: the same candidate two times': {
    input: THEME + '.x { @apply flex flex   underline\n flex; }\n',
  },
  'apply: a circular dependency of utilities': {
    input: THEME + '@utility a { @apply b; }\n@utility b { @apply a; }\n',
  },
  'apply: a circular dependency of three utilities': {
    input: THEME + '@utility a { @apply hover:b; }\n@utility b { @apply c; }\n@utility c { @apply focus:a; }\n',
  },
  'apply: a utility that applies itself': {
    input: THEME + '@utility a { @apply a; }\n',
  },
  'apply: a functional utility that applies itself': {
    input: THEME + '@utility a-* { color: --value(integer); @apply a-4; }\n',
  },
  'apply: one unknown variant': { input: THEME + '.x { @apply nope:flex; }\n' },
  'apply: two unknown variants': { input: THEME + '.x { @apply nope:hover:nada:flex; }\n' },
  'apply: three unknown variants': { input: THEME + '.x { @apply nope:nada:hover:nix:flex; }\n' },
  'apply: an unknown variant and an unknown utility': { input: THEME + '.x { @apply nope:nothing; }\n' },
  'apply: an unknown utility': { input: THEME + '.x { @apply flex nothing; }\n' },
  'apply: an unknown utility with no theme': { input: '.x { @apply nothing; }\n' },
  'apply: a number comes first in the errors': { input: THEME + '.x { @apply nothing 12 3; }\n' },
  'apply: no candidate': { input: THEME + '.x { @apply; }\n' },
  'apply: a mixin stays': { input: THEME + '.x { @apply --my-mixin; @apply --a --b { color: red; } }\n' },
  'apply: a mixin with utilities': { input: THEME + '.x { @apply --my-mixin flex --other; }\n' },
  'apply: a body': { input: THEME + '.x { @apply flex underline { color: red; } }\n' },
  'apply: in keyframes': { input: THEME + '@keyframes x { from { @apply flex; } }\n' },
  'apply: a disabled utility': { input: THEME + '@source not inline("flex");\n.x { @apply flex; }\n' },
  'apply: a missing prefix': {
    input: '@theme prefix(tw) { --color-red: red; }\n.x { @apply tw:flex underline; }\n',
  },
  'apply: with a prefix': {
    input: '@theme prefix(tw) { --color-red: red; }\n.x { @apply tw:flex tw:hover:text-red; }\n',
  },
  'apply: a custom variant': {
    input: THEME + '@custom-variant hocus (&:hover, &:focus);\n@custom-variant tall { @media (min-height: 40rem) { @slot; } }\n.x { @apply hocus:flex tall:hidden; }\n',
  },
  'apply: a utility with theme() and @variant': {
    input: THEME + UTILITIES + '@utility a { color: theme(--color-red-500); margin: --spacing(4); @variant hover { color: theme(colors.blue.500); } }\n.x { @apply a; }\n',
    builds: [['a', 'focus:a']],
  },
  'apply: a functional utility with --spacing of a value': {
    input: THEME + UTILITIES + '@utility pad-* { padding: --spacing(--value(integer)); margin: --value(--spacing); }\n',
    builds: [['pad-0', 'pad-1', 'pad-4']],
  },

  // `theme()` with a key path: the compatibility layer.
  'theme: colors and spacing': themeCase([
    'colors.red.500', 'colors.red.500 / 50%', 'colors.black', 'spacing.4', 'spacing[2.5]', 'spacing.px', 'spacing.DEFAULT, none',
    'backgroundColor.red.500', 'borderColor.DEFAULT', 'ringColor.DEFAULT, none', 'fill.current, none', 'accentColor.auto, none',
  ]),
  'theme: the default theme of the configuration': themeCase([
    'zIndex.10', 'zIndex.auto', 'opacity.50', 'width.1/2', 'width.full', 'height.screen', 'maxWidth.xs', 'maxWidth.prose',
    'aspectRatio.video', 'transitionDuration.DEFAULT', 'transitionDuration.150', 'transitionTimingFunction.DEFAULT',
    'transitionTimingFunction.in-out', 'borderWidth.DEFAULT', 'borderWidth.2', 'flex.1', 'gridTemplateColumns.3',
    'rotate.45', 'scale.95', 'translate.full', 'inset.auto', 'margin.auto', 'padding.4', 'gap.2', 'lineHeight.6',
    'lineHeight.tight', 'letterSpacing.wide', 'fontWeight.bold', 'cursor.pointer', 'order.first', 'blur.sm', 'blur.DEFAULT, none',
    'dropShadow.md', 'boxShadow.md', 'boxShadow.DEFAULT, none', 'borderRadius.lg', 'borderRadius.DEFAULT, none',
    'animation.spin', 'screens.md', 'fontFamily.sans', 'fontFamily.mono', 'fontSize.xs', 'fontSize.xs[1].lineHeight',
    'fontSize.2xl[1].lineHeight', 'content.none', 'listStyleType.disc', 'strokeWidth.2', 'outlineOffset.4',
    'textDecorationThickness.from-font', 'supports.grid, none', 'aria.busy', 'data.x, none', 'columns.3xs', 'columns.3',
    'containers.md, none', 'size.4', 'minHeight.dvh', 'willChange.transform', 'objectPosition.left-top',
  ]),
  'theme: values that are not text': themeCase([
    'keyframes.spin, none', 'fontSize, none', 'colors.red, none', 'colors, none', 'aspectRatio.__BARE_VALUE__, none',
    'fontSize.xs[1], none', 'fontSize.xs.1.lineHeight', 'nothing.at.all, none', 'container.center, none', 'colors.a b, none',
  ]),
  'theme: a custom theme with defaults and nested keys': {
    input:
      '@theme default { --color-brand: blue; --text-huge: 9rem; --text-huge--line-height: 1; --shadow: 0 0 red; --font-display: "A", "B"; --spacing: 2px; --breakpoint-tiny: 10px; }\n' +
      '@theme { --color-brand-dark: navy; --radius: 3px; --radius-big: 9px; --ease-snappy: linear; --animate-wiggle: wiggle 1s; --leading-airy: 3; --tracking-far: 1em; --aspect-wide: 2 / 1; --container-huge: 99rem; --default-transition-duration: 9ms; --color-a_b: red; --blur: 4px; }\n' +
      '.a {\n' +
      [
        'colors.brand', 'colors.brand.DEFAULT', 'colors.brand.dark', 'colors.brand.dark / 20%', 'fontSize.huge', 'fontSize.huge[1].lineHeight',
        'boxShadow.DEFAULT', 'boxShadow', 'fontFamily.display', 'spacing.4, none', 'spacing', 'spacing.DEFAULT', 'screens.tiny', 'screens.md',
        'borderRadius', 'borderRadius.big', 'borderRadius.DEFAULT', 'transitionTimingFunction.snappy', 'animation.wiggle', 'lineHeight.airy',
        'letterSpacing.far', 'aspectRatio.wide', 'maxWidth.huge', 'transitionDuration.DEFAULT', 'colors[a.b]', 'colors.a_b', 'blur', 'blur.DEFAULT',
        'backdropBlur.DEFAULT, none', 'width.4, none', 'zIndex.10', 'colors.red.500, none',
      ]
        .map((p, i) => `  --v${i}: theme(${p});\n`)
        .join('') +
      '}\n',
  },
  'theme: in a candidate and in a media query': {
    input: THEME + UTILITIES + '@media (min-width: theme(screens.md)) { .a { color: theme(colors.red.500 / 0.5); } }\n',
    builds: [['[color:theme(colors.blue.500)]', 'w-[theme(spacing.4)]', 'h-[theme(height.screen)]', 'p-[theme(nothing.here)]']],
  },
  'theme: a path that does not exist': { input: THEME + '.a { color: theme(colors.nope.500); }\n' },

  // The legacy utilities.
  'legacy utilities': {
    input: THEME + UTILITIES,
    builds: [
      (
        'bg-gradient-to-t bg-gradient-to-tr bg-gradient-to-r bg-gradient-to-br bg-gradient-to-b bg-gradient-to-bl bg-gradient-to-l bg-gradient-to-tl ' +
        'bg-left-top bg-right-top bg-left-bottom bg-right-bottom object-left-top object-right-top object-left-bottom object-right-bottom ' +
        'max-w-screen-md max-w-screen-nope max-w-screen-[3px] max-w-screen overflow-ellipsis decoration-slice decoration-clone ' +
        'flex-shrink flex-shrink-0 flex-shrink-2 flex-shrink-[3] flex-shrink-x flex-shrink/2 flex-grow flex-grow-0 flex-grow-[4] flex-grow-1.5 order-none break-words ' +
        'start-auto start-full -start-full start-px -start-px end-auto end-full -end-full end-px -end-px ' +
        'start-4 -start-4 start-1/2 -start-1/2 start-[3px] -start-[3px] start-[3px]/2 start-1.5 -start-2.5 start-x start-1/x start-0 -start-0 ' +
        'end-4 -end-4 end-1/3 -end-1/3 end-[5%] -end-[5%] end-(--x) -end-(--x) start end -start'
      ).split(' '),
    ],
  },
  'legacy utilities with an inset theme and no spacing': {
    input: '@theme { --inset-big: 10rem; --breakpoint-md: 48rem; --spacing-4: 1rem; }\n' + UTILITIES,
    builds: [['start-big', '-start-big', 'end-4', '-end-4', 'start-5', '-start-5', 'max-w-screen-md', 'start-1/2']],
  },

  // The parts of `index.ts`.
  'index: sources': {
    input:
      '@source "./a/**/*.html";\n@source not "./b";\n@source inline("flex {hover:,}underline p-{1..3}");\n@source not inline("p-2 hidden");\n@source inline(  "hidden block"  );\n' +
      THEME + '@tailwind utilities source("./src");\n',
    builds: [[], ['p-2', 'grid'], ['p-2', 'grid']],
  },
  'index: source(none) and a second utilities rule': {
    input: THEME + '@tailwind utilities source(none);\n.x { @tailwind utilities; }\n',
    builds: [['flex']],
  },
  'index: a source that is not quoted': { input: '@tailwind utilities source(./src);\n' },
  'index: a source rule that is not quoted': { input: '@source ./src;\n' },
  'index: a source rule with a body': { input: '@source "./src" { color: red; }\n' },
  'index: a nested source rule': { input: '.x { @source "./src"; }\n' },
  'index: an inline source that is not balanced': { input: '@source inline("p-{1,2");\n' },
  'index: custom variants that use other variants': {
    input:
      THEME + UTILITIES +
      '@custom-variant a { @variant b { @variant hover { @slot; } } }\n@custom-variant b { &:where(.b) { @slot; } }\n@custom-variant c (@media print, &.c, @supports (x: y));\n@custom-variant scoped (@scope (.x));\n@variant old (&.old);\n@variant old2 { &.old2 { @slot; } }\n' +
      '.x { @variant a { color: red; } @variant c { color: blue; } @variant old, old2 { color: green; } }\n',
    builds: [['a:flex', 'b:flex', 'c:flex', 'scoped:flex', 'old:flex', 'old2:flex', 'group-c:flex', 'not-b:flex']],
  },
  'index: a circular dependency of custom variants': {
    input: '@custom-variant a { @variant b { @slot; } }\n@custom-variant b { @variant c { @slot; } }\n@custom-variant c { @variant a { @slot; } }\n',
  },
  'index: a custom variant that uses itself': {
    input: '@custom-variant a { @variant a { @slot; } }\n',
  },
  'index: invalid custom variants': { input: '@custom-variant a- (&.a);\n' },
  'index: a custom variant with a selector and a body': { input: '@custom-variant a (&.a) { @slot; }\n' },
  'index: a custom variant with nothing': { input: '@custom-variant a;\n' },
  'index: a custom variant with an empty selector': { input: '@custom-variant a (&.a, );\n' },
  'index: a nested custom variant': { input: '.x { @custom-variant a (&.a); }\n' },
  'index: theme options and keyframes': {
    input:
      '@theme static { --color-a: red; @keyframes spin { to { rotate: 1turn; } } }\n@theme inline { --color-b: var(--x); }\n@theme reference { --color-c: blue; }\n@theme { --color-*: initial; --color-d: pink; /* a comment */ --animate-spin: spin 1s; }\n' +
      UTILITIES,
    builds: [['text-a', 'text-b', 'text-c', 'text-d', 'animate-spin'], ['--color-d']],
  },
  'index: a variable candidate marks a variable as used': {
    input: '@theme { --color-a: red; --color-b: blue; }\n' + UTILITIES,
    builds: [[], ['--color-a'], ['--color-a'], ['--color-nope'], ['flex'], ['--color-b', 'flex']],
  },
  'index: an invalid theme content': { input: '@theme { --color-a: red; .x { color: red; } }\n' },
  'index: an invalid theme declaration': { input: '@theme { color: red; }\n' },
  'index: an invalid namespace reset': { input: '@theme { --color-*: red; }\n' },
  'index: an invalid prefix': { input: '@theme prefix(tw-x) { --color-a: red; }\n' },
  'index: invalid utilities': { input: '@utility a* { color: red; }\n' },
  'index: an invalid utility with a star in the middle': { input: '@utility a-*-b { color: red; }\n' },
  'index: an invalid utility name': { input: '@utility A { color: red; }\n' },
  'index: an empty utility': { input: '@utility a { }\n' },
  'index: a nested utility': { input: '.x { @utility a { color: red; } }\n' },
  'index: no feature gives the input': { input: '.x  {  color : red  }\n\n/* c */\n', builds: [['flex'], []] },
  'index: important and reference in media': {
    input: '@media important { @theme { --color-a: red; } @tailwind utilities; }\n@media reference { .gone { color: red; } @utility u { color: blue; } }\n@media print and reference { .stays { color: red; } }\n',
    builds: [['text-a', 'u', 'flex']],
  },
  'index: theme(reference) with a rule': {
    input: '@media theme(reference) { @theme { --color-a: red; } .x { color: red; } }\n',
  },
  'index: a plugin with no loader': { input: '@plugin "./plugin.js";\n' },
  'index: a nested plugin': { input: '.x { @plugin "./plugin.js"; }\n' },
  'index: a plugin with no path': { input: '@plugin "";\n' },
  'index: a config with a body': { input: '@config "./x.js" { color: red; }\n' },
  'index: a plugin with an object option': { input: '@plugin "./plugin.js" { a: { b: c }; }\n' },
  'index: a plugin with a rule option': { input: '@plugin "./plugin.js" { .a { color: red; } }\n' },
}

function message(err) {
  return err instanceof Error ? err.message : String(err)
}

let lines = []
for (let [test, spec] of Object.entries(cases)) {
  let stylesheets = []
  async function loadStylesheet(id, base) {
    let entry = { id, base }
    stylesheets.push(entry)
    try {
      let result
      if (id.startsWith('tailwindcss')) {
        result = await upstream.loadStylesheet(id, base)
      } else if (spec.files && id in spec.files) {
        let name = path.posix.basename(id, '.css')
        result = { path: path.posix.join(base, id), base: path.posix.join(base, name), content: spec.files[id] }
      } else {
        throw new Error(`Cannot find the file ${id} in ${base}`)
      }
      entry.result = result
      return result
    } catch (err) {
      entry.error = message(err)
      throw err
    }
  }

  let record = {
    file: 'tests/upstream/tools/gen-extra.mjs',
    test,
    input: spec.input,
    options: { base: '/root', from: null, polyfills: spec.polyfills ?? null, hasLoadModule: false, hasLoadStylesheet: true },
    stylesheets,
    modules: 0,
    moduleRequests: [],
    builds: [],
  }

  try {
    let compiler = await upstream.compile(spec.input, { base: '/root', polyfills: spec.polyfills, loadStylesheet })
    record.sources = compiler.sources
    record.root = compiler.root
    record.features = compiler.features
    for (let candidates of spec.builds ?? [[]]) {
      try {
        record.builds.push({ candidates, css: compiler.build(candidates) })
      } catch (err) {
        record.builds.push({ candidates, error: message(err) })
      }
    }
  } catch (err) {
    record.error = message(err)
  }

  lines.push(JSON.stringify(record))
}

writeGzip(path.join(outDir, 'suite-extra.jsonl.gz'), lines.join('\n') + '\n')
console.log(`suite-extra.jsonl.gz: ${lines.length} records`)
