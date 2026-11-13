//! The engine of a build, and the store of the packages of Tailwind CSS.
//!
//! The program has two engines:
//!
//! - The native engine is the Rust compiler of the program. It follows
//!   one version of Tailwind CSS (`crate::VERSION`), and it is fast.
//! - The script engine (`crate::script`) runs the official compiler of an
//!   npm package `tailwindcss` in QuickJS. It works for each version
//!   whose package the program can find, with no change of the program.
//!
//! The environment selects the version and the engine:
//!
//! - `TAILWINDCSS_VERSION`: the version of Tailwind CSS (default: the
//!   version of the native engine).
//! - `TAILWIND_COM_ENGINE`: `native` or `script`. With no value, each
//!   version from `Version::OLDEST` to the version of the port uses the
//!   native engine, and each other version uses the script engine.
//! - `TAILWIND_COM_PACKAGES`: a directory with one directory for each
//!   version (`4.2.0/package.json`, `4.2.0/dist/lib.js`, ...): the files
//!   of the npm package.
//!
//! The program looks for a package in `TAILWIND_COM_PACKAGES`, then in
//! its own file: `tailwind.com` is also a ZIP file, and the packages are
//! in its directory `tailwindcss/`. Then in the store in the program
//! (`EMBEDDED`): the native NIF objects for BEAM.com have one
//! (`scripts/nif.sh`). A store has the newest package in full, and each
//! older version as a patch file (`patch.rs`).

use crate::index::{Compiler, Root, Source};
use crate::source_maps::DecodedSourceMap;
use crate::patch::{self, Entry, Patch};
use crate::version::Version;
use crate::Result;
use std::cell::RefCell;
use crate::hash::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

/// The variable of the version of Tailwind CSS.
pub const VERSION_VARIABLE: &str = "TAILWINDCSS_VERSION";
/// The variable of the engine.
pub const ENGINE_VARIABLE: &str = "TAILWIND_COM_ENGINE";
/// The variable of the directory of the packages.
pub const PACKAGES_VARIABLE: &str = "TAILWIND_COM_PACKAGES";
/// The directory of the packages in the ZIP file of `tailwind.com`.
const ZIP_PACKAGES: &str = "/zip/tailwindcss";

// The files of the store in the program: `EMBEDDED`, a list of the paths
// in the store and their bytes. Empty, unless the build had the variable
// TAILWIND_COM_EMBED_PACKAGES (build.rs).
include!(concat!(env!("OUT_DIR"), "/embedded_packages.rs"));

/// A store of packages: a directory, or the files in the program. It has,
/// for each version, a directory with the files of the package
/// (`4.3.5/package.json`, ...) or a patch file (`4.3.4.patch`): the
/// differences from another version of the store (`patch.rs`).
#[derive(Clone)]
enum Store {
    Directory(PathBuf),
    Embedded,
}

impl Store {
    /// The bytes of a file of the store. `path` has `/` between its parts.
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        match self {
            Store::Directory(directory) => std::fs::read(directory.join(path)).ok(),
            Store::Embedded => EMBEDDED.iter().find(|(name, _)| *name == path).map(|(_, bytes)| bytes.to_vec()),
        }
    }

    /// The names at the top of the store: the directories and the patch
    /// files of the versions.
    fn names(&self) -> Vec<String> {
        match self {
            Store::Directory(directory) => std::fs::read_dir(directory)
                .map(|entries| entries.flatten().map(|entry| entry.file_name().to_string_lossy().into_owned()).collect())
                .unwrap_or_default(),
            Store::Embedded => {
                let mut names: Vec<String> = Vec::new();
                for (path, _) in EMBEDDED {
                    let name = path.split('/').next().unwrap_or(path).to_string();
                    if !names.contains(&name) {
                        names.push(name);
                    }
                }
                names
            }
        }
    }
}

/// An npm package `tailwindcss` of a store.
pub struct DirectoryPackage {
    store: Store,
    version: String,
    /// The patch files that this package read, by their version.
    patches: RefCell<HashMap<String, Option<Rc<Patch>>>>,
}

/// True for a text that can be a version: it becomes a part of a path.
fn is_version(text: &str) -> bool {
    !text.is_empty() && text.len() <= 64 && text.bytes().all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+')) && !text.starts_with('.')
}

impl DirectoryPackage {
    /// The package of a version in a store, when the store has the
    /// package `tailwindcss` of this version.
    fn open(store: Store, version: &str) -> Option<Self> {
        if !is_version(version) {
            return None;
        }
        let package = Self { store, version: version.to_string(), patches: RefCell::new(HashMap::default()) };
        let json: serde_json::Value = serde_json::from_str(&package.read("package.json")?).ok()?;
        let name = json.get("name").and_then(|value| value.as_str())?;
        let found = json.get("version").and_then(|value| value.as_str())?;
        (name == "tailwindcss" && found == version).then_some(package)
    }

