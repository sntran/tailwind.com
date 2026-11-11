const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ matchUtilities, theme }) {
  matchUtilities(
    {
      type: (value, { modifier }) => ({
        fontSize: value,
        lineHeight: modifier === null ? '1' : modifier,
      }),
    },
    {
      values: { sm: '0.875rem', lg: '1.25rem' },
      modifiers: { tight: '1.1', loose: '1.8' },
    },
  )
  matchUtilities(
    {
      ring2: (value, { modifier }) => ({
        '--ring2-color': value,
        '--ring2-modifier': modifier === null ? 'none' : modifier,
      }),
    },
    { values: { a: 'red', b: 'blue' }, modifiers: 'any' },
  )
  // A color utility. The compiler applies the opacity modifier.
  matchUtilities(
    { glow: (value) => ({ '--glow-color': value }) },
    { values: theme('colors'), type: 'color' },
  )
})
