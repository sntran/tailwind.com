const plugin = require('tailwindcss/plugin')
const withOptions = require('./plugins/with-options')

module.exports = {
  theme: {
    extend: { colors: { fromconfig: '#111111' } },
  },
  plugins: [
    require('./plugins/local.js'),
    require('./plugins/plain'),
    withOptions({ name: 'opt', color: 'gold' }),
    withOptions,
    plugin(function ({ addVariant }) {
      addVariant('inline-variant', '&:is(.inline)')
    }),
    function ({ addUtilities }) {
      addUtilities({ '.inline-plain': { color: 'pink' } })
    },
    { handler: ({ addUtilities }) => addUtilities({ '.object-plugin': { color: 'cyan' } }) },
  ],
}
