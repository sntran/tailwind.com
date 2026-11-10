const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ addUtilities }) {
  addUtilities({ '.before-throw': { color: 'red' } })
  throw new Error('The plugin fails on purpose.')
})
