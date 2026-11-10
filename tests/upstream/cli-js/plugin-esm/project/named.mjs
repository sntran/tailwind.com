// A plain function as the default export. There is no plugin() wrapper.
export const unused = 1

export default function ({ addUtilities }) {
  addUtilities({ '.esm-plain': { color: 'teal' } })
}
