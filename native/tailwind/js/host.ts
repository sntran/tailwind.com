// The call of the host (the Rust compiler). Data goes as JSON text.

declare function __host(op: string, text?: string): string | undefined

// Source maps. The host sets `tracking` when the compiler keeps the source
// locations. Then each declaration and each comment that goes to the host
// gets a number (`__id`): the identity of the object. The host needs it,
// because Tailwind CSS gives one object that is at two places of the tree
// one location in the output. The numbers of this side are odd, and the
// numbers of the host are even. The number is not a property of the
// object: a copy of an object is a new object.
let tracking = false
let identities = new WeakMap<object, number>()
let nextIdentity = 1

export function setTracking(value: boolean) {
  tracking = value
}

export function encode(value: unknown): string {
  if (!tracking) return JSON.stringify(value)
  return JSON.stringify(value, (_key, item) => {
    if (item !== null && typeof item === 'object' && (item.kind === 'declaration' || item.kind === 'comment')) {
      let id = identities.get(item)
      if (id === undefined) {
        id = nextIdentity
        nextIdentity += 2
        identities.set(item, id)
      }
      return { ...item, __id: id }
    }
    return item
  })
}

export function decode(text: string): any {
  if (!tracking) return JSON.parse(text)
  return JSON.parse(text, (_key, item) => {
    if (item !== null && typeof item === 'object' && typeof item.__id === 'number') {
      identities.set(item, item.__id)
      delete item.__id
    }
    return item
  })
}

export function host(op: string, arg?: unknown): any {
  let result = __host(op, arg === undefined ? undefined : encode(arg))
  return result === undefined ? undefined : decode(result)
}

// The functions that the host calls by a number: the compile functions of
// utilities, the functions of variants, and the compare functions.
export let callbacks: Function[] = []

export function register(fn: Function): number {
  callbacks.push(fn)
  return callbacks.length - 1
}
