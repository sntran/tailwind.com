//! The hash maps and sets of the compiler: the maps of `std` and of
//! `indexmap` with the hash
//! of `rustc-hash` (FxHash), not SipHash. The keys are short texts (class
//! names, selectors, properties), and the hash of SipHash took about 6%
//! of the time of a large build. FxHash does not protect a map from keys
//! that an attacker selects; the keys come from the project and from its
//! stylesheets. The order of a map is no part of the output: the compiler
//! sorts what it writes.
//!
//! A map of this module has no `new()`: use `default()`.

pub type HashMap<K, V> = std::collections::HashMap<K, V, rustc_hash::FxBuildHasher>;
pub type HashSet<T> = std::collections::HashSet<T, rustc_hash::FxBuildHasher>;
/// A map in the sequence of its inserts (the crate `indexmap`).
pub type IndexMap<K, V> = indexmap::IndexMap<K, V, rustc_hash::FxBuildHasher>;
/// A set in the sequence of its inserts (the crate `indexmap`).
pub type IndexSet<T> = indexmap::IndexSet<T, rustc_hash::FxBuildHasher>;
