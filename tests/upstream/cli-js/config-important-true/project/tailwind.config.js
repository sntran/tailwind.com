const plugin = require('tailwindcss/plugin')

module.exports = {
  important: true,
  plugins: [
    plugin(function ({ addUtilities, addComponents, addBase }) {
      addUtilities({ '.plug-util': { color: 'red' } })
      addComponents({ '.plug-comp': { color: 'green' } })
      addBase({ h1: { color: 'black' } })
    }),
  ],
}
