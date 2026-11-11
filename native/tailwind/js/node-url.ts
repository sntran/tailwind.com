// `URL` and `URLSearchParams`, and a part of the module `url` of Node.js.
//
// The parser follows the URL Standard for the URLs that a plugin makes:
// `file:`, `http:`, `https:` and `data:` URLs, and relative references.
// It does not have the host parser of the standard (no IDNA, no IPv4
// number forms): a host is lower case text.
import { utf8Decode, utf8Encode } from './encoding'

const SPECIAL: Record<string, string> = { 'ftp:': '21', 'file:': '', 'http:': '80', 'https:': '443', 'ws:': '80', 'wss:': '443' }

function percentEncode(text: string, keep: RegExp): string {
  let result = ''
  for (let char of text) {
    if (keep.test(char)) {
      result += char
    } else {
      for (let byte of utf8Encode(char)) result += '%' + (byte < 16 ? '0' : '') + byte.toString(16).toUpperCase()
    }
  }
  return result
}

function percentDecode(text: string): string {
  if (!text.includes('%')) return text
  let bytes: number[] = []
  let source = utf8Encode(text)
  let hex = (byte: number) => (byte >= 48 && byte <= 57) || (byte >= 65 && byte <= 70) || (byte >= 97 && byte <= 102)
  for (let i = 0; i < source.length; i++) {
    if (source[i] === 0x25 && i + 2 < source.length && hex(source[i + 1]) && hex(source[i + 2])) {
      bytes.push(parseInt(String.fromCharCode(source[i + 1], source[i + 2]), 16))
      i += 2
    } else {
      bytes.push(source[i])
    }
  }
  return utf8Decode(new Uint8Array(bytes))
}

