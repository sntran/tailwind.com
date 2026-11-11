// `Array.prototype.sort` and `toSorted` with the algorithm of V8.
//
// The result of a sort is the same in each engine when the compare
// function is consistent. Tailwind CSS has a compare function that is
// not: for two variants with no breakpoint value, `compare(a, z)` and
// `compare(z, a)` are both 1. Then the result depends on which pairs the
// algorithm compares, and in which sequence. The official programs run in
// V8 and JavaScriptCore; the records of this repository come from V8. So
// this file has the algorithm of V8 (`third_party/v8/builtins/
// array-sort.tq`: TimSort, after the list sort of CPython): the same
// calls of the compare function, in the same sequence.
//
// `tests/sort.mjs` compares this file with the sort of Node.js, call by
// call, also for compare functions that are not consistent.

type Compare = (a: any, z: any) => number

const MIN_GALLOP = 7

// The compare function of a sort with no function: the text of the values.
function defaultCompare(a: any, z: any): number {
  let x = String(a)
  let y = String(z)
  return x < y ? -1 : x > y ? 1 : 0
}

// The number of a result of the compare function, as `ToNumber` gives it.
function order(compare: Compare, a: any, z: any): number {
  let value = +compare(a, z)
  // NaN is 0, as the standard says.
  return value !== value ? 0 : value
}

