const plugin = require('tailwindcss/plugin')

module.exports = {
  important: '#app',
  plugins: [
    plugin(function ({ addUtilities, addComponents, addBase, addVariant }) {
      addUtilities({ '.plug-util': { color: 'red', '&:hover': { color: 'blue' } } })
      addComponents({ '.plug-comp': { color: 'green' } })
      addBase({ h1: { color: 'black' } })
      addVariant('in-nav', 'nav &')
    }),
  ],
}
