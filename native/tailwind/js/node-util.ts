// A part of the module `util` of Node.js: `inspect` and `format` (the
// default options of Node.js), `promisify`, `deprecate`, `types`.
import { Buffer } from './node-buffer'
import { TextDecoder, TextEncoder } from './encoding'

type Context = { depth: number; seen: unknown[]; indentationLvl: number; currentDepth: number; circular: Map<unknown, number> }

const BREAK_LENGTH = 80
const COMPACT = 3
const MAX_ARRAY_LENGTH = 100

function quote(text: string): string {
  let mark = "'"
  if (text.includes("'")) {
    if (!text.includes('"')) mark = '"'
    else if (!text.includes('`') && !text.includes('${')) mark = '`'
  }
  let escaped = text.replace(/[\u0000-\u001f\u007f-\u009f\\]|'/g, (char) => {
    switch (char) {
      case '\n':
        return '\\n'
      case '\t':
        return '\\t'
      case '\r':
        return '\\r'
      case '\b':
        return '\\b'
      case '\f':
        return '\\f'
      case '\v':
        return '\\v'
      case '\\':
        return '\\\\'
      case "'":
        return mark === "'" ? "\\'" : "'"
    }
    return '\\x' + char.charCodeAt(0).toString(16).toUpperCase().padStart(2, '0')
  })
  return mark + escaped + mark
}

function formatNumber(value: number): string {
  return Object.is(value, -0) ? '-0' : String(value)
}

function formatPrimitive(value: unknown): string {
  switch (typeof value) {
    case 'string':
      return quote(value)
    case 'number':
      return formatNumber(value)
    case 'bigint':
      return `${value}n`
    case 'symbol':
      return value.toString()
    default:
      return String(value)
  }
}

function formatKey(key: string | symbol): string {
  if (typeof key === 'symbol') return `[${key.toString()}]`
  if (key === '__proto__') return "['__proto__']"
  return /^[a-zA-Z_][a-zA-Z_0-9]*$/.test(key) ? key : quote(key)
}

function constructorName(value: object): string | null {
  let prototype = Object.getPrototypeOf(value)
  while (prototype !== null) {
    let descriptor = Object.getOwnPropertyDescriptor(prototype, 'constructor')
    if (descriptor && typeof descriptor.value === 'function' && descriptor.value.name !== '') {
      return descriptor.value.name
    }
    prototype = Object.getPrototypeOf(prototype)
  }
  return null
}

function prefix(value: object, fallback: string, size?: number): string {
  let name = constructorName(value)
  let tag = (value as any)[Symbol.toStringTag]
  let text = name === null ? `[${fallback}${size === undefined ? '' : `(${size})`}: null prototype] ` : `${name}${size === undefined ? '' : `(${size})`} `
  if (typeof tag === 'string' && tag !== '' && tag !== name) text += `[${tag}] `
  return text
}

function groupArrayElements(ctx: Context, output: string[], value: unknown[] | undefined): string[] {
  let totalLength = 0
  let maxLength = 0
  let outputLength = output.length
  if (value !== undefined && MAX_ARRAY_LENGTH < value.length) outputLength--
  let separatorSpace = 2
  let dataLen: number[] = new Array(outputLength)
  for (let i = 0; i < outputLength; i++) {
    let len = output[i].length
    dataLen[i] = len
    totalLength += len + separatorSpace
    if (maxLength < len) maxLength = len
  }
  let actualMax = maxLength + separatorSpace
  if (actualMax * 3 + ctx.indentationLvl < BREAK_LENGTH && (totalLength / actualMax > 5 || maxLength <= 6)) {
    let averageBias = Math.sqrt(actualMax - totalLength / output.length)
    let biasedMax = Math.max(actualMax - 3 - averageBias, 1)
    let columns = Math.min(
      Math.round(Math.sqrt(2.5 * biasedMax * outputLength) / biasedMax),
      Math.floor((BREAK_LENGTH - ctx.indentationLvl) / actualMax),
      COMPACT * 4,
      15,
    )
    if (columns <= 1) return output
    let tmp: string[] = []
    let maxLineLength: number[] = []
    for (let i = 0; i < columns; i++) {
      let lineLength = 0
      for (let j = i; j < output.length; j += columns) {
        if (dataLen[j] > lineLength) lineLength = dataLen[j]
      }
      maxLineLength.push(lineLength + separatorSpace)
    }
    let padStart = true
    if (value !== undefined) {
      for (let i = 0; i < output.length; i++) {
        if (typeof value[i] !== 'number' && typeof value[i] !== 'bigint') {
          padStart = false
          break
        }
      }
    }
    for (let i = 0; i < outputLength; i += columns) {
      let max = Math.min(i + columns, outputLength)
      let line = ''
      let j = i
      for (; j < max - 1; j++) {
        let cell = `${output[j]}, `
        line += padStart ? cell.padStart(maxLineLength[j - i], ' ') : cell.padEnd(maxLineLength[j - i], ' ')
      }
      line += padStart ? output[j].padStart(maxLineLength[j - i] - separatorSpace, ' ') : output[j]
      tmp.push(line)
    }
    if (outputLength < output.length) tmp.push(output[outputLength])
    output = tmp
  }
  return output
}

