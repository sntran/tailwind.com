// A part of the module `fs` of Node.js: only reads, through the host.
import { denied } from './denied'
import { base64Decode } from './encoding'
import { host } from './host'
import { Buffer, fromBytes } from './node-buffer'
import { resolve } from './node-path'
import { fileURLToPath, URL } from './node-url'

const MESSAGES: Record<string, string> = {
  ENOENT: 'no such file or directory',
  ENOTDIR: 'not a directory',
  EISDIR: 'illegal operation on a directory',
  EINVAL: 'invalid argument',
}

// The numbers of Linux, as `os.constants.errno`.
const ERRNO: Record<string, number> = { ENOENT: -2, ENOTDIR: -20, EISDIR: -21, EINVAL: -22 }

function fail(code: string, call: string, path: string): never {
  let error: any = new Error(`${code}: ${MESSAGES[code]}, ${call}${call === 'read' ? '' : ` '${path}'`}`)
  error.errno = ERRNO[code]
  error.code = code
  error.syscall = call
  if (call !== 'read') error.path = path
  throw error
}

// A path argument: text, a `Buffer`, or a `file:` URL.
function toPath(path: unknown): string {
  if (path instanceof URL) return fileURLToPath(path)
  if (path instanceof Uint8Array) return fromBytes(path).toString()
  if (typeof path !== 'string') {
    let error: any = new TypeError(
      `The "path" argument must be of type string or an instance of Buffer or URL. Received ${path === null ? 'null' : path === undefined ? 'undefined' : `type ${typeof path} (${String(path)})`}`,
    )
    error.code = 'ERR_INVALID_ARG_TYPE'
    throw error
  }
  return path
}

type Info = { kind: 'file' | 'directory'; size: number; mtimeMs: number; link?: boolean }

function stat(path: string, link = false): Info | null {
  return host(link ? 'fs.lstat' : 'fs.stat', resolve(path)) ?? null
}

function encodingOf(options: unknown): string | null {
  let encoding = typeof options === 'string' ? options : (options as any)?.encoding
  return encoding === undefined || encoding === null || encoding === 'buffer' ? null : String(encoding)
}

export function readFileSync(file: unknown, options?: unknown): string | Buffer {
  let path = toPath(file)
  let encoding = encodingOf(options)
  if (encoding !== null && /^utf-?8$/i.test(encoding)) {
    let text = host('fs.readFile', resolve(path))
    if (typeof text === 'string') return text
  } else {
    let data = host('fs.readFileBytes', resolve(path))
    if (typeof data === 'string') {
      let buffer = fromBytes(base64Decode(data))
      return encoding === null ? buffer : buffer.toString(encoding)
    }
  }
  let info = stat(path)
  if (info && info.kind === 'directory') fail('EISDIR', 'read', path)
  return fail('ENOENT', 'open', path)
}

export function existsSync(path: unknown): boolean {
  try {
    return stat(toPath(path)) !== null
  } catch {
    return false
  }
}

class Dirent {
  name: string
  parentPath: string
  path: string
  #kind: string
  constructor(name: string, parent: string, kind: string) {
    this.name = name
    this.parentPath = parent
    this.path = parent
    this.#kind = kind
  }
  isFile() {
    return this.#kind === 'file'
  }
  isDirectory() {
    return this.#kind === 'directory'
  }
  isSymbolicLink() {
    return false
  }
  isBlockDevice() {
    return false
  }
  isCharacterDevice() {
    return false
  }
  isFIFO() {
    return false
  }
  isSocket() {
    return false
  }
}

export function readdirSync(directory: unknown, options?: unknown): unknown[] {
  let path = toPath(directory)
  let entries: [string, 'file' | 'directory'][] | undefined = host('fs.readdir', resolve(path))
  if (!entries) fail(stat(path) ? 'ENOTDIR' : 'ENOENT', 'scandir', path)
  let settings = typeof options === 'object' && options !== null ? (options as any) : {}
  let result: unknown[] = []
  for (let [name, kind] of entries) {
    result.push(settings.withFileTypes ? new Dirent(name, path, kind) : name)
    if (settings.recursive && kind === 'directory') {
      let nested = readdirSync(`${path.replace(/\/$/, '')}/${name}`, options)
      for (let item of nested) result.push(settings.withFileTypes ? item : `${name}/${item}`)
    }
  }
  return result
}

