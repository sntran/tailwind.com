const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ addBase, addComponents, addUtilities, addVariant }) {
  addBase({ '.a-base': { order: '1' } })
  addComponents({ '.a-component': { order: '2' }, '.shared': { order: 'a' } })
  addUtilities({ '.a-utility': { order: '3' }, '.css-utility': { order: 'a-over-css' } })
  addVariant('a-variant', '&:is(.a)')
})
