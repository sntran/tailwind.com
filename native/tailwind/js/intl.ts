// `Intl.ListFormat` for English.
//
// The engine has no `Intl`. Tailwind CSS uses `Intl.ListFormat` for the
// list of names in an error message ("the `a` and `b` variants do not
// exist"), so the script engine needs it. This file has the English
// patterns only: each locale gives the English text.
// `tests/v8.mjs` compares it with Node.js.

type Style = 'long' | 'short' | 'narrow'
type Type = 'conjunction' | 'disjunction' | 'unit'

// The text between two elements, and before the last element of three
// or more.
const PATTERNS: Record<Type, Record<Style, { pair: string; end: string; middle: string }>> = {
  conjunction: {
    long: { pair: ' and ', end: ', and ', middle: ', ' },
    short: { pair: ' & ', end: ', & ', middle: ', ' },
    narrow: { pair: ', ', end: ', ', middle: ', ' },
  },
  disjunction: {
    long: { pair: ' or ', end: ', or ', middle: ', ' },
    short: { pair: ' or ', end: ', or ', middle: ', ' },
    narrow: { pair: ' or ', end: ', or ', middle: ', ' },
  },
  unit: {
    long: { pair: ', ', end: ', ', middle: ', ' },
    short: { pair: ', ', end: ', ', middle: ', ' },
    narrow: { pair: ' ', end: ' ', middle: ' ' },
  },
}

function option<T extends string>(value: unknown, name: string, allowed: T[], fallback: T): T {
  if (value === undefined) return fallback
  let text = String(value) as T
  if (!allowed.includes(text)) throw new RangeError(`Value ${text} out of range for Intl.ListFormat options property ${name}`)
  return text
}

class ListFormat {
  #style: Style
  #type: Type

  constructor(_locales?: unknown, options?: { style?: Style; type?: Type }) {
    let given = options === undefined ? {} : Object(options)
    this.#type = option(given.type, 'type', ['conjunction', 'disjunction', 'unit'], 'conjunction')
    this.#style = option(given.style, 'style', ['long', 'short', 'narrow'], 'long')
  }

  formatToParts(list: Iterable<string>): { type: 'element' | 'literal'; value: string }[] {
    let items: string[] = []
    for (let item of list ?? []) {
      if (typeof item !== 'string') throw new TypeError('Iterable yielded ' + String(item) + ' which is not a string')
      items.push(item)
    }
    let pattern = PATTERNS[this.#type][this.#style]
    let parts: { type: 'element' | 'literal'; value: string }[] = []
    items.forEach((item, index) => {
      if (index > 0) {
        let literal = items.length === 2 ? pattern.pair : index === items.length - 1 ? pattern.end : pattern.middle
        parts.push({ type: 'literal', value: literal })
      }
      parts.push({ type: 'element', value: item })
    })
    // V8 gives no part for an empty element, and one part for the
    // literals at its two sides.
    let result: typeof parts = []
    for (let part of parts) {
      if (part.value === '') continue
      let last = result[result.length - 1]
      if (part.type === 'literal' && last?.type === 'literal') {
        last.value += part.value
      } else {
        result.push(part)
      }
    }
    return result
  }

  format(list: Iterable<string>): string {
    return this.formatToParts(list)
      .map((part) => part.value)
      .join('')
  }

  resolvedOptions() {
    return { locale: 'en', type: this.#type, style: this.#style }
  }

  static supportedLocalesOf() {
    return ['en']
  }
}

export const Intl = { ListFormat }

let global = globalThis as any
if (global.Intl === undefined) {
  global.Intl = Intl
} else if (global.Intl.ListFormat === undefined) {
  global.Intl.ListFormat = ListFormat
}
