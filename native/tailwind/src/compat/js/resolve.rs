//! The files of the modules of `@plugin` and `@config`: the file system
//! that the compiler gets, and the resolver of module names.
//!
//! The resolver follows Node.js and `@tailwindcss/node` (`compile.ts`,
//! which uses `enhanced-resolve`): relative paths, packages in
//! `node_modules`, and `exports` and `main` of a `package.json`.

use crate::compat::module_loader::{FileKind, ModuleLoader};
use serde_json::Value;

/// Who asks for the module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolveKind {
    /// The path of a `@plugin` or `@config` rule: the conditions of
    /// `import`, and then the conditions of `require`.
    Entry,
    /// `require(…)`.
    Require,
    /// `import`.
    Import,
}

/// The format of a module file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleFormat {
    Esm,
    CommonJs,
    Json,
    /// A `.js` file with no `type` in its package: CommonJS, unless it has
    /// the syntax of an ES module.
    Unknown,
}

impl ModuleFormat {
    pub fn name(self) -> &'static str {
        match self {
            ModuleFormat::Esm => "esm",
            ModuleFormat::CommonJs => "cjs",
            ModuleFormat::Json => "json",
            ModuleFormat::Unknown => "unknown",
        }
    }
}

const EXTENSIONS: [&str; 4] = [".js", ".json", ".cjs", ".mjs"];
const INDEX_FILES: [&str; 4] = ["index.js", "index.json", "index.cjs", "index.mjs"];

/// A path with no `.` and `..` parts.
pub fn normalize(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    format!("/{}", parts.join("/"))
}

/// `path` relative to the directory `base`.
pub fn join(base: &str, path: &str) -> String {
    if path.starts_with('/') {
        normalize(path)
    } else {
        normalize(&format!("{base}/{path}"))
    }
}

/// The directory of a path.
pub fn dirname(path: &str) -> &str {
    match path.rfind('/') {
        Some(0) | None => "/",
        Some(index) => &path[..index],
    }
}

fn is_file(fs: &dyn ModuleLoader, path: &str) -> bool {
    fs.stat(path).is_some_and(|stat| stat.kind == FileKind::File)
}

fn is_dir(fs: &dyn ModuleLoader, path: &str) -> bool {
    fs.stat(path).is_some_and(|stat| stat.kind == FileKind::Directory)
}

fn read_json(fs: &dyn ModuleLoader, path: &str) -> Option<Value> {
    serde_json::from_str(&fs.read(path)?).ok()
}

fn is_relative(name: &str) -> bool {
    name == "." || name == ".." || name.starts_with("./") || name.starts_with("../") || name.starts_with('/')
}

/// The file of the module `name`, for a module in the directory `base`.
pub fn resolve(fs: &dyn ModuleLoader, name: &str, base: &str, kind: ResolveKind) -> Result<String, String> {
    let found = match kind {
        ResolveKind::Entry => resolve_with(fs, name, base, &["node", "import"]).or_else(|| resolve_with(fs, name, base, &["node", "require"])),
        ResolveKind::Require => resolve_with(fs, name, base, &["node", "require"]),
        ResolveKind::Import => resolve_with(fs, name, base, &["node", "import"]),
    };
    let Some(path) = found else {
        return Err(match kind {
            // The message of `enhanced-resolve`.
            ResolveKind::Entry => format!("Can't resolve '{name}' in '{base}'"),
            // The first line of the message of Node.js.
            _ => format!("Cannot find module '{name}'"),
        });
    };
    if [".ts", ".mts", ".cts", ".tsx"].iter().any(|extension| path.ends_with(extension)) {
        return Err(format!(
            "Cannot load '{path}': TypeScript modules are not supported. Use a JavaScript file (.js, .mjs or .cjs)."
        ));
    }
    Ok(path)
}

