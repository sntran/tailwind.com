const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ addVariant }) {
  addVariant('hover-device', {
    '@media (hover: hover)': {
      '&:hover': '@slot',
    },
  })
  addVariant('fancy', {
    '&:hover': '@slot',
    '&:focus-within': { '@slot': {} },
    '@supports (display: grid)': {
      '@media (min-width: 640px)': {
        '.fancy-parent &': '@slot',
      },
    },
  })
  addVariant('slot-only', { '@slot': {} })
  addVariant('rtl2', { '[dir="rtl"] &': { '@slot': {} } })
})
