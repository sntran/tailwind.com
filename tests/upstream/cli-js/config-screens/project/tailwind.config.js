module.exports = {
  theme: {
    screens: {
      sm: '640px',
      md: { min: '768px' },
      lg: { min: '1024px', max: '1279px' },
      'only-max': { max: '500px' },
      tall: { raw: '(min-height: 800px)' },
      print2: { raw: 'print' },
      multi: [{ min: '668px', max: '767px' }, { min: '868px' }],
      rem: '80rem',
      xl: '1280px',
    },
  },
}
