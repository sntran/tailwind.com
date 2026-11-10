// A part of the module `fs` of Node.js: only reads, through the host.
import { host } from './host'
import { resolve } from './node-path'

function fail(code: string, call: string, path: string): never {
  let text = code === 'ENOENT' ? 'no such file or directory' : code === 'ENOTDIR' ? 'not a directory' : 'operation not permitted'
  let error: any = new Error(`${code}: ${text}, ${call} '${path}'`)
  error.code = code
  error.syscall = call
  error.path = path
  throw error
}

function stat(path: string): { kind: 'file' | 'directory'; size: number; mtimeMs: number } | null {
  return host('fs.stat', resolve(String(path))) ?? null
}

export function readFileSync(path: string, options?: unknown): string | String {
  let text = host('fs.readFile', resolve(String(path)))
  if (typeof text !== 'string') fail('ENOENT', 'open', path)
  let encoding = typeof options === 'string' ? options : (options as any)?.encoding
  // With no encoding, Node.js gives a `Buffer`. Here the result is a
  // String object: `toString()` and the string methods work.
  return encoding ? text : new String(text)
}

export function existsSync(path: string): boolean {
  return stat(path) !== null
}

export function readdirSync(path: string, options?: { withFileTypes?: boolean }): unknown[] {
  let entries: [string, 'file' | 'directory'][] | undefined = host('fs.readdir', resolve(String(path)))
  if (!entries) fail(stat(path) ? 'ENOTDIR' : 'ENOENT', 'scandir', path)
  if (options && typeof options === 'object' && options.withFileTypes) {
    return entries.map(([name, kind]) => ({
      name,
      path,
      parentPath: path,
      isFile: () => kind === 'file',
      isDirectory: () => kind === 'directory',
      isSymbolicLink: () => false,
    }))
  }
  return entries.map(([name]) => name)
}

export function statSync(path: string, options?: { throwIfNoEntry?: boolean }) {
  let info = stat(path)
  if (!info) {
    if (options && options.throwIfNoEntry === false) return undefined
    fail('ENOENT', 'stat', path)
  }
  return {
    size: info.size,
    mtimeMs: info.mtimeMs,
    mtime: new Date(info.mtimeMs),
    isFile: () => info.kind === 'file',
    isDirectory: () => info.kind === 'directory',
    isSymbolicLink: () => false,
  }
}

export function realpathSync(path: string): string {
  return resolve(String(path))
}

function readOnly(name: string) {
  return () => {
    throw new Error(`fs.${name} is not available: a plugin can only read files.`)
  }
}

const fs: Record<string, unknown> = {
  readFileSync,
  existsSync,
  readdirSync,
  statSync,
  lstatSync: statSync,
  realpathSync,
  writeFileSync: readOnly('writeFileSync'),
  mkdirSync: readOnly('mkdirSync'),
  unlinkSync: readOnly('unlinkSync'),
  rmSync: readOnly('rmSync'),
}
fs.default = fs

export default fs