function reduceToSingleString(ctx: Context, output: string[], base: string, braces: [string, string], isArray: boolean, level: number, value?: unknown[]): string {
  let entries = output.length
  if (isArray && entries > 6) output = groupArrayElements(ctx, output, value)
  if (ctx.currentDepth - level < COMPACT && entries === output.length) {
    let start = output.length + ctx.indentationLvl + braces[0].length + base.length + 10
    let totalLength = output.length + start
    let below = totalLength + output.length <= BREAK_LENGTH
    for (let i = 0; below && i < output.length; i++) {
      totalLength += output[i].length
      if (totalLength > BREAK_LENGTH) below = false
    }
    if (below && (base === '' || !base.includes('\n'))) {
      let joined = output.join(', ')
      if (!joined.includes('\n')) return `${base ? `${base} ` : ''}${braces[0]} ${joined} ${braces[1]}`
    }
  }
  let indentation = `\n${' '.repeat(ctx.indentationLvl)}`
  return `${base ? `${base} ` : ''}${braces[0]}${indentation}  ${output.join(`,${indentation}  `)}${indentation}${braces[1]}`
}

function formatValue(ctx: Context, value: unknown, level: number): string {
  if (value === null || (typeof value !== 'object' && typeof value !== 'function')) return formatPrimitive(value)
  let object = value as any

  let custom = object[Symbol.for('nodejs.util.inspect.custom')]
  if (typeof custom === 'function' && custom !== inspect) {
    let result = custom.call(object, ctx.depth - level, { depth: ctx.depth }, inspect)
    if (result !== object) return typeof result === 'string' ? result : formatValue(ctx, result, level)
  }

  if (ctx.seen.includes(value)) {
    let index = ctx.circular.get(value)
    if (index === undefined) {
      index = ctx.circular.size + 1
      ctx.circular.set(value, index)
    }
    return `[Circular *${index}]`
  }

  let keys: (string | symbol)[] = Object.keys(object)
  for (let symbol of Object.getOwnPropertySymbols(object)) {
    if (Object.prototype.propertyIsEnumerable.call(object, symbol)) keys.push(symbol)
  }
  let braces: [string, string] = ['{', '}']
  let base = ''
  let isArray = false
  let output: string[] = []
  let entries: (() => string[]) | null = null

  if (Array.isArray(object)) {
    isArray = true
    keys = keys.filter((key) => typeof key === 'symbol' || !/^(0|[1-9][0-9]*)$/.test(key))
    let name = constructorName(object)
    braces = [name === 'Array' ? '[' : `${prefix(object, 'Array', object.length)}[`, ']']
    if (object.length === 0 && keys.length === 0) return `${braces[0]}]`
    entries = () => formatArray(ctx, object, level)
  } else if (ArrayBuffer.isView(object) && !(object instanceof DataView)) {
    if (object instanceof Buffer) {
      let bytes = Array.from(object.subarray(0, 50) as Uint8Array, (byte) => byte.toString(16).padStart(2, '0'))
      let more = object.length - 50
      return `<Buffer${bytes.length > 0 ? ' ' : ''}${bytes.join(' ')}${more > 0 ? ` ... ${more} more byte${more > 1 ? 's' : ''}` : ''}>`
    }
    isArray = true
    let list = object as unknown as ArrayLike<number | bigint>
    keys = keys.filter((key) => typeof key === 'symbol' || !/^(0|[1-9][0-9]*)$/.test(key))
    braces = [`${prefix(object, 'TypedArray', list.length)}[`, ']']
    if (list.length === 0 && keys.length === 0) return `${braces[0]}]`
    entries = () => {
      let result: string[] = []
      let length = Math.min(MAX_ARRAY_LENGTH, list.length)
      for (let i = 0; i < length; i++) result.push(formatPrimitive(list[i]))
      if (list.length > length) result.push(moreItems(list.length - length))
      return result
    }
  } else if (object instanceof Set) {
    braces = [`${prefix(object, 'Set', object.size)}{`, '}']
    if (object.size === 0 && keys.length === 0) return `${braces[0]}}`
    entries = () => {
      ctx.indentationLvl += 2
      let result = Array.from(object as Set<unknown>, (item) => formatValue(ctx, item, level + 1))
      ctx.indentationLvl -= 2
      return result
    }
  } else if (object instanceof Map) {
    braces = [`${prefix(object, 'Map', object.size)}{`, '}']
    if (object.size === 0 && keys.length === 0) return `${braces[0]}}`
    entries = () => {
      ctx.indentationLvl += 2
      let result = Array.from(object as Map<unknown, unknown>, ([key, item]) => `${formatValue(ctx, key, level + 1)} => ${formatValue(ctx, item, level + 1)}`)
      ctx.indentationLvl -= 2
      return result
    }
  } else if (typeof object === 'function') {
    let source = Function.prototype.toString.call(object)
    let kind = source.startsWith('class') ? 'class' : 'Function'
    if (kind === 'class') {
      base = `[class ${object.name || '(anonymous)'}`
      let parent = Object.getPrototypeOf(object)
      if (parent && parent.name) base += ` extends ${parent.name}`
      base += ']'
    } else {
      let type = /^async\s*function\s*\*|^async\s*\*/.test(source)
        ? 'AsyncGeneratorFunction'
        : /^async\b/.test(source)
          ? 'AsyncFunction'
          : /^function\s*\*/.test(source)
            ? 'GeneratorFunction'
            : 'Function'
      base = object.name ? `[${type}: ${object.name}]` : `[${type} (anonymous)]`
    }
    if (keys.length === 0) return base
  } else if (object instanceof RegExp) {
    base = RegExp.prototype.toString.call(object)
    if (keys.length === 0) return base
  } else if (object instanceof Date) {
    base = Number.isNaN(object.getTime()) ? 'Invalid Date' : object.toISOString()
    if (keys.length === 0) return base
  } else if (object instanceof Error) {
    let stack = typeof object.stack === 'string' && object.stack !== '' ? object.stack.replace(/\n+$/, '') : ''
    let head = `${object.name}${object.message ? `: ${object.message}` : ''}`
    base = stack === '' ? `[${head}]` : stack.includes(head) ? stack : `${head}\n${stack}`
    keys = keys.filter((key) => key !== 'cause' || Object.prototype.propertyIsEnumerable.call(object, key))
    if ('cause' in object && !keys.includes('cause') && Object.hasOwn(object, 'cause')) keys.push('cause')
    if (keys.length === 0) return base
  } else if (object instanceof Promise) {
    braces = ['Promise {', '}']
    if (keys.length === 0) return 'Promise { <state> }'
  } else if (object instanceof WeakMap || object instanceof WeakSet) {
    return `${constructorName(object)} { <items unknown> }`
  } else {
    let name = constructorName(object)
    let tag = object[Symbol.toStringTag]
    if (name !== 'Object' || (typeof tag === 'string' && tag !== '')) braces[0] = `${prefix(object, 'Object')}{`
    if (keys.length === 0) return `${braces[0]}}`
  }

  if (level > ctx.depth) {
    let name = constructorName(object) ?? 'Object'
    return Array.isArray(object) ? '[Array]' : `[${name === null ? 'Object: null prototype' : name}]`
  }

  ctx.seen.push(value)
  ctx.currentDepth = level
  if (entries) output = entries()
  for (let key of keys) output.push(formatProperty(ctx, object, key, level))
  ctx.seen.pop()

  let result = reduceToSingleString(ctx, output, base, braces, isArray, level, Array.isArray(object) ? object : undefined)
  let reference = ctx.circular.get(value)
  if (reference !== undefined) result = `<ref *${reference}> ${result}`
  return result
}

