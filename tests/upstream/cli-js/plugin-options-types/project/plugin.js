const plugin = require('tailwindcss/plugin')

module.exports = plugin.withOptions(function (options) {
  return function ({ addUtilities }) {
    let decls = {}
    for (let key of Object.keys(options)) {
      let value = options[key]
      let name = key.startsWith('--') ? '--opt' + key : '--opt-' + key
      decls[name] = JSON.stringify(value)
      decls[name + '-type'] = Array.isArray(value) ? 'array' : value === null ? 'null' : typeof value
    }
    decls['--keys'] = Object.keys(options).join(' ')
    addUtilities({ '.options': decls })
  }
})
