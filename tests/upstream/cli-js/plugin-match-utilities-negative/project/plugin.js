const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ matchUtilities, theme }) {
  matchUtilities(
    { nudge: (value) => ({ transform: 'translateY(' + value + ')' }) },
    { values: { 1: '1px', 2: '2px', half: '50%', v: 'var(--nudge)' }, supportsNegativeValues: true },
  )
  matchUtilities(
    { lift: (value) => ({ top: value }) },
    { values: theme('spacing'), supportsNegativeValues: true },
  )
  matchUtilities(
    { fixed: (value) => ({ bottom: value }) },
    { values: { 1: '1px' } },
  )
})
