//! `Array.prototype.sort` with the algorithm of V8.
//!
//! The result of a sort is the same for each algorithm when the compare
//! function is consistent. Tailwind CSS sorts the variants with a compare
//! function that is not: for two variants with no breakpoint value (a
//! `screens` entry with `min` and `max`, for example), `compare(a, z)` and
//! `compare(z, a)` are both 1. Then the result depends on which pairs the
//! algorithm compares, and in which sequence. The records of this
//! repository come from V8, so this file has the algorithm of V8
//! (`third_party/v8/builtins/array-sort.tq`: TimSort, after the list sort
//! of CPython): the same calls of the compare function, in the same
//! sequence.
//!
//! `native/tailwind/js/sort.ts` is the same algorithm for the JavaScript
//! engine of the program. `tests/v8_sort.rs` replays sorts that Node.js
//! recorded (`tests/upstream/v8-sort.jsonl`).

const MIN_GALLOP: usize = 7;

struct Sorter<'a, T, F: FnMut(&T, &T) -> i64> {
    work: &'a mut Vec<T>,
    compare: F,
    min_gallop: usize,
    run_base: Vec<usize>,
    run_length: Vec<usize>,
}

/// The place in `array[base .. base + length]` before which `key` goes, at
/// the left of equal elements.
fn gallop_left<T, F: FnMut(&T, &T) -> i64>(compare: &mut F, array: &[T], key: &T, base: usize, length: usize, hint: usize) -> usize {
    let mut last_ofs: isize = 0;
    let mut offset: isize = 1;
    let hint = hint as isize;
    let at = |index: isize| &array[(base as isize + index) as usize];
    if compare(at(hint), key) < 0 {
        let max_ofs = length as isize - hint;
        while offset < max_ofs {
            if compare(at(hint + offset), key) >= 0 {
                break;
            }
            last_ofs = offset;
            offset = (offset << 1) + 1;
            if offset <= 0 {
                offset = max_ofs;
            }
        }
        if offset > max_ofs {
            offset = max_ofs;
        }
        last_ofs += hint;
        offset += hint;
    } else {
        let max_ofs = hint + 1;
        while offset < max_ofs {
            if compare(at(hint - offset), key) < 0 {
                break;
            }
            last_ofs = offset;
            offset = (offset << 1) + 1;
            if offset <= 0 {
                offset = max_ofs;
            }
        }
        if offset > max_ofs {
            offset = max_ofs;
        }
        let tmp = last_ofs;
        last_ofs = hint - offset;
        offset = hint - tmp;
    }
    last_ofs += 1;
    while last_ofs < offset {
        let m = last_ofs + ((offset - last_ofs) >> 1);
        if compare(at(m), key) < 0 {
            last_ofs = m + 1;
        } else {
            offset = m;
        }
    }
    offset as usize
}

/// The same, at the right of equal elements.
fn gallop_right<T, F: FnMut(&T, &T) -> i64>(compare: &mut F, array: &[T], key: &T, base: usize, length: usize, hint: usize) -> usize {
    let mut last_ofs: isize = 0;
    let mut offset: isize = 1;
    let hint = hint as isize;
    let at = |index: isize| &array[(base as isize + index) as usize];
    if compare(key, at(hint)) < 0 {
        let max_ofs = hint + 1;
        while offset < max_ofs {
            if compare(key, at(hint - offset)) >= 0 {
                break;
            }
            last_ofs = offset;
            offset = (offset << 1) + 1;
            if offset <= 0 {
                offset = max_ofs;
            }
        }
        if offset > max_ofs {
            offset = max_ofs;
        }
        let tmp = last_ofs;
        last_ofs = hint - offset;
        offset = hint - tmp;
    } else {
        let max_ofs = length as isize - hint;
        while offset < max_ofs {
            if compare(key, at(hint + offset)) < 0 {
                break;
            }
            last_ofs = offset;
            offset = (offset << 1) + 1;
            if offset <= 0 {
                offset = max_ofs;
            }
        }
        if offset > max_ofs {
            offset = max_ofs;
        }
        last_ofs += hint;
        offset += hint;
    }
    last_ofs += 1;
    while last_ofs < offset {
        let m = last_ofs + ((offset - last_ofs) >> 1);
        if compare(key, at(m)) < 0 {
            offset = m;
        } else {
            last_ofs = m + 1;
        }
    }
    offset as usize
}

/// How a merge ends.
enum End {
    /// One run has no more elements: copy the rest of the temporary array.
    Rest,
    /// The other run has one element: it goes to the end (or the front).
    Last,
}

