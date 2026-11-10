const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ addUtilities, addComponents }) {
  addUtilities({
    '.btn-like': {
      '@apply rounded-md px-4 py-2 font-semibold': {},
      '&:hover': { '@apply bg-blue-600 text-white': {} },
    },
    '.center-xy': { '@apply flex items-center justify-center': {} },
  })
  addComponents({
    '.panel': {
      '@apply center-xy border': {},
      borderColor: 'gray',
    },
  })
})
