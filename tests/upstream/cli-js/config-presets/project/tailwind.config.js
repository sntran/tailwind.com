module.exports = {
  presets: [require('./presets/base.js'), require('./presets/brand.js')],
  theme: {
    extend: {
      colors: { own: '#00ff00', shared: 'config-wins' },
      spacing: { own: '3px' },
    },
  },
}
