// A small part of the module `util` of Node.js.

function inspect(value: unknown): string {
  if (typeof value === 'string') return `'${value}'`
  if (typeof value === 'function') return `[Function: ${value.name || '(anonymous)'}]`
  try {
    return JSON.stringify(value, null, 2) ?? String(value)
  } catch {
    return String(value)
  }
}

function format(first: unknown, ...rest: unknown[]): string {
  let values = rest.slice()
  let text =
    typeof first === 'string'
      ? first.replace(/%[sdjoO%]/g, (match) => {
          if (match === '%%') return '%'
          if (values.length === 0) return match
          let value = values.shift()
          return match === '%s' ? String(value) : match === '%d' ? String(Number(value)) : inspect(value)
        })
      : inspect(first)
  return [text, ...values.map((value) => (typeof value === 'string' ? value : inspect(value)))].join(' ')
}

const util: Record<string, unknown> = {
  // The function with no warning.
  deprecate: (fn: Function) => fn,
  inspect,
  format,
  inherits(constructor: any, parent: any) {
    Object.setPrototypeOf(constructor.prototype, parent.prototype)
    Object.setPrototypeOf(constructor, parent)
  },
  isDeepStrictEqual: (a: unknown, b: unknown) => JSON.stringify(a) === JSON.stringify(b),
}
util.default = util

export default util
