//! The patches of the packages of Tailwind CSS.
//!
//! The file tailwind.com has the npm package `tailwindcss` of each
//! version, for the script engine. The compilers of two versions next to
//! each other are almost the same text. So the file has the newest package
//! in full, and each older version as a patch: the difference from the
//! next newer version. A patch of a compiler is about 5 KB, and its full
//! text is about 60 KB with deflate.
//!
//! A patch file (`VERSION.patch`) has the name of its reference version
//! and one entry for each file of the package: the same file as in the
//! reference, a delta, or the full file. To read a file of a version, the
//! program reads the file of the reference version (which can be a patch
//! too) and applies the delta.
//!
//! A delta is a list of steps: some new bytes, then a copy of bytes of
//! the reference. The place of a copy is relative to the place where the
//! last copy ended: in a text with small changes, most copies continue
//! there, and the number is 0. The delta keeps the numbers and the new
//! bytes in four streams, because deflate (the ZIP file) then packs each
//! one better.
//!
//! This file uses only the standard library: the tool `tools/twpatch`
//! (which makes the patches for the build) includes it.

/// The first bytes of a patch file.
const MAGIC: &[u8; 8] = b"TWPATCH1";

/// The shortest copy from an indexed place of the reference.
const INDEX_LENGTH: usize = 24;
/// The shortest copy at the place where the last copy ended.
const NEAR_LENGTH: usize = 4;
/// How far from that place the encoder also looks.
const NEAR_SHIFT: isize = 4;

fn put_number(out: &mut Vec<u8>, mut value: u64) {
    loop {
        let byte = (value & 0x7f) as u8;
        value >>= 7;
        if value == 0 {
            out.push(byte);
            return;
        }
        out.push(byte | 0x80);
    }
}

/// A reader of the numbers and bytes of a stream.
struct Reader<'a> {
    data: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, at: 0 }
    }

    fn done(&self) -> bool {
        self.at >= self.data.len()
    }

    fn number(&mut self) -> Option<u64> {
        let mut value = 0u64;
        let mut shift = 0;
        loop {
            let byte = *self.data.get(self.at)?;
            self.at += 1;
            if shift >= 64 {
                return None;
            }
            value |= u64::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                return Some(value);
            }
            shift += 7;
        }
    }

    fn size(&mut self) -> Option<usize> {
        usize::try_from(self.number()?).ok()
    }

    fn bytes(&mut self, length: usize) -> Option<&'a [u8]> {
        let end = self.at.checked_add(length)?;
        let bytes = self.data.get(self.at..end)?;
        self.at = end;
        Some(bytes)
    }
}

/// A number with a sign as a number with no sign: 0, -1, 1, -2, 2, ...
fn zigzag(value: i64) -> u64 {
    if value >= 0 {
        (value as u64) << 1
    } else {
        ((value.unsigned_abs()) << 1) - 1
    }
}

fn unzigzag(value: u64) -> i64 {
    if value & 1 == 0 {
        (value >> 1) as i64
    } else {
        -(((value >> 1) + 1) as i64)
    }
}

/// The number of equal bytes at the start of two slices.
fn same_length(a: &[u8], b: &[u8]) -> usize {
    a.iter().zip(b).take_while(|(x, y)| x == y).count()
}

