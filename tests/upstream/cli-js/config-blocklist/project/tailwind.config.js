module.exports = {
  blocklist: ['container', 'collapse', 'hover:underline', 'bg-red-500', 'p-[3px]', 'plug-blocked'],
  plugins: [
    function ({ addUtilities }) {
      addUtilities({ '.plug-blocked': { color: 'red' }, '.plug-kept': { color: 'green' } })
    },
  ],
}
