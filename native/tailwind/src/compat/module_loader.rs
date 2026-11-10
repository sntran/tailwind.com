//! The file system for the modules of `@plugin` and `@config`.
//!
//! Upstream gets a `loadModule` function from its caller. Here the caller
//! gives a file system, and the compiler resolves and runs the modules
//! (`compat/js.rs`).

/// The kind of a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileKind {
    File,
    Directory,
}

/// What the compiler knows about a file.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FileStat {
    pub kind: FileKind,
    pub size: u64,
    /// The time of the last change, in milliseconds.
    pub modified_ms: f64,
}

/// The file system for the modules of `@plugin` and `@config`. The
/// compiler has no direct file access: JavaScript reads files only
/// through these functions. All paths are absolute, with `/`.
pub trait ModuleLoader {
    /// The text of a file.
    fn read(&self, path: &str) -> Option<String>;
    /// The kind of a path. `None`: the path is not there.
    fn stat(&self, path: &str) -> Option<FileStat>;
    /// The names in a directory, with their kinds.
    fn read_dir(&self, path: &str) -> Option<Vec<(String, FileKind)>>;
    /// The file of the module of a `@plugin` or `@config` rule, and the
    /// base that upstream gets for it (`customJsResolver` of
    /// `@tailwindcss/node`). `None`: the resolver of the compiler finds the
    /// file, and the base is its directory.
    fn resolve_module(&self, _id: &str, _base: &str) -> Option<(String, String)> {
        None
    }
    /// The compiler loaded this file as a module. A change of the file
    /// needs a new compiler.
    fn on_dependency(&self, _path: &str) {}
    /// The text of a call of `console`. `is_error` is true for
    /// `console.warn` and `console.error` (the standard error of Node.js),
    /// and false for `console.log` (the standard output).
    fn console(&self, _text: &str, _is_error: bool) {}
    /// The directories of `NODE_PATH`: the resolver looks for a package
    /// there after the `node_modules` directories.
    fn node_path(&self) -> Vec<String> {
        Vec::new()
    }
    /// `process.cwd()`.
    fn cwd(&self) -> String {
        "/".to_string()
    }
    /// `process.env`.
    fn env(&self) -> Vec<(String, String)> {
        Vec::new()
    }
}

/// One answer of `resolve_module`: `(id, base)` to `(path, base)`.
pub type Resolution = ((String, String), (String, String));

/// A file system in memory, for tests.
#[derive(Default)]
pub struct MemoryLoader {
    pub files: std::collections::BTreeMap<String, String>,
    pub dependencies: std::cell::RefCell<Vec<String>>,
    pub console: std::cell::RefCell<Vec<String>>,
    /// The answers of `resolve_module`: `(id, base)` to `(path, base)`.
    /// An answer is used one time, in the sequence of the list.
    pub resolutions: std::cell::RefCell<Vec<Resolution>>,
}

impl MemoryLoader {
    pub fn new(files: &[(&str, &str)]) -> Self {
        Self {
            files: files.iter().map(|(path, text)| (path.to_string(), text.to_string())).collect(),
            ..Self::default()
        }
    }
}

impl ModuleLoader for MemoryLoader {
    fn read(&self, path: &str) -> Option<String> {
        self.files.get(path).cloned()
    }

    fn stat(&self, path: &str) -> Option<FileStat> {
        if let Some(text) = self.files.get(path) {
            return Some(FileStat { kind: FileKind::File, size: text.len() as u64, modified_ms: 0.0 });
        }
        let prefix = if path == "/" { "/".to_string() } else { format!("{path}/") };
        self.files
            .keys()
            .any(|file| file.starts_with(&prefix))
            .then_some(FileStat { kind: FileKind::Directory, size: 0, modified_ms: 0.0 })
    }

    fn read_dir(&self, path: &str) -> Option<Vec<(String, FileKind)>> {
        let prefix = if path == "/" { "/".to_string() } else { format!("{path}/") };
        let mut entries: Vec<(String, FileKind)> = Vec::new();
        for file in self.files.keys() {
            let Some(rest) = file.strip_prefix(&prefix) else { continue };
            let (name, kind) = match rest.split_once('/') {
                Some((name, _)) => (name, FileKind::Directory),
                None => (rest, FileKind::File),
            };
            if !entries.iter().any(|(entry, _)| entry == name) {
                entries.push((name.to_string(), kind));
            }
        }
        (!entries.is_empty()).then_some(entries)
    }

    fn resolve_module(&self, id: &str, base: &str) -> Option<(String, String)> {
        let mut resolutions = self.resolutions.borrow_mut();
        let index = resolutions.iter().position(|((i, b), _)| i == id && b == base)?;
        Some(resolutions.remove(index).1)
    }

    fn on_dependency(&self, path: &str) {
        self.dependencies.borrow_mut().push(path.to_string());
    }

    fn console(&self, text: &str, _is_error: bool) {
        self.console.borrow_mut().push(text.to_string());
    }
}
