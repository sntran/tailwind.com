const { rule } = require('./rule.cts')

module.exports = ({ addUtilities }) => addUtilities({ '.from-cjs': rule })
