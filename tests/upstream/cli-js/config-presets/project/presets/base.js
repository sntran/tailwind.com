const plugin = require('tailwindcss/plugin')

module.exports = {
  darkMode: 'selector',
  presets: [require('./nested.js')],
  theme: {
    screens: { phone: '400px', laptop: '1100px' },
    extend: {
      colors: { base: '#aaaaaa', shared: 'base-value' },
      spacing: { base: '1px' },
    },
  },
  plugins: [
    plugin(function ({ addUtilities }) {
      addUtilities({ '.from-base-preset': { color: 'red' } })
    }),
  ],
}
