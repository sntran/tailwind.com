// `TextEncoder` and `TextDecoder` (UTF-8, and the decoders `utf-16le` and
// `latin1`), and the Base64 and hexadecimal text of bytes.

export function utf8Encode(text: string): Uint8Array {
  let bytes: number[] = []
  for (let i = 0; i < text.length; i++) {
    let code = text.charCodeAt(i)
    if (code >= 0xd800 && code <= 0xdbff && i + 1 < text.length) {
      let next = text.charCodeAt(i + 1)
      if (next >= 0xdc00 && next <= 0xdfff) {
        code = 0x10000 + ((code - 0xd800) << 10) + (next - 0xdc00)
        i++
      }
    }
    // A surrogate with no pair is U+FFFD.
    if (code >= 0xd800 && code <= 0xdfff) code = 0xfffd
    if (code < 0x80) bytes.push(code)
    else if (code < 0x800) bytes.push(0xc0 | (code >> 6), 0x80 | (code & 63))
    else if (code < 0x10000) bytes.push(0xe0 | (code >> 12), 0x80 | ((code >> 6) & 63), 0x80 | (code & 63))
    else bytes.push(0xf0 | (code >> 18), 0x80 | ((code >> 12) & 63), 0x80 | ((code >> 6) & 63), 0x80 | (code & 63))
  }
  return new Uint8Array(bytes)
}

// The decoder of the Encoding Standard: each invalid sequence is one U+FFFD.
export function utf8Decode(bytes: Uint8Array, fatal = false): string {
  let text = ''
  let chunk: number[] = []
  let flush = () => {
    text += String.fromCharCode(...chunk)
    chunk = []
  }
  let invalid = () => {
    if (fatal) throw new TypeError('The encoded data was not valid for encoding utf-8')
    chunk.push(0xfffd)
  }
  let i = 0
  while (i < bytes.length) {
    if (chunk.length > 4096) flush()
    let first = bytes[i]
    if (first < 0x80) {
      chunk.push(first)
      i++
      continue
    }
    let needed = first >= 0xc2 && first <= 0xdf ? 1 : first >= 0xe0 && first <= 0xef ? 2 : first >= 0xf0 && first <= 0xf4 ? 3 : 0
    if (needed === 0) {
      invalid()
      i++
      continue
    }
    let code = first & (needed === 1 ? 0x1f : needed === 2 ? 0x0f : 0x07)
    let lower = first === 0xe0 ? 0xa0 : first === 0xf0 ? 0x90 : 0x80
    let upper = first === 0xed ? 0x9f : first === 0xf4 ? 0x8f : 0xbf
    let read = 1
    let ok = true
    for (; read <= needed; read++) {
      let next = bytes[i + read]
      if (next === undefined || next < lower || next > upper) {
        ok = false
        break
      }
      code = (code << 6) | (next & 63)
      lower = 0x80
      upper = 0xbf
    }
    if (!ok) {
      invalid()
      i += read
      continue
    }
    i += needed + 1
    if (code >= 0x10000) {
      code -= 0x10000
      chunk.push(0xd800 + (code >> 10), 0xdc00 + (code & 0x3ff))
    } else {
      chunk.push(code)
    }
  }
  flush()
  return text
}

const BASE64 = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/'
const BASE64_URL = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_'

export function base64Encode(bytes: Uint8Array, url = false): string {
  let table = url ? BASE64_URL : BASE64
  let text = ''
  for (let i = 0; i < bytes.length; i += 3) {
    let a = bytes[i]
    let b = bytes[i + 1]
    let c = bytes[i + 2]
    text += table[a >> 2] + table[((a & 3) << 4) | ((b ?? 0) >> 4)]
    if (b === undefined) {
      if (!url) text += '=='
    } else {
      text += table[((b & 15) << 2) | ((c ?? 0) >> 6)]
      if (c === undefined) {
        if (!url) text += '='
      } else {
        text += table[c & 63]
      }
    }
  }
  return text
}

