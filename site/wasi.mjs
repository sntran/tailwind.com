// A WASI host (preview 1) for tailwind.wasm, with a file system in memory.
// It runs in a browser and in Node.js, with no dependency.
//
//   const files = new Files()
//   files.write('/app/app.css', '@import "tailwindcss";')
//   const module = await WebAssembly.compileStreaming(fetch('tailwind.wasm'))
//   const result = await run(module, { args: ['-i', 'app.css'], files, preopens: { '/': '/app' } })
//   // result: { status, stdout, stderr }
//
// Under WASI preview 1, the working directory of the program is always
// "/". So the directory of the project is the root "/" of the program, as
// with `runWasm` of js/node.mjs. Each run makes a new instance of the
// module, and the files stay for the next run. The host has only
// the functions that tailwind.wasm imports. It has no symbolic links, and
// a wait (`poll_oneoff`) does not wait.

const ERRNO = {
  SUCCESS: 0, BADF: 8, EXIST: 20, INVAL: 28, IO: 29, ISDIR: 31, NOENT: 44,
  NOSYS: 52, NOTDIR: 54, NOTEMPTY: 55, NOTSUP: 58, SPIPE: 70,
}
const FILETYPE = { UNKNOWN: 0, CHARACTER_DEVICE: 2, DIRECTORY: 3, REGULAR_FILE: 4 }
const OFLAGS = { CREAT: 1, DIRECTORY: 2, EXCL: 4, TRUNC: 8 }
const FDFLAGS = { APPEND: 1 }
const ALL_RIGHTS = 0x1fffffffn

class Exit {
  constructor(code) {
    this.code = code
  }
}

class Failure {
  constructor(errno) {
    this.errno = errno
  }
}

let nextInode = 1n

function file(data) {
  return { kind: FILETYPE.REGULAR_FILE, data, inode: nextInode++, time: now() }
}

function directory() {
  return { kind: FILETYPE.DIRECTORY, children: new Map(), inode: nextInode++, time: now() }
}

function now() {
  return BigInt(Date.now()) * 1000000n
}

const encoder = new TextEncoder()
const decoder = new TextDecoder()

// The file system. Paths are absolute, with "/" between the names.
export class Files {
  constructor() {
    this.root = directory()
  }

  // The node of a path, or a Failure.
  find(path, from = this.root) {
    let node = from
    const stack = [from]
    for (const name of path.split('/')) {
      if (name === '' || name === '.') continue
      if (name === '..') {
        if (stack.length > 1) stack.pop()
        node = stack[stack.length - 1]
        continue
      }
      if (node.kind !== FILETYPE.DIRECTORY) throw new Failure(ERRNO.NOTDIR)
      const child = node.children.get(name)
      if (!child) throw new Failure(ERRNO.NOENT)
      node = child
      stack.push(node)
    }
    return node
  }

  // The parent directory of a path, and the last name.
  parent(path, from = this.root) {
    const names = path.split('/').filter((n) => n !== '' && n !== '.')
    const last = names.pop()
    if (last === undefined || last === '..') throw new Failure(ERRNO.INVAL)
    const dir = this.find(names.join('/'), from)
    if (dir.kind !== FILETYPE.DIRECTORY) throw new Failure(ERRNO.NOTDIR)
    return [dir, last]
  }

  mkdir(path) {
    let node = this.root
    for (const name of path.split('/').filter(Boolean)) {
      let child = node.children.get(name)
      if (!child) node.children.set(name, (child = directory()))
      node = child
    }
    return node
  }

  // Write a file (text or bytes). The directories are made.
  write(path, content) {
    const names = path.split('/').filter(Boolean)
    const name = names.pop()
    const dir = this.mkdir(names.join('/'))
    dir.children.set(name, file(typeof content === 'string' ? encoder.encode(content) : content))
  }

  read(path) {
    const node = this.find(path)
    if (node.kind !== FILETYPE.REGULAR_FILE) throw new Error(`${path} is not a file`)
    return node.data
  }

  readText(path) {
    return decoder.decode(this.read(path))
  }

  exists(path) {
    try {
      this.find(path)
      return true
    } catch {
      return false
    }
  }