function moreItems(count: number): string {
  return `... ${count} more item${count > 1 ? 's' : ''}`
}

function formatArray(ctx: Context, value: unknown[], level: number): string[] {
  let output: string[] = []
  let length = Math.min(MAX_ARRAY_LENGTH, value.length)
  let holes = 0
  let flush = () => {
    if (holes > 0) output.push(`<${holes} empty item${holes > 1 ? 's' : ''}>`)
    holes = 0
  }
  for (let i = 0; i < value.length && output.length < length; i++) {
    if (!Object.hasOwn(value, i)) {
      holes++
      continue
    }
    flush()
    if (output.length >= length) break
    ctx.indentationLvl += 2
    output.push(formatValue(ctx, value[i], level + 1))
    ctx.indentationLvl -= 2
  }
  if (output.length < length) flush()
  let remaining = value.length - Math.min(value.length, MAX_ARRAY_LENGTH)
  if (remaining > 0) output.push(moreItems(remaining))
  return output
}

function formatProperty(ctx: Context, object: any, key: string | symbol, level: number): string {
  let descriptor = Object.getOwnPropertyDescriptor(object, key) ?? { value: object[key] }
  let text: string
  if ('value' in descriptor) {
    ctx.indentationLvl += 2
    text = formatValue(ctx, descriptor.value, level + 1)
    ctx.indentationLvl -= 2
  } else if (descriptor.get) {
    text = descriptor.set ? '[Getter/Setter]' : '[Getter]'
  } else {
    text = descriptor.set ? '[Setter]' : 'undefined'
  }
  let name = formatKey(key)
  if (descriptor.enumerable === false) name = `[${name}]`
  return `${name}: ${text}`
}

