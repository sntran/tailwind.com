const plugin = require('tailwindcss/plugin')

module.exports = {
  prefix: 'tw',
  theme: { extend: { colors: { brand: '#00f' } } },
  plugins: [
    plugin(function ({ addUtilities, addComponents, matchUtilities, config, prefix }) {
      addUtilities({ '.plug-util': { color: 'red', '--prefix': JSON.stringify(config('prefix')), '--fn': prefix('x') } })
      addComponents({ '.plug-comp': { '@apply tw:flex': {} } })
      matchUtilities({ 'plug-w': (value) => ({ width: value }) }, { values: { 1: '1px' } })
    }),
  ],
}
