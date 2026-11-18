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
mod collapse;
mod signatures;
mod utilities;
mod variants;

use crate::candidate::Variant;
use crate::design_system::DesignSystem;
use crate::hash::{HashMap, IndexMap, IndexSet};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

/// The options of `canonicalizeCandidates`: `CanonicalizeOptions`.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct CanonicalizeOptions {
    /// The root font size in pixels. With it, `rem` values are `px`
    /// values: `mt-[16px]` with `rem: 16` is `mt-4`.
    pub rem: Option<f64>,
    /// Make one utility of more utilities when possible: `mt-2 mr-2 mb-2
    /// ml-2` is `m-2`.
    pub collapse: bool,
    /// Change the logical properties into the physical properties when
    /// utilities collapse: `mr-2 ml-2` is `mx-2`.
    pub logical_to_physical: bool,
}

/// The root font size as a key of a cache: the bits of the number.
pub type RemKey = Option<u64>;

/// `SignatureOptions`: the options that change a signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SignatureOptions {
    pub rem: RemKey,
    pub features: SignatureFeatures,
}

impl SignatureOptions {
    pub fn rem(&self) -> Option<f64> {
        self.rem.map(f64::from_bits)
    }
}

/// `Features` of `canonicalize-candidates.ts`, as bits.
pub type Features = u8;
pub const FEATURE_NONE: Features = 0;
/// Make one utility of more utilities when possible.
pub const FEATURE_COLLAPSE_UTILITIES: Features = 1 << 0;

/// `InternalCanonicalizeOptions`, with no design system.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InternalOptions {
    pub features: Features,
    pub signature_options: SignatureOptions,
}

/// A signature: the CSS in a normal form, or a value of its own for a
/// candidate that is not valid (a new `Symbol()` upstream).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Signature {
    Css(Rc<str>),
    Invalid(u64),
}

impl Signature {
    pub fn is_css(&self) -> bool {
        matches!(self, Signature::Css(_))
    }
}

/// The spacing scale: the value and the unit of `--spacing`, after the
/// fold (`SPACING_KEY`).
#[derive(Debug, Clone)]
pub struct Spacing {
    value: f64,
    unit: Option<String>,
    rem: Option<f64>,
}

/// A cache by the options of the signatures and a text.
type ByText<T> = RefCell<HashMap<(SignatureOptions, String), T>>;
/// The index of the static utilities: property to value to utilities.
type StaticIndex = IndexMap<String, IndexMap<String, IndexSet<String>>>;
/// A value with the inlined theme variables, and the variables.
type Resolved = (String, Rc<Vec<String>>);
/// A cache by the internal options and a text.
type ByOptions<K, T> = RefCell<HashMap<(InternalOptions, K), T>>;

/// The caches of the canonical form of candidates: the `storage` of the
/// design system upstream. They stay with the design system.
#[derive(Default)]
pub struct Storage {
    symbols: Cell<u64>,
    utility_signatures: ByText<Signature>,
    utility_theme_variables: ByText<Option<Rc<Vec<String>>>>,
    static_utilities: RefCell<HashMap<SignatureOptions, StaticIndex>>,
    utility_properties: ByText<Rc<signatures::PropertyValues>>,
    pre_computed_utilities: RefCell<HashMap<SignatureOptions, Rc<signatures::Lookup>>>,
    variant_signatures: RefCell<HashMap<(RemKey, String), Signature>>,
    pre_computed_variants: RefCell<HashMap<RemKey, Rc<signatures::Lookup>>>,
    /// `SPACING_KEY`: made one time, with the `rem` of the first call.
    spacing: RefCell<Option<Option<Spacing>>>,
    spacing_values: RefCell<HashMap<String, Option<f64>>>,
    candidates: ByOptions<String, String>,
    variants: ByOptions<*const Variant, Rc<Vec<Variant>>>,
    utilities: ByOptions<String, String>,
    /// The normal form of the values of declarations, by `rem` and value.
    normal_values: RefCell<HashMap<(RemKey, String), String>>,
    /// The values with the inlined theme variables, and the variables.
    resolved_values: RefCell<HashMap<String, Resolved>>,
}