impl<T: Clone, F: FnMut(&T, &T) -> i64> Sorter<'_, T, F> {
    /// The length of the run that starts at `low_arg`. A descending run is
    /// reversed in place.
    fn count_and_make_run(&mut self, low_arg: usize, high: usize) -> usize {
        let low = low_arg + 1;
        if low == high {
            return 1;
        }
        let mut length = 2;
        let is_descending = (self.compare)(&self.work[low], &self.work[low - 1]) < 0;
        let mut previous = low;
        for index in low + 1..high {
            let order = (self.compare)(&self.work[index], &self.work[previous]);
            if is_descending {
                if order >= 0 {
                    break;
                }
            } else if order < 0 {
                break;
            }
            previous = index;
            length += 1;
        }
        if is_descending {
            self.work[low_arg..low_arg + length].reverse();
        }
        length
    }

    fn binary_insertion_sort(&mut self, low: usize, start_arg: usize, high: usize) {
        let start = if low == start_arg { start_arg + 1 } else { start_arg };
        for start in start..high {
            let mut left = low;
            let mut right = start;
            let pivot = self.work[right].clone();
            while left < right {
                let mid = left + ((right - left) >> 1);
                if (self.compare)(&pivot, &self.work[mid]) < 0 {
                    right = mid;
                } else {
                    left = mid + 1;
                }
            }
            // The elements from `left` move one place to the right.
            self.work[left..=start].rotate_right(1);
        }
    }

    /// Merge two runs that are next to each other, with the left run the
    /// shorter one: the left run goes to a temporary array.
    fn merge_low(&mut self, base_a: usize, length_a_arg: usize, base_b: usize, length_b_arg: usize) {
        let mut length_a = length_a_arg;
        let mut length_b = length_b_arg;
        let temp: Vec<T> = self.work[base_a..base_a + length_a].to_vec();
        let mut dest = base_a;
        let mut cursor_temp = 0;
        let mut cursor_b = base_b;

        self.work[dest] = self.work[cursor_b].clone();
        dest += 1;
        cursor_b += 1;

        let end = 'merge: {
            length_b -= 1;
            if length_b == 0 {
                break 'merge End::Rest;
            }
            if length_a == 1 {
                break 'merge End::Last;
            }

            let mut gallop = self.min_gallop;
            loop {
                let mut wins_a = 0;
                let mut wins_b = 0;
                loop {
                    if (self.compare)(&self.work[cursor_b], &temp[cursor_temp]) < 0 {
                        self.work[dest] = self.work[cursor_b].clone();
                        dest += 1;
                        cursor_b += 1;
                        wins_b += 1;
                        length_b -= 1;
                        wins_a = 0;
                        if length_b == 0 {
                            break 'merge End::Rest;
                        }
                        if wins_b >= gallop {
                            break;
                        }
                    } else {
                        self.work[dest] = temp[cursor_temp].clone();
                        dest += 1;
                        cursor_temp += 1;
                        wins_a += 1;
                        length_a -= 1;
                        wins_b = 0;
                        if length_a == 1 {
                            break 'merge End::Last;
                        }
                        if wins_a >= gallop {
                            break;
                        }
                    }
                }

                gallop += 1;
                let mut first = true;
                while wins_a >= MIN_GALLOP || wins_b >= MIN_GALLOP || first {
                    first = false;
                    gallop = (gallop - 1).max(1);
                    self.min_gallop = gallop;

                    let key = self.work[cursor_b].clone();
                    wins_a = gallop_right(&mut self.compare, &temp, &key, cursor_temp, length_a, 0);
                    if wins_a > 0 {
                        self.work[dest..dest + wins_a].clone_from_slice(&temp[cursor_temp..cursor_temp + wins_a]);
                        dest += wins_a;
                        cursor_temp += wins_a;
                        length_a -= wins_a;
                        if length_a == 1 {
                            break 'merge End::Last;
                        }
                        // 0 is possible only with a compare function
                        // that is not consistent.
                        if length_a == 0 {
                            break 'merge End::Rest;
                        }
                    }
                    self.work[dest] = self.work[cursor_b].clone();
                    dest += 1;
                    cursor_b += 1;
                    length_b -= 1;
                    if length_b == 0 {
                        break 'merge End::Rest;
                    }

                    wins_b = gallop_left(&mut self.compare, self.work, &temp[cursor_temp], cursor_b, length_b, 0);
                    if wins_b > 0 {
                        self.move_within(cursor_b, dest, wins_b);
                        dest += wins_b;
                        cursor_b += wins_b;
                        length_b -= wins_b;
                        if length_b == 0 {
                            break 'merge End::Rest;
                        }
                    }
                    self.work[dest] = temp[cursor_temp].clone();
                    dest += 1;
                    cursor_temp += 1;
                    length_a -= 1;
                    if length_a == 1 {
                        break 'merge End::Last;
                    }
                }
                gallop += 1;
                self.min_gallop = gallop;
            }
        };
        match end {
            End::Rest => {
                if length_a > 0 {
                    self.work[dest..dest + length_a].clone_from_slice(&temp[cursor_temp..cursor_temp + length_a]);
                }
            }
            // The last element of the left run goes to the end.
            End::Last => {
                self.move_within(cursor_b, dest, length_b);
                self.work[dest + length_b] = temp[cursor_temp].clone();
            }
        }
    }

    /// The same with the right run the shorter one: it goes to a temporary
    /// array, and the merge goes from the end. The cursors are one more
    /// than in the JavaScript form, so that they are not negative.
    fn merge_high(&mut self, base_a: usize, length_a_arg: usize, base_b: usize, length_b_arg: usize) {
        let mut length_a = length_a_arg;
        let mut length_b = length_b_arg;
        let temp: Vec<T> = self.work[base_b..base_b + length_b].to_vec();
        // `dest`, `cursor_temp` and `cursor_a` are the index after the
        // element that the JavaScript form points at.
        let mut dest = base_b + length_b;
        let mut cursor_temp = length_b;
        let mut cursor_a = base_a + length_a;

        self.work[dest - 1] = self.work[cursor_a - 1].clone();
        dest -= 1;
        cursor_a -= 1;

        let end = 'merge: {
            length_a -= 1;
            if length_a == 0 {
                break 'merge End::Rest;
            }
            if length_b == 1 {
                break 'merge End::Last;
            }

            let mut gallop = self.min_gallop;
            loop {
                let mut wins_a = 0;
                let mut wins_b = 0;
                loop {
                    if (self.compare)(&temp[cursor_temp - 1], &self.work[cursor_a - 1]) < 0 {
                        self.work[dest - 1] = self.work[cursor_a - 1].clone();
                        dest -= 1;
                        cursor_a -= 1;
                        wins_a += 1;
                        length_a -= 1;
                        wins_b = 0;
                        if length_a == 0 {
                            break 'merge End::Rest;
                        }
                        if wins_a >= gallop {
                            break;
                        }
                    } else {
                        self.work[dest - 1] = temp[cursor_temp - 1].clone();
                        dest -= 1;
                        cursor_temp -= 1;
                        wins_b += 1;
                        length_b -= 1;
                        wins_a = 0;
                        if length_b == 1 {
                            break 'merge End::Last;
                        }
                        if wins_b >= gallop {
                            break;
                        }
                    }
                }

                gallop += 1;
                let mut first = true;
                while wins_a >= MIN_GALLOP || wins_b >= MIN_GALLOP || first {
                    first = false;
                    gallop = (gallop - 1).max(1);
                    self.min_gallop = gallop;

                    let k = gallop_right(&mut self.compare, self.work, &temp[cursor_temp - 1], base_a, length_a, length_a - 1);
                    wins_a = length_a - k;
                    if wins_a > 0 {
                        dest -= wins_a;
                        cursor_a -= wins_a;
                        self.move_within(cursor_a, dest, wins_a);
                        length_a -= wins_a;
                        if length_a == 0 {
                            break 'merge End::Rest;
                        }
                    }
                    self.work[dest - 1] = temp[cursor_temp - 1].clone();
                    dest -= 1;
                    cursor_temp -= 1;
                    length_b -= 1;
                    if length_b == 1 {
                        break 'merge End::Last;
                    }

                    let key = self.work[cursor_a - 1].clone();
                    let k = gallop_left(&mut self.compare, &temp, &key, 0, length_b, length_b - 1);
                    wins_b = length_b - k;
                    if wins_b > 0 {
                        dest -= wins_b;
                        cursor_temp -= wins_b;
                        self.work[dest..dest + wins_b].clone_from_slice(&temp[cursor_temp..cursor_temp + wins_b]);
                        length_b -= wins_b;
                        if length_b == 1 {
                            break 'merge End::Last;
                        }
                        // 0 is possible only with a compare function
                        // that is not consistent.
                        if length_b == 0 {
                            break 'merge End::Rest;
                        }
                    }
                    self.work[dest - 1] = self.work[cursor_a - 1].clone();
                    dest -= 1;
                    cursor_a -= 1;
                    length_a -= 1;
                    if length_a == 0 {
                        break 'merge End::Rest;
                    }
                }
                gallop += 1;
                self.min_gallop = gallop;
            }
        };
        match end {
            End::Rest => {
                if length_b > 0 {
                    self.work[dest - length_b..dest].clone_from_slice(&temp[..length_b]);
                }
            }
            // The first element of the right run goes to the front.
            End::Last => {
                dest -= length_a;
                cursor_a -= length_a;
                self.move_within(cursor_a, dest, length_a);
                self.work[dest - 1] = temp[cursor_temp - 1].clone();
            }
        }
    }

    /// Copy `count` elements in the work array. The ranges can overlap.
    fn move_within(&mut self, from: usize, to: usize, count: usize) {
        if from == to || count == 0 {
            return;
        }
        if from < to {
            for i in (0..count).rev() {
                self.work[to + i] = self.work[from + i].clone();
            }
        } else {
            for i in 0..count {
                self.work[to + i] = self.work[from + i].clone();
            }
        }
    }

    /// Merge the runs `i` and `i + 1` of the stack.
    fn merge_at(&mut self, i: usize) {
        let mut base_a = self.run_base[i];
        let mut length_a = self.run_length[i];
        let base_b = self.run_base[i + 1];
        let mut length_b = self.run_length[i + 1];

        self.run_length[i] = length_a + length_b;
        if i + 3 == self.run_length.len() {
            self.run_base[i + 1] = self.run_base[i + 2];
            self.run_length[i + 1] = self.run_length[i + 2];
        }
        self.run_base.pop();
        self.run_length.pop();

        // The elements of the left run that are in place already.
        let key = self.work[base_b].clone();
        let k = gallop_right(&mut self.compare, self.work, &key, base_a, length_a, 0);
        base_a += k;
        length_a -= k;
        if length_a == 0 {
            return;
        }

        // The elements of the right run that are in place already.
        let key = self.work[base_a + length_a - 1].clone();
        length_b = gallop_left(&mut self.compare, self.work, &key, base_b, length_b, length_b - 1);
        if length_b == 0 {
            return;
        }

        if length_a <= length_b {
            self.merge_low(base_a, length_a, base_b, length_b);
        } else {
            self.merge_high(base_a, length_a, base_b, length_b);
        }
    }

    fn invariant_established(&self, n: usize) -> bool {
        n < 2 || self.run_length[n - 2] > self.run_length[n - 1] + self.run_length[n]
    }

    fn merge_collapse(&mut self) {
        while self.run_length.len() > 1 {
            let mut n = self.run_length.len() - 2;
            if !self.invariant_established(n + 1) || !self.invariant_established(n) {
                // With n = 0, JavaScript reads the length of run -1, which
                // is `undefined`, and the comparison is false.
                if n > 0 && self.run_length[n - 1] < self.run_length[n + 1] {
                    n -= 1;
                }
                self.merge_at(n);
            } else if self.run_length[n] <= self.run_length[n + 1] {
                self.merge_at(n);
            } else {
                break;
            }
        }
    }

    fn merge_force_collapse(&mut self) {
        while self.run_length.len() > 1 {
            let mut n = self.run_length.len() - 2;
            if n > 0 && self.run_length[n - 1] < self.run_length[n + 1] {
                n -= 1;
            }
            self.merge_at(n);
        }
    }
}

fn min_run_length(length: usize) -> usize {
    let mut n = length;
    let mut r = 0;
    while n >= 64 {
        r |= n & 1;
        n >>= 1;
    }
    n + r
}

/// Sort the values as `Array.prototype.sort(compare)` of V8 does. The
/// compare function gives a number as in JavaScript: negative when the
/// first value goes before the second one.
pub fn sort<T: Clone>(values: &mut Vec<T>, compare: impl FnMut(&T, &T) -> i64) {
    let length = values.len();
    if length < 2 {
        return;
    }
    let mut sorter = Sorter { work: values, compare, min_gallop: MIN_GALLOP, run_base: Vec::new(), run_length: Vec::new() };
    let mut low = 0;
    let mut remaining = length;
    let min_run = min_run_length(remaining);
    while remaining != 0 {
        let mut current = sorter.count_and_make_run(low, low + remaining);
        if current < min_run {
            let forced = min_run.min(remaining);
            sorter.binary_insertion_sort(low, low + current, low + forced);
            current = forced;
        }
        sorter.run_base.push(low);
        sorter.run_length.push(current);
        sorter.merge_collapse();
        low += current;
        remaining -= current;
    }
    sorter.merge_force_collapse();
}
