//! The host operations that each JavaScript engine of the program needs:
//! the modules, the files, the console and the process. The compatibility
//! runtime (`runtime.rs`) and the script engine (`script.rs`) use them.

use super::engine::{Engine, HostResult, JsError};
use super::resolve::{self, ResolveKind};
use crate::compat::module_loader::{FileKind, ModuleLoader};
use crate::Error;
use serde_json::{json, Value};
use std::cell::RefCell;

/// The TypeScript transform (Sucrase). The engine evaluates it at the
/// first TypeScript module (see `js/typescript.ts`).
const TYPESCRIPT_JS: &str = include_str!("../../../assets/typescript.js");

/// The plugins that the standalone program of Tailwind CSS has in its
/// file, each one as one CommonJS file (see `js/modules.ts`).
const PLUGINS: [(&str, &str); 3] = [
    ("@tailwindcss/forms", include_str!("../../../assets/plugins/forms.js")),
    ("@tailwindcss/typography", include_str!("../../../assets/plugins/typography.js")),
    ("@tailwindcss/aspect-ratio", include_str!("../../../assets/plugins/aspect-ratio.js")),
];

/// A JavaScript file of the program, by its name.
pub(crate) fn asset(name: &str) -> Option<&'static str> {
    if name == "typescript" {
        return Some(TYPESCRIPT_JS);
    }
    let plugin = name.strip_prefix("plugin:")?;
    PLUGINS.iter().find(|(id, _)| *id == plugin).map(|(_, source)| *source)
}

/// Base64 text of bytes: the bytes of a file go to JavaScript in this form.
pub(crate) fn base64(bytes: &[u8]) -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b = [chunk[0], chunk.get(1).copied().unwrap_or(0), chunk.get(2).copied().unwrap_or(0)];
        text.push(TABLE[(b[0] >> 2) as usize] as char);
        text.push(TABLE[(((b[0] & 3) << 4) | (b[1] >> 4)) as usize] as char);
        text.push(if chunk.len() > 1 { TABLE[(((b[1] & 15) << 2) | (b[2] >> 6)) as usize] as char } else { '=' });
        text.push(if chunk.len() > 2 { TABLE[(b[2] & 63) as usize] as char } else { '=' });
    }
    text
}

pub(crate) fn to_error(error: JsError) -> Error {
    Error {
        message: error.message,
        name: error.name.map_or(std::borrow::Cow::Borrowed("Error"), std::borrow::Cow::Owned),
        causes: Vec::new(),
    }
}

pub(crate) fn text_arg(value: &Value) -> std::result::Result<&str, String> {
    value.as_str().ok_or_else(|| format!("The host operation needs text, not {value}."))
}

pub(crate) fn field<'a>(value: &'a Value, name: &str) -> std::result::Result<&'a str, String> {
    value.get(name).and_then(Value::as_str).ok_or_else(|| format!("The host operation needs the text field `{name}`."))
}

