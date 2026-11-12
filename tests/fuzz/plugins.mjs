// The JavaScript parts of the fuzzer: real plugins, and random plugins and
// configs as source text.
//
// Each generator gives a part: `{ line, files, copy, utilities, variants }`.
// `line` is the line of the stylesheet (`@plugin "..."` or `@config
// "..."`), `files` the files to write (name to text), `copy` the files to
// copy from this repository, and `utilities` and `variants` the names that
// the part makes, for the random candidates.

// The real plugins. The fuzzer loads each one with no options and with
// options, and takes their classes from the official class list.
export const REAL_PLUGINS = [
  { line: '@plugin "@tailwindcss/typography";' },
  { line: '@plugin "@tailwindcss/typography" {\n  className: wysiwyg;\n}' },
  { line: '@plugin "@tailwindcss/forms";' },
  { line: '@plugin "@tailwindcss/forms" {\n  strategy: class;\n}' },
  { line: '@plugin "@tailwindcss/aspect-ratio";' },
  { line: '@plugin "tailwindcss-animate";' },
  { line: '@plugin "./daisyui.js";', copy: { 'daisyui.js': 'tests/upstream/cli-js/daisyui-bundle/project/vendor/daisyui.js' } },
  {
    line: '@plugin "./daisyui.js" {\n  themes: light --default, dark --prefersdark;\n}',
    copy: { 'daisyui.js': 'tests/upstream/cli-js/daisyui-bundle/project/vendor/daisyui.js' },
  },
  {
    line: '@plugin "./daisyui.js" {\n  themes: false;\n  prefix: d-;\n}',
    copy: { 'daisyui.js': 'tests/upstream/cli-js/daisyui-bundle/project/vendor/daisyui.js' },
  },
]

// Some classes of the real plugins that the class list does not give (a
// plugin can make classes for any value).
export const REAL_CLASSES = `
prose prose-lg prose-sm prose-xl prose-2xl prose-slate prose-zinc prose-invert prose-headings:underline prose-a:text-blue-600
prose-img:rounded-xl prose-p:my-2 not-prose lead wysiwyg wysiwyg-lg max-w-none prose-pre:bg-black/50 dark:prose-invert
form-input form-select form-checkbox form-radio form-textarea form-multiselect
aspect-w-16 aspect-h-9 aspect-w-4 aspect-h-3 aspect-none
animate-in animate-out fade-in fade-out zoom-in-95 zoom-out-90 slide-in-from-top-2 slide-out-to-left-1/2 spin-in-6 duration-300
delay-150 ease-in-out fill-mode-forwards direction-reverse repeat-infinite running paused fade-in-[0.3] slide-in-from-bottom-[10px]
btn btn-primary btn-secondary btn-outline btn-sm btn-lg btn-ghost card card-body card-title alert alert-info badge badge-primary
toggle toggle-primary modal modal-box navbar menu dropdown tabs tab input input-bordered select checkbox loading loading-spinner
d-btn d-card d-badge
`.trim().split(/\s+/)

