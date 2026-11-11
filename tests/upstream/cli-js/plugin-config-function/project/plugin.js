const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ addUtilities, config }) {
  addUtilities({
    '.c-values': {
      '--dark-mode': String(config('darkMode')),
      '--important': String(config('important')),
      '--prefix': JSON.stringify(config('prefix')),
      '--answer': String(config('custom.answer')),
      '--list': JSON.stringify(config('custom.list')),
      '--deep': String(config('custom.nested.deep')),
      '--missing': String(config('custom.missing')),
      '--default': config('custom.missing', 'fallback'),
      '--brand': String(config('theme.colors.brand')),
      '--red': String(config('theme.colors.red.500')),
      '--separator': String(config('separator')),
      '--keys': Object.keys(config()).sort().join(' '),
    },
  })
})
