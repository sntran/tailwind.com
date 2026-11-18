//! The memory of the canonical form of candidates. The index of the class
//! list and the collapse compile and parse about 200,000 candidates; the
//! caches keep only the entries that can change a result
//! (`DesignSystem::transient_compiles`). A counter of the allocations
//! measures the live bytes after 400 lists of classes
//! (`tests/upstream/cli/canonicalize-random/stdin.txt`). The caches kept
//! about 550 MB before `transient_compiles`, and about 110 MB with it.
#![cfg(feature = "canonicalize")]

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

struct Counter;
static LIVE: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counter {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        LIVE.fetch_add(layout.size(), Relaxed);
        unsafe { System.alloc(layout) }
    }
    unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Relaxed);
        unsafe { System.dealloc(pointer, layout) }
    }
    unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        LIVE.fetch_add(size, Relaxed);
        LIVE.fetch_sub(layout.size(), Relaxed);
        unsafe { System.realloc(pointer, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: Counter = Counter;

#[test]
fn the_caches_of_400_lists_stay_small() {
    let load = |id: &str, _: &str| -> tailwind::Result<tailwind::Stylesheet> {
        let name = if id == "tailwindcss" { "index.css" } else { id.trim_start_matches("tailwindcss/") };
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/upstream/css").join(name);
        Ok(tailwind::Stylesheet { path: name.into(), base: "/".into(), content: std::fs::read_to_string(path).unwrap() })
    };
    let options = tailwind::CompileOptions { base: "/".into(), load_stylesheet: Some(&load), ..Default::default() };
    let design_system = tailwind::index::load_design_system("@import \"tailwindcss\";", options).unwrap();
    let before = LIVE.load(Relaxed);

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/upstream/cli/canonicalize-random/stdin.txt");
    let options = tailwind::canonicalize::CanonicalizeOptions { rem: None, collapse: true, logical_to_physical: true };
    for line in std::fs::read_to_string(path).unwrap().lines() {
        let list: Vec<String> = line.split(' ').map(String::from).collect();
        design_system.canonicalize_candidates(&list, &options);
    }

    let live = (LIVE.load(Relaxed) - before) as f64 / 1e6;
    assert!(live < 200.0, "the caches keep {live:.0} MB");
}
