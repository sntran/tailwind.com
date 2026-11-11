const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ addBase, addComponents, addUtilities, matchUtilities, addVariant, theme }) {
  addBase({
    h1: { fontSize: '2rem', '&:hover': { textDecoration: 'underline' } },
    '@media (min-width: 640px)': { body: { margin: '0 auto' } },
  })
  addComponents({ '.btn': { padding: theme('spacing.4'), '@media print': { display: 'none' } } })
  addUtilities({
    '.card': { borderRadius: '4px', '&:hover': { color: 'red' } },
    '@keyframes wiggle': { from: { rotate: '-3deg' }, to: { rotate: '3deg' } },
  })
  matchUtilities({ tab: (value) => ({ tabSize: value }) }, { values: { 2: '2', 4: '4' } })
  addVariant('hocus', ['&:hover', '&:focus'])
})
