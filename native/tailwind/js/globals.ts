// The globals of Node.js that plugins use: `console`, `process`, `Buffer`,
// `URL`, the text encoders, the timers, `structuredClone`.
import './intl'
import './sort'
import { denied } from './denied'
import { base64Decode, base64Encode, TextDecoder, TextEncoder } from './encoding'
import { host } from './host'
import { Buffer } from './node-buffer'
import { URL, URLSearchParams } from './node-url'
import { format, inspect } from './node-util'
import { install as installTimers } from './timers'

// `console.log` writes to the standard output, as in Node.js: a line of a
// plugin comes before the CSS. `console.warn` and `console.error` write to
// the standard error.
function print(...values: unknown[]) {
  host('console', { error: false, text: format(...values) })
}

function printError(...values: unknown[]) {
  host('console', { error: true, text: format(...values) })
}

let global = globalThis as any

global.console = {
  log: print,
  info: print,
  debug: print,
  warn: printError,
  error: printError,
  trace: (...values: unknown[]) => printError(`Trace: ${format(...values)}`),
  dir: (value: unknown, options?: { depth?: number | null }) => print(inspect(value, options)),
  assert: (value: unknown, ...values: unknown[]) => {
    if (!value) printError(values.length > 0 ? `Assertion failed: ${format(...values)}` : 'Assertion failed')
  },
  group: print,
  groupEnd() {},
  time() {},
  timeEnd() {},
  timeLog() {},
  table: (value: unknown) => print(inspect(value)),
}

function invalidCharacter(): never {
  let error: any = new Error('Invalid character')
  error.name = 'InvalidCharacterError'
  error.code = 5
  throw error
}

global.atob = (text: unknown): string => {
  let input = String(text).replace(/[\t\n\f\r ]/g, '')
  if (input.length % 4 === 0) input = input.replace(/==?$/, '')
  if (input.length % 4 === 1 || /[^A-Za-z0-9+/]/.test(input)) invalidCharacter()
  let bytes = base64Decode(input)
  let output = ''
  for (let i = 0; i < bytes.length; i++) output += String.fromCharCode(bytes[i])
  return output
}

global.btoa = (text: unknown): string => {
  let input = String(text)
  let bytes = new Uint8Array(input.length)
  for (let i = 0; i < input.length; i++) {
    let code = input.charCodeAt(i)
    if (code > 255) invalidCharacter()
    bytes[i] = code
  }
  return base64Encode(bytes)
}

// The environment of the program, the arguments and the system, from the
// host.
let info: { env: Record<string, string>; argv: string[]; platform: string; arch: string; versions: Record<string, string> } = host('process')

let process: any = {
  env: info.env,
  argv: info.argv,
  argv0: info.argv[0],
  execArgv: [],
  execPath: info.argv[0],
  platform: info.platform,
  arch: info.arch,
  version: `v${info.versions.node}`,
  versions: info.versions,
  release: { name: 'node' },
  pid: 1,
  title: 'tailwindcss',
  exitCode: undefined,
  stdout: { isTTY: false, write: (text: unknown) => (host('console', { error: false, text: String(text).replace(/\n$/, '') }), true) },
  stderr: { isTTY: false, write: (text: unknown) => (host('console', { error: true, text: String(text).replace(/\n$/, '') }), true) },
  cwd: () => host('cwd'),
  chdir: denied('process.chdir', 'process'),
  exit: denied('process.exit', 'process'),
  kill: denied('process.kill', 'process'),
  hrtime: Object.assign(() => [0, 0], { bigint: () => 0n }),
  uptime: () => 0,
  memoryUsage: () => ({ rss: 0, heapTotal: 0, heapUsed: 0, external: 0, arrayBuffers: 0 }),
  nextTick: (fn: Function, ...args: unknown[]) => {
    Promise.resolve().then(() => fn(...args))
  },
  emitWarning: (warning: unknown, type?: unknown) => {
    let name = typeof type === 'string' ? type : warning instanceof Error ? warning.name : 'Warning'
    printError(`${name}: ${warning instanceof Error ? warning.message : String(warning)}`)
  },
  on: () => process,
  once: () => process,
  off: () => process,
  removeListener: () => process,
  emit: () => false,
}

