import { host } from './host'

// The official program prints the `cause` chain of an error. Give the
// host the text of each cause of the exception, as `${cause}`.
export function withCauses<T>(fn: () => T): T {
  try {
    return fn()
  } catch (error) {
    giveCauses(error)
    throw error
  }
}

// `withCauses` for an async function.
export async function withCausesAsync<T>(fn: () => Promise<T>): Promise<T> {
  try {
    return await fn()
  } catch (error) {
    giveCauses(error)
    throw error
  }
}

// Give the host the text of each cause of the exception.
function giveCauses(error: unknown) {
  let causes: string[] = []
  let seen = new Set<unknown>()
  let current: any = error
  while (typeof current === 'object' && current !== null && 'cause' in current && current.cause != null) {
    if (seen.add(current).has(current.cause)) break
    current = current.cause
    try {
      causes.push(`${current}`)
    } catch {
      causes.push('[object]')
    }
  }
  host('error.causes', causes)
}
