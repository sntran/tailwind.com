// A part of the module `assert` of Node.js.
import { inspect, isDeepStrictEqual } from './node-util'

export class AssertionError extends Error {
  code = 'ERR_ASSERTION'
  actual: unknown
  expected: unknown
  operator: string
  generatedMessage: boolean
  constructor(options: { message?: unknown; actual?: unknown; expected?: unknown; operator: string; fallback: string }) {
    super(options.message === undefined ? options.fallback : String(options.message))
    this.name = 'AssertionError'
    this.generatedMessage = options.message === undefined
    this.actual = options.actual
    this.expected = options.expected
    this.operator = options.operator
  }
}

function fail(message: unknown, actual: unknown, expected: unknown, operator: string, fallback: string): never {
  if (message instanceof Error) throw message
  let error = new AssertionError({ message, actual, expected, operator, fallback })
  // Node.js: the message of `throws` and `doesNotThrow` is not a generated one.
  if (operator === 'throws' || operator === 'doesNotThrow' || operator === 'rejects') error.generatedMessage = false
  throw error
}

function ok(value: unknown, message?: unknown): asserts value {
  if (!value) fail(message, value, true, '==', `The expression evaluated to a falsy value:\n\n  assert.ok(${inspect(value)})\n`)
}

function check(pass: boolean, actual: unknown, expected: unknown, message: unknown, operator: string, text: string) {
  if (!pass) fail(message, actual, expected, operator, `${text}\n\n${inspect(actual)} ${operator.startsWith('not') ? 'should not equal' : '!=='} ${inspect(expected)}\n`)
}

function matches(error: unknown, expected: unknown): boolean {
  if (expected === undefined) return true
  if (expected instanceof RegExp) return expected.test(String(error))
  if (typeof expected === 'function') {
    if (expected.prototype !== undefined && error instanceof expected) return true
    if (Error.isPrototypeOf(expected)) return false
    return expected.call({}, error) === true
  }
  if (typeof expected === 'object' && expected !== null) {
    for (let key of Object.keys(expected)) {
      let value = (expected as any)[key]
      let actual = (error as any)?.[key]
      if (value instanceof RegExp && typeof actual === 'string' ? !value.test(actual) : !isDeepStrictEqual(actual, value)) return false
    }
    return true
  }
  return false
}

const assert: any = (value: unknown, message?: unknown) => ok(value, message)

Object.assign(assert, {
  ok,
  AssertionError,
  fail(message?: unknown) {
    if (message instanceof Error) throw message
    throw new AssertionError({ message, operator: 'fail', fallback: 'Failed' })
  },
  equal: (actual: unknown, expected: unknown, message?: unknown) =>
    check(actual == expected || (actual !== actual && expected !== expected), actual, expected, message, '==', 'Expected values to be loosely equal:'),
  notEqual: (actual: unknown, expected: unknown, message?: unknown) =>
    check(!(actual == expected || (actual !== actual && expected !== expected)), actual, expected, message, '!=', 'Expected "actual" to be loosely unequal to:'),
  strictEqual: (actual: unknown, expected: unknown, message?: unknown) =>
    check(Object.is(actual, expected), actual, expected, message, 'strictEqual', 'Expected values to be strictly equal:'),
  notStrictEqual: (actual: unknown, expected: unknown, message?: unknown) =>
    check(!Object.is(actual, expected), actual, expected, message, 'notStrictEqual', 'Expected "actual" to be strictly unequal to:'),
  deepEqual: (actual: unknown, expected: unknown, message?: unknown) =>
    check(isDeepStrictEqual(actual, expected, false), actual, expected, message, 'deepEqual', 'Expected values to be loosely deep-equal:'),
  notDeepEqual: (actual: unknown, expected: unknown, message?: unknown) =>
    check(!isDeepStrictEqual(actual, expected, false), actual, expected, message, 'notDeepEqual', 'Expected "actual" not to be loosely deep-equal to:'),
  deepStrictEqual: (actual: unknown, expected: unknown, message?: unknown) =>
    check(isDeepStrictEqual(actual, expected), actual, expected, message, 'deepStrictEqual', 'Expected values to be strictly deep-equal:'),
  notDeepStrictEqual: (actual: unknown, expected: unknown, message?: unknown) =>
    check(!isDeepStrictEqual(actual, expected), actual, expected, message, 'notDeepStrictEqual', 'Expected "actual" not to be strictly deep-equal to:'),
  match: (text: unknown, pattern: RegExp, message?: unknown) =>
    check(typeof text === 'string' && pattern.test(text), text, pattern, message, 'match', 'The input did not match the regular expression.'),
  doesNotMatch: (text: unknown, pattern: RegExp, message?: unknown) =>
    check(typeof text !== 'string' || !pattern.test(text), text, pattern, message, 'doesNotMatch', 'The input was expected to not match the regular expression.'),
  throws(fn: () => unknown, expected?: unknown, message?: unknown) {
    if (typeof expected === 'string') [expected, message] = [undefined, expected]
    let thrown: unknown
    try {
      fn()
    } catch (error) {
      thrown = error
      if (matches(error, expected)) return
      if (typeof expected === 'function' && Error.isPrototypeOf(expected)) throw error
    }
    if (thrown === undefined) {
      fail(message === undefined ? undefined : `Missing expected exception: ${message}`, undefined, expected, 'throws', 'Missing expected exception.')
    }
    fail(message, thrown, expected, 'throws', 'The error did not match the expected value.')
  },
  doesNotThrow(fn: () => unknown, _expected?: unknown, message?: unknown) {
    try {
      fn()
    } catch (error) {
      fail(message, error, undefined, 'doesNotThrow', `Got unwanted exception.\nActual message: "${(error as any)?.message}"`)
    }
  },
  async rejects(promise: Promise<unknown> | (() => Promise<unknown>), expected?: unknown, message?: unknown) {
    if (typeof expected === 'string') [expected, message] = [undefined, expected]
    try {
      await (typeof promise === 'function' ? promise() : promise)
    } catch (error) {
      if (matches(error, expected)) return
      fail(message, error, expected, 'rejects', 'The error did not match the expected value.')
    }
    fail(message, undefined, expected, 'rejects', 'Missing expected rejection.')
  },
  ifError(value: unknown) {
    if (value !== null && value !== undefined) throw value
  },
})
// The strict mode: `equal` is `strictEqual`, and so on.
const strict: any = (value: unknown, message?: unknown) => ok(value, message)
Object.assign(strict, assert, {
  equal: assert.strictEqual,
  notEqual: assert.notStrictEqual,
  deepEqual: assert.deepStrictEqual,
  notDeepEqual: assert.notDeepStrictEqual,
})
strict.strict = strict
strict.default = strict
assert.strict = strict
assert.default = assert

export default assert