  remove(path) {
    const [dir, name] = this.parent(path)
    dir.children.delete(name)
  }
}

// Run the module once. options: args, env (an object), files (default: a
// new Files), stdin (text or bytes), and preopens: the directories that
// the program sees, as { "path in the program": "path in files" }
// (default { "/": "/" }). A second directory, such as { "/packages":
// "/packages" }, is not in the root of the program, so the scanner of
// the project does not see it.
export async function run(module, options = {}) {
  const files = options.files ?? new Files()
  const args = ['tailwind', ...(options.args ?? [])]
  const env = { PWD: '/', ...(options.env ?? {}) }
  const stdin = typeof options.stdin === 'string' ? encoder.encode(options.stdin) : (options.stdin ?? new Uint8Array())
  const stdout = []
  const stderr = []

  // The open files. 0, 1, 2: the standard streams. 3 and more: the
  // directories of `preopens`.
  const fds = new Map([
    [0, { stream: 'stdin', position: 0 }],
    [1, { stream: 'stdout' }],
    [2, { stream: 'stderr' }],
  ])
  let nextFd = 3
  for (const [guest, host] of Object.entries(options.preopens ?? { '/': '/' })) {
    const node = files.exists(host) ? files.find(host) : null
    if (node?.kind !== FILETYPE.DIRECTORY) throw new Error(`${host} is not a directory`)
    fds.set(nextFd++, { node, preopen: guest })
  }

  let memory
  const view = () => new DataView(memory.buffer)
  const bytes = () => new Uint8Array(memory.buffer)
  const text = (pointer, length) => decoder.decode(bytes().subarray(pointer, pointer + length))

  function entry(fd) {
    const e = fds.get(fd)
    if (!e) throw new Failure(ERRNO.BADF)
    return e
  }

  function directoryOf(fd) {
    const e = entry(fd)
    if (!e.node || e.node.kind !== FILETYPE.DIRECTORY) throw new Failure(ERRNO.NOTDIR)
    return e.node
  }

  function writeFilestat(pointer, node) {
    const v = view()
    v.setBigUint64(pointer, 0n, true)
    v.setBigUint64(pointer + 8, node?.inode ?? 0n, true)
    v.setUint8(pointer + 16, node ? node.kind : FILETYPE.CHARACTER_DEVICE)
    v.setBigUint64(pointer + 24, 1n, true)
    v.setBigUint64(pointer + 32, BigInt(node?.kind === FILETYPE.REGULAR_FILE ? node.data.length : 0), true)
    for (const offset of [40, 48, 56]) v.setBigUint64(pointer + offset, node?.time ?? 0n, true)
  }

  // The data of a list of iovecs or ciovecs.
  function vectors(pointer, count) {
    const v = view()
    const list = []
    for (let i = 0; i < count; i++) list.push([v.getUint32(pointer + i * 8, true), v.getUint32(pointer + i * 8 + 4, true)])
    return list
  }

  // The functions of the module "wasi_snapshot_preview1". Each one gives
  // an errno; a Failure gives its errno.
  const functions = {
    args_sizes_get(countPointer, sizePointer) {
      view().setUint32(countPointer, args.length, true)
      view().setUint32(sizePointer, args.reduce((n, a) => n + encoder.encode(a).length + 1, 0), true)
    },
    args_get(listPointer, bufferPointer) {
      putStrings(args, listPointer, bufferPointer)
    },
    environ_sizes_get(countPointer, sizePointer) {
      const list = Object.entries(env).map(([k, v]) => `${k}=${v}`)
      view().setUint32(countPointer, list.length, true)
      view().setUint32(sizePointer, list.reduce((n, a) => n + encoder.encode(a).length + 1, 0), true)
    },
    environ_get(listPointer, bufferPointer) {
      putStrings(Object.entries(env).map(([k, v]) => `${k}=${v}`), listPointer, bufferPointer)
    },
    random_get(pointer, length) {
      const target = bytes().subarray(pointer, pointer + length)
      for (let i = 0; i < length; i += 65536) crypto.getRandomValues(target.subarray(i, Math.min(length, i + 65536)))
    },
    clock_time_get(id, _precision, pointer) {
      const time = id === 0 ? now() : BigInt(Math.round(performance.now() * 1e6))
      view().setBigUint64(pointer, time, true)
    },
    sched_yield() {},
    poll_oneoff(inPointer, outPointer, count, eventsPointer) {
      const v = view()
      for (let i = 0; i < count; i++) {
        const sub = inPointer + i * 48
        const event = outPointer + i * 32
        v.setBigUint64(event, v.getBigUint64(sub, true), true)
        v.setUint16(event + 8, 0, true)
        v.setUint8(event + 10, v.getUint8(sub + 8))
      }
      v.setUint32(eventsPointer, count, true)
    },
    proc_exit(code) {
      throw new Exit(code)
    },

    fd_prestat_get(fd, pointer) {
      const e = entry(fd)
      if (!e.preopen) throw new Failure(ERRNO.BADF)
      view().setUint8(pointer, 0)
      view().setUint32(pointer + 4, encoder.encode(e.preopen).length, true)
    },
    fd_prestat_dir_name(fd, pointer, length) {
      const e = entry(fd)
      if (!e.preopen) throw new Failure(ERRNO.BADF)
      bytes().set(encoder.encode(e.preopen).subarray(0, length), pointer)
    },
    fd_fdstat_get(fd, pointer) {
      const e = entry(fd)
      const v = view()
      v.setUint8(pointer, e.node ? e.node.kind : FILETYPE.CHARACTER_DEVICE)
      v.setUint16(pointer + 2, e.append ? FDFLAGS.APPEND : 0, true)
      // With all the rights a standard stream is not a terminal for
      // wasi-libc: the program writes no colors.
      v.setBigUint64(pointer + 8, ALL_RIGHTS, true)
      v.setBigUint64(pointer + 16, ALL_RIGHTS, true)
    },
    fd_filestat_get(fd, pointer) {
      writeFilestat(pointer, entry(fd).node)
    },
    fd_close(fd) {
      entry(fd)
      fds.delete(fd)
    },
    fd_seek(fd, offset, whence, pointer) {
      const e = entry(fd)
      if (!e.node || e.node.kind !== FILETYPE.REGULAR_FILE) throw new Failure(ERRNO.SPIPE)
      const base = whence === 0 ? 0 : whence === 1 ? e.position : e.node.data.length
      const position = base + Number(offset)
      if (position < 0) throw new Failure(ERRNO.INVAL)
      e.position = position
      view().setBigUint64(pointer, BigInt(position), true)
    },
    fd_read(fd, iovs, count, pointer) {
      const e = entry(fd)
      let source
      if (e.stream === 'stdin') source = stdin
      else if (e.node?.kind === FILETYPE.REGULAR_FILE) source = e.node.data
      else throw new Failure(e.node ? ERRNO.ISDIR : ERRNO.BADF)
      let total = 0
      for (const [buffer, length] of vectors(iovs, count)) {
        const part = source.subarray(e.position, e.position + length)
        bytes().set(part, buffer)
        e.position += part.length
        total += part.length
        if (part.length < length) break
      }
      view().setUint32(pointer, total, true)
    },
    fd_write(fd, iovs, count, pointer) {
      const e = entry(fd)
      const parts = vectors(iovs, count).map(([buffer, length]) => bytes().slice(buffer, buffer + length))
      const total = parts.reduce((n, p) => n + p.length, 0)
      if (e.stream === 'stdout') stdout.push(...parts)
      else if (e.stream === 'stderr') stderr.push(...parts)
      else if (e.node?.kind === FILETYPE.REGULAR_FILE) {
        const node = e.node
        if (e.append) e.position = node.data.length
        const end = e.position + total
        if (end > node.data.length) {
          const larger = new Uint8Array(Math.max(end, node.data.length * 2))
          larger.set(node.data)
          node.data = larger.subarray(0, end)
        }
        for (const part of parts) {
          node.data.set(part, e.position)
          e.position += part.length
        }
        node.time = now()
      } else throw new Failure(ERRNO.BADF)
      view().setUint32(pointer, total, true)
    },
    fd_readdir(fd, buffer, length, cookie, pointer) {
      const node = directoryOf(fd)
      const names = ['.', '..', ...node.children.keys()]
      const v = view()
      let used = 0
      for (let i = Number(cookie); i < names.length; i++) {
        const name = encoder.encode(names[i])
        const child = i < 2 ? node : node.children.get(names[i])
        const header = new DataView(new ArrayBuffer(24))
        header.setBigUint64(0, BigInt(i + 1), true)
        header.setBigUint64(8, child.inode, true)
        header.setUint32(16, name.length, true)
        header.setUint8(20, child.kind)
        const record = new Uint8Array(24 + name.length)
        record.set(new Uint8Array(header.buffer))
        record.set(name, 24)
        // A record that does not fit is cut: the caller then asks again
        // with a larger buffer.
        const part = record.subarray(0, Math.min(record.length, length - used))
        bytes().set(part, buffer + used)
        used += part.length
        if (used === length) break
      }
      v.setUint32(pointer, used, true)
    },
    path_filestat_get(fd, _flags, pathPointer, pathLength, pointer) {
      writeFilestat(pointer, files.find(text(pathPointer, pathLength), directoryOf(fd)))
    },
    path_create_directory(fd, pathPointer, pathLength) {
      const [dir, name] = files.parent(text(pathPointer, pathLength), directoryOf(fd))
      if (dir.children.has(name)) throw new Failure(ERRNO.EXIST)
      dir.children.set(name, directory())
    },
    path_readlink(fd, pathPointer, pathLength) {
      // No symbolic links: a path that is there is not one.
      files.find(text(pathPointer, pathLength), directoryOf(fd))
      throw new Failure(ERRNO.INVAL)
    },
    path_open(fd, _dirflags, pathPointer, pathLength, oflags, _base, _inheriting, fdflags, pointer) {
      const path = text(pathPointer, pathLength)
      const from = directoryOf(fd)
      let node
      try {
        node = files.find(path, from)
        if (oflags & OFLAGS.EXCL && oflags & OFLAGS.CREAT) throw new Failure(ERRNO.EXIST)
      } catch (error) {
        if (!(error instanceof Failure) || error.errno !== ERRNO.NOENT || !(oflags & OFLAGS.CREAT)) throw error
        const [dir, name] = files.parent(path, from)
        node = file(new Uint8Array())
        dir.children.set(name, node)
      }
      if (oflags & OFLAGS.DIRECTORY && node.kind !== FILETYPE.DIRECTORY) throw new Failure(ERRNO.NOTDIR)
      if (oflags & OFLAGS.TRUNC) {
        if (node.kind === FILETYPE.DIRECTORY) throw new Failure(ERRNO.ISDIR)
        node.data = new Uint8Array()
        node.time = now()
      }
      const opened = nextFd++
      fds.set(opened, { node, position: 0, append: Boolean(fdflags & FDFLAGS.APPEND) })
      view().setUint32(pointer, opened, true)
    },
  }

  function putStrings(list, listPointer, bufferPointer) {
    const v = view()
    let at = bufferPointer
    list.forEach((item, i) => {
      const data = encoder.encode(item)
      v.setUint32(listPointer + i * 4, at, true)
      bytes().set(data, at)
      bytes()[at + data.length] = 0
      at += data.length + 1
    })
  }

  const imports = {}
  for (const [name, fn] of Object.entries(functions)) {
    imports[name] = (...values) => {
      try {
        fn(...values)
        return ERRNO.SUCCESS
      } catch (error) {
        if (error instanceof Failure) return error.errno
        throw error
      }
    }
  }
  for (const { module: from, name } of WebAssembly.Module.imports(module)) {
    if (from === 'wasi_snapshot_preview1' && !imports[name]) imports[name] = () => ERRNO.NOSYS
  }

  const instance = await WebAssembly.instantiate(module, { wasi_snapshot_preview1: imports })
  memory = instance.exports.memory
  let status = 0
  try {
    instance.exports._start()
  } catch (error) {
    if (!(error instanceof Exit)) throw error
    status = error.code
  }
  const join = (parts) => {
    const all = new Uint8Array(parts.reduce((n, p) => n + p.length, 0))
    let at = 0
    for (const p of parts) {
      all.set(p, at)
      at += p.length
    }
    return decoder.decode(all)
  }
  return { status, stdout: join(stdout), stderr: join(stderr) }
}
