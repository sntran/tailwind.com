// A part of `Buffer` of Node.js: bytes from text and to text.
import { base64Decode, base64Encode, hexDecode, hexEncode, latin1Decode, utf16Decode, utf8Decode, utf8Encode } from './encoding'

function normalizeEncoding(encoding: unknown): string {
  let name = String(encoding ?? 'utf8').toLowerCase()
  switch (name) {
    case 'utf8':
    case 'utf-8':
      return 'utf8'
    case 'ucs2':
    case 'ucs-2':
    case 'utf16le':
    case 'utf-16le':
      return 'utf16le'
    case 'latin1':
    case 'binary':
      return 'latin1'
    case 'ascii':
    case 'base64':
    case 'base64url':
    case 'hex':
      return name
  }
  let error: any = new TypeError(`Unknown encoding: ${encoding}`)
  error.code = 'ERR_UNKNOWN_ENCODING'
  throw error
}

function encode(text: string, encoding: unknown): Uint8Array {
  switch (normalizeEncoding(encoding)) {
    case 'utf8':
      return utf8Encode(text)
    case 'base64':
    case 'base64url':
      return base64Decode(text)
    case 'hex':
      return hexDecode(text)
    case 'utf16le': {
      let bytes = new Uint8Array(text.length * 2)
      for (let i = 0; i < text.length; i++) {
        let code = text.charCodeAt(i)
        bytes[i * 2] = code & 0xff
        bytes[i * 2 + 1] = code >> 8
      }
      return bytes
    }
    default: {
      // `latin1` and `ascii`: the low byte of each code unit.
      let bytes = new Uint8Array(text.length)
      for (let i = 0; i < text.length; i++) bytes[i] = text.charCodeAt(i) & 0xff
      return bytes
    }
  }
}

export class Buffer extends Uint8Array {
  // `new Buffer(text)` is the old form of `Buffer.from(text)`.
  constructor(value?: any, encodingOrOffset?: any, length?: number) {
    if (typeof value === 'string') {
      super(0)
      return Buffer.from(value, encodingOrOffset)
    }
    if (value === undefined) super(0)
    else if (length !== undefined) super(value, encodingOrOffset, length)
    else if (encodingOrOffset !== undefined) super(value, encodingOrOffset)
    else super(value)
  }

  static from(value: unknown, encodingOrOffset?: unknown, length?: number): Buffer {
    if (typeof value === 'string') return wrap(encode(value, encodingOrOffset))
    if (value instanceof ArrayBuffer) {
      return wrap(new Uint8Array(value, (encodingOrOffset as number) ?? 0, length))
    }
    if (ArrayBuffer.isView(value)) {
      let view = new Uint8Array(value.buffer, value.byteOffset, value.byteLength)
      return wrap(value instanceof Uint8Array ? new Uint8Array(view) : Uint8Array.from(value as any))
    }
    if (value !== null && typeof value === 'object') {
      let object = value as any
      if (object.type === 'Buffer' && Array.isArray(object.data)) return wrap(Uint8Array.from(object.data))
      if (typeof object.length === 'number') return wrap(Uint8Array.from(object as ArrayLike<number>))
    }
    let error: any = new TypeError(
      `The first argument must be of type string or an instance of Buffer, ArrayBuffer, or Array or an Array-like Object. Received ${value === null ? 'null' : typeof value === 'object' ? 'an instance of Object' : `type ${typeof value} (${String(value)})`}`,
    )
    error.code = 'ERR_INVALID_ARG_TYPE'
    throw error
  }

  static alloc(size: number, fill?: unknown, encoding?: unknown): Buffer {
    let buffer = wrap(new Uint8Array(size))
    if (fill !== undefined && fill !== 0) buffer.fill(fill as any, undefined, undefined, encoding)
    return buffer
  }

  static allocUnsafe(size: number): Buffer {
    return wrap(new Uint8Array(size))
  }

  static isBuffer(value: unknown): value is Buffer {
    return value instanceof Buffer
  }

  static isEncoding(encoding: unknown): boolean {
    try {
      normalizeEncoding(encoding)
      return typeof encoding === 'string'
    } catch {
      return false
    }
  }

  static byteLength(value: unknown, encoding?: unknown): number {
    if (typeof value === 'string') return encode(value, encoding).length
    if (ArrayBuffer.isView(value) || value instanceof ArrayBuffer) return value.byteLength
    let error: any = new TypeError(
      'The "string" argument must be of type string or an instance of Buffer or ArrayBuffer.',
    )
    error.code = 'ERR_INVALID_ARG_TYPE'
    throw error
  }

