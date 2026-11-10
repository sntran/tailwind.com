const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ matchComponents, theme }) {
  matchComponents(
    {
      avatar: (value) => ({
        width: value,
        height: value,
        borderRadius: '9999px',
        '& > img': { width: '100%' },
      }),
    },
    { values: { sm: '2rem', md: '3rem', DEFAULT: '2.5rem' } },
  )
  matchComponents(
    {
      icon: (value) => ({
        mask: 'url(' + value.path + ')',
        width: value.size,
      }),
    },
    { values: { star: { path: '/star.svg', size: '1rem' }, moon: { path: '/moon.svg', size: '2rem' } } },
  )
  matchComponents(
    { tag: (value, { modifier }) => ({ color: value, '--tag-mod': String(modifier) }) },
    { values: theme('colors'), type: 'color', modifiers: 'any' },
  )
})
