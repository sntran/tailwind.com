const plugin = require('tailwindcss/plugin')

module.exports = plugin.withOptions(function (options = {}) {
  return function ({ addUtilities }) {
    addUtilities({ ['.' + (options.name || 'default-name')]: { color: options.color || 'silver' } })
  }
})
