// The timers of Node.js. There is no clock: the job loop of the engine
// runs the next timer when no job of a promise is pending (`js.c` calls
// `__tw_run_timer`). The timers run in the sequence of their times, and
// then of their creation, with no real delay.

type Timer = { id: number; fn: Function; args: unknown[]; time: number; interval: number | null; runs: number }

let timers = new Map<number, Timer>()
let nextId = 1
// The time of the timer that ran last.
let now = 0
// An interval with no end must not keep the program in a loop.
const INTERVAL_LIMIT = 10000

class Timeout {
  id: number
  constructor(id: number) {
    this.id = id
  }
  ref() {
    return this
  }
  unref() {
    return this
  }
  hasRef() {
    return true
  }
  refresh() {
    let timer = timers.get(this.id)
    if (timer) timer.time = now + (timer.interval ?? 0)
    return this
  }
  close() {
    timers.delete(this.id)
    return this
  }
  [Symbol.toPrimitive]() {
    return this.id
  }
}

function add(fn: unknown, delay: unknown, args: unknown[], repeat: boolean): Timeout {
  if (typeof fn !== 'function') {
    let error: any = new TypeError(`The "callback" argument must be of type function. Received ${fn === null ? 'null' : typeof fn}`)
    error.code = 'ERR_INVALID_ARG_TYPE'
    throw error
  }
  let ms = Number(delay)
  if (!(ms >= 1)) ms = 1
  let id = nextId++
  timers.set(id, { id, fn, args, time: now + ms, interval: repeat ? ms : null, runs: 0 })
  return new Timeout(id)
}

function clear(timer: unknown) {
  if (timer === undefined || timer === null) return
  timers.delete(typeof timer === 'object' ? (timer as Timeout).id : Number(timer))
}

// Run the next timer. The result is false when there is no timer.
export function runTimer(): boolean {
  let next: Timer | null = null
  for (let timer of timers.values()) {
    if (next === null || timer.time < next.time) next = timer
  }
  if (next === null) return false
  now = next.time
  if (next.interval === null || ++next.runs >= INTERVAL_LIMIT) timers.delete(next.id)
  else next.time = now + next.interval
  next.fn(...next.args)
  return true
}

// Run the timers that are left, and the jobs that they make. An interval
// does not run again here. `wait` is `__await`: it runs the jobs and the
// timers until a promise settles.
export function drainTimers(wait: (promise: Promise<unknown>) => unknown) {
  for (let timer of timers.values()) {
    if (timer.interval !== null) timers.delete(timer.id)
  }
  while (timers.size > 0) {
    let last = 0
    for (let timer of timers.values()) last = Math.max(last, timer.time)
    wait(
      new Promise<void>((resolve) => {
        let id = nextId++
        timers.set(id, { id, fn: resolve, args: [], time: last, interval: null, runs: 0 })
      }),
    )
  }
}

export function install(global: any) {
  global.setTimeout = (fn: unknown, delay?: unknown, ...args: unknown[]) => add(fn, delay, args, false)
  global.setInterval = (fn: unknown, delay?: unknown, ...args: unknown[]) => add(fn, delay, args, true)
  // Node.js runs an immediate before the timers: here it is a timer of
  // the current time.
  global.setImmediate = (fn: unknown, ...args: unknown[]) => {
    let timer = add(fn, 1, args, false)
    timers.get(timer.id)!.time = now
    return timer
  }
  global.clearTimeout = clear
  global.clearInterval = clear
  global.clearImmediate = clear
  global.queueMicrotask ??= (fn: Function) => {
    Promise.resolve().then(() => fn())
  }
  global.__tw_run_timer = runTimer
}

const timersModule: Record<string, unknown> = {
  get setTimeout() {
    return (globalThis as any).setTimeout
  },
  get setInterval() {
    return (globalThis as any).setInterval
  },
  get setImmediate() {
    return (globalThis as any).setImmediate
  },
  clearTimeout: clear,
  clearInterval: clear,
  clearImmediate: clear,
  promises: {
    setTimeout: (delay?: number, value?: unknown) => new Promise((resolve) => add(() => resolve(value), delay, [], false)),
    setImmediate: (value?: unknown) => new Promise((resolve) => (globalThis as any).setImmediate(() => resolve(value))),
  },
}
timersModule.default = timersModule

export default timersModule
