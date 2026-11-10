/** @type {import('tailwindcss').Config} */
module.exports = {
  theme: {
    extend: {
      colors: {
        brand: {
          DEFAULT: '#0f766e',
          light: '#5eead4',
          dark: { DEFAULT: '#134e4a', deep: '#042f2e' },
          50: 'rgb(240 253 250)',
          500: 'hsl(173 80% 40%)',
          'with-alpha': 'rgb(15 118 110 / <alpha-value>)',
        },
        flat: 'tomato',
        'kebab-name': '#010203',
        camelName: '#040506',
        'var-color': 'var(--my-color)',
        current2: 'currentColor',
        red: { 500: '#ff0000', 1000: '#330000' },
      },
    },
  },
}