/// The delta that makes `target` from `reference`.
pub fn encode(reference: &[u8], target: &[u8]) -> Vec<u8> {
    // The places of each sequence of INDEX_LENGTH bytes of the reference,
    // the first places first.
    let mut index: std::collections::HashMap<&[u8], Vec<u32>> = std::collections::HashMap::new();
    if reference.len() >= INDEX_LENGTH {
        for at in 0..=reference.len() - INDEX_LENGTH {
            let places = index.entry(&reference[at..at + INDEX_LENGTH]).or_default();
            if places.len() < 4 {
                places.push(at as u32);
            }
        }
    }

    let (mut new_counts, mut places, mut lengths, mut new_bytes) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let mut pending = 0u64;
    let mut at = 0usize;
    // The place of the reference where the last copy ended, moved by the
    // new bytes after it.
    let mut expected = 0isize;
    while at < target.len() {
        let rest = &target[at..];
        let mut best = 0usize;
        let mut best_place = 0usize;
        // The expected place, and the places near it.
        for step in 0..=(2 * NEAR_SHIFT) {
            let shift = if step == 0 { 0 } else if step % 2 == 1 { (step + 1) / 2 } else { -(step / 2) };
            let place = expected + shift;
            if place < 0 || place as usize >= reference.len() {
                continue;
            }
            let length = same_length(&reference[place as usize..], rest);
            let (needed, better) = if shift == 0 { (NEAR_LENGTH, best) } else { (NEAR_LENGTH + 2, best + 1) };
            if length >= needed && length > better {
                best = length;
                best_place = place as usize;
            }
            if shift == 0 && length >= 32 {
                break;
            }
        }
        // A place from the index, when it is much longer.
        if best < INDEX_LENGTH && rest.len() >= INDEX_LENGTH {
            if let Some(found) = index.get(&rest[..INDEX_LENGTH]) {
                for &place in found {
                    let length = same_length(&reference[place as usize..], rest);
                    if length > best + 3 {
                        best = length;
                        best_place = place as usize;
                    }
                }
            }
        }
        if best == 0 {
            new_bytes.push(target[at]);
            pending += 1;
            at += 1;
            expected += 1;
            continue;
        }
        put_number(&mut new_counts, pending);
        put_number(&mut places, zigzag(best_place as i64 - expected as i64));
        put_number(&mut lengths, best as u64);
        pending = 0;
        at += best;
        expected = (best_place + best) as isize;
    }
    put_number(&mut new_counts, pending);

    let mut out = Vec::with_capacity(new_counts.len() + places.len() + lengths.len() + new_bytes.len() + 12);
    put_number(&mut out, new_counts.len() as u64);
    put_number(&mut out, places.len() as u64);
    put_number(&mut out, lengths.len() as u64);
    out.extend(new_counts);
    out.extend(places);
    out.extend(lengths);
    out.extend(new_bytes);
    out
}

/// Make the target from the reference and a delta. `None`: the delta is
/// not for this reference, or it is damaged.
pub fn apply(reference: &[u8], delta: &[u8]) -> Option<Vec<u8>> {
    let mut header = Reader::new(delta);
    let (counts_length, places_length, lengths_length) = (header.size()?, header.size()?, header.size()?);
    let mut new_counts = Reader::new(header.bytes(counts_length)?);
    let mut places = Reader::new(header.bytes(places_length)?);
    let mut lengths = Reader::new(header.bytes(lengths_length)?);
    let mut new_bytes = Reader::new(&delta[header.at..]);

    let mut out = Vec::with_capacity(reference.len());
    let mut expected = 0i64;
    loop {
        let count = new_counts.size()?;
        out.extend_from_slice(new_bytes.bytes(count)?);
        expected = expected.checked_add(i64::try_from(count).ok()?)?;
        if places.done() {
            break;
        }
        let place = expected.checked_add(unzigzag(places.number()?))?;
        let length = lengths.size()?;
        let start = usize::try_from(place).ok()?;
        let end = start.checked_add(length)?;
        out.extend_from_slice(reference.get(start..end)?);
        expected = i64::try_from(end).ok()?;
    }
    (new_counts.done() && lengths.done() && new_bytes.done()).then_some(out)
}

/// One file of a patch.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    /// The file of the reference version, with no change.
    Same,
    /// A delta for the file of the reference version.
    Delta(Vec<u8>),
    /// The full file: the reference version has no such file.
    Full(Vec<u8>),
}

/// The patch of one version: its files, as differences from a reference
/// version.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Patch {
    /// The version that the entries refer to.
    pub reference: String,
    /// The files of the version, with their names in the package.
    pub files: Vec<(String, Entry)>,
}

impl Patch {
    /// The patch that makes the files `target` from the files
    /// `reference`. Each list has the name and the bytes of a file.
    pub fn make(reference_version: &str, reference: &[(String, Vec<u8>)], target: &[(String, Vec<u8>)]) -> Self {
        let files = target
            .iter()
            .map(|(name, bytes)| {
                let entry = match reference.iter().find(|(other, _)| other == name) {
                    Some((_, old)) if old == bytes => Entry::Same,
                    Some((_, old)) => {
                        let delta = encode(old, bytes);
                        // A file with nothing in common is smaller in full.
                        if delta.len() < bytes.len() {
                            Entry::Delta(delta)
                        } else {
                            Entry::Full(bytes.clone())
                        }
                    }
                    None => Entry::Full(bytes.clone()),
                };
                (name.clone(), entry)
            })
            .collect();
        Self { reference: reference_version.to_string(), files }
    }

