module.exports = {
  theme: {
    extend: {
      colors: ({ colors }) => ({
        primary: colors.indigo,
        neutral2: colors.slate[500],
      }),
      spacing: ({ theme }) => ({
        header: '4.5rem',
        'header-double': 'calc(' + theme('spacing.header') + ' * 2)',
      }),
      width: ({ theme }) => ({
        sidebar: theme('spacing.64'),
        page: theme('screens.lg'),
      }),
      borderColor: ({ theme }) => ({
        DEFAULT: theme('colors.gray.300'),
        strong: theme('colors.primary.700'),
      }),
      backgroundColor: (utils) => ({
        'utils-keys': Object.keys(utils).sort().join('-') === '' ? 'red' : 'green',
        mixed: utils.theme('colors.red.500 / 50%'),
      }),
      opacity: { 15: '0.15' },
      zIndex: { modal: '1000', 'behind': '-1' },
    },
  },
}
