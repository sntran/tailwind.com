const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ matchUtilities }) {
  matchUtilities({ 'Bad_Name!': (value) => ({ color: value }) }, { values: { a: 'red' } })
})
