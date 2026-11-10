const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ addUtilities }) {
  addUtilities({
    '@media print': {
      '.print-hidden': { display: 'none' },
    },
  })
})
