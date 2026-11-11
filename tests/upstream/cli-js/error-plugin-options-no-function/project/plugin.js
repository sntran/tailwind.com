const plugin = require('tailwindcss/plugin')

// This plugin does not accept options.
module.exports = plugin(function ({ addUtilities }) {
  addUtilities({ '.x': { color: 'red' } })
})
