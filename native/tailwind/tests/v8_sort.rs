//! The sort of `src/utils/v8_sort.rs` against sorts that V8 made.
//!
//! `node tests/v8.mjs --record tests/upstream/v8-sort.jsonl` writes the
//! lines: the name of a compare function, its salt, the keys of the
//! values, the sequence of the values after `Array.prototype.sort` of
//! Node.js, and the number of calls of the compare function. Some of the
//! compare functions are not consistent: then the result depends on the
//! algorithm, and the Rust form must give the result of V8 with the same
//! number of calls.

use tailwind::utils::v8_sort;

#[derive(Clone)]
struct Item {
    key: i32,
    id: usize,
}

/// `hash` of tests/v8.mjs: a number from two values, the same for the
/// same pair.
fn hash(a: &Item, z: &Item, salt: i32) -> u32 {
    let mut h = (a.key ^ salt).wrapping_mul(0x9e3779b1u32 as i32) ^ z.key.wrapping_add(salt).wrapping_mul(0x85ebca6bu32 as i32);
    h ^= ((h as u32) >> 13) as i32;
    h = h.wrapping_mul(0xc2b2ae35u32 as i32);
    (h ^ ((h as u32) >> 16) as i32) as u32
}

/// The compare functions of tests/v8.mjs.
fn compare(name: &str, salt: i32, a: &Item, z: &Item) -> i64 {
    let (ak, zk) = (i64::from(a.key), i64::from(z.key));
    match name {
        "consistent" => ak - zk,
        "reversed" => zk - ak,
        "coarse" => (ak >> 4) - (zk >> 4),
        "holes" => {
            if (ak + i64::from(salt)) % 5 == 0 || (zk + i64::from(salt)) % 5 == 0 {
                1
            } else {
                ak - zk
            }
        }
        "noise" => i64::from(hash(a, z, salt) % 3) - 1,
        "mostly" => {
            if hash(a, z, salt).is_multiple_of(11) {
                zk - ak
            } else {
                ak - zk
            }
        }
        "always" => 1,
        "never" => -1,
        "zero" => 0,
        other => panic!("no compare function {other}"),
    }
}

#[test]
fn the_sort_gives_the_result_of_v8() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/upstream/v8-sort.jsonl");
    let text = std::fs::read_to_string(path).expect("tests/upstream/v8-sort.jsonl");
    let mut cases = 0;
    let mut not_consistent = 0;
    for line in text.lines().filter(|line| !line.is_empty()) {
        let case: serde_json::Value = serde_json::from_str(line).expect("a JSON case");
        let name = case["compare"].as_str().unwrap();
        let salt = case["salt"].as_i64().unwrap() as i32;
        let mut items: Vec<Item> =
            case["keys"].as_array().unwrap().iter().enumerate().map(|(id, key)| Item { key: key.as_i64().unwrap() as i32, id }).collect();
        let expected: Vec<usize> = case["sorted"].as_array().unwrap().iter().map(|id| id.as_u64().unwrap() as usize).collect();

        let mut calls = 0u64;
        v8_sort::sort(&mut items, |a, z| {
            calls += 1;
            compare(name, salt, a, z)
        });
        let sorted: Vec<usize> = items.iter().map(|item| item.id).collect();
        assert_eq!(sorted, expected, "case {cases}: {} values, compare \"{name}\"", expected.len());
        assert_eq!(calls, case["calls"].as_u64().unwrap(), "case {cases}: the number of calls, compare \"{name}\"");
        cases += 1;
        if matches!(name, "holes" | "noise" | "mostly" | "always" | "never") {
            not_consistent += 1;
        }
    }
    assert!(cases >= 100 && not_consistent >= 40, "{cases} cases, {not_consistent} with a compare function that is not consistent");
}

#[test]
fn a_consistent_compare_function_gives_a_stable_sort() {
    let mut values: Vec<(u32, usize)> = (0..500).map(|id| ((id * 7919 % 13) as u32, id)).collect();
    let mut expected = values.clone();
    expected.sort_by_key(|value| value.0);
    v8_sort::sort(&mut values, |a, z| i64::from(a.0) - i64::from(z.0));
    assert_eq!(values, expected);
}