export function createGenerators(random) {
  let uid = 0
  const id = () => `${(uid++).toString(36)}${random.int(1000)}`

  const COLOR_VALUES = ["'red'", "'#0088cc'", "'oklch(0.7 0.1 200)'", "theme('colors.blue.500')", "'currentColor'", "'rgb(0 0 0 / 50%)'"]
  const LENGTH_VALUES = ["'1px'", "'2rem'", "theme('spacing.4')", "'calc(1rem + 2px)'", "'0'", "theme('spacing.0.5')"]

  // A CSS-in-JS object as source text.
  function cssObject(depth = 0) {
    const props = []
    for (let n = 1 + random.int(3); n > 0; n--) {
      const r = random.next()
      if (r < 0.25) props.push(`color: ${random.pick(COLOR_VALUES)}`)
      else if (r < 0.45) props.push(`${random.pick(['padding', 'marginTop', 'width', 'gap'])}: ${random.pick(LENGTH_VALUES)}`)
      else if (r < 0.55) props.push(`'--tw-x': ${random.pick(["'1'", "'var(--y)'", 'theme("opacity.50")'])}`)
      else if (r < 0.65) props.push(`fontSize: theme('fontSize.${random.pick(['sm', 'lg', '2xl'])}')`)
      else if (r < 0.72) props.push(`fontFamily: theme('fontFamily.${random.pick(['sans', 'mono'])}')`)
      else if (r < 0.8) props.push(`display: ${random.pick(["'flex'", "'grid'", "'none'"])}`)
      else if (depth < 2) {
        const key = random.pick(["'&:hover'", "'& > svg'", "'.dark &'", "'@media (min-width: 640px)'", "'@supports (display: grid)'", "'&::before'"])
        props.push(`${key}: ${cssObject(depth + 1)}`)
      } else props.push(`opacity: '0.5'`)
    }
    return `{ ${props.join(', ')} }`
  }

  // One call of the plugin API, as source text, with the names it makes.
  function apiCall(made) {
    const r = random.next()
    if (r < 0.2) {
      const names = Array.from({ length: 1 + random.int(3) }, () => `u${id()}`)
      made.utilities.push(...names)
      const body = names.map((n) => `'.${n}': ${cssObject()}`).join(', ')
      return random.chance(0.3) ? `addUtilities([{ ${body} }])` : `addUtilities({ ${body} })`
    }
    if (r < 0.45) {
      const name = `m${id()}`
      const valueSets = [
        "theme('colors')", "theme('spacing')", "{ a: '1px', b: '2px', DEFAULT: '3px' }", "{ sm: '0.5rem', lg: '2rem' }",
        "theme('opacity')", 'undefined', "{ red: 'red', '1/2': '50%' }",
      ]
      const types = ["'color'", "'length'", "'any'", "['color', 'any']", "'number'", "'percentage'", "['length', 'percentage']"]
      const opt = [`values: ${random.pick(valueSets)}`]
      if (random.chance(0.6)) opt.push(`type: ${random.pick(types)}`)
      if (random.chance(0.4)) opt.push(`modifiers: ${random.pick(["'any'", "theme('opacity')", "{ half: '0.5', full: '1' }"])}`)
      if (random.chance(0.3)) opt.push('supportsNegativeValues: true')
      for (const v of ['a', 'b', 'red', 'sm', '4', '[3px]', '[#123]', '(--x)', '1/2', 'red-500', '[color:red]', '50', '-4', '[2]']) {
        made.utilities.push(`${name}-${v}`)
      }
      made.utilities.push(name, `-${name}-4`, `${name}-red/50`, `${name}-a/half`, `${name}-[red]/[0.3]`)
      const fn = random.pick([
        `(value) => ({ ${random.pick(["color", "width", "'--tw-m'"])}: value })`,
        `(value, { modifier }) => ({ color: value, opacity: modifier ?? '1' })`,
        `(value) => [{ width: value }, { '&:hover': { height: value } }]`,
        `(value) => ({ '& > *': { margin: value } })`,
        `(value) => ({})`,
        `(value) => value === 'red' ? null : { color: value }`,
      ])
      return `matchUtilities({ '${name}': ${fn} }, { ${opt.join(', ')} })`
    }
    if (r < 0.55) {
      const names = [`c${id()}`]
      made.utilities.push(...names)
      return `addComponents({ '.${names[0]}': ${cssObject()} })`
    }
    if (r < 0.62) {
      const name = `mc${id()}`
      for (const v of ['a', 'b', '[1px]', 'red']) made.utilities.push(`${name}-${v}`)
      return `matchComponents({ '${name}': (value) => ({ padding: value }) }, { values: { a: '1px', b: '2px' } })`
    }
    if (r < 0.77) {
      const name = `v${id()}`
      made.variants.push(name, `group-${name}`, `peer-${name}`, `not-${name}`, `has-${name}`)
      return `addVariant('${name}', ${random.pick([
        "'&:hover'", "['&:hover', '&:focus']", "'@media (min-width: 100px)'", "'&[data-open]'", "'.dark &'",
        "{ '&:hover': { '@slot': {} } }", "['@media (pointer: fine)', '&:active']", "'&::before'", "'@supports (display: grid)'",
      ])})`
    }
    if (r < 0.87) {
      const name = `mv${id()}`
      made.variants.push(`${name}-a`, `${name}-b`, `${name}-[x]`, name, `${name}-[open]`, `group-${name}-a`)
      const sort = random.chance(0.4) ? ', sort: (a, z) => a.value.localeCompare(z.value)' : ''
      return `matchVariant('${name}', ${random.pick([
        '(value) => `&[data-x=${value}]`',
        '(value, { modifier }) => `&:nth-child(${value})${modifier ? "." + modifier : ""}`',
        '(value) => [`&.${value}`, `.${value} &`]',
        '(value) => `@media (min-width: ${value})`',
      ])}, { values: { a: 'a', b: 'b', DEFAULT: 'd' }${sort} })`
    }
    if (r < 0.95) return `addBase({ '${random.pick(['h1', ':root', 'a', 'html', '.base'])}': ${cssObject()} })`
    return `addUtilities({ '.x${id()}': { content: JSON.stringify(config('theme.colors') ? 'yes' : 'no') } })`
  }

  // A random plugin file.
  function plugin() {
    const made = { utilities: [], variants: [] }
    const name = `plugin-${id()}.${random.pick(['js', 'js', 'cjs', 'mjs'])}`
    const calls = Array.from({ length: 1 + random.int(4) }, () => apiCall(made))
    const body = `function ({ addUtilities, matchUtilities, addComponents, matchComponents, addVariant, matchVariant, addBase, theme, config, prefix }) {\n  ${calls.join('\n  ')}\n}`
    const extend = random.chance(0.3) ? `, { theme: { extend: { colors: { p${id()}: '#abcdef' } } } }` : ''
    let options = ''
    let wrapped
    const r = random.next()
    if (r < 0.2) {
      // plugin.withOptions, with options of the stylesheet.
      wrapped = `plugin.withOptions(function (options = {}) {\n  return ${body.replace('{\n  ', '{\n  const size = options.size ?? "1px";\n  ')}\n})`
      if (random.chance(0.7)) options = ` {\n  size: ${random.pick(['2px', '3rem', '10', 'abc'])};\n}`
    } else if (r < 0.3) {
      // A plain function, with no wrapper.
      wrapped = body
    } else {
      wrapped = `plugin(${body}${extend})`
    }
    const esm = name.endsWith('.mjs')
    const text = esm
      ? `import plugin from 'tailwindcss/plugin'\nexport default ${wrapped}\n`
      : `const plugin = require('tailwindcss/plugin')\nmodule.exports = ${wrapped}\n`
    // An at-rule with a block has no `;` after it.
    return { line: `@plugin "./${name}"${options || ';'}`, files: { [name]: text }, ...made }
  }

  // A random config file.
  function config() {
    const made = { utilities: [], variants: [] }
    const name = `config-${id()}.${random.pick(['js', 'cjs', 'mjs'])}`
    const extend = []
    const c = `c${id()}`
    if (random.chance(0.7)) {
      extend.push(`colors: { ${c}: { DEFAULT: '#123456', light: '#abcdef', 500: 'rgb(0 128 255 / <alpha-value>)' }, flat${c}: 'red' }`)
      made.utilities.push(`bg-${c}`, `text-${c}-light`, `bg-${c}-500/50`, `border-flat${c}`, `from-${c}`, `bg-${c}/25`)
    }
    if (random.chance(0.5)) {
      extend.push(`spacing: { huge${c}: '42rem', '13': '3.25rem' }`)
      made.utilities.push(`p-huge${c}`, `m-13`, `w-huge${c}`, `-mt-13`)
    }
    if (random.chance(0.4)) {
      extend.push(`fontSize: { tiny${c}: ['0.5rem', { lineHeight: '0.75rem', letterSpacing: '0.1em' }], big${c}: '3rem' }`)
      made.utilities.push(`text-tiny${c}`, `text-big${c}`, `text-tiny${c}/7`)
    }
    if (random.chance(0.3)) {
      extend.push(`screens: { tab${c}: '700px', range${c}: { min: '400px', max: '600px' } }`)
      made.variants.push(`tab${c}`, `max-tab${c}`, `range${c}`)
    }
    if (random.chance(0.3)) {
      extend.push(`keyframes: { w${c}: { '0%, 100%': { transform: 'rotate(-3deg)' }, '50%': { transform: 'rotate(3deg)' } } }, animation: { w${c}: 'w${c} 1s ease-in-out infinite' }`)
      made.utilities.push(`animate-w${c}`)
    }
    if (random.chance(0.3)) {
      extend.push(`borderRadius: { blob${c}: '30% 70%' }, boxShadow: { soft${c}: '0 2px 4px rgb(0 0 0 / 0.1)' }`)
      made.utilities.push(`rounded-blob${c}`, `shadow-soft${c}`, `shadow-soft${c}/50`)
    }
    if (random.chance(0.2)) {
      extend.push(`fontFamily: { display${c}: ['"Inter"', 'sans-serif'] }`)
      made.utilities.push(`font-display${c}`)
    }
    const top = [`theme: { ${random.chance(0.15) ? `colors: { only${c}: 'red' }, ` : ''}extend: { ${extend.join(', ')} } }`]
    if (random.chance(0.15)) made.utilities.push(`bg-only${c}`)
    if (random.chance(0.3)) top.push(`darkMode: ${random.pick(["'class'", "'selector'", "['selector', '[data-theme=dark]']", "'media'", "['variant', '&:is(.dark *)']"])}`)
    if (random.chance(0.15)) top.push(`important: ${random.pick(['true', "'#app'"])}`)
    if (random.chance(0.2)) {
      const u = `inline${c}`
      made.utilities.push(u)
      top.push(`plugins: [function ({ addUtilities }) { addUtilities({ '.${u}': { color: 'blue' } }) }]`)
    }
    const object = `{\n  ${top.join(',\n  ')}\n}`
    const text = name.endsWith('.mjs') ? `export default ${object}\n` : `module.exports = ${object}\n`
    return { line: `@config "./${name}";`, files: { [name]: text }, ...made }
  }

  function real() {
    const p = random.pick(REAL_PLUGINS)
    return { line: p.line, copy: p.copy ?? {}, files: {}, utilities: [], variants: [] }
  }

  return { plugin, config, real }
}
