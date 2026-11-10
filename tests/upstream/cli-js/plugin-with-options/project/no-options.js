const plugin = require('tailwindcss/plugin')

// A plugin with options. The CSS gives no options.
module.exports = plugin.withOptions(function (options) {
  return function ({ addUtilities }) {
    addUtilities({ '.no-options': { '--type': typeof options, '--json': JSON.stringify(options === undefined ? null : options) } })
  }
})