global.process = process
global.global = globalThis
global.Buffer = Buffer
global.URL = URL
global.URLSearchParams = URLSearchParams
global.TextEncoder = TextEncoder
global.TextDecoder = TextDecoder
installTimers(global)

for (let name of ['fetch', 'XMLHttpRequest', 'WebSocket', 'EventSource']) {
  global[name] = denied(name, 'network')
}

function cloneError(value: unknown): never {
  let error: any = new Error(`${typeof value === 'function' ? Function.prototype.toString.call(value) : String(value)} could not be cloned.`)
  error.name = 'DataCloneError'
  error.code = 25
  throw error
}

// `structuredClone` for the values of JavaScript: plain objects, arrays,
// maps, sets, dates, regular expressions, errors, boxed values, array
// buffers and their views.
function clone(value: any, seen: Map<unknown, unknown>): any {
  if (typeof value === 'function' || typeof value === 'symbol') cloneError(value)
  if (value === null || typeof value !== 'object') return value
  if (seen.has(value)) return seen.get(value)
  let copy: any
  if (value instanceof Date) copy = new Date(value.getTime())
  else if (value instanceof RegExp) copy = new RegExp(value.source, value.flags)
  else if (value instanceof Boolean || value instanceof Number || value instanceof String || value instanceof BigInt) copy = Object(value.valueOf())
  else if (value instanceof ArrayBuffer) copy = value.slice(0)
  else if (ArrayBuffer.isView(value)) {
    let buffer = clone(value.buffer, seen)
    let length = value instanceof DataView ? value.byteLength : (value as any).length
    // A `Buffer` becomes a `Uint8Array`, as in Node.js.
    let constructor: any = value instanceof Buffer ? Uint8Array : value.constructor
    copy = new constructor(buffer, value.byteOffset, length)
  }
  if (copy !== undefined) {
    seen.set(value, copy)
    return copy
  }
  if (Array.isArray(value)) {
    copy = new Array(value.length)
    seen.set(value, copy)
    for (let key of Object.keys(value)) copy[key] = clone((value as any)[key], seen)
    return copy
  }
  if (value instanceof Map) {
    copy = new Map()
    seen.set(value, copy)
    for (let [key, item] of value) copy.set(clone(key, seen), clone(item, seen))
    return copy
  }
  if (value instanceof Set) {
    copy = new Set()
    seen.set(value, copy)
    for (let item of value) copy.add(clone(item, seen))
    return copy
  }
  if (value instanceof Error) {
    let constructors: Record<string, ErrorConstructor> = { EvalError, RangeError, ReferenceError, SyntaxError, TypeError, URIError }
    let constructor = constructors[value.name] ?? Error
    copy = new constructor(value.message)
    seen.set(value, copy)
    if (!Object.hasOwn(constructors, value.name) && value.name !== 'Error') {
      Object.defineProperty(copy, 'name', { value: 'Error', writable: true, configurable: true })
    }
    if (typeof value.stack === 'string') Object.defineProperty(copy, 'stack', { value: value.stack, writable: true, configurable: true })
    if (Object.hasOwn(value, 'cause')) Object.defineProperty(copy, 'cause', { value: clone((value as any).cause, seen), writable: true, configurable: true })
    return copy
  }
  if (value instanceof WeakMap || value instanceof WeakSet || value instanceof Promise) {
    let error: any = new Error(`#<${value.constructor.name}> could not be cloned.`)
    error.name = 'DataCloneError'
    error.code = 25
    throw error
  }
  copy = {}
  seen.set(value, copy)
  for (let key of Object.keys(value)) copy[key] = clone(value[key], seen)
  return copy
}

global.structuredClone = (value: unknown) => clone(value, new Map())
