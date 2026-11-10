const plugin = require('tailwindcss/plugin')

module.exports = plugin.withOptions(
  function (options = {}) {
    let name = options['class-name'] || 'box'
    return function ({ addUtilities, matchUtilities }) {
      addUtilities({
        ['.' + name]: {
          color: options.color || 'black',
          '--debug': String(options.debug),
          '--options': JSON.stringify(options),
        },
      })
      let values = {}
      for (let size of options.sizes || []) values[size] = size * 4 + 'px'
      matchUtilities({ [name + '-size']: (value) => ({ width: value }) }, { values })
    }
  },
  function (options = {}) {
    return {
      theme: {
        extend: {
          colors: { [options['class-name'] || 'box']: options.color || 'black' },
        },
      },
    }
  },
)