export function inspect(value: unknown, options?: { depth?: number | null } | boolean, depth?: number | null): string {
  let limit: number | null | undefined = 2
  if (typeof options === 'object' && options !== null) {
    if (options.depth !== undefined) limit = options.depth
  } else if (depth !== undefined) {
    limit = depth
  }
  let ctx: Context = { depth: limit === null ? Infinity : limit!, seen: [], indentationLvl: 0, currentDepth: 0, circular: new Map() }
  return formatValue(ctx, value, 0)
}
;(inspect as any).custom = Symbol.for('nodejs.util.inspect.custom')
;(inspect as any).defaultOptions = { depth: 2, breakLength: BREAK_LENGTH, compact: COMPACT, maxArrayLength: MAX_ARRAY_LENGTH }

function hasOwnToString(value: object): boolean {
  if (value instanceof Error || value instanceof Date) return true
  let prototype: any = value
  while (prototype !== null && prototype !== Object.prototype) {
    if (Object.hasOwn(prototype, 'toString')) return typeof prototype.toString === 'function'
    prototype = Object.getPrototypeOf(prototype)
  }
  return false
}

export function format(...values: unknown[]): string {
  let first = values[0]
  let index = 0
  let text = ''
  if (typeof first === 'string') {
    index = 1
    if (values.length === 1) return first
    text = first.replace(/%[sdifjoOc%]/g, (match) => {
      if (match === '%%') return '%'
      if (index >= values.length) return match
      let value = values[index++]
      switch (match) {
        case '%s':
          if (typeof value === 'number') return formatNumber(value)
          if (typeof value === 'bigint') return `${value}n`
          if (typeof value === 'object' && value !== null && !hasOwnToString(value)) return inspect(value, { depth: 0 })
          return String(value)
        case '%d':
          if (typeof value === 'bigint') return `${value}n`
          if (typeof value === 'symbol') return 'NaN'
          return formatNumber(Number(value))
        case '%i':
          if (typeof value === 'bigint') return `${value}n`
          if (typeof value === 'symbol') return 'NaN'
          return formatNumber(parseInt(value as string))
        case '%f':
          return typeof value === 'symbol' ? 'NaN' : formatNumber(parseFloat(value as string))
        case '%j':
          try {
            return JSON.stringify(value)
          } catch (error: any) {
            if (String(error?.message).includes('ircular')) return '[Circular]'
            throw error
          }
        case '%o':
          return inspect(value, { depth: 4 })
        case '%O':
          return inspect(value)
        default:
          return ''
      }
    })
  }
  for (; index < values.length; index++) {
    let value = values[index]
    text += (index === 0 ? '' : ' ') + (typeof value === 'string' ? value : inspect(value))
  }
  return text
}

