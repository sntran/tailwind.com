const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ addUtilities }) {
  addUtilities([
    { '.rotate-y-180': { transform: 'rotateY(180deg)' } },
    { '.skew-10': { transform: 'skewY(-10deg)' }, '.skew-15': { transform: 'skewY(-15deg)' } },
    { '.rotate-y-180': { backfaceVisibility: 'hidden' } },
  ])
  addUtilities({ '.skew-10': { transformOrigin: 'top left' } })
})