fn resolve_with(fs: &dyn ModuleLoader, name: &str, base: &str, conditions: &[&str]) -> Option<String> {
    if is_relative(name) {
        return resolve_path(fs, &join(base, name), conditions);
    }
    if name.starts_with('#') {
        return None;
    }

    // `package` or `@scope/package`, and the path in the package.
    let mut parts = name.splitn(if name.starts_with('@') { 3 } else { 2 }, '/');
    let package = if name.starts_with('@') {
        format!("{}/{}", parts.next()?, parts.next()?)
    } else {
        parts.next()?.to_string()
    };
    let subpath = parts.next().unwrap_or("");

    let mut directory = normalize(base);
    loop {
        if !directory.ends_with("/node_modules") {
            let root = join(&directory, &format!("node_modules/{package}"));
            if is_dir(fs, &root) {
                if let Some(found) = resolve_in_package(fs, &root, subpath, conditions) {
                    return Some(found);
                }
            }
        }
        if directory == "/" {
            break;
        }
        directory = dirname(&directory).to_string();
    }

    for directory in fs.node_path() {
        let root = join(&directory, &package);
        if is_dir(fs, &root) {
            if let Some(found) = resolve_in_package(fs, &root, subpath, conditions) {
                return Some(found);
            }
        }
    }
    None
}

fn resolve_in_package(fs: &dyn ModuleLoader, root: &str, subpath: &str, conditions: &[&str]) -> Option<String> {
    let manifest = read_json(fs, &format!("{root}/package.json"));
    if let Some(exports) = manifest.as_ref().and_then(|manifest| manifest.get("exports")) {
        let key = if subpath.is_empty() { ".".to_string() } else { format!("./{subpath}") };
        let target = resolve_exports(exports, &key, conditions)?;
        let path = join(root, &target);
        return is_file(fs, &path).then_some(path);
    }
    if subpath.is_empty() {
        resolve_directory(fs, root, conditions)
    } else {
        resolve_path(fs, &format!("{root}/{subpath}"), conditions)
    }
}

/// A file, a file with an extension, or a directory.
fn resolve_path(fs: &dyn ModuleLoader, path: &str, conditions: &[&str]) -> Option<String> {
    if is_file(fs, path) {
        return Some(path.to_string());
    }
    for extension in EXTENSIONS.iter().chain([".ts"].iter()) {
        let file = format!("{path}{extension}");
        if is_file(fs, &file) {
            return Some(file);
        }
    }
    if is_dir(fs, path) {
        return resolve_directory(fs, path, conditions);
    }
    None
}

/// The main file of a directory: `main` of its `package.json` (then
/// `module`), or an index file.
fn resolve_directory(fs: &dyn ModuleLoader, directory: &str, _conditions: &[&str]) -> Option<String> {
    if let Some(manifest) = read_json(fs, &format!("{directory}/package.json")) {
        for field in ["main", "module"] {
            let Some(main) = manifest.get(field).and_then(Value::as_str) else { continue };
            let path = join(directory, main);
            if is_file(fs, &path) {
                return Some(path);
            }
            for extension in EXTENSIONS {
                let file = format!("{path}{extension}");
                if is_file(fs, &file) {
                    return Some(file);
                }
            }
            for index in INDEX_FILES {
                let file = format!("{path}/{index}");
                if is_file(fs, &file) {
                    return Some(file);
                }
            }
        }
    }
    INDEX_FILES.iter().map(|index| format!("{directory}/{index}")).find(|file| is_file(fs, file))
}

/// The target of `key` (`.` or `./path`) in the `exports` of a package.
fn resolve_exports(exports: &Value, key: &str, conditions: &[&str]) -> Option<String> {
    let has_paths = matches!(exports, Value::Object(map) if map.keys().any(|k| k.starts_with('.')));
    if !has_paths {
        return if key == "." { resolve_target(exports, "", conditions) } else { None };
    }
    let Value::Object(map) = exports else { return None };
    if let Some(target) = map.get(key) {
        return resolve_target(target, "", conditions);
    }

    // A pattern with one `*`. The longest prefix wins.
    let mut best: Option<(&str, &str, &Value)> = None;
    for (pattern, target) in map {
        let Some((prefix, suffix)) = pattern.split_once('*') else {
            // `./dir/`: an old form of `./dir/*`.
            if pattern.ends_with('/') && key.starts_with(pattern.as_str()) && best.is_none_or(|(p, _, _)| pattern.len() > p.len()) {
                best = Some((pattern, "", target));
            }
            continue;
        };
        if key.len() >= prefix.len() + suffix.len()
            && key.starts_with(prefix)
            && key.ends_with(suffix)
            && best.is_none_or(|(p, _, _)| prefix.len() > p.len())
        {
            best = Some((prefix, suffix, target));
        }
    }
    let (prefix, suffix, target) = best?;
    let middle = &key[prefix.len()..key.len() - suffix.len()];
    let resolved = resolve_target(target, middle, conditions)?;
    // The old form `./dir/` adds the rest of the path.
    if prefix.ends_with('/') && suffix.is_empty() && !target_has_star(target) {
        return Some(format!("{resolved}{middle}"));
    }
    Some(resolved)
}

