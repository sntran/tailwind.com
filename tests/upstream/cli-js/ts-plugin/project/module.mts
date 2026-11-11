import plugin from 'tailwindcss/plugin'
import { basename } from 'node:path'

const name = basename(import.meta.filename) satisfies string

export default plugin(({ addUtilities }) => {
  addUtilities({ '.mts-rule': { content: JSON.stringify(name) } })
})
