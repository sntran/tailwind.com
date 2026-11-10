const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ matchUtilities, theme }) {
  matchUtilities(
    {
      tab: (value) => ({ tabSize: value }),
    },
    { values: { DEFAULT: '4', 1: '1', 2: '2', 8: '8', big: '16' } },
  )
  matchUtilities(
    {
      'scroll-m': (value) => ({ 'scroll-margin': value }),
      'scroll-mx': (value) => ({ 'scroll-margin-left': value, 'scroll-margin-right': value }),
    },
    { values: theme('spacing') },
  )
  matchUtilities({ 'no-values': (value) => ({ '--no-values': value }) })
})
