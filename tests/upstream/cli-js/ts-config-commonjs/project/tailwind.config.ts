const plugin = require('tailwindcss/plugin')
const { accent } = require('./colors.ts')

type Theme = { extend: Record<string, Record<string, string>> }

const theme: Theme = {
  extend: {
    colors: { accent: accent as string },
    spacing: { wide: '6rem' },
  },
}

module.exports = {
  content: ['./index.html'],
  theme,
  plugins: [
    plugin(function ({ addUtilities }: { addUtilities: (rules: object) => void }) {
      addUtilities({ '.stripe': { 'border-left': '4px solid' } })
    }),
  ],
}
