// The call of the host (the Rust compiler). Data goes as JSON text.

declare function __host(op: string, text?: string): string | undefined

export function host(op: string, arg?: unknown): any {
  let result = __host(op, arg === undefined ? undefined : JSON.stringify(arg))
  return result === undefined ? undefined : JSON.parse(result)
}

// The functions that the host calls by a number: the compile functions of
// utilities, the functions of variants, and the compare functions.
export let callbacks: Function[] = []

export function register(fn: Function): number {
  callbacks.push(fn)
  return callbacks.length - 1
}
