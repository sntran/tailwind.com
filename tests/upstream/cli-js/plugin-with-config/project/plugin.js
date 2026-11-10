const plugin = require('tailwindcss/plugin')

// The second argument of plugin() is a config.
module.exports = plugin(
  function ({ addUtilities, matchUtilities, theme }) {
    addUtilities({ '.uses-config': { color: theme('colors.plug.500'), gap: theme('spacing.gutter') } })
    matchUtilities({ depth: (value) => ({ '--depth': value }) }, { values: theme('depth') })
  },
  {
    theme: {
      depth: { 1: '10px', 2: '20px' },
      extend: {
        colors: { plug: { 500: '#00ff88', 900: '#003311' } },
        spacing: { gutter: '1.25rem' },
        screens: { wide: '1500px' },
      },
    },
  },
)
