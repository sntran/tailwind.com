// The globals of Node.js that plugins use: `process` and `console`.
import { host } from './host'

function format(value: unknown): string {
  if (typeof value === 'string') return value
  if (value instanceof Error) return value.stack ? `${value.message}\n${value.stack}` : String(value)
  if (typeof value === 'object' && value !== null) {
    try {
      return JSON.stringify(value, null, 2)
    } catch {
      return String(value)
    }
  }
  return String(value)
}

// `console.log` writes to the standard output, as in Node.js: a line of a
// plugin comes before the CSS. `console.warn` and `console.error` write to
// the standard error.
function print(...values: unknown[]) {
  host('console', { error: false, text: values.map(format).join(' ') })
}

function printError(...values: unknown[]) {
  host('console', { error: true, text: values.map(format).join(' ') })
}

let global = globalThis as any

global.console = {
  log: print,
  info: print,
  debug: print,
  warn: printError,
  error: printError,
  trace: printError,
}

const BASE64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'

global.atob ??= (text: string): string => {
  let input = String(text).replace(/[\t\n\f\r ]/g, '').replace(/=+$/, '')
  let output = ''
  let bits = 0
  let value = 0
  for (let char of input) {
    let index = BASE64.indexOf(char)
    if (index === -1) throw new Error('The string to be decoded is not correctly encoded.')
    value = (value << 6) | index
    bits += 6
    if (bits >= 8) {
      bits -= 8
      output += String.fromCharCode((value >> bits) & 0xff)
    }
  }
  return output
}

global.btoa ??= (text: string): string => {
  let input = String(text)
  let output = ''
  for (let i = 0; i < input.length; i += 3) {
    let a = input.charCodeAt(i)
    let b = input.charCodeAt(i + 1)
    let c = input.charCodeAt(i + 2)
    if (a > 255 || b > 255 || c > 255) throw new Error('Invalid character')
    let triple = (a << 16) | ((b || 0) << 8) | (c || 0)
    output += BASE64[(triple >> 18) & 63] + BASE64[(triple >> 12) & 63]
    output += Number.isNaN(b) ? '=' : BASE64[(triple >> 6) & 63]
    output += Number.isNaN(c) ? '=' : BASE64[triple & 63]
  }
  return output
}

let env: Record<string, string> = host('env') ?? {}

global.process = {
  env,
  argv: [],
  platform: 'linux',
  versions: {},
  cwd: () => host('cwd'),
}

global.global = globalThis

// `structuredClone` for plain data: objects, arrays, maps, sets and dates.
global.structuredClone ??= function structuredClone<T>(value: T, seen = new Map<unknown, unknown>()): T {
  if (typeof value === 'function' || typeof value === 'symbol') {
    throw new Error(`${String(value)} could not be cloned.`)
  }
  if (value === null || typeof value !== 'object') return value
  if (seen.has(value)) return seen.get(value) as T
  if (value instanceof Date) return new Date(value.getTime()) as T
  if (value instanceof RegExp) return new RegExp(value.source, value.flags) as T
  if (Array.isArray(value)) {
    let copy: unknown[] = []
    seen.set(value, copy)
    for (let item of value) copy.push(structuredClone(item, seen))
    return copy as T
  }
  if (value instanceof Map) {
    let copy = new Map()
    seen.set(value, copy)
    for (let [key, item] of value) copy.set(structuredClone(key, seen), structuredClone(item, seen))
    return copy as T
  }
  if (value instanceof Set) {
    let copy = new Set()
    seen.set(value, copy)
    for (let item of value) copy.add(structuredClone(item, seen))
    return copy as T
  }
  let copy: Record<string, unknown> = {}
  seen.set(value, copy)
  for (let key of Object.keys(value)) copy[key] = structuredClone((value as any)[key], seen)
  return copy as T
}
