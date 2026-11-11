//! The script engine: the official compiler of Tailwind CSS in QuickJS.
//!
//! The npm package `tailwindcss` has its compiler in one JavaScript file
//! (`dist/lib.js`) with no dependency. This engine runs that file with no
//! change, so the output is the output of that version of Tailwind CSS.
//! The program can thus compile for a version that its Rust compiler
//! (`index.rs`, the native engine) does not follow: a version needs only
//! the files of its package, and no change of this program.
//!
//! The caller gives the package (`Package`), the loader of the
//! stylesheets and the file system of the modules, as for the native
//! engine. The scanner and the optimizer are not a part of the compiler:
//! they stay the Rust code of the program for each version.

use crate::at_import::{LoadStylesheet, Stylesheet};
use crate::compat::js::engine::{Engine, HostResult, Limits};
use crate::compat::js::platform::{self, done, field, to_error, Platform};
use crate::compat::module_loader::ModuleLoader;
use crate::index::{Root, Source};
use crate::source_maps::{DecodedMapping, DecodedSource, DecodedSourceMap, OriginalPosition, Position};
use crate::{Error, Features, Result};
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// The runtime of the engine: the loader of modules and the parts of
/// Node.js (`js/script-entry.ts`). It has no code of Tailwind CSS.
const SCRIPT_JS: &str = include_str!("../assets/script.js");

/// The files of one npm package `tailwindcss`.
pub trait Package {
    /// The version of the package.
    fn version(&self) -> &str;
    /// The text of a file of the package, by its path in the package
    /// (`package.json`, `dist/lib.js`, `theme.css`).
    fn read(&self, name: &str) -> Option<String>;
}

/// The options of `compile`.
pub struct ScriptOptions<'a> {
    /// The directory of the stylesheet, for its imports and sources.
    pub base: String,
    /// The path of the stylesheet. The official program always gives it.
    pub from: Option<String>,
    /// The loader of the stylesheets of `@import`: `(id, base)`.
    pub load_stylesheet: Option<LoadStylesheet<'a>>,
    /// The file system for the modules of `@plugin` and `@config`.
    pub module_loader: Rc<dyn ModuleLoader>,
}

/// The address of the stylesheet loader of the `compile` call that runs
/// now. The loader lives only for the time of that call.
type LoaderSlot = Cell<Option<*const Loader<'static>>>;

/// The function of the stylesheet loader.
type Loader<'a> = dyn Fn(&str, &str) -> Result<Stylesheet> + 'a;

struct Runtime {
    engine: Engine,
    loader: Rc<dyn ModuleLoader>,
    package: Rc<dyn Package>,
    stylesheets: LoaderSlot,
    /// The first error of the stylesheet loader in a `compile` call: its
    /// name must not get lost in JavaScript.
    stylesheet_error: RefCell<Option<Error>>,
    /// The `cause` chain of the exception that JavaScript throws now.
    causes: RefCell<Vec<String>>,
}

impl Runtime {
    fn new(loader: Rc<dyn ModuleLoader>, package: Rc<dyn Package>) -> Result<Rc<Self>> {
        // The engine needs the host function before the runtime is there.
        let slot: Rc<RefCell<Weak<Runtime>>> = Rc::new(RefCell::new(Weak::new()));
        let host = slot.clone();
        let engine = Engine::new(
            Limits::default(),
            Box::new(move |op, text| {
                let runtime = host.borrow().upgrade();
                match runtime {
                    Some(runtime) => runtime.host(op, text),
                    None => Err("The JavaScript engine is not ready.".to_string()),
                }
            }),
        )
        .map_err(to_error)?;
        let runtime = Rc::new(Runtime {
            engine,
            loader,
            package,
            stylesheets: Cell::new(None),
            stylesheet_error: RefCell::new(None),
            causes: RefCell::new(Vec::new()),
        });
        *slot.borrow_mut() = Rc::downgrade(&runtime);
        runtime.engine.eval(SCRIPT_JS, "tailwindcss/script.js").map_err(to_error)?;
        Ok(runtime)
    }

    /// An exception of JavaScript with its `cause` chain.
    fn error_from(&self, error: crate::compat::js::engine::JsError) -> Error {
        let mut error = to_error(error);
        error.causes = std::mem::take(&mut *self.causes.borrow_mut());
        error
    }

    fn host(&self, op: &str, text: &str) -> HostResult {
        // The loader of the engine gives the name of a module as plain text.
        if op == "module.load" {
            return self.engine.call("__tw_module_source", text).map_err(|error| error.message);
        }
        // A file of the package, or a JavaScript file of the program.
        if op == "asset" {
            return Ok(match text.strip_prefix("tailwindcss:") {
                Some(name) => self.package.read(name),
                None => platform::asset(text).map(str::to_string),
            });
        }

        let argument: Value = if text.is_empty() { Value::Null } else { serde_json::from_str(text).map_err(|error| error.to_string())? };
        let arg = &argument;

        if op == "stylesheet.load" {
            let Some(load) = self.stylesheets.get() else {
                return Err("No stylesheet loader is active.".to_string());
            };
            // SAFETY: `compile` sets the address for the time of its call
            // into JavaScript, and it holds the loader for that time.
            let load = unsafe { &*load };
            return match load(field(arg, "id")?, field(arg, "base")?) {
                Ok(sheet) => done(json!({ "path": sheet.path, "base": sheet.base, "content": sheet.content })),
                Err(error) => {
                    let message = error.message.clone();
                    self.stylesheet_error.borrow_mut().get_or_insert(error);
                    Err(message)
                }
            };
        }

        let platform = Platform { engine: &self.engine, loader: &*self.loader, causes: &self.causes, version: self.package.version() };
        platform.op(op, arg).unwrap_or_else(|| Err(format!("The host has no operation `{op}`.")))
    }
}

