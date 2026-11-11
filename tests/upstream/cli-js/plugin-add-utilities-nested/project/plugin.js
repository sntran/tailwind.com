const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ addUtilities }) {
  addUtilities({
    '.card-link': {
      color: 'inherit',
      '&:hover': { color: 'red' },
      '&::after': { content: '"→"' },
      '> svg': { width: '1em' },
      '.dark &': { color: 'white' },
      '& + &': { marginLeft: '1rem' },
    },
    '.stack > * + *': { marginTop: '1rem' },
    '.group-box:hover .inner': { opacity: '1' },
    '.clearfix::after': { content: '""', display: 'table', clear: 'both' },
  })
})
