import plugin from 'tailwindcss/plugin'

type Options = { prefix?: string; width?: string }

export default plugin.withOptions(function (options: Options = {}) {
  const prefix: string = options.prefix ?? 'x'
  return function ({ addComponents }) {
    addComponents({ [`.${prefix}-box`]: { border: `${options.width as string} solid currentColor` } })
  }
})
