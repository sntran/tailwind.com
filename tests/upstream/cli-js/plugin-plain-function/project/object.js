// The object form: a handler and a config.
module.exports = {
  handler({ addUtilities, theme }) {
    addUtilities({ '.object-form': { color: theme('colors.objectbrand') } })
  },
  config: {
    theme: { extend: { colors: { objectbrand: '#abcdef' } } },
  },
}
