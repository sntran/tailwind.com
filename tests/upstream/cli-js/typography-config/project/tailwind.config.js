module.exports = {
  theme: {
    extend: {
      typography: ({ theme }) => ({
        DEFAULT: {
          css: {
            maxWidth: '80ch',
            a: { color: theme('colors.sky.600'), '&:hover': { color: theme('colors.sky.800') } },
          },
        },
        brand: {
          css: {
            '--tw-prose-body': theme('colors.teal.900'),
            '--tw-prose-headings': theme('colors.teal.950'),
          },
        },
      }),
    },
  },
  plugins: [require('@tailwindcss/typography')({ className: 'wysiwyg' })],
}
