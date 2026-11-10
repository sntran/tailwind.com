const helper = require('./helper.cjs')

module.exports = ({ addUtilities }) => {
  addUtilities({ '.arrow-fn': { color: helper.color } })
}
