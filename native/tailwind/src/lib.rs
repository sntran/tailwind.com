//! A Tailwind CSS compiler.
//!
//! The modules follow the files of `packages/tailwindcss/src` of Tailwind
//! CSS, so a change of Tailwind CSS has one clear place in this crate.

pub mod apply;
pub mod ast;
pub mod at_import;
pub mod attribute_selector_parser;
pub mod candidate;
#[cfg(feature = "cli")]
pub mod cli;
pub mod compat;
pub mod compile;
pub mod css_functions;
pub mod css_parser;
pub mod design_system;
pub mod index;
pub mod optimize;
pub mod property_order;
pub mod selector_parser;
pub mod source_maps;
pub mod theme;
pub mod to_css;
pub mod utilities;
pub mod utils;
pub mod value_parser;
pub mod variants;
pub mod walk;

pub use compat::module_loader::{FileKind, FileStat, MemoryLoader, ModuleLoader};
pub use index::{compile, CompileOptions, Compiler, Root, Source, Stylesheet, VERSION};

/// An error of the compiler. The message is the message of Tailwind CSS.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    pub message: String,
    /// The name of the error, as JavaScript gives it: `Error`,
    /// `CssSyntaxError` for an error of the CSS parser, or the name of a
    /// JavaScript exception of a plugin, for example `TypeError`.
    pub name: std::borrow::Cow<'static, str>,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self { message: message.into(), name: std::borrow::Cow::Borrowed("Error") }
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl From<css_parser::CssSyntaxError> for Error {
    fn from(e: css_parser::CssSyntaxError) -> Self {
        Error { message: e.message, name: std::borrow::Cow::Borrowed("CssSyntaxError") }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// The features that a stylesheet uses, as bits.
pub type Features = u32;
pub const FEATURES_NONE: Features = 0;
/// `@apply` was used.
pub const FEATURES_AT_APPLY: Features = 1 << 0;
/// `@import` was used.
pub const FEATURES_AT_IMPORT: Features = 1 << 1;
/// `@plugin` or `@config` was used.
pub const FEATURES_JS_PLUGIN_COMPAT: Features = 1 << 2;
/// `theme(…)` was used.
pub const FEATURES_THEME_FUNCTION: Features = 1 << 3;
/// `@tailwind utilities` was used.
pub const FEATURES_UTILITIES: Features = 1 << 4;
/// `@variant` was used.
pub const FEATURES_VARIANTS: Features = 1 << 5;
/// `@theme` was used.
pub const FEATURES_AT_THEME: Features = 1 << 6;

/// The polyfills of the output, as bits.
pub type Polyfills = u8;
pub const POLYFILLS_NONE: Polyfills = 0;
/// Fallbacks for `@property`.
pub const POLYFILLS_AT_PROPERTY: Polyfills = 1 << 0;
/// Fallbacks for `color-mix(…)`.
pub const POLYFILLS_COLOR_MIX: Polyfills = 1 << 1;
pub const POLYFILLS_ALL: Polyfills = POLYFILLS_AT_PROPERTY | POLYFILLS_COLOR_MIX;
