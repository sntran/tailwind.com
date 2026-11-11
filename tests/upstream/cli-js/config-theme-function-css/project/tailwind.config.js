module.exports = {
  theme: {
    extend: {
      colors: {
        brand: { DEFAULT: '#7c3aed', dark: '#4c1d95' },
      },
      spacing: { gutter: '1.5rem', sidebar: '16rem' },
      fontFamily: { display: ['Oswald', 'sans-serif'] },
      fontSize: { hero: ['4rem', { lineHeight: '1.1' }] },
      borderRadius: { card: '1rem' },
      boxShadow: { card: '0 2px 4px rgb(0 0 0 / 0.2)' },
      zIndex: { modal: '1000' },
      screens: { tablet: '700px', tall: { raw: '(min-height: 800px)' } },
    },
  },
}
