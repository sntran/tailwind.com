const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ matchUtilities }) {
  matchUtilities(
    {
      stripes: (value) => [
        { backgroundSize: value + ' ' + value },
        { backgroundImage: 'linear-gradient(45deg, currentColor 25%, transparent 25%)' },
      ],
      halo: (value) => ({
        boxShadow: '0 0 0 ' + value + ' currentColor',
        '&:hover': { boxShadow: '0 0 0 calc(' + value + ' * 2) currentColor' },
        '@media (prefers-reduced-motion: no-preference)': { transition: 'box-shadow 150ms' },
        '& > .halo-inner': { padding: value },
      }),
      nothing: (value) => (value === 'none' ? {} : { '--nothing': value }),
    },
    { values: { sm: '4px', lg: '16px', none: 'none' } },
  )
})
