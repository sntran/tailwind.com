const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ matchUtilities, theme }) {
  // The same root with two types. The type selects the function for an arbitrary value.
  matchUtilities(
    { mark: (value) => ({ 'text-decoration-color': value }) },
    { values: { brand: '#0ea5e9', accent: 'rgb(255 0 128)' }, type: 'color' },
  )
  matchUtilities(
    { mark: (value) => ({ 'text-decoration-thickness': value }) },
    { values: { thin: '1px', thick: '4px' }, type: 'length' },
  )
  matchUtilities(
    { shade: (value) => ({ 'accent-color': value }) },
    { values: theme('colors'), type: ['color', 'any'] },
  )
  matchUtilities({ anything: (value) => ({ '--anything': value }) }, { type: 'any' })
  matchUtilities(
    { span: (value) => ({ 'column-span': value }) },
    { values: { all: 'all' }, type: ['number', 'percentage'] },
  )
})
