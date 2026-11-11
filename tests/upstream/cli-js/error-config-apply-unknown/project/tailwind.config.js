module.exports = {
  plugins: [
    function ({ addComponents }) {
      addComponents({ '.bad-apply': { '@apply no-such-utility': {} } })
    },
  ],
}
