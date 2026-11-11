const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ addUtilities, addBase, theme }) {
  addUtilities({
    '.t-dot': { color: theme('colors.red.500') },
    '.t-bracket': { color: theme('colors[red][500]') },
    '.t-brand': { color: theme('colors.brand') },
    '.t-default': { color: theme('colors.nope.500', 'tomato') },
    '.t-missing': { color: String(theme('colors.nope.500')) },
    '.t-opacity': { color: theme('colors.red.500 / 50%') },
    '.t-opacity-num': { color: theme('colors.blue.500 / 0.25') },
    '.t-opacity-brand': { color: theme('colors.brand / 75%') },
    '.t-spacing': { padding: theme('spacing.4') },
    '.t-spacing-fraction': { padding: theme('spacing[2.5]') },
    '.t-spacing-custom': { padding: theme('spacing.gutter') },
    '.t-font-size': { fontSize: theme('fontSize.xl') },
    '.t-font-size-tuple': { '--value': JSON.stringify(theme('fontSize.xl')) },
    '.t-line-height': { lineHeight: theme('fontSize.xl[1].lineHeight') },
    '.t-font-family': { fontFamily: theme('fontFamily.sans') },
    '.t-font-display': { fontFamily: theme('fontFamily.display') },
    '.t-screen': { maxWidth: theme('screens.md') },
    '.t-screen-custom': { maxWidth: theme('screens.3xl') },
    '.t-radius': { borderRadius: theme('borderRadius.lg') },
    '.t-shadow': { boxShadow: theme('boxShadow.md') },
    '.t-width': { width: theme('width.1/2') },
    '.t-z': { zIndex: theme('zIndex.50') },
    '.t-opacity-scale': { opacity: theme('opacity.50') },
    '.t-duration': { transitionDuration: theme('transitionDuration.300') },
    '.t-typeof': {
      '--colors': typeof theme('colors'),
      '--red': Object.keys(theme('colors.red')).join(' '),
      '--screens': Object.keys(theme('screens')).join(' '),
    },
  })
  addBase({
    ':root': {
      '--from-plugin-brand': theme('colors.brand'),
      '--from-plugin-accent': theme('accentColor.brand', 'none'),
      '--from-plugin-bg': theme('backgroundColor.red.500'),
    },
  })
})
