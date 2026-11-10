//! The resolver of `@import`: `loadStylesheet` of `@tailwindcss/node`, and
//! the embedded files of the standalone program of Tailwind CSS.

use crate::index::Stylesheet;
use crate::{Error, Result};
use std::path::{Path, PathBuf};

/// The base of the embedded stylesheets of Tailwind CSS.
pub const EMBEDDED_BASE: &str = "/$tailwindcss";

const INDEX_CSS: &str = include_str!("../../assets/index.css");
const THEME_CSS: &str = include_str!("../../assets/theme.css");
const PREFLIGHT_CSS: &str = include_str!("../../assets/preflight.css");
const UTILITIES_CSS: &str = include_str!("../../assets/utilities.css");

/// An embedded stylesheet of Tailwind CSS, for an `@import` of
/// `tailwindcss` or of `tailwindcss/NAME`.
fn embedded(id: &str, base: &str) -> Option<Stylesheet> {
    let name = if id == "tailwindcss" {
        "index"
    } else if let Some(rest) = id.strip_prefix("tailwindcss/") {
        rest
    } else if base == EMBEDDED_BASE {
        id.strip_prefix("./").unwrap_or(id)
    } else {
        return None;
    };
    let name = name.strip_suffix(".css").unwrap_or(name);
    let content = match name {
        "index" => INDEX_CSS,
        "theme" => THEME_CSS,
        "preflight" => PREFLIGHT_CSS,
        "utilities" => UTILITIES_CSS,
        _ => return None,
    };
    Some(Stylesheet {
        path: format!("{EMBEDDED_BASE}/{name}.css"),
        base: EMBEDDED_BASE.to_string(),
        content: content.to_string(),
    })
}

/// The file of a path: the path, the path with `.css`, or `index.css` in
/// the directory.
fn as_file(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    let mut with_extension = path.as_os_str().to_owned();
    with_extension.push(".css");
    let with_extension = PathBuf::from(with_extension);
    if with_extension.is_file() {
        return Some(with_extension);
    }
    if path.is_dir() {
        if let Some(found) = package_entry(path, ".") {
            return Some(found);
        }
        let index = path.join("index.css");
        if index.is_file() {
            return Some(index);
        }
    }
    None
}

/// The target of a condition object or text of `exports` for the `style`
/// condition.
fn style_target(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(s) => Some(s.clone()),
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                if key == "style" || key == "default" {
                    if let Some(found) = style_target(value) {
                        return Some(found);
                    }
                }
            }
            None
        }
        serde_json::Value::Array(items) => items.iter().find_map(style_target),
        _ => None,
    }
}

/// The stylesheet of a package directory for a subpath (`.` or `./NAME`),
/// from `exports` or `style` of its `package.json`.
fn package_entry(dir: &Path, subpath: &str) -> Option<PathBuf> {
    let text = std::fs::read_to_string(dir.join("package.json")).ok()?;
    let json: serde_json::Value = serde_json::from_str(&text).ok()?;

    if let Some(exports) = json.get("exports") {
        let has_subpaths = exports.as_object().is_some_and(|m| m.keys().any(|k| k.starts_with('.')));
        let target = if has_subpaths {
            let map = exports.as_object()?;
            match map.get(subpath) {
                Some(value) => style_target(value),
                None => map.iter().find_map(|(pattern, value)| {
                    // A pattern with one `*`, such as `./themes/*`.
                    let (before, after) = pattern.split_once('*')?;
                    let middle = subpath.strip_prefix(before)?.strip_suffix(after)?;
                    Some(style_target(value)?.replace('*', middle))
                }),
            }
        } else if subpath == "." {
            style_target(exports)
        } else {
            None
        };
        if let Some(target) = target {
            let file = dir.join(target);
            if file.is_file() {
                return Some(file);
            }
        }
        return None;
    }

    if subpath == "." {
        if let Some(style) = json.get("style").and_then(|v| v.as_str()) {
            let file = dir.join(style);
            if file.is_file() {
                return Some(file);
            }
        }
    }
    None
}

/// The stylesheet of a package name with an optional subpath, from the
/// `node_modules` directories above `base` and from `NODE_PATH`.
fn from_packages(id: &str, base: &Path) -> Option<PathBuf> {
    // `@scope/name/sub/path` or `name/sub/path`.
    let mut parts = id.splitn(if id.starts_with('@') { 3 } else { 2 }, '/');
    let name = if id.starts_with('@') {
        format!("{}/{}", parts.next()?, parts.next()?)
    } else {
        parts.next()?.to_string()
    };
    let rest = parts.next();
    let subpath = match rest {
        Some(rest) => format!("./{rest}"),
        None => ".".to_string(),
    };

    let mut roots: Vec<PathBuf> = base.ancestors().map(|dir| dir.join("node_modules")).collect();
    if let Some(node_path) = std::env::var_os("NODE_PATH") {
        roots.extend(std::env::split_paths(&node_path));
    }

    for root in roots {
        let dir = root.join(&name);
        if !dir.is_dir() {
            continue;
        }
        if dir.join("package.json").is_file() {
            if let Some(found) = package_entry(&dir, &subpath) {
                return Some(found);
            }
            let has_exports = std::fs::read_to_string(dir.join("package.json"))
                .ok()
                .and_then(|t| serde_json::from_str::<serde_json::Value>(&t).ok())
                .is_some_and(|j| j.get("exports").is_some());
            if has_exports {
                continue;
            }
        }
        let found = match rest {
            Some(rest) => as_file(&dir.join(rest)),
            None => as_file(&dir),
        };
        if found.is_some() {
            return found;
        }
    }
    None
}

/// The path of a stylesheet for an `@import`.
pub fn resolve_css_id(id: &str, base: &str) -> Option<PathBuf> {
    let base = Path::new(base);
    let path = Path::new(id);
    if path.is_absolute() {
        return as_file(path);
    }
    if id.starts_with("./") || id.starts_with("../") || id == "." || id == ".." {
        return as_file(&base.join(path));
    }
    from_packages(id, base)
}

/// Load the stylesheet of an `@import`. `on_dependency` gets the path of
/// each file.
pub fn load_stylesheet(id: &str, base: &str, on_dependency: &mut dyn FnMut(&Path)) -> Result<Stylesheet> {
    if let Some(sheet) = embedded(id, base) {
        return Ok(sheet);
    }
    let not_found = || Error::new(format!("Could not resolve '{id}' from '{base}'"));
    let path = resolve_css_id(id, base).ok_or_else(not_found)?;
    on_dependency(&path);
    let content = std::fs::read_to_string(&path).map_err(|e| Error::new(format!("{}: {e}", path.display())))?;
    Ok(Stylesheet {
        path: path.to_string_lossy().into_owned(),
        base: path.parent().unwrap_or(Path::new("")).to_string_lossy().into_owned(),
        content,
    })
}
