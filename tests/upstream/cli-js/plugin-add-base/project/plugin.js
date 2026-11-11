const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ addBase, theme }) {
  addBase({
    h1: { fontSize: theme('fontSize.2xl')[0], marginBottom: theme('spacing.4'), fontWeight: '700' },
    'h2, h3': { fontSize: '1.25rem' },
    a: {
      color: 'blue',
      '&:hover': { textDecoration: 'underline' },
    },
    '@media (min-width: 640px)': {
      body: { margin: '0 auto' },
    },
    ':root': { '--plugin-base': '1' },
  })
  addBase([{ '.from-array': { color: 'red' } }, { 'ul > li': { listStyle: 'disc' } }])
})