impl Storage {
    /// A new signature for a candidate that is not valid.
    fn new_symbol(&self) -> Signature {
        let id = self.symbols.get() + 1;
        self.symbols.set(id);
        Signature::Invalid(id)
    }
}

/// `createSignatureOptions`.
pub fn signature_options(options: &CanonicalizeOptions) -> SignatureOptions {
    let mut features = SIGNATURE_NONE;
    if options.collapse {
        features |= SIGNATURE_EXPAND_PROPERTIES;
    }
    if options.logical_to_physical {
        features |= SIGNATURE_LOGICAL_TO_PHYSICAL;
    }
    SignatureOptions { rem: options.rem.map(f64::to_bits), features }
}

/// `canonicalizeCandidates`: the canonical form of each candidate, with
/// no duplicates. With `collapse`, one utility can replace more utilities.
pub fn canonicalize_candidates(ds: &DesignSystem, candidates: &[String], options: &CanonicalizeOptions) -> Vec<String> {
    let signature_options = signature_options(options);
    utilities::prepare_spacing(ds, options.rem);
    let features = if options.collapse { FEATURE_COLLAPSE_UTILITIES } else { FEATURE_NONE };
    let options = InternalOptions { features, signature_options };

    let mut result: IndexSet<String> = IndexSet::default();
    for candidate in candidates {
        result.insert(canonicalize_candidate(ds, options, candidate));
    }

    let result: Vec<String> = result.into_iter().collect();
    if result.len() <= 1 || options.features & FEATURE_COLLAPSE_UTILITIES == 0 {
        result
    } else {
        collapse::collapse_candidates(ds, options, result)
    }
}

impl DesignSystem {
    /// `canonicalizeCandidates` of the design system.
    pub fn canonicalize_candidates(&self, candidates: &[String], options: &CanonicalizeOptions) -> Vec<String> {
        canonicalize_candidates(self, candidates, options)
    }
}

/// `CANONICALIZE_CANDIDATE_KEY`: the canonical form of one candidate. The
/// variants and the utility have their own caches.
fn canonicalize_candidate(ds: &DesignSystem, options: InternalOptions, raw: &str) -> String {
    let key = (options, raw.to_string());
    if let Some(found) = ds.canonical.candidates.borrow().get(&key) {
        return found.clone();
    }
    let result = canonicalize_candidate_uncached(ds, options, raw);
    ds.canonical.candidates.borrow_mut().insert(key, result.clone());
    result
}

fn canonicalize_candidate_uncached(ds: &DesignSystem, options: InternalOptions, raw: &str) -> String {
    let prefix = match ds.theme.prefix.as_deref().filter(|p| !p.is_empty()) {
        Some(prefix) => format!("{prefix}:"),
        None => String::new(),
    };
    for candidate in ds.parse_candidate(raw).iter() {
        let mut variants: Vec<Variant> = Vec::new();
        for variant in candidate.variants().iter().rev() {
            variants.extend(variants::canonicalize_variant(ds, options, variant).iter().cloned());
        }
        let important = candidate.important();

        // The canonical form of the utility, then the variants and the
        // important flag again: the caches of the utility and of each
        // variant get more hits.
        if important || !variants.is_empty() {
            let base = utilities::without_variants(candidate);
            let mut result = canonicalize_candidate(ds, options, &ds.print_candidate(&base));

            // The variants are between the prefix and the utility.
            if ds.theme.prefix.is_some() && !variants.is_empty() {
                result = js_slice_from(&result, prefix.len());
            }
            if !variants.is_empty() {
                let printed: Vec<String> = variants.iter().map(|v| ds.print_variant(v)).collect();
                result = format!("{}:{result}", printed.join(":"));
            }
            if important {
                result.push('!');
            }
            if ds.theme.prefix.is_some() && !variants.is_empty() {
                result = format!("{prefix}{result}");
            }
            return result;
        }

        // No variants and no important flag: only the utility.
        let result = utilities::canonicalize_utility(ds, options, raw);
        if result != raw {
            return result;
        }
    }
    raw.to_string()
}

/// `text.slice(start)` of JavaScript for a start on a character boundary.
fn js_slice_from(text: &str, start: usize) -> String {
    text.get(start..).unwrap_or("").to_string()
}
