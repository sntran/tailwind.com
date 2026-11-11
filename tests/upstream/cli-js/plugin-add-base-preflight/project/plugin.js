const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ addBase }) {
  addBase({
    h1: { fontSize: '2rem' },
    '@font-face': { fontFamily: 'Plugin Font', src: 'url("/font.woff2")' },
  })
})
