const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ addUtilities, addComponents, matchUtilities, addBase, prefix, theme }) {
  addUtilities({
    '.plain-util': { color: 'red' },
    '.with-var': { color: theme('colors.blue.500') },
    '.applied': { '@apply tw:flex tw:p-4': {} },
  })
  addComponents({ '.plain-comp': { color: 'green' } })
  matchUtilities({ tab: (value) => ({ tabSize: value }) }, { values: { 2: '2', 4: '4' } })
  addBase({
    ':root': {
      '--prefix-of-flex': prefix('flex'),
      '--prefix-of-dot': prefix('.btn'),
    },
  })
})
