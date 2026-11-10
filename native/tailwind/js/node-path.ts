// The module `path` of Node.js for POSIX paths, in JavaScript.

function normalizeParts(parts: string[], absolute: boolean): string[] {
  let result: string[] = []
  for (let part of parts) {
    if (part === '' || part === '.') continue
    if (part === '..') {
      if (result.length > 0 && result[result.length - 1] !== '..') result.pop()
      else if (!absolute) result.push('..')
      continue
    }
    result.push(part)
  }
  return result
}

export function isAbsolute(path: string): boolean {
  return path.startsWith('/')
}

export function normalize(path: string): string {
  if (path === '') return '.'
  let absolute = isAbsolute(path)
  let trailing = path.endsWith('/')
  let result = normalizeParts(path.split('/'), absolute).join('/')
  if (result === '' && !absolute) result = '.'
  if (result !== '' && trailing) result += '/'
  return (absolute ? '/' : '') + result
}

export function join(...paths: string[]): string {
  let joined = paths.filter((path) => path !== '').join('/')
  return joined === '' ? '.' : normalize(joined)
}

export function resolve(...paths: string[]): string {
  let resolved = ''
  for (let i = paths.length - 1; i >= -1 && !isAbsolute(resolved); i--) {
    let path = i >= 0 ? paths[i] : (globalThis as any).process.cwd()
    if (path === '') continue
    resolved = resolved === '' ? path : `${path}/${resolved}`
  }
  let result = normalizeParts(resolved.split('/'), true).join('/')
  return '/' + result
}

export function dirname(path: string): string {
  if (path === '') return '.'
  let end = path.length
  while (end > 1 && path[end - 1] === '/') end--
  let index = path.lastIndexOf('/', end - 1)
  if (index === -1) return '.'
  if (index === 0) return '/'
  return path.slice(0, index)
}

export function basename(path: string, suffix?: string): string {
  let end = path.length
  while (end > 1 && path[end - 1] === '/') end--
  let base = path.slice(path.lastIndexOf('/', end - 1) + 1, end)
  if (suffix && base !== suffix && base.endsWith(suffix)) base = base.slice(0, -suffix.length)
  return base
}

export function extname(path: string): string {
  let base = basename(path)
  let index = base.lastIndexOf('.')
  return index <= 0 ? '' : base.slice(index)
}

export function relative(from: string, to: string): string {
  let a = resolve(from).split('/').filter(Boolean)
  let b = resolve(to).split('/').filter(Boolean)
  let same = 0
  while (same < a.length && same < b.length && a[same] === b[same]) same++
  return [...a.slice(same).map(() => '..'), ...b.slice(same)].join('/')
}

export function parse(path: string) {
  let base = basename(path)
  let ext = extname(path)
  let dir = dirname(path)
  return {
    root: isAbsolute(path) ? '/' : '',
    dir: dir === '.' && !path.startsWith('.') ? '' : dir,
    base,
    ext,
    name: ext ? base.slice(0, -ext.length) : base,
  }
}

export function format(parts: { dir?: string; root?: string; base?: string; name?: string; ext?: string }) {
  let dir = parts.dir || parts.root || ''
  let base = parts.base || `${parts.name ?? ''}${parts.ext ?? ''}`
  if (!dir) return base
  return dir === parts.root ? `${dir}${base}` : `${dir}/${base}`
}

export const sep = '/'
export const delimiter = ':'

const path: Record<string, unknown> = {
  isAbsolute,
  normalize,
  join,
  resolve,
  dirname,
  basename,
  extname,
  relative,
  parse,
  format,
  sep,
  delimiter,
}
path.posix = path
path.default = path

export default path
