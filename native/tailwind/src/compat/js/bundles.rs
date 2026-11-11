//! The plugin layer of each version that the Rust compiler serves.
//!
//! The plugin layer is JavaScript: `compat/` of the source of Tailwind
//! CSS, with the shims of `native/tailwind/js`, in one file. The file of
//! the version of the port is `assets/compat.js`. The source of each
//! older version, with no change, gives the file of that version, and the
//! program has it as a patch (`assets/compat/VERSION.patch`): the
//! difference from the file of the next newer version (`patch.rs`).
//! `scripts/compat.sh` makes all these files.
//!
//! So a change of Tailwind CSS in its plugin layer needs no switch: the
//! Rust compiler as version N runs the plugin layer of version N.

use crate::patch::{self, Entry, Patch};
use crate::version::Version;
use std::borrow::Cow;

/// The plugin layer of the version of the port.
const COMPAT_JS: &str = include_str!("../../../assets/compat.js");

/// The name of the file in a patch.
const FILE: &str = "compat.js";

// `COMPAT_PATCHES`: the patch files, from `build.rs`.
include!(concat!(env!("OUT_DIR"), "/compat_patches.rs"));

/// The plugin layer of a version. `None`: the program does not have it.
pub fn compat_js(version: Version) -> Option<Cow<'static, str>> {
    if version == Version::NATIVE {
        return Some(Cow::Borrowed(COMPAT_JS));
    }
    String::from_utf8(bytes(&version.to_string(), 0)?).ok().map(Cow::Owned)
}

/// The file of a version: the file of the reference version of its patch,
/// with the delta.
fn bytes(version: &str, depth: usize) -> Option<Vec<u8>> {
    if Version::parse(version) == Some(Version::NATIVE) {
        return Some(COMPAT_JS.as_bytes().to_vec());
    }
    // A chain of patches has an end: no version refers to itself.
    if depth > COMPAT_PATCHES.len() {
        return None;
    }
    let (_, file) = COMPAT_PATCHES.iter().find(|(name, _)| *name == version)?;
    let patch = Patch::parse(file)?;
    match patch.entry(FILE)? {
        Entry::Same => bytes(&patch.reference, depth + 1),
        Entry::Delta(delta) => patch::apply(&bytes(&patch.reference, depth + 1)?, delta),
        Entry::Full(bytes) => Some(bytes.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_served_version_has_its_plugin_layer() {
        // The versions of the patch files, and the version of the port.
        let mut versions: Vec<Version> = COMPAT_PATCHES.iter().filter_map(|(name, _)| Version::parse(name)).collect();
        assert_eq!(versions.len(), COMPAT_PATCHES.len());
        versions.push(Version::NATIVE);
        assert!(versions.contains(&Version::OLDEST_WITH_PLUGINS), "no plugin layer for Version::OLDEST_WITH_PLUGINS");
        for version in versions {
            assert!(version >= Version::OLDEST_WITH_PLUGINS && version <= Version::NATIVE, "{version}");
            let text = compat_js(version).unwrap_or_else(|| panic!("the plugin layer of {version}"));
            // The second line of the file names the version of its source.
            assert!(text.contains(&format!("The plugin layer of Tailwind CSS v{version} ")), "{version}");
        }
        assert!(compat_js(Version(3, 4, 0)).is_none());
    }
}
