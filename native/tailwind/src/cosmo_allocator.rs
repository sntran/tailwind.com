//! The allocator of the file tailwind.com (the feature `ape-mimalloc`):
//! mimalloc 2, which scripts/build.sh compiles with cosmocc from the C
//! source of the crate `libmimalloc-sys` (scripts/cosmo.sh). The program
//! of `cargo build` gets mimalloc from the crate `mimalloc` (src/main.rs).
//!
//! The library is the program of the file: its staticlib is the last
//! artifact of Rust, so the allocator is here, not in a `main`.

use std::alloc::{GlobalAlloc, Layout};
use std::ffi::c_void;

extern "C" {
    fn mi_malloc_aligned(size: usize, alignment: usize) -> *mut c_void;
    fn mi_zalloc_aligned(size: usize, alignment: usize) -> *mut c_void;
    fn mi_realloc_aligned(p: *mut c_void, newsize: usize, alignment: usize) -> *mut c_void;
    fn mi_free(p: *mut c_void);
}

struct Mimalloc;

unsafe impl GlobalAlloc for Mimalloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe { mi_malloc_aligned(layout.size(), layout.align()).cast() }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        unsafe { mi_zalloc_aligned(layout.size(), layout.align()).cast() }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        unsafe { mi_free(ptr.cast()) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        unsafe { mi_realloc_aligned(ptr.cast(), new_size, layout.align()).cast() }
    }
}

#[global_allocator]
static ALLOCATOR: Mimalloc = Mimalloc;
