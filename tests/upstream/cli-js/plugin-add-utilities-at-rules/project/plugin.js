const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ addUtilities }) {
  addUtilities({
    '.motion-ok': {
      '@media (prefers-reduced-motion: no-preference)': {
        transition: 'all 150ms',
      },
    },
    '.grid-ok': {
      display: 'block',
      '@supports (display: grid)': {
        display: 'grid',
        '&:hover': { gap: '1rem' },
      },
    },
    '.print-hidden': {
      '@media print': { display: 'none' },
    },
    '.deep': {
      '@media (min-width: 640px)': {
        '@supports (gap: 1px)': {
          '@container (min-width: 10rem)': { gap: '1px' },
        },
      },
      '@layer plugin-layer': { color: 'red' },
    },
    '.spin-slow': {
      animation: 'plugin-spin 3s linear infinite',
      '@keyframes plugin-spin': {
        from: { transform: 'rotate(0deg)' },
        to: { transform: 'rotate(360deg)' },
      },
    },
  })
})