// As `Buffer.from(text, 'base64')`: both alphabets, and the text ends at
// the first `=` or at the first character that is not in an alphabet.
export function base64Decode(text: string): Uint8Array {
  let bytes: number[] = []
  let bits = 0
  let value = 0
  for (let i = 0; i < text.length; i++) {
    let char = text[i]
    if (char === '=') break
    let index = char === '-' ? 62 : char === '_' ? 63 : BASE64.indexOf(char)
    if (index === -1) continue
    value = (value << 6) | index
    bits += 6
    if (bits >= 8) {
      bits -= 8
      bytes.push((value >> bits) & 0xff)
    }
  }
  return new Uint8Array(bytes)
}

export function hexEncode(bytes: Uint8Array): string {
  let text = ''
  for (let i = 0; i < bytes.length; i++) text += (bytes[i] < 16 ? '0' : '') + bytes[i].toString(16)
  return text
}

// As `Buffer.from(text, 'hex')`: the bytes before the first invalid pair.
export function hexDecode(text: string): Uint8Array {
  let bytes: number[] = []
  for (let i = 0; i + 1 < text.length; i += 2) {
    let pair = text.slice(i, i + 2)
    if (!/^[0-9a-fA-F]{2}$/.test(pair)) break
    bytes.push(parseInt(pair, 16))
  }
  return new Uint8Array(bytes)
}

function toBytes(input: unknown): Uint8Array {
  if (input instanceof Uint8Array) return input
  if (ArrayBuffer.isView(input)) return new Uint8Array(input.buffer, input.byteOffset, input.byteLength)
  if (input instanceof ArrayBuffer) return new Uint8Array(input)
  throw new TypeError('The "input" argument must be an instance of ArrayBuffer or ArrayBufferView.')
}

export class TextEncoder {
  get encoding() {
    return 'utf-8'
  }
  encode(text = ''): Uint8Array {
    return utf8Encode(String(text))
  }
  encodeInto(text: string, target: Uint8Array) {
    let bytes = utf8Encode(String(text))
    let written = Math.min(bytes.length, target.length)
    // Only whole characters.
    while (written > 0 && written < bytes.length && (bytes[written] & 0xc0) === 0x80) written--
    target.set(bytes.subarray(0, written))
    return { read: utf8Decode(bytes.subarray(0, written)).length, written }
  }
}

const LABELS: Record<string, string> = {
  'utf-8': 'utf-8',
  utf8: 'utf-8',
  'unicode-1-1-utf-8': 'utf-8',
  'utf-16le': 'utf-16le',
  'utf-16': 'utf-16le',
  ucs2: 'utf-16le',
  'ucs-2': 'utf-16le',
  latin1: 'windows-1252',
  'iso-8859-1': 'windows-1252',
  ascii: 'windows-1252',
  'us-ascii': 'windows-1252',
  'windows-1252': 'windows-1252',
}

export class TextDecoder {
  encoding: string
  fatal: boolean
  ignoreBOM: boolean
  constructor(label = 'utf-8', options: { fatal?: boolean; ignoreBOM?: boolean } = {}) {
    let encoding = LABELS[String(label).trim().toLowerCase()]
    if (!encoding) throw new RangeError(`The "${label}" encoding is not supported`)
    this.encoding = encoding
    this.fatal = Boolean(options.fatal)
    this.ignoreBOM = Boolean(options.ignoreBOM)
  }
  decode(input?: unknown): string {
    if (input === undefined) return ''
    let bytes = toBytes(input)
    let text: string
    if (this.encoding === 'utf-8') {
      text = utf8Decode(bytes, this.fatal)
    } else if (this.encoding === 'utf-16le') {
      text = utf16Decode(bytes)
    } else {
      text = latin1Decode(bytes)
    }
    if (!this.ignoreBOM && this.encoding !== 'windows-1252' && text.charCodeAt(0) === 0xfeff) text = text.slice(1)
    return text
  }
}

export function latin1Decode(bytes: Uint8Array): string {
  let text = ''
  for (let i = 0; i < bytes.length; i += 4096) text += String.fromCharCode(...bytes.subarray(i, i + 4096))
  return text
}

export function utf16Decode(bytes: Uint8Array): string {
  let text = ''
  for (let i = 0; i + 1 < bytes.length; i += 2) text += String.fromCharCode(bytes[i] | (bytes[i + 1] << 8))
  return text
}