// The sets of the URL Standard: the characters that stay as they are.
const PATH_KEEP = /[^\u0000- "#<>?`{}\u007f-￿]/u
const QUERY_KEEP = /[^\u0000- "#<>\u007f-￿]/u
const SPECIAL_QUERY_KEEP = /[^\u0000- "#<>'\u007f-￿]/u
const FRAGMENT_KEEP = /[^\u0000- "<>`\u007f-￿]/u
const USERINFO_KEEP = /[^\u0000- "#<>?`{}/:;=@[\\\]^|\u007f-￿]/u
const FORM_KEEP = /[A-Za-z0-9*\-._]/

function invalid(input: string): never {
  let error: any = new TypeError('Invalid URL')
  error.code = 'ERR_INVALID_URL'
  error.input = input
  throw error
}

function normalizePath(path: string, special: boolean): string {
  let parts: string[] = []
  let segments = (special ? path.replace(/\\/g, '/') : path).split('/')
  for (let i = 0; i < segments.length; i++) {
    let segment = segments[i]
    let lower = segment.toLowerCase()
    let last = i === segments.length - 1
    if (lower === '..' || lower === '.%2e' || lower === '%2e.' || lower === '%2e%2e') {
      if (parts.length > 1) parts.pop()
      if (last) parts.push('')
    } else if (lower === '.' || lower === '%2e') {
      if (last) parts.push('')
    } else {
      parts.push(segment)
    }
  }
  return parts.join('/')
}

type Parts = {
  protocol: string
  username: string
  password: string
  hostname: string
  port: string
  pathname: string
  search: string
  hash: string
  opaque: boolean
  // True for a URL with `//`: it has a host, possibly empty.
  authority: boolean
}

function parse(input: string, base?: Parts): Parts {
  let text = input.replace(/^[\u0000- ]+|[\u0000- ]+$/g, '').replace(/[\t\n\r]/g, '')
  let hash = ''
  let search = ''
  let index = text.indexOf('#')
  if (index !== -1) {
    hash = text.slice(index)
    text = text.slice(0, index)
  }
  let scheme = /^[A-Za-z][A-Za-z0-9+\-.]*:/.exec(text)
  let protocol = scheme ? scheme[0].toLowerCase() : ''
  let rest = scheme ? text.slice(scheme[0].length) : text
  let special = Object.hasOwn(SPECIAL, protocol || base?.protocol || '')
  let result: Parts

  let slashes = special ? /^[/\\]{2}/ : /^\/\//
  if (protocol && (!base || protocol !== base.protocol || !special || slashes.test(rest))) {
    // An absolute URL.
    index = rest.indexOf('?')
    if (index !== -1) {
      search = rest.slice(index)
      rest = rest.slice(0, index)
    }
    result = { protocol, username: '', password: '', hostname: '', port: '', pathname: '', search, hash, opaque: false, authority: false }
    if (special && protocol !== 'file:') rest = rest.replace(/^[/\\]*/, '//')
    if (slashes.test(rest)) {
      parseAuthority(result, rest.slice(2), special, input)
    } else if (protocol === 'file:') {
      result.authority = true
      result.pathname = normalizePath(rest.startsWith('/') || rest.startsWith('\\') ? rest : '/' + rest, true)
    } else if (rest.startsWith('/')) {
      result.pathname = normalizePath(rest, false)
    } else {
      result.opaque = true
      result.pathname = rest
    }
  } else {
    // A relative reference.
    if (!base) invalid(input)
    if (base.opaque) {
      if (text !== '' || hash === '') invalid(input)
      return { ...base, hash: encodeHash(hash) }
    }
    if (protocol) text = rest
    index = text.indexOf('?')
    let hasSearch = index !== -1
    if (hasSearch) {
      search = text.slice(index)
      text = text.slice(0, index)
    }
    result = { ...base, search: hasSearch ? search : '', hash }
    if (special) text = text.replace(/\\/g, '/')
    if (text.startsWith('//')) {
      result.username = result.password = result.hostname = result.port = ''
      parseAuthority(result, text.slice(2), special, input)
    } else if (text.startsWith('/')) {
      result.pathname = normalizePath(text, special)
    } else if (text !== '') {
      let directory = base.pathname.slice(0, base.pathname.lastIndexOf('/') + 1)
      result.pathname = normalizePath((directory || '/') + text, special)
    } else if (!hasSearch) {
      result.search = base.search
    }
  }

  if (!result.opaque) {
    if (special && result.pathname === '') result.pathname = '/'
    result.pathname = percentEncode(result.pathname, PATH_KEEP)
  } else {
    result.pathname = percentEncode(result.pathname, /[^\u0000-\u001f\u007f-￿]/u)
  }
  result.search = result.search === '?' ? '' : percentEncode(result.search, special ? SPECIAL_QUERY_KEEP : QUERY_KEEP)
  result.hash = encodeHash(result.hash)
  return result
}

function encodeHash(hash: string): string {
  return hash === '#' ? '' : percentEncode(hash, FRAGMENT_KEEP)
}

function parseAuthority(result: Parts, rest: string, special: boolean, input: string) {
  let end = rest.search(special ? /[/\\]/ : /\//)
  let authority = end === -1 ? rest : rest.slice(0, end)
  let path = end === -1 ? '' : rest.slice(end)
  let at = authority.lastIndexOf('@')
  if (at !== -1) {
    let [username, ...password] = authority.slice(0, at).split(':')
    result.username = percentEncode(username, USERINFO_KEEP)
    result.password = percentEncode(password.join(':'), USERINFO_KEEP)
    authority = authority.slice(at + 1)
  }
  let port = /:(\d*)$/.exec(authority.startsWith('[') ? authority.slice(authority.indexOf(']') + 1) : authority)
  if (port) authority = authority.slice(0, authority.length - port[0].length)
  else if (/:[^\]]*$/.test(authority) && !authority.startsWith('[')) invalid(input)
  let hostname = special ? percentDecode(authority).toLowerCase() : authority
  if (special && result.protocol !== 'file:' && hostname === '') invalid(input)
  if (/[\u0000- #/<>?@[\\\]^|]/.test(hostname.startsWith('[') ? '' : hostname)) invalid(input)
  if (result.protocol === 'file:' && hostname === 'localhost') hostname = ''
  result.authority = true
  result.hostname = hostname
  if (port && port[1] !== '') {
    let number = Number(port[1])
    if (number > 65535) invalid(input)
    result.port = String(number) === SPECIAL[result.protocol] ? '' : String(number)
  }
  result.pathname = normalizePath(path, special)
}

const state = new WeakMap<object, Parts>()

export class URL {
  searchParams: URLSearchParams

  constructor(input: unknown, base?: unknown) {
    let text = String(input)
    let baseParts = base === undefined ? undefined : state.get(base instanceof URL ? base : new URL(String(base)))
    state.set(this, parse(text, baseParts))
    this.searchParams = new URLSearchParams(this.search)
    link.set(this.searchParams, this)
  }

  static canParse(input: unknown, base?: unknown): boolean {
    try {
      new URL(input, base)
      return true
    } catch {
      return false
    }
  }

  static parse(input: unknown, base?: unknown): URL | null {
    try {
      return new URL(input, base)
    } catch {
      return null
    }
  }

  get protocol() {
    return state.get(this)!.protocol
  }
  get username() {
    return state.get(this)!.username
  }
  get password() {
    return state.get(this)!.password
  }
  get hostname() {
    return state.get(this)!.hostname
  }
  get port() {
    return state.get(this)!.port
  }
  get host() {
    let parts = state.get(this)!
    return parts.hostname + (parts.port ? ':' + parts.port : '')
  }
  get origin() {
    let parts = state.get(this)!
    return Object.hasOwn(SPECIAL, parts.protocol) && parts.protocol !== 'file:' ? `${parts.protocol}//${this.host}` : 'null'
  }
  get pathname() {
    return state.get(this)!.pathname
  }
  set pathname(value: string) {
    let parts = state.get(this)!
    if (parts.opaque) return
    let special = Object.hasOwn(SPECIAL, parts.protocol)
    let path = String(value)
    if (!path.startsWith('/') && !(special && path.startsWith('\\'))) path = '/' + path
    parts.pathname = percentEncode(normalizePath(path, special), PATH_KEEP)
  }
  get search() {
    return state.get(this)!.search
  }
  set search(value: string) {
    let parts = state.get(this)!
    let text = String(value)
    if (text !== '' && !text.startsWith('?')) text = '?' + text
    parts.search = text === '?' ? '' : percentEncode(text, Object.hasOwn(SPECIAL, parts.protocol) ? SPECIAL_QUERY_KEEP : QUERY_KEEP)
    this.searchParams = new URLSearchParams(parts.search)
    link.set(this.searchParams, this)
  }
  get hash() {
    return state.get(this)!.hash
  }
  set hash(value: string) {
    let text = String(value)
    if (text !== '' && !text.startsWith('#')) text = '#' + text
    state.get(this)!.hash = encodeHash(text)
  }
  get href() {
    let parts = state.get(this)!
    let text = parts.protocol
    if (parts.authority) {
      text += '//'
      if (parts.username || parts.password) {
        text += parts.username + (parts.password ? ':' + parts.password : '') + '@'
      }
      text += this.host
    }
    return text + parts.pathname + parts.search + parts.hash
  }
  set href(value: string) {
    state.set(this, parse(String(value)))
    this.searchParams = new URLSearchParams(this.search)
    link.set(this.searchParams, this)
  }
  toString() {
    return this.href
  }
  toJSON() {
    return this.href
  }
}

// The URL of a `URLSearchParams`: a change of the parameters changes it.
const link = new WeakMap<URLSearchParams, URL>()

function formEncode(text: string): string {
  return percentEncode(text, FORM_KEEP).replace(/%20/g, '+')
}

function formDecode(text: string): string {
  return percentDecode(text.replace(/\+/g, ' '))
}

export class URLSearchParams {
  #list: [string, string][] = []

  constructor(init?: unknown) {
    if (init === undefined || init === null) return
    if (typeof init === 'object' && typeof (init as any)[Symbol.iterator] === 'function') {
      for (let pair of init as Iterable<[unknown, unknown]>) {
        let items = Array.from(pair as Iterable<unknown>)
        if (items.length !== 2) {
          throw new TypeError('Each query pair must be an iterable [name, value] tuple')
        }
        this.#list.push([String(items[0]), String(items[1])])
      }
    } else if (typeof init === 'object') {
      for (let key of Object.keys(init)) this.#list.push([key, String((init as any)[key])])
    } else {
      let text = String(init)
      if (text.startsWith('?')) text = text.slice(1)
      for (let part of text.split('&')) {
        if (part === '') continue
        let index = part.indexOf('=')
        let name = index === -1 ? part : part.slice(0, index)
        let value = index === -1 ? '' : part.slice(index + 1)
        this.#list.push([formDecode(name), formDecode(value)])
      }
    }
  }

  #update() {
    let url = link.get(this)
    if (!url) return
    let text = this.toString()
    state.get(url)!.search = text === '' ? '' : '?' + text
  }

  get size() {
    return this.#list.length
  }
  append(name: string, value: string) {
    this.#list.push([String(name), String(value)])
    this.#update()
  }
  delete(name: string, value?: string) {
    this.#list = this.#list.filter(([key, item]) => key !== String(name) || (value !== undefined && item !== String(value)))
    this.#update()
  }
  get(name: string): string | null {
    return this.#list.find(([key]) => key === String(name))?.[1] ?? null
  }
  getAll(name: string): string[] {
    return this.#list.filter(([key]) => key === String(name)).map(([, value]) => value)
  }
  has(name: string, value?: string): boolean {
    return this.#list.some(([key, item]) => key === String(name) && (value === undefined || item === String(value)))
  }
  set(name: string, value: string) {
    let index = this.#list.findIndex(([key]) => key === String(name))
    if (index === -1) {
      this.#list.push([String(name), String(value)])
    } else {
      this.#list[index][1] = String(value)
      this.#list = this.#list.filter(([key], at) => at <= index || key !== String(name))
    }
    this.#update()
  }
  sort() {
    // A stable sort by the code units of the names.
    this.#list = this.#list
      .map((pair, index) => ({ pair, index }))
      .sort((a, z) => (a.pair[0] < z.pair[0] ? -1 : a.pair[0] > z.pair[0] ? 1 : a.index - z.index))
      .map(({ pair }) => pair)
    this.#update()
  }
  forEach(callback: (value: string, name: string, params: URLSearchParams) => void, thisArg?: unknown) {
    for (let [name, value] of this.#list) callback.call(thisArg, value, name, this)
  }
  *entries(): IterableIterator<[string, string]> {
    for (let [name, value] of this.#list) yield [name, value]
  }
  *keys(): IterableIterator<string> {
    for (let [name] of this.#list) yield name
  }
  *values(): IterableIterator<string> {
    for (let [, value] of this.#list) yield value
  }
  [Symbol.iterator]() {
    return this.entries()
  }
  toString(): string {
    return this.#list.map(([name, value]) => `${formEncode(name)}=${formEncode(value)}`).join('&')
  }
}

function invalidArgument(name: string, value: unknown): never {
  let error: any = new TypeError(
    `The "${name}" argument must be of type string or an instance of URL. Received ${value === null ? 'null' : typeof value === 'object' ? 'an instance of ' + (value?.constructor?.name ?? 'Object') : `type ${typeof value} (${String(value)})`}`,
  )
  error.code = 'ERR_INVALID_ARG_TYPE'
  throw error
}

export function fileURLToPath(url: unknown): string {
  if (typeof url !== 'string' && !(url instanceof URL)) invalidArgument('path', url)
  let parsed = typeof url === 'string' ? new URL(url) : url
  if (parsed.protocol !== 'file:') {
    let error: any = new TypeError('The URL must be of scheme file')
    error.code = 'ERR_INVALID_URL_SCHEME'
    throw error
  }
  if (parsed.hostname !== '') {
    let error: any = new TypeError(`File URL host must be "localhost" or empty on ${(globalThis as any).process?.platform ?? 'linux'}`)
    error.code = 'ERR_INVALID_FILE_URL_HOST'
    throw error
  }
  if (/%2f/i.test(parsed.pathname)) {
    let error: any = new TypeError('File URL path must not include encoded / characters')
    error.code = 'ERR_INVALID_FILE_URL_PATH'
    throw error
  }
  return percentDecode(parsed.pathname)
}

// The characters that `pathToFileURL` of Node.js encodes.
const FILE_PATH_KEEP = /[^\u0000- "#%<>?`{}\\\u007f-￿]/u

export function pathToFileURL(path: string): URL {
  if (typeof path !== 'string') invalidArgument('path', path)
  let absolute = path.startsWith('/') ? path : `${(globalThis as any).process.cwd()}/${path}`
  let trailing = absolute.endsWith('/') && absolute.length > 1
  let parts: string[] = []
  for (let part of absolute.split('/')) {
    if (part === '' || part === '.') continue
    if (part === '..') parts.pop()
    else parts.push(part)
  }
  let normal = '/' + parts.join('/') + (trailing && parts.length > 0 ? '/' : '')
  let url = new URL('file://')
  state.get(url)!.pathname = percentEncode(normal, FILE_PATH_KEEP)
  return url
}

const url: Record<string, unknown> = {
  URL,
  URLSearchParams,
  fileURLToPath,
  pathToFileURL,
  // The old API: `url.parse` for the common fields.
  parse(text: string) {
    let parsed = URL.parse(text)
    if (!parsed) return { href: text, pathname: text, path: text, protocol: null, host: null, hostname: null, search: null, query: null, hash: null }
    return {
      href: parsed.href,
      protocol: parsed.protocol,
      host: parsed.host || null,
      hostname: parsed.hostname || null,
      port: parsed.port || null,
      pathname: parsed.pathname,
      search: parsed.search || null,
      query: parsed.search ? parsed.search.slice(1) : null,
      path: parsed.pathname + parsed.search,
      hash: parsed.hash || null,
    }
  },
  format(value: unknown) {
    return value instanceof URL ? value.href : String(value)
  },
}
url.default = url

export default url
