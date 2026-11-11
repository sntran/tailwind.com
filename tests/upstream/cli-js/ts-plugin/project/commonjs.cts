import plugin = require('tailwindcss/plugin')
import path = require('node:path')

type Rule = { [property: string]: string }

const rule: Rule = { content: JSON.stringify(path.posix.basename(__filename)) }

export = plugin(function ({ addUtilities }): void {
  addUtilities({ '.cts-rule': rule })
})
