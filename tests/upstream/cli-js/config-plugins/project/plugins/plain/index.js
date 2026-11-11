// A directory with an index.js file.
module.exports = function ({ addUtilities }) {
  addUtilities({ '.plain-plugin': { color: 'lime' } })
}
