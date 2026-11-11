// The cases for the parts of Node.js that the program gives to the
// JavaScript of a plugin or config. Each case gives a value that JSON can
// hold. `tests/upstream/tools/gen-node-api.mjs` runs this file in Node.js
// and records the values (`node_api.json`), and `tests/node_api.rs` runs
// it in the engine of the program and compares.
//
// `dir` is a directory with these files: `text.txt`, `bytes.bin`,
// `sub/a.txt`, `sub/b.json`, `sub/deep/c.txt`.
const assert = require('node:assert')
const fs = require('node:fs')
const fsPromises = require('node:fs/promises')
const nodeModule = require('node:module')
const os = require('node:os')
const path = require('node:path')
const url = require('node:url')
const util = require('node:util')

function error(fn) {
  try {
    fn()
  } catch (e) {
    return { name: e.name, code: e.code, message: e.message }
  }
  return 'no error'
}

async function rejection(fn) {
  try {
    await fn()
  } catch (e) {
    return { name: e.name, code: e.code, message: e.message }
  }
  return 'no error'
}

const cases = {
  // Buffer
  'Buffer.from utf8': () => Array.from(Buffer.from('héllo €𝄞')),
  'Buffer.from default encoding': () => Buffer.from('abc').toString(),
  'Buffer.from base64': () => Buffer.from('aGVsbG8gd29ybGQ=', 'base64').toString('utf8'),
  'Buffer.from base64 no padding and url alphabet': () => Array.from(Buffer.from('-_8', 'base64')),
  'Buffer.from base64 with spaces': () => Buffer.from('aGVs\nbG8=', 'base64').toString(),
  'Buffer.from hex': () => Array.from(Buffer.from('00ff7f80', 'hex')),
  'Buffer.from hex invalid tail': () => Array.from(Buffer.from('abzz12', 'hex')),
  'Buffer.from latin1': () => Array.from(Buffer.from('é€a', 'latin1')),
  'Buffer.from utf16le': () => Array.from(Buffer.from('aé', 'utf16le')),
  'Buffer.from array': () => Buffer.from([104, 105, 256 + 33]).toString(),
  'Buffer.from buffer is a copy': () => {
    let a = Buffer.from('ab')
    let b = Buffer.from(a)
    b[0] = 122
    return [a.toString(), b.toString()]
  },
  'Buffer.from ArrayBuffer': () => Buffer.from(new Uint8Array([1, 2, 3, 4]).buffer, 1, 2).toJSON(),
  'Buffer.from number throws': () => error(() => Buffer.from(5)),
  'Buffer.from unknown encoding': () => error(() => Buffer.from('a', 'nope')),
  'Buffer toString utf8': () => Buffer.from([0xe2, 0x82, 0xac, 0xf0, 0x9d, 0x84, 0x9e]).toString('utf8'),
  'Buffer toString utf8 invalid bytes': () => Buffer.from([0x61, 0xff, 0xe2, 0x82, 0x62, 0xc0, 0x80, 0xed, 0xa0, 0x80]).toString(),
  'Buffer toString base64': () => [Buffer.from('').toString('base64'), Buffer.from('a').toString('base64'), Buffer.from('ab').toString('base64'), Buffer.from('abc').toString('base64'), Buffer.from([251, 255]).toString('base64')],
  'Buffer toString base64url': () => Buffer.from([251, 255, 254]).toString('base64url'),
  'Buffer toString hex': () => Buffer.from('héllo').toString('hex'),
  'Buffer toString latin1 and ascii': () => [Buffer.from([0xe9, 0x41]).toString('latin1'), Buffer.from([0xe9, 0x41]).toString('ascii')],
  'Buffer toString range': () => Buffer.from('abcdef').toString('utf8', 1, 4),
  'Buffer.byteLength': () => [Buffer.byteLength('héllo €𝄞'), Buffer.byteLength('aGVsbG8=', 'base64'), Buffer.byteLength('abcd', 'hex'), Buffer.byteLength(new Uint8Array(7))],
  'Buffer.concat': () => Buffer.concat([Buffer.from('ab'), Buffer.from('cd'), new Uint8Array([101])]).toString(),
  'Buffer.concat with length': () => [Buffer.concat([Buffer.from('abc'), Buffer.from('def')], 4).toString(), Buffer.concat([Buffer.from('a')], 3).toJSON()],
  'Buffer.isBuffer': () => [Buffer.isBuffer(Buffer.from('a')), Buffer.isBuffer(new Uint8Array(1)), Buffer.isBuffer('a'), Buffer.from('a') instanceof Uint8Array],
  'Buffer.isEncoding': () => ['utf8', 'UTF-8', 'hex', 'base64', 'latin1', 'nope', 5].map((e) => Buffer.isEncoding(e)),
  'Buffer.alloc': () => [Buffer.alloc(3).toJSON(), Buffer.alloc(4, 'ab').toString(), Buffer.alloc(2, 1).toJSON()],
  'Buffer.compare and equals': () => [Buffer.compare(Buffer.from('a'), Buffer.from('b')), Buffer.from('a').equals(Buffer.from('a')), Buffer.from('ab').compare(Buffer.from('a'))],
  'Buffer slice shares memory': () => {
    let a = Buffer.from('abc')
    let b = a.subarray(1)
    b[0] = 120
    return [a.toString(), Buffer.isBuffer(b), Buffer.isBuffer(a.slice(0, 1))]
  },
  'Buffer indexOf and includes': () => [Buffer.from('hello').indexOf('l'), Buffer.from('hello').indexOf('lo'), Buffer.from('hello').includes('z'), Buffer.from('hello').indexOf(111)],
  'Buffer toJSON': () => JSON.stringify(Buffer.from('hi')),
  'Buffer write': () => {
    let a = Buffer.alloc(5)
    let n = a.write('héy', 1)
    return [n, a.toJSON()]
  },
  'buffer module': () => [require('buffer').Buffer === Buffer, require('node:buffer').Buffer === Buffer],
  'atob and btoa': () => [btoa('héllo'), atob('aOlsbG8='), error(() => btoa('€')).name, atob(' aGk= ')],

  // TextEncoder, TextDecoder
  'TextEncoder': () => [Array.from(new TextEncoder().encode('a€𝄞')), new TextEncoder().encoding, Array.from(new TextEncoder().encode())],
  'TextEncoder lone surrogate': () => Array.from(new TextEncoder().encode('a\ud800b')),
  'TextEncoder encodeInto': () => {
    let target = new Uint8Array(4)
    let result = new TextEncoder().encodeInto('a€€', target)
    return [result, Array.from(target)]
  },
  'TextDecoder': () => [new TextDecoder().decode(new Uint8Array([0xe2, 0x82, 0xac])), new TextDecoder().encoding, new TextDecoder('utf8').decode(), new TextDecoder().decode(new Uint8Array([0xef, 0xbb, 0xbf, 0x61]))],
  'TextDecoder invalid bytes': () => new TextDecoder().decode(new Uint8Array([0x61, 0xff, 0xf0, 0x9d, 0x62])),
  'TextDecoder fatal': () => error(() => new TextDecoder('utf-8', { fatal: true }).decode(new Uint8Array([0xff]))).name,
  'TextDecoder ArrayBuffer and view': () => [new TextDecoder().decode(new Uint8Array([104, 105]).buffer), new TextDecoder().decode(new DataView(new Uint8Array([104, 105]).buffer, 1))],
  'TextDecoder utf-16le': () => new TextDecoder('utf-16le').decode(new Uint8Array([0x61, 0, 0xac, 0x20])),
  'TextDecoder unknown label': () => error(() => new TextDecoder('nope')).name,

  // URL
  'URL parts': () => {
    let u = new URL('https://user:pa%20ss@Example.COM:8443/a/b/../c%20d/?x=1&y=two#frag')
    return [u.href, u.protocol, u.username, u.password, u.host, u.hostname, u.port, u.pathname, u.search, u.hash, u.origin]
  },
  'URL default port': () => [new URL('https://example.com:443/').href, new URL('http://example.com:80').host, new URL('http://example.com:8080').port],
  'URL relative': () => ['../x', './y?z', '/abs', '?q', '#h', '//other.org/p', 'g/h', ''].map((r) => new URL(r, 'https://example.com/a/b/c?old#frag').href),
  'URL file': () => {
    let u = new URL('file:///home/user/my%20file.txt')
    return [u.href, u.pathname, u.host, u.origin, new URL('sub/x.js', 'file:///a/b/').href, new URL('file:///a/./b/../c').href]
  },
  'URL encoding': () => [new URL('https://example.com/a b/é?q=a b&é#h h').href, new URL('http://example.com/a\\b').pathname],
  'URL data and opaque': () => {
    let u = new URL('data:text/plain;base64,aGk=')
    return [u.protocol, u.pathname, u.href, u.origin, new URL('mailto:a@b.c').pathname]
  },
  'URL invalid': () => [error(() => new URL('not a url')), error(() => new URL('/relative')).code, error(() => new URL('http://')).code],
  'URL canParse and parse': () => [URL.canParse('https://a.b'), URL.canParse('nope'), URL.canParse('x', 'https://a.b/'), URL.parse('nope'), URL.parse('https://a.b/c').pathname],
  'URL toString and toJSON': () => [String(new URL('https://a.b/c')), JSON.stringify({ u: new URL('https://a.b/c') })],
  'URL searchParams is live': () => {
    let u = new URL('https://a.b/?x=1')
    u.searchParams.append('y', 'a b')
    u.searchParams.set('x', '2')
    return [u.search, u.href, u.searchParams.get('y')]
  },
  'URL setters': () => {
    let u = new URL('https://a.b/c?d#e')
    u.pathname = '/x y'
    u.search = 'q=1'
    u.hash = 'top'
    return u.href
  },
  'URLSearchParams from text': () => {
    let p = new URLSearchParams('?a=1&b=two+words&a=3&c=%C3%A9&d')
    return [p.get('a'), p.getAll('a'), p.get('b'), p.get('c'), p.get('d'), p.get('z'), p.has('b'), p.size, [...p.keys()]]
  },
  'URLSearchParams from object and pairs': () => [new URLSearchParams({ a: '1', b: 'x y' }).toString(), new URLSearchParams([['a', '1'], ['a', '2']]).toString(), new URLSearchParams().toString()],
  'URLSearchParams toString encoding': () => new URLSearchParams({ q: "a b&c=d/é~*'()!", 'k k': '+' }).toString(),
  'URLSearchParams change': () => {
    let p = new URLSearchParams('b=2&a=1&b=3')
    p.append('c', '4')
    p.delete('a')
    p.set('b', '9')
    p.sort()
    let seen = []
    p.forEach((value, name) => seen.push(`${name}:${value}`))
    return [p.toString(), seen, [...p], [...p.values()]]
  },
  'url.fileURLToPath': () => [url.fileURLToPath('file:///home/user/my%20file%C3%A9.txt'), url.fileURLToPath(new URL('file:///a/b')), url.fileURLToPath('file://localhost/a')],
  'url.fileURLToPath errors': () => [error(() => url.fileURLToPath('https://a.b/c')).code, error(() => url.fileURLToPath('file://host/a')).code, error(() => url.fileURLToPath('file:///a%2Fb')).code, error(() => url.fileURLToPath(5)).code],
  'url.pathToFileURL': () => [url.pathToFileURL('/home/user/my file é#?%.txt').href, url.pathToFileURL('/a/b/../c/').href, url.pathToFileURL('/').href],
  'url module has URL': () => [url.URL === URL, url.URLSearchParams === URLSearchParams],

  // Timers and jobs
  'setTimeout order': async () => {
    let seen = []
    await new Promise((resolve) => {
      setTimeout(() => seen.push('b 20'), 20)
      setTimeout(() => seen.push('a 10'), 10)
      setTimeout(() => seen.push('c 10'), 10)
      setTimeout(() => {
        seen.push('d 30')
        resolve()
      }, 30)
      queueMicrotask(() => seen.push('microtask'))
      Promise.resolve().then(() => seen.push('promise'))
      seen.push('sync')
    })
    return seen
  },
  'setTimeout arguments': () => new Promise((resolve) => setTimeout((a, b) => resolve([a, b]), 1, 'x', 'y')),
  'clearTimeout': async () => {
    let seen = []
    let timer = setTimeout(() => seen.push('cleared'), 5)
    clearTimeout(timer)
    clearTimeout(undefined)
    await new Promise((resolve) => setTimeout(resolve, 10))
    return seen
  },
  'nested timers': async () => {
    let seen = []
    await new Promise((resolve) => {
      setTimeout(() => {
        seen.push('outer')
        setTimeout(() => {
          seen.push('inner')
          resolve()
        }, 5)
      }, 5)
      setTimeout(() => seen.push('later'), 50)
    })
    return seen
  },
  'setImmediate': async () => {
    let seen = []
    await new Promise((resolve) => {
      setTimeout(() => {
        seen.push('timeout 50')
        resolve()
      }, 50)
      setImmediate((value) => seen.push(`immediate ${value}`), 'x')
      let cleared = setImmediate(() => seen.push('cleared'))
      clearImmediate(cleared)
    })
    return seen
  },
  'setInterval': () =>
    new Promise((resolve) => {
      let count = 0
      let timer = setInterval(() => {
        if (++count === 3) {
          clearInterval(timer)
          resolve(count)
        }
      }, 2)
    }),
  'timer object': () => {
    let timer = setTimeout(() => {}, 1)
    let result = [typeof timer, typeof timer.ref, typeof timer.unref, timer.unref() === timer, typeof timer[Symbol.toPrimitive]()]
    clearTimeout(timer)
    return result
  },
  'setTimeout needs a function': () => error(() => setTimeout('code', 1)).code,
  'timers/promises': async () => [await require('node:timers/promises').setTimeout(1, 'value'), typeof require('timers').setTimeout],
  'process.nextTick': () => new Promise((resolve) => process.nextTick((a) => resolve(a), 'tick')),

  // structuredClone
  'structuredClone data': () => {
    let source = { a: [1, { b: 2 }], d: new Date(5), r: /x/gi, m: new Map([[1, { deep: true }]]), s: new Set([1, 2]), n: null, u: undefined }
    let copy = structuredClone(source)
    return [copy.a, copy.a !== source.a, copy.d.getTime(), copy.d instanceof Date, String(copy.r), [...copy.m], [...copy.s], 'u' in copy, copy.m.get(1) !== source.m.get(1)]
  },
  'structuredClone cycle': () => {
    let a = { name: 'a' }
    a.self = a
    let copy = structuredClone(a)
    return [copy.self === copy, copy !== a]
  },
  'structuredClone bytes': () => {
    let copy = structuredClone(new Uint8Array([1, 2, 3]))
    let buffer = structuredClone(Buffer.from('ab'))
    return [copy instanceof Uint8Array, Array.from(copy), Buffer.isBuffer(buffer), Array.from(buffer)]
  },
  'structuredClone drops the prototype': () => {
    class Point {
      constructor() {
        this.x = 1
      }
    }
    let copy = structuredClone(new Point())
    return [copy.x, copy instanceof Point, Object.getPrototypeOf(copy) === Object.prototype]
  },
  'structuredClone error': () => {
    let copy = structuredClone(new TypeError('bad', { cause: 'why' }))
    return [copy instanceof TypeError, copy.message, copy.name, copy.cause]
  },
  'structuredClone function throws': () => {
    let e = error(() => structuredClone({ f() {} }))
    return [e.name, e.code]
  },

  // process
  'process.env': () => [process.env.NODE_API_TEST, process.env.NOT_SET_ANYWHERE, typeof process.env.HOME, 'NODE_API_TEST' in process.env],
  'process basics': () => [typeof process.cwd(), path.isAbsolute(process.cwd()), process.platform, Array.isArray(process.argv), process.argv.length >= 2, typeof process.versions, typeof process.versions.node, /^v\d+\.\d+\.\d+$/.test(process.version)],
  'process module': () => [require('process') === process, require('node:process') === process],

  // os
  'os': () => [os.EOL, os.platform(), os.homedir(), os.tmpdir(), os.type(), os.endianness()],

  // util
  'util.format': () => [
    util.format('%s:%s', 'a', 'b', 'c'),
    util.format('%d %i %f', '42.5', '42.5', '42.5'),
    util.format('%j', { a: [1] }),
    util.format('%s %s %s %s', 1, -0, 10n, null),
    util.format('%s', { a: 1 }),
    util.format('%s', new Error('boom')).split('\n')[0],
    util.format('100%% %s', 'done'),
    util.format('%s'),
    util.format('a', 'b', 1, { c: 2 }),
    util.format(1, 2, '3'),
    util.format('%O', { a: { b: { c: { d: 1 } } } }),
    util.format('%x', 1),
    util.format(),
  ],
  'util.inspect primitives': () => [1, -0, 'str', "it's", `"both' kinds`, true, null, undefined, 10n, Symbol('s'), 'line\nbreak'].map((v) => util.inspect(v)),
  'util.inspect objects': () => [
    util.inspect({}),
    util.inspect({ a: 1, 'b-c': 'x', 3: [1, 2] }),
    util.inspect([]),
    util.inspect([1, 'two', [3]]),
    util.inspect({ a: { b: { c: { d: 1 } } } }),
    util.inspect({ a: { b: { c: { d: 1 } } } }, { depth: 0 }),
    util.inspect({ a: { b: { c: { d: 1 } } } }, { depth: null }),
    util.inspect(new Map([['k', { v: 1 }]])),
    util.inspect(new Set([1, 'a'])),
    util.inspect(new Date(0)),
    util.inspect(/re/g),
    util.inspect(Object.create(null)),
    util.inspect([1, , 3, , ,]),
    util.inspect({ [Symbol('key')]: 1 }),
  ],
  'util.inspect functions and classes': () => {
    class Animal {
      constructor() {
        this.legs = 4
      }
    }
    class Dog extends Animal {}
    return [util.inspect(function named() {}), util.inspect(() => {}), util.inspect(class A {}), util.inspect(Dog), util.inspect(new Dog()), util.inspect(async () => {}), util.inspect({ f() {}, g: Math.max })]
  },
  'util.inspect long values': () => [
    util.inspect({ first: 'a long string value to fill the line', second: 'another long string value here', third: 3 }),
    util.inspect(Array.from({ length: 30 }, (_, i) => i * 3)),
    util.inspect(Array.from({ length: 8 }, (_, i) => `item-${i}`)),
    util.inspect(Array.from({ length: 120 }, (_, i) => i)).split('\n').pop(),
    util.inspect({ list: [{ a: 1, b: [1, 2, { c: 3 }] }, { a: 2 }], name: 'nested' }),
    util.inspect([['a', 1], ['b', 2]]),
  ],
  'util.inspect other': () => {
    let circular = { name: 'c' }
    circular.self = circular
    return [
      util.inspect(circular),
      util.inspect(Buffer.from('abc')),
      util.inspect(new Uint8Array([1, 2])),
      util.inspect({ get a() { return 1 }, set b(v) {} }),
      util.inspect({ [util.inspect.custom]: () => 'custom!' }),
      util.inspect(new Error('boom')).split('\n')[0],
      util.inspect({ a: 1 }, false, 0),
    ]
  },
  'util.promisify': async () => {
    let read = util.promisify((value, callback) => (value === 'bad' ? callback(new Error('failed')) : callback(null, value + '!')))
    return [await read('ok'), (await rejection(() => read('bad'))).message, error(() => util.promisify('x')).code]
  },
  'util.promisify fs.readFile': async ({ dir }) => (await util.promisify(fs.readFile)(path.join(dir, 'text.txt'), 'utf8')).length,
  'util.deprecate': () => {
    let fn = util.deprecate((a, b) => a + b, 'old function', 'DEP_TEST')
    return [fn(1, 2), fn(3, 4), typeof fn]
  },
  'util.types': () => [
    util.types.isPromise(Promise.resolve()), util.types.isPromise({ then() {} }), util.types.isRegExp(/a/), util.types.isDate(new Date()), util.types.isNativeError(new TypeError()),
    util.types.isMap(new Map()), util.types.isSet(new Set()), util.types.isTypedArray(new Uint8Array()), util.types.isTypedArray(Buffer.from('')), util.types.isUint8Array([]),
    util.types.isArrayBuffer(new ArrayBuffer(1)), util.types.isAsyncFunction(async () => {}), util.types.isAsyncFunction(() => {}), util.types.isGeneratorFunction(function* () {}),
    require('node:util/types').isDate(1),
  ],
  'util.inherits': () => {
    function Base() {}
    Base.prototype.hello = () => 'hello'
    function Child() {}
    util.inherits(Child, Base)
    return [new Child().hello(), Child.super_ === Base, new Child() instanceof Base]
  },
  'util.isDeepStrictEqual': () => [
    util.isDeepStrictEqual({ a: [1, { b: 2 }] }, { a: [1, { b: 2 }] }), util.isDeepStrictEqual({ a: 1 }, { a: '1' }), util.isDeepStrictEqual([1, 2], [2, 1]),
    util.isDeepStrictEqual(new Map([[1, { a: 1 }]]), new Map([[1, { a: 1 }]])), util.isDeepStrictEqual(new Set([1, 2]), new Set([2, 1])), util.isDeepStrictEqual(NaN, NaN),
    util.isDeepStrictEqual(0, -0), util.isDeepStrictEqual({ a: undefined }, {}), util.isDeepStrictEqual(new Date(1), new Date(1)), util.isDeepStrictEqual([], {}),
  ],
  'util text encoders': () => [util.TextEncoder === TextEncoder, util.TextDecoder === TextDecoder],

  // assert
  'assert passes': () => {
    assert(true)
    assert.ok(1)
    assert.equal(1, '1')
    assert.strictEqual('a', 'a')
    assert.notStrictEqual(1, 2)
    assert.deepEqual({ a: 1 }, { a: '1' })
    assert.deepStrictEqual({ a: [1, 2] }, { a: [1, 2] })
    assert.notDeepStrictEqual({ a: 1 }, { a: 2 })
    assert.throws(() => {
      throw new TypeError('x')
    }, TypeError)
    assert.throws(() => {
      throw new Error('message here')
    }, /message/)
    assert.throws(() => {
      throw new Error('m')
    }, { message: 'm', name: 'Error' })
    assert.doesNotThrow(() => {})
    assert.match('abc', /b/)
    assert.strict.equal(1, 1)
    return 'done'
  },
  'assert failures': () => {
    let fail = (fn) => {
      try {
        fn()
      } catch (e) {
        return [e.name, e.code, e.operator, e instanceof assert.AssertionError, e instanceof Error, e.generatedMessage]
      }
      return 'no error'
    }
    return [
      fail(() => assert(false)), fail(() => assert.ok(0)), fail(() => assert.equal(1, 2)), fail(() => assert.strictEqual(1, '1')), fail(() => assert.notStrictEqual(1, 1)),
      fail(() => assert.deepStrictEqual({ a: 1 }, { a: '1' })), fail(() => assert.deepEqual([1], [2])), fail(() => assert.throws(() => {})), fail(() => assert.doesNotThrow(() => { throw new Error('x') })),
      fail(() => assert.match('abc', /z/)), fail(() => assert.fail()), fail(() => assert.strict.equal(1, '1')),
    ]
  },
  'assert messages': () => [
    error(() => assert(false, 'my message')).message,
    error(() => assert.throws(() => {}, 'no exception')).message,
    error(() => assert.fail('failed here')).message,
    error(() => assert.fail()).message,
    error(() => assert.ok(false, new RangeError('own error'))).name,
    error(() => assert.strictEqual(1, 2)).message,
    error(() => assert.throws(() => {})).message,
  ],
  'assert error fields': () => {
    try {
      assert.strictEqual('actual value', 'expected value')
    } catch (e) {
      return [e.actual, e.expected, e.operator]
    }
  },
  'assert.rejects': async () => {
    await assert.rejects(Promise.reject(new TypeError('x')), TypeError)
    return (await rejection(() => assert.rejects(Promise.resolve(1)))).code
  },

  // fs
  'fs.readFileSync text': ({ dir }) => [fs.readFileSync(path.join(dir, 'text.txt'), 'utf8'), fs.readFileSync(path.join(dir, 'text.txt'), { encoding: 'utf-8' }).length],
  'fs.readFileSync Buffer': ({ dir }) => {
    let data = fs.readFileSync(path.join(dir, 'text.txt'))
    return [Buffer.isBuffer(data), data.length, data.toString(), data.toString('base64'), String(data).length]
  },
  'fs.readFileSync bytes': ({ dir }) => {
    let data = fs.readFileSync(path.join(dir, 'bytes.bin'))
    return [Array.from(data), data.toString('hex'), fs.readFileSync(path.join(dir, 'bytes.bin'), 'latin1').length, fs.readFileSync(path.join(dir, 'bytes.bin'), 'utf8').length, fs.readFileSync(path.join(dir, 'bytes.bin'), 'base64')]
  },
  'fs.readFileSync URL and Buffer path': ({ dir }) => [fs.readFileSync(url.pathToFileURL(path.join(dir, 'sub/a.txt')), 'utf8'), fs.readFileSync(Buffer.from(path.join(dir, 'sub/a.txt')), 'utf8')],
  'fs.readFileSync errors': ({ dir }) => [error(() => fs.readFileSync(path.join(dir, 'missing.txt'))), error(() => fs.readFileSync(dir)), error(() => fs.readFileSync({})).code],
  'fs.readdirSync': ({ dir }) => [fs.readdirSync(dir), fs.readdirSync(path.join(dir, 'sub')), fs.readdirSync(dir + '/')],
  'fs.readdirSync withFileTypes': ({ dir }) => fs.readdirSync(dir, { withFileTypes: true }).map((entry) => [entry.name, entry.isFile(), entry.isDirectory(), entry.isSymbolicLink(), entry.parentPath === dir]),
  'fs.readdirSync recursive': ({ dir }) => fs.readdirSync(dir, { recursive: true }).sort(),
  'fs.readdirSync errors': ({ dir }) => [error(() => fs.readdirSync(path.join(dir, 'missing'))), error(() => fs.readdirSync(path.join(dir, 'text.txt')))],
  'fs.existsSync': ({ dir }) => [fs.existsSync(dir), fs.existsSync(path.join(dir, 'text.txt')), fs.existsSync(path.join(dir, 'missing')), fs.existsSync(), fs.existsSync(5)],
  'fs.statSync': ({ dir }) => {
    let file = fs.statSync(path.join(dir, 'text.txt'))
    let directory = fs.statSync(path.join(dir, 'sub'))
    return [file.isFile(), file.isDirectory(), file.isSymbolicLink(), file.size, file.mtime instanceof Date, typeof file.mtimeMs, directory.isFile(), directory.isDirectory(), file instanceof fs.Stats]
  },
  'fs.statSync errors': ({ dir }) => [error(() => fs.statSync(path.join(dir, 'missing'))), fs.statSync(path.join(dir, 'missing'), { throwIfNoEntry: false })],
  'fs.lstatSync': ({ dir }) => {
    let file = fs.lstatSync(path.join(dir, 'text.txt'))
    return [file.isFile(), file.isSymbolicLink(), error(() => fs.lstatSync(path.join(dir, 'missing')))]
  },
  'fs.realpathSync': ({ dir }) => [fs.realpathSync(path.join(dir, 'sub/../text.txt')) === path.join(fs.realpathSync(dir), 'text.txt'), error(() => fs.realpathSync(path.join(dir, 'missing'))), typeof fs.realpathSync.native],
  'fs.accessSync': ({ dir }) => [fs.accessSync(path.join(dir, 'text.txt')), fs.accessSync(dir, fs.constants.R_OK), error(() => fs.accessSync(path.join(dir, 'missing'))), [fs.constants.F_OK, fs.constants.R_OK, fs.constants.W_OK, fs.constants.X_OK]],
  'fs.promises': async ({ dir }) => [
    await fs.promises.readFile(path.join(dir, 'text.txt'), 'utf8'),
    Buffer.isBuffer(await fs.promises.readFile(path.join(dir, 'text.txt'))),
    await fs.promises.readdir(path.join(dir, 'sub')),
    (await fs.promises.stat(dir)).isDirectory(),
    (await fs.promises.lstat(path.join(dir, 'text.txt'))).isFile(),
    await fs.promises.access(dir),
    (await fs.promises.realpath(dir)) === fs.realpathSync(dir),
    await rejection(() => fs.promises.readFile(path.join(dir, 'missing'))),
    fs.promises.readFile(path.join(dir, 'text.txt')) instanceof Promise,
    fsPromises === fs.promises,
    require('fs/promises') === fs.promises,
  ],
  'fs callbacks': ({ dir }) =>
    new Promise((resolve) => {
      let seen = ['before']
      fs.readFile(path.join(dir, 'sub/a.txt'), 'utf8', (error, text) => {
        seen.push(error, text)
        fs.stat(path.join(dir, 'missing'), (error) => {
          seen.push(error.code)
          resolve(seen)
        })
      })
      seen.push('after')
    }),
  'fs.readFileSync JSON': ({ dir }) => JSON.parse(fs.readFileSync(path.join(dir, 'sub/b.json'))),

  // module
  'module.createRequire': ({ dir }) => {
    let fromPath = nodeModule.createRequire(path.join(dir, 'anything.js'))
    let fromUrl = nodeModule.createRequire(url.pathToFileURL(path.join(dir, 'sub/x.js')))
    let fromText = nodeModule.createRequire(url.pathToFileURL(path.join(dir, 'x.js')).href)
    return [fromPath('./sub/b.json'), fromUrl('./b.json'), fromText('./sub/b.json').list, typeof fromPath.resolve, fromPath.resolve('./sub/b.json') === path.join(dir, 'sub/b.json'), fromPath('node:path') === path]
  },
  'module.createRequire errors': ({ dir }) => [error(() => nodeModule.createRequire('relative/path.js')).code, error(() => nodeModule.createRequire(path.join(dir, 'x.js'))('./missing')).code, error(() => nodeModule.createRequire(path.join(dir, 'x.js')).resolve('./missing')).code],
  'module.isBuiltin': () => ['fs', 'node:fs', 'path', 'url', 'os', 'util', 'assert', 'module', 'buffer', 'fs/promises', 'not-a-module', './fs'].map((name) => nodeModule.isBuiltin(name)),
  'require.resolve': ({ dir }) => [require.resolve('node:fs'), require.resolve('path'), typeof require.resolve, error(() => require.resolve('./not-there-at-all')).code],
  'require of the modules': () => ['assert', 'buffer', 'fs', 'fs/promises', 'module', 'os', 'path', 'process', 'timers', 'url', 'util', 'node:assert/strict', 'node:timers/promises', 'node:util/types'].map((name) => typeof require(name)),
  'require of a missing module': () => {
    let e = error(() => require('this-package-is-not-there'))
    return [e.code, e.message.split('\n')[0]]
  },
}

module.exports = async function run(dir) {
  let results = {}
  for (let [name, fn] of Object.entries(cases)) {
    try {
      let value = await fn({ dir })
      results[name] = value === undefined ? null : value
    } catch (e) {
      results[name] = { failed: `${e && e.name}: ${e && e.message}` }
    }
  }
  // JSON has no `undefined`, no symbol and no bigint: text for them.
  return JSON.parse(JSON.stringify(results, (key, value) => (value === undefined ? '[undefined]' : typeof value === 'bigint' ? `${value}n` : typeof value === 'symbol' ? String(value) : value)))
}
