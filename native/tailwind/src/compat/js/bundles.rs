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

/// The plugin layer of the version of the port. The engine of the host has
/// it itself (js/engine.mjs): the program evaluates an empty file with the
/// name `tailwindcss/compat.js`, and the host evaluates its copy.
#[cfg(not(tw_host_js))]
const COMPAT_JS: &str = include_str!("../../../assets/compat.js");
#[cfg(tw_host_js)]
const COMPAT_JS: &str = "";

/// The bytecode of `assets/compat.js` (`build.rs`, `c_src/bytecode.c`):
/// a start of the plugin layer reads it, and does not compile the text.
/// Empty when the build could not make it.
#[cfg(not(tw_host_js))]
static COMPAT_BYTECODE: &[u8] = include_bytes!(concat!(env!("OUT_DIR"), "/compat.bc"));

#[cfg(test)]
thread_local! {
    /// True: the plugin layer of the port starts from its text, as for
    /// the other versions. For the tests that compare the two starts.
    pub static FROM_TEXT: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// The name of the file in a patch.
const FILE: &str = "compat.js";

/// Start the plugin layer of a version from its bytecode. `Ok(false)`:
/// the program has no bytecode for this version (only the version of the
/// port has it), or the engine cannot read it. Then nothing ran, and the
/// caller evaluates the text of `compat_js`.
pub fn eval_bytecode(engine: &super::engine::Engine, version: Version) -> Result<bool, super::engine::JsError> {
    #[cfg(test)]
    if FROM_TEXT.with(|from_text| from_text.get()) {
        return Ok(false);
    }
    #[cfg(not(tw_host_js))]
    if version == Version::NATIVE {
        return engine.eval_bytecode(COMPAT_BYTECODE);
    }
    let _ = (engine, version);
    Ok(false)
}

// `COMPAT_PATCHES`: the patch files, from `build.rs`.
include!(concat!(env!("OUT_DIR"), "/compat_patches.rs"));

/// The plugin layer of a version. `None`: the program does not have it.
pub fn compat_js(version: Version) -> Option<Cow<'static, str>> {
    if version == Version::NATIVE {
        return Some(Cow::Borrowed(COMPAT_JS));
    }
    // The engine of the host has only the version of the port.
    if cfg!(tw_host_js) {
        return None;
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

    #[cfg(all(tw_js, not(tw_host_js)))]
    #[test]
    fn the_program_has_the_bytecode_of_the_plugin_layer() {
        // A build with QuickJS makes it (build.rs). If the build cannot, it
        // gives a warning, and this test fails.
        assert!(!COMPAT_BYTECODE.is_empty());
        // The engine reads the bytecode and runs it. Here it has no host,
        // so the plugin layer stops at its first question to the host.
        let engine = super::super::engine::Engine::new(Default::default(), Box::new(|_, _| Ok(None))).unwrap();
        assert_ne!(eval_bytecode(&engine, Version::NATIVE), Ok(false));
        // The other versions start from their text.
        assert_eq!(eval_bytecode(&engine, Version(4, 1, 0)), Ok(false));
    }

    #[cfg(all(tw_js, not(tw_host_js)))]
    #[test]
    fn the_bytecode_and_the_text_of_the_plugin_layer_give_the_same_css() {
        use crate::{compile, CompileOptions, MemoryLoader};
        use std::rc::Rc;

        let plugin = "module.exports = function ({ addUtilities, addVariant, matchUtilities, theme }) {\n\
            addUtilities({ '.from-plugin': { color: theme('colors.red.500') } })\n\
            addVariant('hocus', ['&:hover', '&:focus'])\n\
            matchUtilities({ tab: (value) => ({ tabSize: value }) }, { values: theme('spacing') })\n\
        }\n";
        let config = "module.exports = { theme: { extend: { colors: { brand: '#123456' } } } }\n";
        let css = "@theme { --color-red-500: #ef4444; --spacing-4: 1rem; }\n@tailwind utilities;\n\
            @plugin \"./plugin.js\";\n@config \"./tailwind.config.js\";\n";
        let candidates: Vec<String> =
            ["from-plugin", "hocus:underline", "tab-4", "bg-brand", "text-brand/50", "p-4", "flex"].iter().map(|c| c.to_string()).collect();
        let run = |from_text: bool| {
            FROM_TEXT.with(|flag| flag.set(from_text));
            let loader = Rc::new(MemoryLoader::new(&[("/app/plugin.js", plugin), ("/app/tailwind.config.js", config)]));
            let options = CompileOptions { base: "/app".into(), test_mode: true, module_loader: Some(loader), ..Default::default() };
            let css = compile(css, options).unwrap().build(&candidates);
            FROM_TEXT.with(|flag| flag.set(false));
            css
        };
        let from_bytecode = run(false);
        assert!(from_bytecode.contains(".from-plugin {") && from_bytecode.contains(".bg-brand {"), "{from_bytecode}");
        assert_eq!(run(true), from_bytecode);
    }

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