pub(crate) fn text_list(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items.iter().filter_map(|item| item.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

pub(crate) fn done(value: Value) -> HostResult {
    Ok(Some(value.to_string()))
}

/// The names of the shared host operations.
const OPERATIONS: [&str; 13] = [
    "module.resolve",
    "module.read",
    "fs.readFile",
    "fs.readFileBytes",
    "fs.stat",
    "fs.lstat",
    "fs.realpath",
    "fs.readdir",
    "console",
    "process",
    "error.causes",
    "cwd",
    "version",
];

/// The engine and the file system of one runtime, for the shared host
/// operations.
pub(crate) struct Platform<'a> {
    pub engine: &'a Engine,
    pub loader: &'a dyn ModuleLoader,
    /// The `cause` chain of the exception that JavaScript throws now.
    pub causes: &'a RefCell<Vec<String>>,
    /// The version of Tailwind CSS that JavaScript sees.
    pub version: &'a str,
}

impl Platform<'_> {
    /// One shared host operation. `None`: this file has no such operation.
    pub fn op(&self, op: &str, arg: &Value) -> Option<HostResult> {
        OPERATIONS.contains(&op).then(|| self.run(op, arg))
    }

    fn run(&self, op: &str, arg: &Value) -> HostResult {
        match op {
            "module.resolve" => {
                let name = field(arg, "name")?;
                let base = field(arg, "base")?;
                let kind = match arg.get("kind").and_then(Value::as_str) {
                    Some("entry") => ResolveKind::Entry,
                    Some("require") => ResolveKind::Require,
                    _ => ResolveKind::Import,
                };
                // The engine asks for an `import`, and gets plain text.
                if kind == ResolveKind::Import {
                    // The bundle knows the modules of the program.
                    if self.engine.call("__tw_is_builtin", name).map_err(|error| error.message)?.is_some() {
                        return Ok(Some(name.to_string()));
                    }
                    let directory = if base.starts_with('/') { resolve::dirname(base).to_string() } else { self.loader.cwd() };
                    return resolve::resolve(self.loader, name, &directory, kind).map(Some);
                }
                if kind == ResolveKind::Entry {
                    if let Some((path, base)) = self.loader.resolve_module(name, base) {
                        return done(json!({ "path": path, "base": base }));
                    }
                    let path = resolve::resolve(self.loader, name, base, kind)?;
                    let base = resolve::dirname(&path).to_string();
                    return done(json!({ "path": path, "base": base }));
                }
                // The base of `require` is a file.
                resolve::resolve(self.loader, name, resolve::dirname(base), kind).and_then(|path| done(json!(path)))
            }
            "module.read" => {
                let path = text_arg(arg)?;
                // The caller gets the path before the read: it records the
                // state of the file from before the read.
                if !path.contains("/node_modules/") {
                    self.loader.on_dependency(path);
                }
                let Some(source) = self.loader.read(path) else {
                    return Err(format!("Cannot read the module '{path}'"));
                };
                done(json!({ "format": resolve::module_format(self.loader, path).name(), "source": source }))
            }
            "fs.readFile" => match self.loader.read(text_arg(arg)?) {
                Some(text) => done(json!(text)),
                None => Ok(None),
            },
            "fs.readFileBytes" => match self.loader.read_bytes(text_arg(arg)?) {
                Some(bytes) => done(json!(base64(&bytes))),
                None => Ok(None),
            },
            "fs.stat" | "fs.lstat" => {
                let path = text_arg(arg)?;
                let link = op == "fs.lstat" && self.loader.is_symlink(path);
                match self.loader.stat(path) {
                    Some(stat) => done(json!({
                        "kind": if stat.kind == FileKind::File { "file" } else { "directory" },
                        "size": stat.size,
                        "mtimeMs": stat.modified_ms,
                        "link": link,
                    })),
                    // A link with no target is there for `lstat`.
                    None if link => done(json!({ "kind": "file", "size": 0, "mtimeMs": 0, "link": true })),
                    None => Ok(None),
                }
            }
            "fs.realpath" => match self.loader.real_path(text_arg(arg)?) {
                Some(path) => done(json!(path)),
                None => Ok(None),
            },
            "fs.readdir" => match self.loader.read_dir(text_arg(arg)?) {
                Some(entries) => done(Value::Array(
                    entries
                        .into_iter()
                        .map(|(name, kind)| json!([name, if kind == FileKind::File { "file" } else { "directory" }]))
                        .collect(),
                )),
                None => Ok(None),
            },
            "console" => {
                self.loader.console(field(arg, "text")?, arg.get("error").and_then(Value::as_bool).unwrap_or(false));
                Ok(None)
            }
            "process" => {
                let env: serde_json::Map<String, Value> = self.loader.env().into_iter().map(|(name, value)| (name, Value::String(value))).collect();
                let arch = match std::env::consts::ARCH {
                    "x86_64" => "x64",
                    "aarch64" => "arm64",
                    "x86" => "ia32",
                    other => other,
                };
                done(json!({
                    "env": env,
                    "argv": self.loader.argv(),
                    "platform": self.loader.platform(),
                    "arch": arch,
                    // `node` is the version of Node.js whose API the
                    // modules of the program follow.
                    "versions": { "node": "22.0.0", "tailwindcss": self.version },
                }))
            }
            "error.causes" => {
                *self.causes.borrow_mut() = text_list(Some(arg));
                Ok(None)
            }
            "cwd" => done(json!(self.loader.cwd())),
            "version" => done(json!(self.version)),

            _ => Err(format!("The host has no operation `{op}`.")),
        }
    }
}
