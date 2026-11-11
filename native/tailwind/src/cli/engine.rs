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
//! - `TAILWIND_COM_ENGINE`: `native` or `script`. With no value, the
//!   version of the native engine uses the native engine, and each other
//!   version uses the script engine.
//! - `TAILWIND_COM_PACKAGES`: a directory with one directory for each
//!   version (`4.2.0/package.json`, `4.2.0/dist/lib.js`, ...): the files
//!   of the npm package.
//!
//! The program looks for a package in `TAILWIND_COM_PACKAGES`, and then
//! in its own file: `tailwind.com` is also a ZIP file, and the packages
//! are in its directory `tailwindcss/`.

use crate::index::{Compiler, Root, Source};
use crate::source_maps::DecodedSourceMap;
use crate::Result;
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

/// An npm package `tailwindcss` in a directory.
pub struct DirectoryPackage {
    directory: PathBuf,
    version: String,
}

impl DirectoryPackage {
    /// The package of a directory, when it is the package `tailwindcss`
    /// of this version.
    fn open(directory: PathBuf, version: &str) -> Option<Self> {
        let text = std::fs::read_to_string(directory.join("package.json")).ok()?;
        let json: serde_json::Value = serde_json::from_str(&text).ok()?;
        let name = json.get("name").and_then(|value| value.as_str())?;
        let found = json.get("version").and_then(|value| value.as_str())?;
        (name == "tailwindcss" && found == version).then(|| Self { directory, version: version.to_string() })
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
        std::fs::read_to_string(self.directory.join(name)).ok()
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

/// The directories that can have the package of a version.
fn package_directories(version: &str) -> Vec<PathBuf> {
    let mut directories = Vec::new();
    if let Some(packages) = std::env::var_os(PACKAGES_VARIABLE).filter(|value| !value.is_empty()) {
        directories.push(PathBuf::from(packages).join(version));
    }
    directories.push(PathBuf::from(ZIP_PACKAGES).join(version));
    directories
}

/// The package of a version, from the store.
fn find_package(version: &str) -> Option<DirectoryPackage> {
    package_directories(version).into_iter().find_map(|directory| DirectoryPackage::open(directory, version))
}

/// The versions that are in the store, sorted.
pub fn stored_versions() -> Vec<String> {
    let mut roots = Vec::new();
    if let Some(packages) = std::env::var_os(PACKAGES_VARIABLE).filter(|value| !value.is_empty()) {
        roots.push(PathBuf::from(packages));
    }
    roots.push(PathBuf::from(ZIP_PACKAGES));
    let mut versions: Vec<String> = Vec::new();
    for root in roots {
        let Ok(entries) = std::fs::read_dir(&root) else { continue };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !versions.contains(&name) && DirectoryPackage::open(root.join(&name), &name).is_some() {
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
    /// The Rust compiler of the program.
    Native,
    /// The official compiler of this package.
    Script(Rc<DirectoryPackage>),
}

impl Selection {
    /// The version of Tailwind CSS.
    pub fn version(&self) -> &str {
        match self {
            Selection::Native => crate::index::VERSION,
            Selection::Script(package) => &package.version,
        }
    }

    /// The name of the engine.
    pub fn engine(&self) -> &'static str {
        match self {
            Selection::Native => "native",
            Selection::Script(_) => "script",
        }
    }

    /// The package of the script engine.
    pub fn package(&self) -> Option<&DirectoryPackage> {
        match self {
            Selection::Native => None,
            Selection::Script(package) => Some(package),
        }
    }
}

/// The version that the environment asks for.
pub fn requested_version() -> String {
    match std::env::var(VERSION_VARIABLE) {
        Ok(version) if !version.trim().is_empty() => version.trim().trim_start_matches('v').to_string(),
        _ => crate::index::VERSION.to_string(),
    }
}

/// The engine and the version that the environment asks for.
pub fn select() -> std::result::Result<Selection, String> {
    let version = requested_version();
    let engine = std::env::var(ENGINE_VARIABLE).unwrap_or_default();
    let native_version = crate::index::VERSION;

    let script = |version: &str| -> std::result::Result<Selection, String> {
        if !cfg!(tw_js) {
            return Err(format!(
                "This build of the program has no JavaScript engine, and its compiler is Tailwind CSS v{native_version}. It cannot compile for v{version}."
            ));
        }
        match find_package(version) {
            Some(package) => Ok(Selection::Script(Rc::new(package))),
            None => {
                let stored = stored_versions();
                let mut message = format!("Tailwind CSS v{version} is not in this program.");
                if version.split('.').next() != Some("4") {
                    message.push_str(" The program is for Tailwind CSS 4.");
                } else {
                    message.push_str(&format!(
                        " Put the files of the npm package tailwindcss@{version} in the directory `{version}` of a directory, and give that directory in the variable {PACKAGES_VARIABLE}."
                    ));
                }
                let mut known = vec![format!("{native_version} (native)")];
                known.extend(stored.into_iter().filter(|stored| stored != native_version));
                message.push_str(&format!("\nThe versions of this program: {}", known.join(", ")));
                Err(message)
            }
        }
    };

    match engine.as_str() {
        "" => {
            if version == native_version {
                Ok(Selection::Native)
            } else {
                script(&version)
            }
        }
        "native" => {
            if version == native_version {
                Ok(Selection::Native)
            } else {
                Err(format!("The native engine is Tailwind CSS v{native_version}. Tailwind CSS v{version} needs the script engine."))
            }
        }
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
