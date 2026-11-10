/*
 * For the build of QuickJS for WASI (wasm32-wasip1).
 *
 * dtoa.c of QuickJS includes <setjmp.h> and uses nothing of it. The
 * <setjmp.h> of wasi-libc is an error without the exception handling of
 * WebAssembly. native/tailwind/build.rs puts this directory before the
 * directories of the system, so this empty file is the <setjmp.h> of
 * QuickJS, and the files of QuickJS have no change.
 */
