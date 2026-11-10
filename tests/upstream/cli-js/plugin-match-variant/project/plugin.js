const plugin = require('tailwindcss/plugin')

module.exports = plugin(function ({ matchVariant }) {
  matchVariant('nth', (value) => '&:nth-child(' + value + ')', {
    values: { 1: '1', 2: '2', odd: 'odd', DEFAULT: 'n' },
  })
  matchVariant('tooltip', (value) => ['&[data-tip="' + value + '"]', '&[data-tooltip="' + value + '"]'], {
    values: { top: 'top', bottom: 'bottom' },
  })
  matchVariant('minh', (value) => '@media (min-height: ' + value + ')', {
    values: { sm: '400px', lg: '800px', md: '600px' },
    sort(a, z) {
      return parseInt(a.value) - parseInt(z.value)
    },
  })
  matchVariant('theme', (value) => '.theme-' + value + ' &')
  matchVariant(
    'pane',
    (value, { modifier }) =>
      modifier ? '@container ' + modifier + ' (min-width: ' + value + ')' : '@container (min-width: ' + value + ')',
    { values: { sm: '320px', lg: '640px' } },
  )
  matchVariant('group-state', (value, { modifier }) =>
    modifier ? ':merge(.group\\/' + modifier + ')[data-state="' + value + '"] &' : ':merge(.group)[data-state="' + value + '"] &',
    { values: { open: 'open', closed: 'closed' } },
  )
})
