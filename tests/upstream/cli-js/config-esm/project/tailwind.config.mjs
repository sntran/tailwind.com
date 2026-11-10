import colors from 'tailwindcss/colors'
import defaultTheme from 'tailwindcss/defaultTheme'
import plugin from 'tailwindcss/plugin'
import { sizes } from './sizes.mjs'

export default {
  theme: {
    extend: {
      colors: {
        accent: colors.emerald,
        ink: colors.zinc[900],
      },
      spacing: sizes,
      screens: {
        xs: '30rem',
        '3xl': '1920px',
      },
      fontFamily: {
        sans: ['"Inter Var"', ...defaultTheme.fontFamily.sans],
      },
      maxWidth: {
        prose2: '70ch',
      },
    },
  },
  plugins: [
    plugin(({ addUtilities }) => {
      addUtilities({ '.from-esm-config': { color: colors.rose[500] } })
    }),
  ],
}
