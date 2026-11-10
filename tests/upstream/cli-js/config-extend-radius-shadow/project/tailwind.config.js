module.exports = {
  theme: {
    extend: {
      borderRadius: {
        DEFAULT: '0.3rem',
        card: '1.25rem',
        blob: '30% 70% 70% 30% / 30% 30% 70% 70%',
        '4xl': '2.5rem',
      },
      boxShadow: {
        DEFAULT: '0 1px 2px rgb(0 0 0 / 0.2)',
        card: '0 10px 30px -10px rgb(0 0 0 / 0.3)',
        outline2: '0 0 0 3px rgb(59 130 246 / 0.5), 0 1px 2px rgb(0 0 0 / 0.1)',
        'inner-lg': 'inset 0 4px 8px rgb(0 0 0 / 0.15)',
        flat: 'none',
      },
      dropShadow: {
        glow: '0 0 8px rgb(255 255 255 / 0.8)',
        double: ['0 1px 1px rgb(0 0 0 / 0.2)', '0 4px 4px rgb(0 0 0 / 0.1)'],
      },
      borderWidth: { 3: '3px', hair: '0.5px' },
      blur: { xs2: '1px' },
      aspectRatio: { cinema: '21 / 9' },
    },
  },
}