    /// The entry of a file.
    pub fn entry(&self, name: &str) -> Option<&Entry> {
        self.files.iter().find(|(other, _)| other == name).map(|(_, entry)| entry)
    }

    /// The bytes of the patch file.
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = MAGIC.to_vec();
        put_number(&mut out, self.reference.len() as u64);
        out.extend_from_slice(self.reference.as_bytes());
        put_number(&mut out, self.files.len() as u64);
        for (name, entry) in &self.files {
            put_number(&mut out, name.len() as u64);
            out.extend_from_slice(name.as_bytes());
            let (kind, length) = match entry {
                Entry::Same => (0u8, 0),
                Entry::Delta(data) => (1, data.len()),
                Entry::Full(data) => (2, data.len()),
            };
            out.push(kind);
            put_number(&mut out, length as u64);
        }
        // The data of all the files comes after the list: deflate then
        // sees the similar parts of the deltas together.
        for (_, entry) in &self.files {
            if let Entry::Delta(data) | Entry::Full(data) = entry {
                out.extend_from_slice(data);
            }
        }
        out
    }

    /// Read a patch file. `None`: the bytes are not a patch file.
    pub fn parse(bytes: &[u8]) -> Option<Self> {
        let mut reader = Reader::new(bytes);
        if reader.bytes(MAGIC.len())? != MAGIC {
            return None;
        }
        let text = |reader: &mut Reader| -> Option<String> {
            let length = reader.size()?;
            String::from_utf8(reader.bytes(length)?.to_vec()).ok()
        };
        let reference = text(&mut reader)?;
        let count = reader.size()?;
        let mut list = Vec::new();
        for _ in 0..count {
            let name = text(&mut reader)?;
            let kind = *reader.bytes(1)?.first()?;
            let length = reader.size()?;
            list.push((name, kind, length));
        }
        let mut files = Vec::with_capacity(list.len());
        for (name, kind, length) in list {
            let entry = match kind {
                0 if length == 0 => Entry::Same,
                1 => Entry::Delta(reader.bytes(length)?.to_vec()),
                2 => Entry::Full(reader.bytes(length)?.to_vec()),
                _ => return None,
            };
            files.push((name, entry));
        }
        reader.done().then_some(Self { reference, files })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A small generator of numbers, the same in each run.
    struct Random(u64);

    impl Random {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0
        }
        fn below(&mut self, limit: usize) -> usize {
            (self.next() % limit.max(1) as u64) as usize
        }
    }

    fn text(random: &mut Random, length: usize) -> Vec<u8> {
        // Words of a small alphabet, as minified code: many sequences are
        // in the text more than one time.
        let words: [&[u8]; 8] = [b"function(", b"return ", b"let a=", b"){", b"}", b"e.kind===\"rule\"", b";", b"x"];
        let mut out = Vec::new();
        while out.len() < length {
            out.extend_from_slice(words[random.below(words.len())]);
        }
        out
    }

    fn round_trip(reference: &[u8], target: &[u8]) -> usize {
        let delta = encode(reference, target);
        assert_eq!(apply(reference, &delta).as_deref(), Some(target));
        delta.len()
    }

    #[test]
    fn the_numbers_with_a_sign() {
        for value in [0i64, 1, -1, 2, -2, 63, -64, 1 << 40, -(1 << 40), i64::MAX, i64::MIN + 1] {
            assert_eq!(unzigzag(zigzag(value)), value);
        }
        assert_eq!([zigzag(0), zigzag(-1), zigzag(1), zigzag(-2)], [0, 1, 2, 3]);
    }

    #[test]
    fn empty_and_equal_files() {
        assert_eq!(round_trip(b"", b""), 4);
        round_trip(b"", b"new text");
        round_trip(b"old text", b"");
        let same = b"the same text, the same text, the same text".to_vec();
        // One copy of all the text: a few bytes.
        assert!(round_trip(&same, &same) <= 8);
    }

    #[test]
    fn a_small_change_gives_a_small_delta() {
        let mut random = Random(11);
        let reference = text(&mut random, 200_000);
        let mut target = reference.clone();
        // A name with another length, 100 times.
        for k in 0..100 {
            let at = 1000 + k * 1900;
            target.splice(at..at + 2, *b"new");
        }
        let length = round_trip(&reference, &target);
        assert!(length < 1500, "the delta has {length} bytes");
    }

    #[test]
    fn moved_and_removed_parts() {
        let mut random = Random(5);
        let a = text(&mut random, 5000);
        let b = text(&mut random, 5000);
        let c = text(&mut random, 5000);
        let reference = [a.clone(), b.clone(), c.clone()].concat();
        let target = [c, a, b"between".to_vec()].concat();
        round_trip(&reference, &target);
    }

    #[test]
    fn random_changes() {
        let mut random = Random(2026);
        for case in 0..300 {
            let length = random.below(3000);
            let reference: Vec<u8> = if case % 3 == 0 {
                (0..length).map(|_| random.next() as u8).collect()
            } else {
                text(&mut random, length)
            };
            let mut target = reference.clone();
            for _ in 0..random.below(12) {
                let at = random.below(target.len() + 1);
                match random.below(3) {
                    0 => {
                        let end = (at + random.below(40)).min(target.len());
                        target.drain(at..end);
                    }
                    1 => {
                        let new: Vec<u8> = (0..random.below(40)).map(|_| random.next() as u8).collect();
                        target.splice(at..at, new);
                    }
                    _ => {
                        let from = random.below(reference.len() + 1);
                        let end = (from + random.below(200)).min(reference.len());
                        target.splice(at..at, reference[from..end].to_vec());
                    }
                }
            }
            round_trip(&reference, &target);
        }
    }

    #[test]
    fn a_delta_for_another_reference_is_an_error_or_another_text() {
        let delta = encode(b"0123456789 0123456789 0123456789", b"0123456789 0123456789 0123456789!");
        // The reference is too short for the copy.
        assert_eq!(apply(b"short", &delta), None);
        // Damaged deltas do not panic.
        for cut in 0..delta.len() {
            let _ = apply(b"0123456789 0123456789 0123456789", &delta[..cut]);
        }
        assert_eq!(apply(b"x", &[0xff, 0xff, 0xff]), None);
    }

    #[test]
    fn a_patch_file() {
        let reference = vec![
            ("package.json".to_string(), b"{\"version\":\"2\"}".to_vec()),
            ("LICENSE".to_string(), b"MIT".to_vec()),
            ("dist/lib.js".to_string(), b"function compile(css){return css+css+css+css+css+css}".to_vec()),
        ];
        let target = vec![
            ("package.json".to_string(), b"{\"version\":\"1\"}".to_vec()),
            ("LICENSE".to_string(), b"MIT".to_vec()),
            ("dist/lib.js".to_string(), b"function compile(css){return css+css+css+css+css+css+css}".to_vec()),
            ("only-here.css".to_string(), b"a{}".to_vec()),
        ];
        let patch = Patch::make("2.0.0", &reference, &target);
        assert_eq!(patch.entry("LICENSE"), Some(&Entry::Same));
        assert!(matches!(patch.entry("dist/lib.js"), Some(Entry::Delta(_))));
        assert_eq!(patch.entry("only-here.css"), Some(&Entry::Full(b"a{}".to_vec())));
        assert_eq!(patch.entry("no-such-file"), None);

        let bytes = patch.to_bytes();
        assert_eq!(Patch::parse(&bytes), Some(patch.clone()));
        assert_eq!(Patch::parse(b"not a patch"), None);
        for cut in 0..bytes.len() {
            assert_eq!(Patch::parse(&bytes[..cut]), None);
        }
        let Some(Entry::Delta(delta)) = patch.entry("dist/lib.js") else { unreachable!() };
        assert_eq!(apply(&reference[2].1, delta).as_deref(), Some(target[2].1.as_slice()));
    }
}
