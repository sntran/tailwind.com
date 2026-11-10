const plugin = require('tailwindcss/plugin')
const data = require('./data.json')

module.exports = plugin(
  function ({ addUtilities, theme }) {
    addUtilities({
      '.local-plugin': {
        color: theme('colors.fromconfig'),
        background: theme('colors.fromplugin'),
        '--json': data.value,
      },
    })
  },
  { theme: { extend: { colors: { fromplugin: '#222222' } } } },
)