class Stats {
  size: number
  mtimeMs: number
  atimeMs: number
  ctimeMs: number
  birthtimeMs: number
  mtime: Date
  atime: Date
  ctime: Date
  birthtime: Date
  mode: number
  #info: Info
  constructor(info: Info) {
    this.#info = info
    this.size = info.size
    this.mtimeMs = this.atimeMs = this.ctimeMs = this.birthtimeMs = info.mtimeMs
    this.mtime = this.atime = this.ctime = this.birthtime = new Date(info.mtimeMs)
    this.mode = info.link ? 0o120777 : info.kind === 'directory' ? 0o040755 : 0o100644
  }
  isFile() {
    return !this.#info.link && this.#info.kind === 'file'
  }
  isDirectory() {
    return !this.#info.link && this.#info.kind === 'directory'
  }
  isSymbolicLink() {
    return this.#info.link === true
  }
  isBlockDevice() {
    return false
  }
  isCharacterDevice() {
    return false
  }
  isFIFO() {
    return false
  }
  isSocket() {
    return false
  }
}

function statWith(link: boolean, call: string) {
  return (file: unknown, options?: { throwIfNoEntry?: boolean }) => {
    let path = toPath(file)
    let info = stat(path, link)
    if (!info) {
      if (options && options.throwIfNoEntry === false) return undefined
      fail('ENOENT', call, path)
    }
    return new Stats(info)
  }
}

export const statSync = statWith(false, 'stat')
export const lstatSync = statWith(true, 'lstat')

export function realpathSync(file: unknown): string {
  let path = toPath(file)
  let real: string | undefined = host('fs.realpath', resolve(path))
  if (typeof real !== 'string') fail('ENOENT', 'lstat', path)
  return real
}
;(realpathSync as any).native = realpathSync

const constants = { F_OK: 0, R_OK: 4, W_OK: 2, X_OK: 1 }

export function accessSync(file: unknown, _mode?: number): void {
  let path = toPath(file)
  if (!stat(path)) fail('ENOENT', 'access', path)
}

// The reads with a promise, and with a callback.
const READS: Record<string, [string, Function]> = {
  readFile: ['readFileSync', readFileSync],
  readdir: ['readdirSync', readdirSync],
  stat: ['statSync', statSync],
  lstat: ['lstatSync', lstatSync],
  realpath: ['realpathSync', realpathSync],
  access: ['accessSync', accessSync],
}

// The functions of `fs` that change the file system or open a file.
const WRITES = [
  'appendFile',
  'chmod',
  'chown',
  'copyFile',
  'cp',
  'lchmod',
  'lchown',
  'link',
  'lutimes',
  'mkdir',
  'mkdtemp',
  'open',
  'rename',
  'rm',
  'rmdir',
  'symlink',
  'truncate',
  'unlink',
  'utimes',
  'write',
  'writeFile',
  'writev',
]

const promises: Record<string, unknown> = { constants }
const fs: Record<string, unknown> = { constants, promises, Stats, Dirent, ...constants }

for (let [name, [syncName, fn]] of Object.entries(READS)) {
  fs[syncName] = fn
  promises[name] = async (...args: unknown[]) => fn(...args)
  fs[name] = (...args: unknown[]) => {
    let callback = args.pop()
    if (typeof callback !== 'function') {
      let error: any = new TypeError(`The "cb" argument must be of type function. Received ${callback === undefined ? 'undefined' : typeof callback}`)
      error.code = 'ERR_INVALID_ARG_TYPE'
      throw error
    }
    let result: unknown
    try {
      result = fn(...args)
    } catch (error) {
      queueMicrotask(() => callback(error))
      return
    }
    queueMicrotask(() => callback(null, result))
  }
}
fs.existsSync = existsSync
fs.exists = (path: unknown, callback: (exists: boolean) => void) => {
  let exists = existsSync(path)
  queueMicrotask(() => callback(exists))
}

for (let name of WRITES) {
  fs[name] = denied(`fs.${name}`, 'write')
  fs[`${name}Sync`] = denied(`fs.${name}Sync`, 'write')
  promises[name] = denied(`fs.promises.${name}`, 'write')
}
for (let name of ['createWriteStream', 'createReadStream', 'watch', 'watchFile', 'openAsBlob']) {
  fs[name] = denied(`fs.${name}`, name === 'createWriteStream' ? 'write' : 'write')
}
fs.default = fs
promises.default = promises

export { promises }
export default fs