/// A compiled stylesheet of the script engine. `try_build` gives the CSS
/// for a set of candidates.
pub struct ScriptCompiler {
    runtime: Rc<Runtime>,
    pub sources: Vec<Source>,
    pub root: Root,
    pub features: Features,
}

/// Compile a stylesheet with the compiler of the package.
pub fn compile(css: &str, options: ScriptOptions<'_>, package: Rc<dyn Package>) -> Result<ScriptCompiler> {
    let runtime = Runtime::new(options.module_loader, package)?;

    struct Clear<'a>(&'a LoaderSlot);
    impl Drop for Clear<'_> {
        fn drop(&mut self) {
            self.0.set(None);
        }
    }
    let result = {
        let _clear = Clear(&runtime.stylesheets);
        if let Some(load) = options.load_stylesheet {
            // SAFETY: the address stays in the slot only for the time of this
            // function (`Clear`), and `load` lives longer. The change of the
            // lifetime lets the slot hold it.
            let load: *const Loader<'_> = load;
            runtime.stylesheets.set(Some(unsafe { std::mem::transmute::<*const Loader<'_>, *const Loader<'static>>(load) }));
        }

        let mut argument = json!({ "css": css, "base": options.base });
        if let Some(from) = options.from {
            argument["from"] = json!(from);
        }
        runtime.engine.call("__tw_script_compile", &argument.to_string()).map_err(|error| {
            let error = runtime.error_from(error);
            // An error of the stylesheet loader keeps its name and its text.
            match runtime.stylesheet_error.borrow_mut().take() {
                Some(original) if error.message.contains(&original.message) => original,
                _ => error,
            }
        })
    }?;
    let value: Value = serde_json::from_str(&result.unwrap_or_default()).map_err(|error| Error::new(error.to_string()))?;

    let text = |value: &Value, name: &str| value.get(name).and_then(Value::as_str).unwrap_or_default().to_string();
    let sources = value
        .get("sources")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| Source {
                    base: text(item, "base"),
                    pattern: text(item, "pattern"),
                    negated: item.get("negated").and_then(Value::as_bool).unwrap_or(false),
                })
                .collect()
        })
        .unwrap_or_default();
    let root = match value.get("root") {
        Some(Value::String(none)) if none == "none" => Root::None,
        Some(root @ Value::Object(_)) => Root::Pattern { base: text(root, "base"), pattern: text(root, "pattern") },
        _ => Root::Default,
    };
    let features = value.get("features").and_then(Value::as_u64).unwrap_or(0) as Features;

    Ok(ScriptCompiler { runtime, sources, root, features })
}

impl ScriptCompiler {
    /// The version of Tailwind CSS of the compiler.
    pub fn version(&self) -> &str {
        self.runtime.package.version()
    }

    /// The CSS for the candidates. The compiler keeps the candidates of
    /// the earlier calls, as the native engine.
    pub fn try_build(&mut self, candidates: &[String]) -> Result<String> {
        let text = serde_json::to_string(candidates).map_err(|error| Error::new(error.to_string()))?;
        let css = self.runtime.engine.call("__tw_script_build", &text).map_err(|error| self.runtime.error_from(error))?;
        Ok(css.unwrap_or_default())
    }

    /// The decoded source map of the last build (`buildSourceMap()`).
    pub fn build_source_map(&self) -> Result<DecodedSourceMap> {
        let text = self.runtime.engine.call("__tw_script_source_map", "").map_err(|error| self.runtime.error_from(error))?;
        let value: Value = serde_json::from_str(&text.unwrap_or_default()).map_err(|error| Error::new(error.to_string()))?;
        let optional = |value: Option<&Value>| value.and_then(Value::as_str).map(str::to_string);
        let number = |value: Option<&Value>| value.and_then(Value::as_i64).unwrap_or(0);

        let sources = value
            .get("sources")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|item| DecodedSource {
                        url: optional(item.get("url")),
                        content: optional(item.get("content")),
                        ignore: item.get("ignore").and_then(Value::as_bool).unwrap_or(false),
                    })
                    .collect()
            })
            .unwrap_or_default();
        let mappings = value
            .get("mappings")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .map(|item| {
                        let source = number(item.get(2));
                        DecodedMapping {
                            generated_position: Position { line: number(item.get(0)) as usize, column: number(item.get(1)) as usize },
                            original_position: (source >= 0).then(|| OriginalPosition {
                                source: source as usize,
                                line: number(item.get(3)) as usize,
                                column: number(item.get(4)) as usize,
                            }),
                            name: optional(item.get(5)),
                        }
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(DecodedSourceMap { file: optional(value.get("file")), sources, mappings })
    }
}
