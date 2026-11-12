// The difference of two texts, line by line (the algorithm of Myers,
// "An O(ND) difference algorithm and its variations", 1986).
//
//   diff('a\nb', 'a\nc')
//   // [{ kind: 'same', text: 'a' }, { kind: 'remove', text: 'b' }, { kind: 'add', text: 'c' }]
//
// With more than `limit` different lines, the result is null: the texts
// are too different to show as a list of changes.
export function diff(before, after, limit = 2000) {
  const a = before.split('\n')
  const b = after.split('\n')
  const n = a.length
  const m = b.length
  const max = Math.min(n + m, limit)
  const offset = max + 1
  const v = new Int32Array(2 * max + 3)
  const trace = []
  for (let d = 0; d <= max; d++) {
    // Only the part of the state that the next step can read.
    trace.push({ start: -d - 1, values: v.slice(offset - d - 1, offset + d + 2) })
    for (let k = -d; k <= d; k += 2) {
      let x = k === -d || (k !== d && v[offset + k - 1] < v[offset + k + 1]) ? v[offset + k + 1] : v[offset + k - 1] + 1
      let y = x - k
      while (x < n && y < m && a[x] === b[y]) {
        x++
        y++
      }
      v[offset + k] = x
      if (x >= n && y >= m) return path(trace, a, b, d)
    }
  }
  return null
}

// Go back through the saved states from the end to the start.
function path(trace, a, b, depth) {
  const out = []
  let x = a.length
  let y = b.length
  for (let d = depth; d > 0; d--) {
    const { start, values } = trace[d]
    const at = (k) => values[k - start]
    const k = x - y
    const down = k === -d || (k !== d && at(k - 1) < at(k + 1))
    const previous = down ? k + 1 : k - 1
    const px = at(previous)
    const py = px - previous
    while (x > px && y > py) {
      y--
      out.push({ kind: 'same', text: a[--x] })
    }
    if (down) out.push({ kind: 'add', text: b[--y] })
    else out.push({ kind: 'remove', text: a[--x] })
  }
  while (x > 0 && y > 0) {
    y--
    out.push({ kind: 'same', text: a[--x] })
  }
  return out.reverse()
}