    /// The version of the package.
    pub fn version(&self) -> &str {
        &self.version
    }

    /// The text of a file of the package.
    pub fn read(&self, name: &str) -> Option<String> {
        // Only a file in the package.
        if name.starts_with('/') || name.split('/').any(|part| part == ".." || part.is_empty()) {
            return None;
        }
        String::from_utf8(self.bytes(&self.version, name, 0)?).ok()
    }

    /// The patch file of a version, when the store has one.
    fn patch(&self, version: &str) -> Option<Rc<Patch>> {
        if let Some(known) = self.patches.borrow().get(version) {
            return known.clone();
        }
        let patch = self.store.read(&format!("{version}.patch")).and_then(|bytes| Patch::parse(&bytes)).map(Rc::new);
        self.patches.borrow_mut().insert(version.to_string(), patch.clone());
        patch
    }

    /// The bytes of a file of a version: the file of its directory, or
    /// the file of the reference version of its patch with the delta.
    fn bytes(&self, version: &str, name: &str, depth: usize) -> Option<Vec<u8>> {
        // A chain of patches has an end: no version refers to itself.
        if depth > 1024 || !is_version(version) {
            return None;
        }
        if let Some(bytes) = self.store.read(&format!("{version}/{name}")) {
            return Some(bytes);
        }
        let patch = self.patch(version)?;
        match patch.entry(name)? {
            Entry::Same => self.bytes(&patch.reference, name, depth + 1),
            Entry::Delta(delta) => patch::apply(&self.bytes(&patch.reference, name, depth + 1)?, delta),
            Entry::Full(bytes) => Some(bytes.clone()),
        }
    }
}

#[cfg(tw_js)]
impl crate::script::Package for DirectoryPackage {
    fn version(&self) -> &str {
        &self.version
    }

    fn read(&self, name: &str) -> Option<String> {
        DirectoryPackage::read(self, name)
    }
}

/// The stores: the directory of the environment, the ZIP directory of the
/// file of the program, and the store in the program.
fn stores() -> Vec<Store> {
    let mut stores = Vec::new();
    if let Some(packages) = super::context::var(PACKAGES_VARIABLE).filter(|value| !value.is_empty()) {
        stores.push(Store::Directory(PathBuf::from(packages)));
    }
    stores.push(Store::Directory(PathBuf::from(ZIP_PACKAGES)));
    if !EMBEDDED.is_empty() {
        stores.push(Store::Embedded);
    }
    stores
}

/// The package of a version, from the first store that has it.
fn find_package(version: &str) -> Option<DirectoryPackage> {
    stores().into_iter().find_map(|store| DirectoryPackage::open(store, version))
}

/// The versions that are in the stores, sorted.
pub fn stored_versions() -> Vec<String> {
    let mut versions: Vec<String> = Vec::new();
    for store in stores() {
        for name in store.names() {
            let name = name.strip_suffix(".patch").unwrap_or(&name).to_string();
            if !versions.contains(&name) && DirectoryPackage::open(store.clone(), &name).is_some() {
                versions.push(name);
            }
        }
    }
    versions.sort_by_key(|version| version.split('.').map(|part| part.parse::<u64>().unwrap_or(0)).collect::<Vec<_>>());
    versions
}

/// The engine and the version of a run of the program.
#[derive(Clone)]
pub enum Selection {
    /// The Rust compiler of the program, as this version. For another
    /// version than the version of the port, the stylesheets come from
    /// the package of that version.
    Native { version: Version, package: Option<Rc<DirectoryPackage>> },
    /// The official compiler of this package.
    Script(Rc<DirectoryPackage>),
}

impl Selection {
    /// The version of Tailwind CSS.
    pub fn version(&self) -> String {
        match self {
            Selection::Native { version, .. } => version.to_string(),
            Selection::Script(package) => package.version.clone(),
        }
    }

    /// The name of the engine.
    pub fn engine(&self) -> &'static str {
        match self {
            Selection::Native { .. } => "native",
            Selection::Script(_) => "script",
        }
    }

    /// The package that gives the stylesheets of Tailwind CSS. `None`:
    /// the stylesheets of the program.
    pub fn package(&self) -> Option<&DirectoryPackage> {
        match self {
            Selection::Native { package, .. } => package.as_deref(),
            Selection::Script(package) => Some(package),
        }
    }
}