function sortValues(work: any[], compare: Compare) {
  let length = work.length
  if (length < 2) return

  let minGallop = MIN_GALLOP
  let runBase: number[] = []
  let runLength: number[] = []

  function reverseRange(from: number, to: number) {
    let low = from
    let high = to - 1
    while (low < high) {
      let element = work[low]
      work[low++] = work[high]
      work[high--] = element
    }
  }

  // The length of the run that starts at `lowArg`. A descending run is
  // reversed in place.
  function countAndMakeRun(lowArg: number, high: number): number {
    let low = lowArg + 1
    if (low === high) return 1
    let length = 2
    let elementLow = work[low]
    let elementHigh = work[low - 1]
    let isDescending = order(compare, elementLow, elementHigh) < 0
    let previous = elementLow
    for (let index = low + 1; index < high; ++index) {
      let current = work[index]
      let result = order(compare, current, previous)
      if (isDescending) {
        if (result >= 0) break
      } else {
        if (result < 0) break
      }
      previous = current
      ++length
    }
    if (isDescending) reverseRange(lowArg, lowArg + length)
    return length
  }

  function binaryInsertionSort(low: number, startArg: number, high: number) {
    let start = low === startArg ? startArg + 1 : startArg
    for (; start < high; ++start) {
      let left = low
      let right = start
      let pivot = work[right]
      while (left < right) {
        let mid = left + ((right - left) >> 1)
        if (order(compare, pivot, work[mid]) < 0) {
          right = mid
        } else {
          left = mid + 1
        }
      }
      for (let p = start; p > left; --p) work[p] = work[p - 1]
      work[left] = pivot
    }
  }

  // The position in `array[base .. base + length]` before which `key`
  // goes, at the left of equal elements.
  function gallopLeft(array: any[], key: any, base: number, length: number, hint: number): number {
    let lastOfs = 0
    let offset = 1
    if (order(compare, array[base + hint], key) < 0) {
      let maxOfs = length - hint
      while (offset < maxOfs) {
        if (order(compare, array[base + hint + offset], key) >= 0) break
        lastOfs = offset
        offset = (offset << 1) + 1
        if (offset <= 0) offset = maxOfs
      }
      if (offset > maxOfs) offset = maxOfs
      lastOfs += hint
      offset += hint
    } else {
      let maxOfs = hint + 1
      while (offset < maxOfs) {
        if (order(compare, array[base + hint - offset], key) < 0) break
        lastOfs = offset
        offset = (offset << 1) + 1
        if (offset <= 0) offset = maxOfs
      }
      if (offset > maxOfs) offset = maxOfs
      let tmp = lastOfs
      lastOfs = hint - offset
      offset = hint - tmp
    }
    lastOfs++
    while (lastOfs < offset) {
      let m = lastOfs + ((offset - lastOfs) >> 1)
      if (order(compare, array[base + m], key) < 0) {
        lastOfs = m + 1
      } else {
        offset = m
      }
    }
    return offset
  }

  // The same, at the right of equal elements.
  function gallopRight(array: any[], key: any, base: number, length: number, hint: number): number {
    let lastOfs = 0
    let offset = 1
    if (order(compare, key, array[base + hint]) < 0) {
      let maxOfs = hint + 1
      while (offset < maxOfs) {
        if (order(compare, key, array[base + hint - offset]) >= 0) break
        lastOfs = offset
        offset = (offset << 1) + 1
        if (offset <= 0) offset = maxOfs
      }
      if (offset > maxOfs) offset = maxOfs
      let tmp = lastOfs
      lastOfs = hint - offset
      offset = hint - tmp
    } else {
      let maxOfs = length - hint
      while (offset < maxOfs) {
        if (order(compare, key, array[base + hint + offset]) < 0) break
        lastOfs = offset
        offset = (offset << 1) + 1
        if (offset <= 0) offset = maxOfs
      }
      if (offset > maxOfs) offset = maxOfs
      lastOfs += hint
      offset += hint
    }
    lastOfs++
    while (lastOfs < offset) {
      let m = lastOfs + ((offset - lastOfs) >> 1)
      if (order(compare, key, array[base + m]) < 0) {
        offset = m
      } else {
        lastOfs = m + 1
      }
    }
    return offset
  }

  // Copy `count` elements. The ranges can be parts of one array.
  function copy(source: any[], from: number, target: any[], to: number, count: number) {
    if (from < to) {
      for (let i = count - 1; i >= 0; --i) target[to + i] = source[from + i]
    } else {
      for (let i = 0; i < count; ++i) target[to + i] = source[from + i]
    }
  }

  // Merge two runs that are next to each other, with the left run the
  // shorter one: the left run goes to a temporary array.
  function mergeLow(baseA: number, lengthAArg: number, baseB: number, lengthBArg: number) {
    let lengthA = lengthAArg
    let lengthB = lengthBArg
    let temp = work.slice(baseA, baseA + lengthA)
    let dest = baseA
    let cursorTemp = 0
    let cursorB = baseB

    work[dest++] = work[cursorB++]

    merge: {
      copyB: {
        if (--lengthB === 0) break merge
        if (lengthA === 1) break copyB

        let gallop = minGallop
        while (true) {
          let winsA = 0
          let winsB = 0
          while (true) {
            if (order(compare, work[cursorB], temp[cursorTemp]) < 0) {
              work[dest++] = work[cursorB++]
              ++winsB
              --lengthB
              winsA = 0
              if (lengthB === 0) break merge
              if (winsB >= gallop) break
            } else {
              work[dest++] = temp[cursorTemp++]
              ++winsA
              --lengthA
              winsB = 0
              if (lengthA === 1) break copyB
              if (winsA >= gallop) break
            }
          }

          ++gallop
          let first = true
          while (winsA >= MIN_GALLOP || winsB >= MIN_GALLOP || first) {
            first = false
            gallop = Math.max(1, gallop - 1)
            minGallop = gallop

            winsA = gallopRight(temp, work[cursorB], cursorTemp, lengthA, 0)
            if (winsA > 0) {
              copy(temp, cursorTemp, work, dest, winsA)
              dest += winsA
              cursorTemp += winsA
              lengthA -= winsA
              if (lengthA === 1) break copyB
              // 0 is possible only with a compare function that is not
              // consistent.
              if (lengthA === 0) break merge
            }
            work[dest++] = work[cursorB++]
            if (--lengthB === 0) break merge

            winsB = gallopLeft(work, temp[cursorTemp], cursorB, lengthB, 0)
            if (winsB > 0) {
              copy(work, cursorB, work, dest, winsB)
              dest += winsB
              cursorB += winsB
              lengthB -= winsB
              if (lengthB === 0) break merge
            }
            work[dest++] = temp[cursorTemp++]
            if (--lengthA === 1) break copyB
          }
          ++gallop
          minGallop = gallop
        }
      }
      // The last element of the left run goes to the end.
      copy(work, cursorB, work, dest, lengthB)
      work[dest + lengthB] = temp[cursorTemp]
      return
    }
    if (lengthA > 0) copy(temp, cursorTemp, work, dest, lengthA)
  }

  // The same with the right run the shorter one: it goes to a temporary
  // array, and the merge goes from the end.
  function mergeHigh(baseA: number, lengthAArg: number, baseB: number, lengthBArg: number) {
    let lengthA = lengthAArg
    let lengthB = lengthBArg
    let temp = work.slice(baseB, baseB + lengthB)
    let dest = baseB + lengthB - 1
    let cursorTemp = lengthB - 1
    let cursorA = baseA + lengthA - 1

    work[dest--] = work[cursorA--]

    merge: {
      copyA: {
        if (--lengthA === 0) break merge
        if (lengthB === 1) break copyA

        let gallop = minGallop
        while (true) {
          let winsA = 0
          let winsB = 0
          while (true) {
            if (order(compare, temp[cursorTemp], work[cursorA]) < 0) {
              work[dest--] = work[cursorA--]
              ++winsA
              --lengthA
              winsB = 0
              if (lengthA === 0) break merge
              if (winsA >= gallop) break
            } else {
              work[dest--] = temp[cursorTemp--]
              ++winsB
              --lengthB
              winsA = 0
              if (lengthB === 1) break copyA
              if (winsB >= gallop) break
            }
          }

          ++gallop
          let first = true
          while (winsA >= MIN_GALLOP || winsB >= MIN_GALLOP || first) {
            first = false
            gallop = Math.max(1, gallop - 1)
            minGallop = gallop

            let k = gallopRight(work, temp[cursorTemp], baseA, lengthA, lengthA - 1)
            winsA = lengthA - k
            if (winsA > 0) {
              dest -= winsA
              cursorA -= winsA
              copy(work, cursorA + 1, work, dest + 1, winsA)
              lengthA -= winsA
              if (lengthA === 0) break merge
            }
            work[dest--] = temp[cursorTemp--]
            if (--lengthB === 1) break copyA

            k = gallopLeft(temp, work[cursorA], 0, lengthB, lengthB - 1)
            winsB = lengthB - k
            if (winsB > 0) {
              dest -= winsB
              cursorTemp -= winsB
              copy(temp, cursorTemp + 1, work, dest + 1, winsB)
              lengthB -= winsB
              if (lengthB === 1) break copyA
              // 0 is possible only with a compare function that is not
              // consistent.
              if (lengthB === 0) break merge
            }
            work[dest--] = work[cursorA--]
            if (--lengthA === 0) break merge
          }
          ++gallop
          minGallop = gallop
        }
      }
      // The first element of the right run goes to the front.
      dest -= lengthA
      cursorA -= lengthA
      copy(work, cursorA + 1, work, dest + 1, lengthA)
      work[dest] = temp[cursorTemp]
      return
    }
    if (lengthB > 0) copy(temp, 0, work, dest - (lengthB - 1), lengthB)
  }

  // Merge the runs `i` and `i + 1` of the stack.
  function mergeAt(i: number) {
    let baseA = runBase[i]
    let lengthA = runLength[i]
    let baseB = runBase[i + 1]
    let lengthB = runLength[i + 1]

    runLength[i] = lengthA + lengthB
    if (i === runLength.length - 3) {
      runBase[i + 1] = runBase[i + 2]
      runLength[i + 1] = runLength[i + 2]
    }
    runBase.pop()
    runLength.pop()

    // The elements of the left run that are in place already.
    let k = gallopRight(work, work[baseB], baseA, lengthA, 0)
    baseA += k
    lengthA -= k
    if (lengthA === 0) return

    // The elements of the right run that are in place already.
    lengthB = gallopLeft(work, work[baseA + lengthA - 1], baseB, lengthB, lengthB - 1)
    if (lengthB === 0) return

    if (lengthA <= lengthB) {
      mergeLow(baseA, lengthA, baseB, lengthB)
    } else {
      mergeHigh(baseA, lengthA, baseB, lengthB)
    }
  }

  function invariantEstablished(n: number): boolean {
    if (n < 2) return true
    return runLength[n - 2] > runLength[n - 1] + runLength[n]
  }

  function mergeCollapse() {
    while (runLength.length > 1) {
      let n = runLength.length - 2
      if (!invariantEstablished(n + 1) || !invariantEstablished(n)) {
        if (runLength[n - 1] < runLength[n + 1]) --n
        mergeAt(n)
      } else if (runLength[n] <= runLength[n + 1]) {
        mergeAt(n)
      } else {
        break
      }
    }
  }

  function mergeForceCollapse() {
    while (runLength.length > 1) {
      let n = runLength.length - 2
      if (n > 0 && runLength[n - 1] < runLength[n + 1]) --n
      mergeAt(n)
    }
  }

  function minRunLength(nArg: number): number {
    let n = nArg
    let r = 0
    while (n >= 64) {
      r |= n & 1
      n >>= 1
    }
    return n + r
  }

  let low = 0
  let remaining = length
  let minRun = minRunLength(remaining)
  while (remaining !== 0) {
    let current = countAndMakeRun(low, low + remaining)
    if (current < minRun) {
      let forced = Math.min(minRun, remaining)
      binaryInsertionSort(low, low + current, low + forced)
      current = forced
    }
    runBase.push(low)
    runLength.push(current)
    mergeCollapse()
    low += current
    remaining -= current
  }
  mergeForceCollapse()
}

