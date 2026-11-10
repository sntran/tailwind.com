module.exports = {
  theme: {
    extend: {
      colors: { brand: '#3b82f6' },
      spacing: { gutter: '1.5rem' },
      keyframes: { wiggle: { '0%, 100%': { rotate: '-3deg' }, '50%': { rotate: '3deg' } } },
      animation: { wiggle: 'wiggle 1s ease-in-out infinite' },
      supports: { grid: 'display: grid' },
    },
  },
  plugins: [
    function ({ addBase, addUtilities }) {
      addBase({ h2: { fontSize: '1.5rem' } })
      addUtilities({ '.from-config': { color: 'green', '@media print': { color: 'black' } } })
    },
  ],
}