/// The version that the environment asks for.
pub fn requested_version() -> String {
    match super::context::var(VERSION_VARIABLE) {
        Some(version) if !version.trim().is_empty() => version.trim().trim_start_matches('v').to_string(),
        _ => crate::index::VERSION.to_string(),
    }
}

/// The engine and the version that the environment asks for.
///
/// The Rust compiler can compile as each version from `Version::OLDEST`
/// to its own version (`version.rs`). With no engine in the environment,
/// these versions use it, and each other version uses the script engine.
pub fn select() -> std::result::Result<Selection, String> {
    let version = requested_version();
    let engine = super::context::var(ENGINE_VARIABLE).unwrap_or_default();
    let native_version = crate::index::VERSION;

    let missing = |version: &str| -> String {
        let mut message = format!("Tailwind CSS v{version} is not in this program.");
        if version.split('.').next() != Some("4") {
            message.push_str(" The program is for Tailwind CSS 4.");
        } else {
            message.push_str(&format!(
                " Put the files of the npm package tailwindcss@{version} in the directory `{version}` of a directory, and give that directory in the variable {PACKAGES_VARIABLE}."
            ));
        }
        let mut known = vec![format!("{native_version} (native)")];
        known.extend(stored_versions().into_iter().filter(|stored| stored != native_version));
        message.push_str(&format!("\nThe versions of this program: {}", known.join(", ")));
        message
    };
    let script = |version: &str| -> std::result::Result<Selection, String> {
        if !cfg!(tw_js) {
            return Err(format!(
                "This build of the program has no JavaScript engine, and its compiler is Tailwind CSS v{native_version}. It cannot compile for v{version}."
            ));
        }
        find_package(version).map(|package| Selection::Script(Rc::new(package))).ok_or_else(|| missing(version))
    };
    // The Rust compiler as this version, when it can.
    let native = |version: &str| -> Option<std::result::Result<Selection, String>> {
        let parsed = Version::parse(version)?;
        if parsed == Version::NATIVE {
            return Some(Ok(Selection::Native { version: parsed, package: None }));
        }
        if parsed < Version::OLDEST || parsed > Version::NATIVE {
            return None;
        }
        // The stylesheets of that version are in its package.
        Some(match find_package(version) {
            Some(package) => Ok(Selection::Native { version: parsed, package: Some(Rc::new(package)) }),
            None => Err(missing(version)),
        })
    };

    match engine.as_str() {
        "" => native(&version).unwrap_or_else(|| script(&version)),
        "native" => native(&version).unwrap_or_else(|| {
            Err(format!(
                "The native engine has Tailwind CSS v{} to v{native_version}. Tailwind CSS v{version} needs the script engine.",
                Version::OLDEST
            ))
        }),
        "script" => script(&version),
        other => Err(format!("The variable {ENGINE_VARIABLE} must be `native` or `script`, not `{other}`.")),
    }
}

/// A compiled stylesheet of one of the engines.
pub enum AnyCompiler {
    Native(Box<Compiler>),
    #[cfg(tw_js)]
    Script(crate::script::ScriptCompiler),
}

impl AnyCompiler {
    pub fn root(&self) -> &Root {
        match self {
            AnyCompiler::Native(compiler) => &compiler.root,
            #[cfg(tw_js)]
            AnyCompiler::Script(compiler) => &compiler.root,
        }
    }

    pub fn sources(&self) -> &[Source] {
        match self {
            AnyCompiler::Native(compiler) => &compiler.sources,
            #[cfg(tw_js)]
            AnyCompiler::Script(compiler) => &compiler.sources,
        }
    }

    /// The warnings of the native compiler since the last call. The
    /// script engine prints its warnings itself (`console.warn`).
    pub fn take_warnings(&self) -> Vec<String> {
        match self {
            AnyCompiler::Native(compiler) => compiler.take_warnings(),
            #[cfg(tw_js)]
            AnyCompiler::Script(_) => Vec::new(),
        }
    }

    pub fn try_build(&mut self, candidates: &[String]) -> Result<String> {
        match self {
            AnyCompiler::Native(compiler) => compiler.try_build(candidates),
            #[cfg(tw_js)]
            AnyCompiler::Script(compiler) => compiler.try_build(candidates),
        }
    }

    pub fn build_source_map(&self) -> Result<DecodedSourceMap> {
        match self {
            AnyCompiler::Native(compiler) => Ok(compiler.build_source_map()),
            #[cfg(tw_js)]
            AnyCompiler::Script(compiler) => compiler.build_source_map(),
        }
    }
}
