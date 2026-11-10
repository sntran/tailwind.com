const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ addUtilities }) {
  addUtilities({
    '.content-auto': { 'content-visibility': 'auto' },
    '.scroll-snap-none': { scrollSnapType: 'none' },
    '.text-trim': {
      textBoxTrim: 'trim-both',
      textBoxEdge: 'cap alphabetic',
    },
    '.multi-a, .multi-b': { color: 'red' },
    '.fallback': { display: ['flex', 'grid'] },
    '.never-used': { color: 'blue' },
  })
})
