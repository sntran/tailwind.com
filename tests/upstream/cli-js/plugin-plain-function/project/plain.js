module.exports = function ({ addUtilities, addVariant, matchUtilities }) {
  addUtilities({ '.plain-fn': { color: 'red' } })
  addVariant('plain-hover', '&:hover')
  matchUtilities({ 'plain-w': (value) => ({ width: value }) }, { values: { a: '1px' } })
}