  static concat(list: Uint8Array[], totalLength?: number): Buffer {
    let total = totalLength ?? list.reduce((sum, item) => sum + item.length, 0)
    let buffer = wrap(new Uint8Array(total))
    let offset = 0
    for (let item of list) {
      if (offset >= total) break
      let part = item.subarray(0, total - offset)
      buffer.set(part, offset)
      offset += part.length
    }
    return buffer
  }

  static compare(a: Uint8Array, b: Uint8Array): number {
    let length = Math.min(a.length, b.length)
    for (let i = 0; i < length; i++) {
      if (a[i] !== b[i]) return a[i] < b[i] ? -1 : 1
    }
    return a.length === b.length ? 0 : a.length < b.length ? -1 : 1
  }

  // @ts-ignore: the signature of Node.js, and not the one of Uint8Array.
  toString(encoding?: unknown, start = 0, end = this.length): string {
    let bytes = start === 0 && end === this.length ? this : this.subarray(start, end)
    switch (normalizeEncoding(encoding)) {
      case 'utf8':
        return utf8Decode(bytes)
      case 'base64':
        return base64Encode(bytes)
      case 'base64url':
        return base64Encode(bytes, true)
      case 'hex':
        return hexEncode(bytes)
      case 'utf16le':
        return utf16Decode(bytes)
      case 'ascii':
        return latin1Decode(bytes.map((byte) => byte & 0x7f))
      default:
        return latin1Decode(bytes)
    }
  }

  toJSON() {
    return { type: 'Buffer', data: Array.from(this) }
  }

  equals(other: Uint8Array): boolean {
    return Buffer.compare(this, other) === 0
  }

  compare(other: Uint8Array): number {
    return Buffer.compare(this, other)
  }

  // @ts-ignore: a `Buffer` and not a `Uint8Array`.
  slice(start?: number, end?: number): Buffer {
    return this.subarray(start, end)
  }

  // @ts-ignore
  subarray(start?: number, end?: number): Buffer {
    return wrap(super.subarray(start, end))
  }

  write(text: string, offset = 0, length?: number, encoding?: unknown): number {
    if (typeof offset === 'string') [encoding, offset] = [offset, 0]
    if (typeof length === 'string') [encoding, length] = [length, undefined]
    let bytes = encode(text, encoding).subarray(0, Math.min(length ?? Infinity, this.length - offset))
    this.set(bytes, offset)
    return bytes.length
  }

  // @ts-ignore
  fill(value: unknown, start = 0, end = this.length, encoding?: unknown): this {
    if (typeof value === 'string') {
      let bytes = encode(value, encoding)
      if (bytes.length === 0) return super.fill(0, start, end) as this
      for (let i = start; i < end; i++) this[i] = bytes[(i - start) % bytes.length]
      return this
    }
    return super.fill(Number(value) & 0xff, start, end) as this
  }

  // @ts-ignore
  indexOf(value: unknown, start = 0, encoding?: unknown): number {
    if (typeof value === 'number') return super.indexOf(value & 0xff, start)
    let needle = typeof value === 'string' ? encode(value, encoding) : (value as Uint8Array)
    outer: for (let i = Math.max(0, start); i + needle.length <= this.length; i++) {
      for (let j = 0; j < needle.length; j++) {
        if (this[i + j] !== needle[j]) continue outer
      }
      return i
    }
    return -1
  }

  // @ts-ignore
  includes(value: unknown, start = 0, encoding?: unknown): boolean {
    return this.indexOf(value, start, encoding) !== -1
  }

  copy(target: Uint8Array, targetStart = 0, start = 0, end = this.length): number {
    let part = Uint8Array.prototype.subarray.call(this, start, Math.min(end, start + target.length - targetStart))
    target.set(part, targetStart)
    return part.length
  }

  readUInt8(offset = 0): number {
    return this[offset]
  }
}

// A `Buffer` over the memory of the bytes.
function wrap(bytes: Uint8Array): Buffer {
  Object.setPrototypeOf(bytes, Buffer.prototype)
  return bytes as Buffer
}

export function fromBytes(bytes: Uint8Array): Buffer {
  return wrap(bytes)
}

const buffer: Record<string, unknown> = { Buffer, kMaxLength: 2 ** 31 - 1, constants: { MAX_LENGTH: 2 ** 31 - 1 } }
buffer.default = buffer

export default buffer
