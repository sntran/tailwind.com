const defaultTheme = require('tailwindcss/defaultTheme')

module.exports = {
  theme: {
    extend: {
      fontFamily: {
        display: ['Oswald', 'ui-serif', 'serif'],
        body: '"Open Sans", sans-serif',
        code: ['"Fira Code"', ...defaultTheme.fontFamily.mono],
        settings: ['Inter', { fontFeatureSettings: '"cv11", "ss01"', fontVariationSettings: '"opsz" 32' }],
      },
      fontSize: {
        tiny: '0.625rem',
        lead: ['1.375rem', '1.9'],
        hero: ['4rem', { lineHeight: '1.05', letterSpacing: '-0.02em', fontWeight: '800' }],
        loose: ['1rem', { lineHeight: '2rem' }],
        '2xs': ['0.6875rem', '1rem'],
        base: ['1.0625rem', '1.6'],
      },
      fontWeight: { hairline: '50', chunky: 950 },
      lineHeight: { extra: '2.5', 13: '3.25rem' },
      letterSpacing: { mega: '0.25em' },
    },
  },
}