// The values of the array that are not `undefined`, sorted. The holes and
// the `undefined` values go to the end, and the compare function does not
// get them.
function sorted(array: any, compare: Compare | undefined): { values: any[]; undefineds: number } {
  let length = array.length >>> 0
  let values: any[] = []
  let undefineds = 0
  for (let i = 0; i < length; i++) {
    if (!(i in array)) continue
    let value = array[i]
    if (value === undefined) {
      undefineds++
    } else {
      values.push(value)
    }
  }
  sortValues(values, compare ?? defaultCompare)
  return { values, undefineds }
}

function check(compare: unknown) {
  if (compare !== undefined && typeof compare !== 'function') {
    throw new TypeError('The comparison function must be either a function or undefined')
  }
}

export function sort(this: any, compare?: Compare) {
  check(compare)
  let array = Object(this)
  let length = array.length >>> 0
  let { values, undefineds } = sorted(array, compare)
  let index = 0
  for (; index < values.length; index++) array[index] = values[index]
  for (let end = index + undefineds; index < end; index++) array[index] = undefined
  for (; index < length; index++) delete array[index]
  return array
}

export function toSorted(this: any, compare?: Compare) {
  check(compare)
  let array = Object(this)
  let length = array.length >>> 0
  // `toSorted` reads a hole as `undefined`.
  let { values, undefineds } = sorted(array, compare)
  let result = values
  let holes = length - values.length - undefineds
  for (let i = 0; i < undefineds + holes; i++) result.push(undefined)
  return result
}

Object.defineProperty(Array.prototype, 'sort', { value: sort, writable: true, configurable: true })
Object.defineProperty(Array.prototype, 'toSorted', { value: toSorted, writable: true, configurable: true })
