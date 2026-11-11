// The rules for the JavaScript of a plugin or config: it reads files, and
// it does nothing more outside the engine. Each call that a rule forbids
// throws an error that names the rule.

export type Rule = 'write' | 'network' | 'process'

const RULES: Record<Rule, string> = {
  write: 'the JavaScript of a plugin or config has read-only access to files',
  network: 'the JavaScript of a plugin or config has no network access',
  process: 'the JavaScript of a plugin or config cannot start a process',
}

export function denied(name: string, rule: Rule): (...args: unknown[]) => never {
  // A function and not an arrow function: `new` on it gives the error too.
  let fn = function () {
    let error: any = new Error(`${name} is not permitted: ${RULES[rule]}.`)
    error.code = 'ERR_ACCESS_DENIED'
    error.permission = rule
    throw error
  }
  Object.defineProperty(fn, 'name', { value: name.split('.').pop() })
  return fn
}

// A module of Node.js that the program does not give: it loads, and each
// of its functions throws.
export function deniedModule(module: string, rule: Rule): any {
  let cache = new Map<string, unknown>()
  let target: any = {}
  let proxy: any = new Proxy(target, {
    get(_target, key) {
      if (typeof key === 'symbol' || key === 'then') return undefined
      if (key === 'default') return proxy
      if (key === '__esModule') return false
      if (!cache.has(key)) cache.set(key, denied(`${module}.${key}`, rule))
      return cache.get(key)
    },
    has: () => true,
  })
  return proxy
}
