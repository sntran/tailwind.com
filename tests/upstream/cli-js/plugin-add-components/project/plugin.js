const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ addComponents, theme }) {
  addComponents({
    '.btn': {
      padding: theme('spacing.2') + ' ' + theme('spacing.4'),
      borderRadius: theme('borderRadius.md'),
      fontWeight: '600',
      '&:hover': { opacity: '0.9' },
      '&.btn-wide': { width: '100%' },
    },
    '.btn-blue': {
      backgroundColor: theme('colors.blue.500'),
      color: '#fff',
    },
    '.card': {
      display: 'flex',
      '.card-title': { fontWeight: '700' },
      '@media (min-width: 768px)': { flexDirection: 'row' },
    },
  })
  addComponents([{ '.badge': { borderRadius: '9999px' } }])
})
