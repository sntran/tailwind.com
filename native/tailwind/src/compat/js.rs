//! JavaScript for `@plugin` and `@config`.
//!
//! The plugin layer is the JavaScript of Tailwind CSS itself
//! (`assets/compat.js`: `compat/` of its source, made by
//! `tests/upstream/tools/build-compat-bundle.mjs`). It runs in QuickJS
//! (`engine.rs`). The theme, the utilities and the variants stay in Rust:
//! the JavaScript calls host operations (`runtime.rs`), and the Rust
//! compiler calls the functions of the plugins.

#[cfg(tw_js)]
pub mod engine;
pub mod json;
pub mod resolve;
#[cfg(tw_js)]
mod runtime;

#[cfg(tw_js)]
pub use runtime::{apply_compatibility_hooks, JsRuntime};
