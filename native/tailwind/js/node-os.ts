// A part of the module `os` of Node.js.

function process(): any {
  return (globalThis as any).process
}

const os: Record<string, unknown> = {
  get EOL() {
    return process().platform === 'win32' ? '\r\n' : '\n'
  },
  get devNull() {
    return process().platform === 'win32' ? '\\\\.\\nul' : '/dev/null'
  },
  platform: () => process().platform,
  arch: () => process().arch,
  type() {
    let platform = process().platform
    return platform === 'win32' ? 'Windows_NT' : platform === 'darwin' ? 'Darwin' : platform === 'linux' ? 'Linux' : platform[0].toUpperCase() + platform.slice(1)
  },
  homedir() {
    let env = process().env
    return env.HOME ?? env.USERPROFILE ?? '/'
  },
  tmpdir() {
    let env = process().env
    if (process().platform === 'win32') return (env.TEMP ?? env.TMP ?? `${env.SystemRoot ?? env.windir ?? ''}\\temp`).replace(/(?<!:)\\$/, '')
    let path = env.TMPDIR ?? env.TMP ?? env.TEMP ?? '/tmp'
    return path.length > 1 && path.endsWith('/') ? path.slice(0, -1) : path
  },
  endianness: () => 'LE',
  hostname: () => 'localhost',
  release: () => '',
  cpus: () => [],
  availableParallelism: () => 1,
}
os.default = os

export default os
