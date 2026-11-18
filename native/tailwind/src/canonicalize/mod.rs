//! The canonical form of candidates: `canonicalize-candidates.ts` of
//! Tailwind CSS, and the modules that only it uses.
//!
//! This code follows the newest version of Tailwind CSS only. It has no
//! switches for the older versions.

/// The features of a signature, as bits: `SignatureFeatures` of
/// `canonicalize-candidates.ts`.
pub type SignatureFeatures = u8;
pub const SIGNATURE_NONE: SignatureFeatures = 0;
/// Expand a shorthand property into its longhand properties.
pub const SIGNATURE_EXPAND_PROPERTIES: SignatureFeatures = 1 << 0;
/// Change the logical properties into the physical properties.
pub const SIGNATURE_LOGICAL_TO_PHYSICAL: SignatureFeatures = 1 << 1;

pub mod canonicalize_calc_expressions;
pub mod constant_fold_declaration;
pub mod expand_declaration;