const CUSTOM_PROMISIFY = Symbol.for('nodejs.util.promisify.custom')

function promisify(fn: any) {
  if (typeof fn !== 'function') {
    let error: any = new TypeError(`The "original" argument must be of type function. Received ${fn === null ? 'null' : typeof fn}`)
    error.code = 'ERR_INVALID_ARG_TYPE'
    throw error
  }
  if (typeof fn[CUSTOM_PROMISIFY] === 'function') return fn[CUSTOM_PROMISIFY]
  let wrapped = function (this: unknown, ...args: unknown[]) {
    return new Promise((resolve, reject) => {
      fn.call(this, ...args, (error: unknown, value: unknown) => {
        if (error) reject(error)
        else resolve(value)
      })
    })
  }
  Object.setPrototypeOf(wrapped, Object.getPrototypeOf(fn))
  return Object.defineProperties(wrapped, Object.getOwnPropertyDescriptors(fn))
}
promisify.custom = CUSTOM_PROMISIFY

function callbackify(fn: (...args: unknown[]) => Promise<unknown>) {
  return function (this: unknown, ...args: unknown[]) {
    let callback = args.pop() as (error: unknown, value?: unknown) => void
    fn.apply(this, args).then(
      (value) => callback(null, value),
      (error) => callback(error ?? new Error('Promise was rejected with a falsy value')),
    )
  }
}

// The warnings of `deprecate` that the program printed: one time for a code.
let warned = new Set<string>()

function deprecate<T extends Function>(fn: T, message: string, code?: string): T {
  let done = false
  let wrapped = function (this: unknown, ...args: unknown[]) {
    if (!done) {
      done = true
      if (code === undefined || !warned.has(code)) {
        if (code !== undefined) warned.add(code)
        let process = (globalThis as any).process
        if (!process?.noDeprecation) {
          ;(globalThis as any).console.error(`${code ? `[${code}] ` : ''}DeprecationWarning: ${message}`)
        }
      }
    }
    return new.target ? Reflect.construct(fn, args, new.target) : fn.apply(this, args)
  }
  Object.setPrototypeOf(wrapped, fn)
  if (fn.prototype) wrapped.prototype = fn.prototype
  return wrapped as unknown as T
}

function tagOf(value: unknown): string {
  return Object.prototype.toString.call(value)
}

