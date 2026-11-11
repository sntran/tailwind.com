const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ addBase, addComponents, addUtilities, addVariant }) {
  addBase({ '.b-base': { order: '1' } })
  addComponents({ '.b-component': { order: '2' }, '.shared': { order: 'b' } })
  addUtilities({ '.b-utility': { order: '3' } })
  addVariant('b-variant', '&:is(.b)')
})
