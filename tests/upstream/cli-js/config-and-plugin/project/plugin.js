const plugin = require('tailwindcss/plugin')

module.exports = plugin(
  function ({ addUtilities, theme }) {
    addUtilities({
      '.seen-by-plugin': {
        '--brand': theme('colors.brand'),
        '--css-only': theme('colors.css-only'),
        '--config-only': theme('colors.config-only'),
        '--plugin-only': theme('colors.plugin-only'),
        '--late': theme('colors.late'),
        '--gutter': theme('spacing.gutter'),
      },
    })
  },
  { theme: { extend: { colors: { brand: '#444444', 'plugin-only': '#555555' } } } },
)