export function isDeepStrictEqual(a: unknown, b: unknown, strict = true, seen: [unknown, unknown][] = []): boolean {
  if (strict ? Object.is(a, b) : a == b || (a !== a && b !== b)) return true
  if (a === null || b === null || typeof a !== 'object' || typeof b !== 'object') {
    return false
  }
  if (strict && Object.getPrototypeOf(a) !== Object.getPrototypeOf(b)) return false
  if (tagOf(a) !== tagOf(b)) return false
  if (seen.some(([x, y]) => x === a && y === b)) return true
  seen.push([a, b])
  let left = a as any
  let right = b as any
  if (a instanceof Date) return a.getTime() === (b as Date).getTime()
  if (a instanceof RegExp) return String(a) === String(b) && a.lastIndex === (b as RegExp).lastIndex
  if (a instanceof Error && (a.message !== right.message || a.name !== right.name)) return false
  if (ArrayBuffer.isView(a) && !(a instanceof DataView)) {
    if (left.length !== right.length) return false
    for (let i = 0; i < left.length; i++) if (!Object.is(left[i], right[i])) return false
  }
  if (a instanceof Set) {
    if (a.size !== right.size) return false
    outer: for (let item of a) {
      if (right.has(item)) continue
      if (typeof item === 'object' && item !== null) {
        for (let other of right) if (isDeepStrictEqual(item, other, strict, seen)) continue outer
      }
      return false
    }
  }
  if (a instanceof Map) {
    if (a.size !== right.size) return false
    for (let [key, item] of a) {
      if (!right.has(key) || !isDeepStrictEqual(item, right.get(key), strict, seen)) return false
    }
  }
  let keys = Object.keys(left)
  if (keys.length !== Object.keys(right).length) return false
  for (let key of keys) {
    if (!Object.hasOwn(right, key) || !isDeepStrictEqual(left[key], right[key], strict, seen)) return false
  }
  if (strict) {
    let symbols = Object.getOwnPropertySymbols(left).filter((symbol) => Object.prototype.propertyIsEnumerable.call(left, symbol))
    let others = Object.getOwnPropertySymbols(right).filter((symbol) => Object.prototype.propertyIsEnumerable.call(right, symbol))
    if (symbols.length !== others.length) return false
    for (let symbol of symbols) {
      if (!others.includes(symbol) || !isDeepStrictEqual(left[symbol], right[symbol], strict, seen)) return false
    }
  }
  return true
}

const TYPED_ARRAY = Object.getPrototypeOf(Uint8Array)

export const types: Record<string, (value: unknown) => boolean> = {
  isPromise: (value) => value instanceof Promise,
  isRegExp: (value) => value instanceof RegExp,
  isDate: (value) => value instanceof Date,
  isNativeError: (value) => value instanceof Error,
  isMap: (value) => value instanceof Map,
  isSet: (value) => value instanceof Set,
  isWeakMap: (value) => value instanceof WeakMap,
  isWeakSet: (value) => value instanceof WeakSet,
  isTypedArray: (value) => value instanceof TYPED_ARRAY,
  isUint8Array: (value) => value instanceof Uint8Array,
  isArrayBuffer: (value) => value instanceof ArrayBuffer,
  isAnyArrayBuffer: (value) => value instanceof ArrayBuffer || tagOf(value) === '[object SharedArrayBuffer]',
  isArrayBufferView: (value) => ArrayBuffer.isView(value),
  isDataView: (value) => value instanceof DataView,
  isAsyncFunction: (value) => tagOf(value) === '[object AsyncFunction]',
  isGeneratorFunction: (value) => tagOf(value) === '[object GeneratorFunction]' || tagOf(value) === '[object AsyncGeneratorFunction]',
  isGeneratorObject: (value) => tagOf(value) === '[object Generator]',
  isProxy: () => false,
  isBoxedPrimitive: (value) => value instanceof Number || value instanceof String || value instanceof Boolean || value instanceof BigInt || value instanceof Symbol,
  isSymbolObject: (value) => value instanceof Symbol,
  isStringObject: (value) => value instanceof String,
  isNumberObject: (value) => value instanceof Number,
  isBooleanObject: (value) => value instanceof Boolean,
  isModuleNamespaceObject: (value) => tagOf(value) === '[object Module]',
}

const util: Record<string, unknown> = {
  deprecate,
  inspect,
  format,
  promisify,
  callbackify,
  types,
  TextEncoder,
  TextDecoder,
  inherits(constructor: any, parent: any) {
    Object.defineProperty(constructor, 'super_', { value: parent, writable: true, configurable: true })
    Object.setPrototypeOf(constructor.prototype, parent.prototype)
  },
  isDeepStrictEqual: (a: unknown, b: unknown) => isDeepStrictEqual(a, b),
  isArray: Array.isArray,
  stripVTControlCharacters: (text: string) => String(text).replace(/\u001b\[[0-9;]*[A-Za-z]/g, ''),
}
util.default = util

export default util
