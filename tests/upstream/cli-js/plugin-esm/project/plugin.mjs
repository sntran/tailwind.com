import plugin from 'tailwindcss/plugin'
import colors from 'tailwindcss/colors'
import defaultTheme from 'tailwindcss/defaultTheme'
import { shade } from './lib/shade.mjs'

export default plugin(function ({ addUtilities, addVariant }) {
  addUtilities({
    '.esm-util': {
      color: colors.sky[500],
      fontFamily: defaultTheme.fontFamily.mono.join(', '),
      '--shade': shade(3),
    },
  })
  addVariant('esm-variant', '&:is(.esm)')
})
