const plugin = require('tailwindcss/plugin')

// The import of the utilities has no source(none). Thus the selector is in the output.
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