fn target_has_star(target: &Value) -> bool {
    match target {
        Value::String(text) => text.contains('*'),
        Value::Array(items) => items.iter().any(target_has_star),
        Value::Object(map) => map.values().any(target_has_star),
        _ => false,
    }
}

fn resolve_target(target: &Value, star: &str, conditions: &[&str]) -> Option<String> {
    match target {
        Value::String(text) => text.starts_with("./").then(|| text.replace('*', star)),
        Value::Array(items) => items.iter().find_map(|item| resolve_target(item, star, conditions)),
        Value::Object(map) => map
            .iter()
            .filter(|(condition, _)| *condition == "default" || conditions.contains(&condition.as_str()))
            .find_map(|(_, value)| resolve_target(value, star, conditions)),
        _ => None,
    }
}

/// The format of a module file: its extension, or the `type` of the
/// nearest `package.json`.
pub fn module_format(fs: &dyn ModuleLoader, path: &str) -> ModuleFormat {
    if path.ends_with(".mjs") {
        return ModuleFormat::Esm;
    }
    if path.ends_with(".cjs") {
        return ModuleFormat::CommonJs;
    }
    if path.ends_with(".json") {
        return ModuleFormat::Json;
    }
    let mut directory = dirname(path).to_string();
    loop {
        if let Some(manifest) = read_json(fs, &format!("{}/package.json", directory.trim_end_matches('/'))) {
            return match manifest.get("type").and_then(Value::as_str) {
                Some("module") => ModuleFormat::Esm,
                Some("commonjs") => ModuleFormat::CommonJs,
                _ => ModuleFormat::Unknown,
            };
        }
        if directory == "/" || directory.ends_with("/node_modules") {
            return ModuleFormat::Unknown;
        }
        directory = dirname(&directory).to_string();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compat::module_loader::MemoryLoader;

    fn fs() -> MemoryLoader {
        MemoryLoader::new(&[
            ("/app/css/plugin.js", ""),
            ("/app/css/data.json", "{}"),
            ("/app/css/esm.mjs", ""),
            ("/app/css/lib/index.js", ""),
            ("/app/css/tailwind.config.ts", ""),
            ("/app/vendor/pkgdir/package.json", r#"{"main": "./lib/main"}"#),
            ("/app/vendor/pkgdir/lib/main.js", ""),
            ("/app/node_modules/simple/index.js", ""),
            ("/app/node_modules/with-main/package.json", r#"{"main": "dist/cjs.js", "module": "dist/esm.js"}"#),
            ("/app/node_modules/with-main/dist/cjs.js", ""),
            ("/app/node_modules/with-main/dist/esm.js", ""),
            ("/app/node_modules/with-main/other.js", ""),
            ("/app/node_modules/only-module/package.json", r#"{"module": "esm.js"}"#),
            ("/app/node_modules/only-module/esm.js", ""),
            (
                "/app/node_modules/dual/package.json",
                r#"{"exports": {".": {"import": "./esm.mjs", "require": "./cjs.cjs"}, "./theme": {"default": "./theme.js"},
                    "./features/*": "./src/features/*.js", "./package.json": "./package.json"}}"#,
            ),
            ("/app/node_modules/dual/esm.mjs", ""),
            ("/app/node_modules/dual/cjs.cjs", ""),
            ("/app/node_modules/dual/theme.js", ""),
            ("/app/node_modules/dual/hidden.js", ""),
            ("/app/node_modules/dual/src/features/a.js", ""),
            ("/app/node_modules/@scope/pkg/package.json", r#"{"exports": "./main.js", "type": "module"}"#),
            ("/app/node_modules/@scope/pkg/main.js", ""),
            ("/app/node_modules/nested/node_modules/simple/index.js", ""),
            ("/app/node_modules/nested/index.js", ""),
            ("/app/node_modules/order/package.json", r#"{"exports": {"default": "./default.js", "import": "./import.js"}}"#),
            ("/app/node_modules/order/default.js", ""),
            ("/app/node_modules/order/import.js", ""),
            ("/node_modules/root-package/index.js", ""),
        ])
    }

    fn entry(name: &str) -> Result<String, String> {
        resolve(&fs(), name, "/app/css", ResolveKind::Entry)
    }

    #[test]
    fn resolves_relative_paths() {
        assert_eq!(entry("./plugin.js").unwrap(), "/app/css/plugin.js");
        assert_eq!(entry("./plugin").unwrap(), "/app/css/plugin.js");
        assert_eq!(entry("./data").unwrap(), "/app/css/data.json");
        assert_eq!(entry("./esm").unwrap(), "/app/css/esm.mjs");
        assert_eq!(entry("./lib").unwrap(), "/app/css/lib/index.js");
        assert_eq!(entry("../vendor/pkgdir").unwrap(), "/app/vendor/pkgdir/lib/main.js");
        assert_eq!(entry("/app/css/plugin.js").unwrap(), "/app/css/plugin.js");
        assert_eq!(entry("./missing").unwrap_err(), "Can't resolve './missing' in '/app/css'");
    }

    #[test]
    fn resolves_packages() {
        assert_eq!(entry("simple").unwrap(), "/app/node_modules/simple/index.js");
        assert_eq!(entry("with-main").unwrap(), "/app/node_modules/with-main/dist/cjs.js");
        assert_eq!(entry("with-main/other").unwrap(), "/app/node_modules/with-main/other.js");
        assert_eq!(entry("only-module").unwrap(), "/app/node_modules/only-module/esm.js");
        assert_eq!(entry("@scope/pkg").unwrap(), "/app/node_modules/@scope/pkg/main.js");
        assert_eq!(entry("root-package").unwrap(), "/node_modules/root-package/index.js");
        assert_eq!(entry("missing").unwrap_err(), "Can't resolve 'missing' in '/app/css'");
        // The nearest node_modules wins.
        assert_eq!(
            resolve(&fs(), "simple", "/app/node_modules/nested", ResolveKind::Require).unwrap(),
            "/app/node_modules/nested/node_modules/simple/index.js"
        );
    }

    #[test]
    fn resolves_the_exports_of_a_package() {
        let fs = fs();
        assert_eq!(entry("dual").unwrap(), "/app/node_modules/dual/esm.mjs");
        assert_eq!(resolve(&fs, "dual", "/app", ResolveKind::Require).unwrap(), "/app/node_modules/dual/cjs.cjs");
        assert_eq!(resolve(&fs, "dual", "/app", ResolveKind::Import).unwrap(), "/app/node_modules/dual/esm.mjs");
        assert_eq!(entry("dual/theme").unwrap(), "/app/node_modules/dual/theme.js");
        assert_eq!(entry("dual/features/a").unwrap(), "/app/node_modules/dual/src/features/a.js");
        assert_eq!(entry("dual/package.json").unwrap(), "/app/node_modules/dual/package.json");
        // A file that `exports` does not name is private.
        assert!(entry("dual/hidden.js").is_err());
        // The sequence of the conditions in the file has a meaning.
        assert_eq!(resolve(&fs, "order", "/app", ResolveKind::Import).unwrap(), "/app/node_modules/order/default.js");
    }

    #[test]
    fn refuses_typescript() {
        let error = entry("./tailwind.config.ts").unwrap_err();
        assert!(error.contains("TypeScript modules are not supported"), "{error}");
        assert!(entry("./tailwind.config").unwrap_err().contains("TypeScript"));
    }

    #[test]
    fn gives_the_format_of_a_file() {
        let fs = fs();
        assert_eq!(module_format(&fs, "/app/css/esm.mjs"), ModuleFormat::Esm);
        assert_eq!(module_format(&fs, "/app/node_modules/dual/cjs.cjs"), ModuleFormat::CommonJs);
        assert_eq!(module_format(&fs, "/app/css/data.json"), ModuleFormat::Json);
        assert_eq!(module_format(&fs, "/app/css/plugin.js"), ModuleFormat::Unknown);
        assert_eq!(module_format(&fs, "/app/node_modules/@scope/pkg/main.js"), ModuleFormat::Esm);
        assert_eq!(module_format(&fs, "/app/node_modules/with-main/dist/cjs.js"), ModuleFormat::Unknown);
    }

    #[test]
    fn normalizes_paths() {
        assert_eq!(normalize("/a/./b/../c//d/"), "/a/c/d");
        assert_eq!(join("/a/b", "../c"), "/a/c");
        assert_eq!(join("/a/b", "/x"), "/x");
        assert_eq!(dirname("/a/b.js"), "/a");
        assert_eq!(dirname("/b.js"), "/");
    }
}
