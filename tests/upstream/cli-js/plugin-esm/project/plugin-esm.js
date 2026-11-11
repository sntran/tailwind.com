import plugin from 'tailwindcss/plugin'

export default plugin(({ addUtilities }) => {
  addUtilities({ '.esm-in-js': { color: 'purple' } })
})
