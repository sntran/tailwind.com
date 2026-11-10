// The shape of a bundle: a CommonJS module with the exports __esModule and default.
const plugin = require('tailwindcss/plugin')

Object.defineProperty(exports, '__esModule', { value: true })
exports.default = plugin.withOptions(function (options = {}) {
  return function ({ addUtilities }) {
    addUtilities({ '.bundle-default': { color: options.color || 'none' } })
  }
})
