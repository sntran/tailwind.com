//! The engine with the compatibility bundle, and the host operations.
//!
//! JavaScript calls `__host(op, json)`. The operations are the methods of
//! the theme, the utilities and the variants that `compat/` of Tailwind
//! CSS uses, `@apply` and the CSS functions on a tree, and the files of
//! the modules.
//!
//! A function of a plugin stays in JavaScript. The bundle gives it a
//! number, and the Rust closure of the utility or variant calls the bundle
//! with this number.
//!
//! The design system is not owned by this runtime. Each call into
//! JavaScript puts the address of the design system on a stack for the
//! time of the call, and the host operations read it there.

use super::engine::{Engine, HostResult, JsError, Limits};
use super::json::{
    ast_from_json, ast_to_json, LocTable, functional_candidate_to_json, node_from_json, node_to_json, static_candidate_to_json,
    variant_from_json, variant_to_json,
};
use super::resolve::{self, ResolveKind};
use crate::apply::substitute_at_apply;
use crate::ast::AstNode;
use crate::candidate::Variant;
use crate::compat::module_loader::{FileKind, ModuleLoader};
use crate::css_functions::substitute_functions;
use crate::design_system::DesignSystem;
use crate::index::Source;
use crate::utilities::{Compiled, UtilityKind, UtilityOptions};
use crate::variants::{CompareFn, VariantFn, VariantKind, VariantOptions};
use crate::{Error, Features, Result};
use serde_json::{json, Value};
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};

/// The compatibility bundle.
const COMPAT_JS: &str = include_str!("../../../assets/compat.js");

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
fn asset(name: &str) -> Option<&'static str> {
    if name == "typescript" {
        return Some(TYPESCRIPT_JS);
    }
    let plugin = name.strip_prefix("plugin:")?;
    PLUGINS.iter().find(|(id, _)| *id == plugin).map(|(_, source)| *source)
}

/// Base64 text of bytes: the bytes of a file go to JavaScript in this form.
fn base64(bytes: &[u8]) -> String {
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

/// The design system of a call into JavaScript.
#[derive(Clone, Copy)]
enum Access {
    /// `apply_compatibility_hooks`: the plugins register utilities and
    /// variants.
    Setup(*mut DesignSystem),
    /// A build: a utility or a variant of a plugin runs.
    Build(*const DesignSystem),
}

/// The JavaScript of one design system.
pub struct JsRuntime {
    engine: Engine,
    loader: Rc<dyn ModuleLoader>,
    this: Weak<JsRuntime>,
    scopes: RefCell<Vec<Access>>,
    /// The number of shared references to the design system that host
    /// operations hold now.
    shared: Cell<usize>,
    /// True while a host operation holds the exclusive reference.
    exclusive: Cell<bool>,
    /// The functions of variants that JavaScript keeps (`variants.get`).
    stash: RefCell<Vec<VariantFn>>,
    /// The first exception of a plugin function in a build.
    error: RefCell<Option<Error>>,
    /// The `cause` chain of the exception that JavaScript throws now.
    causes: RefCell<Vec<String>>,
    /// The source locations of the nodes that are in JavaScript.
    locs: LocTable,
}

/// Sets a flag or a counter back at the end of a host operation.
struct Reset<'a, T: Copy>(&'a Cell<T>, T);

impl<T: Copy> Drop for Reset<'_, T> {
    fn drop(&mut self) {
        self.0.set(self.1);
    }
}

fn to_error(error: JsError) -> Error {
    Error {
        message: error.message,
        name: error.name.map_or(std::borrow::Cow::Borrowed("Error"), std::borrow::Cow::Owned),
        causes: Vec::new(),
    }
}

fn text_arg(value: &Value) -> std::result::Result<&str, String> {
    value.as_str().ok_or_else(|| format!("The host operation needs text, not {value}."))
}

fn field<'a>(value: &'a Value, name: &str) -> std::result::Result<&'a str, String> {
    value.get(name).and_then(Value::as_str).ok_or_else(|| format!("The host operation needs the text field `{name}`."))
}

fn text_list(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::Array(items)) => items.iter().filter_map(|item| item.as_str().map(str::to_string)).collect(),
        _ => Vec::new(),
    }
}

fn number(value: Option<&Value>) -> Option<i64> {
    let value = value?;
    value.as_i64().or_else(|| value.as_f64().map(|n| n as i64))
}

fn done(value: Value) -> HostResult {
    Ok(Some(value.to_string()))
}

fn kind_name(kind: VariantKind) -> &'static str {
    match kind {
        VariantKind::Arbitrary => "arbitrary",
        VariantKind::Static => "static",
        VariantKind::Functional => "functional",
        VariantKind::Compound => "compound",
    }
}

impl JsRuntime {
    /// A new engine with the compatibility bundle.
    pub fn new(loader: Rc<dyn ModuleLoader>) -> Result<Rc<Self>> {
        // The engine needs the host function before the runtime is there.
        let slot: Rc<RefCell<Weak<JsRuntime>>> = Rc::new(RefCell::new(Weak::new()));
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
        let runtime = Rc::new_cyclic(|this: &Weak<JsRuntime>| JsRuntime {
            engine,
            loader,
            this: this.clone(),
            scopes: RefCell::new(Vec::new()),
            shared: Cell::new(0),
            exclusive: Cell::new(false),
            stash: RefCell::new(Vec::new()),
            error: RefCell::new(None),
            causes: RefCell::new(Vec::new()),
            locs: LocTable::default(),
        });
        *slot.borrow_mut() = Rc::downgrade(&runtime);
        runtime.engine.eval(COMPAT_JS, "tailwindcss/compat.js").map_err(to_error)?;
        Ok(runtime)
    }

    /// The engine, for tests.
    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    /// Take the first exception of a plugin function in a build.
    pub fn take_error(&self) -> Option<Error> {
        self.error.borrow_mut().take()
    }

    fn set_error(&self, message: String) {
        let mut slot = self.error.borrow_mut();
        if slot.is_none() {
            *slot = Some(Error::new(message));
        }
    }

    /// Run `f` with a design system on the stack.
    fn scope<R>(&self, access: Access, f: impl FnOnce() -> R) -> R {
        struct Pop<'a>(&'a RefCell<Vec<Access>>);
        impl Drop for Pop<'_> {
            fn drop(&mut self) {
                self.0.borrow_mut().pop();
            }
        }
        self.scopes.borrow_mut().push(access);
        let _pop = Pop(&self.scopes);
        f()
    }

    /// An exception of JavaScript with its `cause` chain (`error.causes`).
    fn error_from(&self, error: JsError) -> Error {
        let mut error = to_error(error);
        error.causes = std::mem::take(&mut *self.causes.borrow_mut());
        error
    }

    /// Call a function of the bundle in a build.
    fn call(&self, design_system: &DesignSystem, name: &str, argument: &Value) -> std::result::Result<Option<String>, String> {
        self.scope(Access::Build(design_system), || self.engine.call(name, &argument.to_string())).map_err(|error| {
            // Keep the first exception with its name, for `take_error`.
            let message = error.message.clone();
            let mut slot = self.error.borrow_mut();
            if slot.is_none() {
                *slot = Some(self.error_from(error));
            }
            message
        })
    }

    /// A shared reference to the design system, for one host operation.
    fn with<R>(&self, f: impl FnOnce(&DesignSystem) -> std::result::Result<R, String>) -> std::result::Result<R, String> {
        let Some(access) = self.scopes.borrow().last().copied() else {
            return Err("No design system is active.".to_string());
        };
        if self.exclusive.get() {
            return Err("The design system changes now.".to_string());
        }
        let _reset = Reset(&self.shared, self.shared.get());
        self.shared.set(self.shared.get() + 1);
        // SAFETY: `scope` keeps the address on the stack only for the time
        // of a call whose caller holds a reference to the design system,
        // so the address is valid. No exclusive reference is live: the
        // check above refuses this operation while `with_mut` runs, and
        // `with_mut` refuses to start while `shared` is not 0.
        let design_system: &DesignSystem = unsafe {
            match access {
                Access::Setup(pointer) => &*pointer,
                Access::Build(pointer) => &*pointer,
            }
        };
        f(design_system)
    }

    /// The exclusive reference to the design system, for one host
    /// operation of the setup. `f` must not call JavaScript.
    fn with_mut<R>(&self, f: impl FnOnce(&mut DesignSystem) -> std::result::Result<R, String>) -> std::result::Result<R, String> {
        let Some(Access::Setup(pointer)) = self.scopes.borrow().last().copied() else {
            return Err("A plugin cannot change the design system during a build.".to_string());
        };
        if self.exclusive.get() || self.shared.get() > 0 {
            return Err("A plugin cannot change the design system now.".to_string());
        }
        let _reset = Reset(&self.exclusive, false);
        self.exclusive.set(true);
        // SAFETY: the address comes from the `&mut DesignSystem` of
        // `apply_compatibility_hooks`, which does not use its reference
        // while JavaScript runs. No other reference is live: `shared` is
        // 0, and `exclusive` refuses each other operation until `f` ends.
        let design_system: &mut DesignSystem = unsafe { &mut *pointer };
        f(design_system)
    }

    fn this(&self) -> Rc<JsRuntime> {
        self.this.upgrade().expect("the runtime is alive")
    }

    /// The compile function of a utility of a plugin.
    fn compile_utility(&self, id: i64, candidate: Value, design_system: &DesignSystem) -> Compiled {
        let result = self.call(design_system, "__tw_utility", &json!({ "id": id, "candidate": candidate }));
        let text = match result {
            Ok(Some(text)) => text,
            Ok(None) => return Compiled::Undefined,
            Err(message) => {
                self.set_error(message);
                return Compiled::Undefined;
            }
        };
        match serde_json::from_str::<Value>(&text) {
            Ok(Value::Null) => Compiled::Null,
            Ok(value) => match ast_from_json(&value, &self.locs) {
                Ok(nodes) => Compiled::Nodes(nodes),
                Err(message) => {
                    self.set_error(message);
                    Compiled::Undefined
                }
            },
            Err(error) => {
                self.set_error(error.to_string());
                Compiled::Undefined
            }
        }
    }

    /// The function of a variant of a plugin.
    fn apply_variant(&self, id: i64, node: &mut AstNode, variant: &Variant, design_system: &DesignSystem) -> Option<()> {
        let argument = json!({ "id": id, "rule": node_to_json(node, &self.locs), "variant": variant_to_json(variant) });
        let result = self.call(design_system, "__tw_variant", &argument).and_then(|text| {
            let value: Value = serde_json::from_str(&text.unwrap_or_default()).map_err(|error| error.to_string())?;
            let rule = node_from_json(value.get("rule").unwrap_or(&Value::Null), &self.locs)?;
            Ok((rule, value.get("applies").and_then(Value::as_bool).unwrap_or(false)))
        });
        match result {
            Ok((rule, applies)) => {
                *node = rule;
                applies.then_some(())
            }
            Err(message) => {
                self.set_error(message);
                None
            }
        }
    }

    /// The compare function of a group of variants of a plugin.
    fn compare_variants(&self, id: i64, a: &Variant, z: &Variant, design_system: &DesignSystem) -> i32 {
        let argument = json!({ "id": id, "a": variant_to_json(a), "z": variant_to_json(z) });
        match self.call(design_system, "__tw_compare", &argument) {
            Ok(text) => {
                let number = text.and_then(|text| serde_json::from_str::<Value>(&text).ok()).and_then(|value| value.as_f64()).unwrap_or(0.0);
                if number < 0.0 {
                    -1
                } else if number > 0.0 {
                    1
                } else {
                    0
                }
            }
            Err(message) => {
                self.set_error(message);
                0
            }
        }
    }

    fn variant_fn(&self, id: i64) -> impl Fn(&mut AstNode, &Variant, &DesignSystem) -> Option<()> + 'static {
        let runtime = self.this();
        move |node, variant, design_system| runtime.apply_variant(id, node, variant, design_system)
    }

    fn variant_options(argument: &Value) -> VariantOptions {
        VariantOptions {
            compounds: number(argument.get("compounds")).map(|compounds| compounds as u8),
            order: number(argument.get("order")),
        }
    }

    /// One host operation.
    fn host(&self, op: &str, text: &str) -> HostResult {
        // The loader of the engine gives the name of a module as plain text.
        if op == "module.load" {
            return self.engine.call("__tw_module_source", text).map_err(|error| error.message);
        }
        // A JavaScript file of the program, as plain text.
        if op == "asset" {
            return Ok(asset(text).map(str::to_string));
        }

        let argument: Value = if text.is_empty() { Value::Null } else { serde_json::from_str(text).map_err(|error| error.to_string())? };
        let arg = &argument;

        match op {
            // The theme.
            "theme.prefix" => self.with(|ds| Ok(ds.theme.prefix.clone())).and_then(|prefix| done(json!(prefix))),
            "theme.setPrefix" => {
                let prefix = arg.as_str().map(str::to_string);
                self.with_mut(|ds| {
                    ds.theme.prefix = prefix;
                    Ok(None)
                })
            }
            "theme.add" => {
                let (key, value) = (text_arg(&arg[0])?, text_arg(&arg[1])?);
                let options = number(arg.get(2)).unwrap_or(0) as u8;
                self.with(|ds| ds.theme.add(key, value, options).map_err(|error| error.message))?;
                Ok(None)
            }
            "theme.get" => {
                let keys = text_list(Some(arg));
                let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
                self.with(|ds| Ok(ds.theme.get(&keys))).and_then(|value| done(json!(value)))
            }
            "theme.getOptions" => {
                let key = text_arg(arg)?;
                self.with(|ds| Ok(ds.theme.get_options(key))).and_then(|options| done(json!(options)))
            }
            "theme.hasDefault" => {
                let key = text_arg(arg)?;
                self.with(|ds| Ok(ds.theme.has_default(key))).and_then(|has| done(json!(has)))
            }
            "theme.keysInNamespaces" => {
                let keys = text_list(Some(arg));
                let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
                self.with(|ds| Ok(ds.theme.keys_in_namespaces(&keys))).and_then(|keys| done(json!(keys)))
            }
            "theme.namespace" => {
                let namespace = text_arg(arg)?;
                let entries = self.with(|ds| Ok(ds.theme.namespace(namespace)))?;
                done(Value::Array(entries.into_iter().map(|(key, value)| json!([key, value])).collect()))
            }
            "theme.clearNamespace" => {
                let namespace = text_arg(&arg[0])?;
                let options = number(arg.get(1)).unwrap_or(0) as u8;
                self.with(|ds| {
                    ds.theme.clear_namespace(namespace, options);
                    Ok(None)
                })
            }
            "theme.resolve" | "theme.resolveValue" => {
                let value = arg[0].as_str();
                let keys = text_list(arg.get(1));
                let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
                let options = number(arg.get(2)).unwrap_or(0) as u8;
                let resolved = self.with(|ds| {
                    Ok(if op == "theme.resolve" {
                        ds.theme.resolve_with_options(value, &keys, options)
                    } else {
                        ds.theme.resolve_value(value, &keys)
                    })
                })?;
                done(json!(resolved))
            }
            "theme.markUsedVariable" => {
                let key = text_arg(arg)?;
                self.with(|ds| Ok(ds.theme.mark_used_variable(key))).and_then(|marked| done(json!(marked)))
            }
            "theme.entries" => {
                let entries = self.with(|ds| Ok(ds.theme.entries()))?;
                done(Value::Array(
                    entries.into_iter().map(|(key, value)| json!([key, { "value": value.value, "options": value.options }])).collect(),
                ))
            }
            "theme.addKeyframes" => {
                let AstNode::AtRule(rule) = node_from_json(arg, &self.locs)? else {
                    return Err("`addKeyframes` needs an at-rule.".to_string());
                };
                self.with(|ds| {
                    ds.theme.add_keyframes(rule);
                    Ok(None)
                })
            }
            "theme.getKeyframes" => {
                let rules = self.with(|ds| Ok(ds.theme.get_keyframes()))?;
                let nodes: Vec<AstNode> = rules.into_iter().map(AstNode::AtRule).collect();
                done(ast_to_json(&nodes, &self.locs))
            }

            // The utilities.
            "utilities.static" => {
                let name = field(arg, "name")?.to_string();
                let id = number(arg.get("id")).ok_or("No function.")?;
                let runtime = self.this();
                self.with_mut(|ds| {
                    ds.utilities
                        .static_full(name, move |candidate, ds| runtime.compile_utility(id, static_candidate_to_json(candidate), ds));
                    Ok(None)
                })
            }
            "utilities.functional" => {
                let name = field(arg, "name")?.to_string();
                let id = number(arg.get("id")).ok_or("No function.")?;
                let options = arg.get("types").filter(|types| types.is_array()).map(|types| UtilityOptions { types: text_list(Some(types)) });
                let runtime = self.this();
                self.with_mut(|ds| {
                    ds.utilities.functional_full(
                        name,
                        move |candidate, ds| runtime.compile_utility(id, functional_candidate_to_json(candidate), ds),
                        options,
                    );
                    Ok(None)
                })
            }
            "utilities.has" => {
                let name = field(arg, "name")?;
                let kind = if field(arg, "kind")? == "static" { UtilityKind::Static } else { UtilityKind::Functional };
                self.with(|ds| Ok(ds.utilities.has(name, kind))).and_then(|has| done(json!(has)))
            }

            // The variants.
            "variants.static" | "variants.functional" | "variants.compound" => {
                let name = field(arg, "name")?.to_string();
                let id = number(arg.get("id")).ok_or("No function.")?;
                let options = Self::variant_options(arg);
                let apply_fn = self.variant_fn(id);
                let compounds_with = number(arg.get("compoundsWith")).unwrap_or(0) as u8;
                self.with_mut(|ds| {
                    match op {
                        "variants.static" => ds.variants.r#static(name, apply_fn, options),
                        "variants.functional" => ds.variants.functional(name, apply_fn, options),
                        _ => ds.variants.compound(name, compounds_with, apply_fn, options),
                    }
                    Ok(None)
                })
            }
            "variants.fromAst" => {
                let name = field(arg, "name")?.to_string();
                let ast = ast_from_json(arg.get("ast").unwrap_or(&Value::Null), &self.locs)?;
                self.with_mut(|ds| {
                    ds.variants.from_ast(name, ast);
                    Ok(None)
                })
            }
            "variants.groupStart" => {
                let compare_fn: Option<CompareFn> = number(arg.get("id")).map(|id| {
                    let runtime = self.this();
                    Rc::new(move |a: &Variant, z: &Variant, ds: &DesignSystem| runtime.compare_variants(id, a, z, ds)) as CompareFn
                });
                self.with_mut(|ds| {
                    ds.variants.group_start(compare_fn);
                    Ok(None)
                })
            }
            "variants.groupEnd" => self.with_mut(|ds| {
                ds.variants.group_end();
                Ok(None)
            }),
            "variants.has" => {
                let name = text_arg(arg)?;
                self.with(|ds| Ok(ds.variants.has(name))).and_then(|has| done(json!(has)))
            }
            "variants.kind" => {
                let name = text_arg(arg)?;
                let kind = self.with(|ds| Ok(ds.variants.kind(name)))?;
                done(json!(kind.map(kind_name)))
            }
            "variants.get" => {
                let name = text_arg(arg)?;
                let Some(info) = self.with(|ds| Ok(ds.variants.get(name).cloned()))? else {
                    return Ok(None);
                };
                let mut stash = self.stash.borrow_mut();
                stash.push(info.apply_fn.clone());
                done(json!({
                    "kind": kind_name(info.kind),
                    "order": info.order,
                    "compounds": info.compounds,
                    "compoundsWith": info.compounds_with,
                    "stash": stash.len() - 1,
                }))
            }
            "variants.apply" => {
                let index = number(arg.get("stash")).unwrap_or(-1);
                let apply_fn = self.stash.borrow().get(index as usize).cloned().ok_or("The variant function is not there.")?;
                let mut rule = node_from_json(arg.get("rule").unwrap_or(&Value::Null), &self.locs)?;
                let variant = variant_from_json(arg.get("variant").unwrap_or(&Value::Null))?;
                let applies = self.with(|ds| Ok(apply_fn(&mut rule, &variant, ds).is_some()))?;
                done(json!({ "rule": node_to_json(&rule, &self.locs), "applies": applies }))
            }
            "variants.compoundsWith" => {
                let parent = field(arg, "parent")?;
                let child = arg.get("child").unwrap_or(&Value::Null);
                let result = match child {
                    Value::String(name) => self.with(|ds| Ok(ds.variants.compounds_with_name(parent, name)))?,
                    child => {
                        let child = variant_from_json(child)?;
                        self.with(|ds| Ok(ds.variants.compounds_with(parent, &child)))?
                    }
                };
                done(json!(result))
            }
            "variants.orders" => {
                let orders = self.with(|ds| Ok(ds.variants.entries().map(|(name, info)| json!([name, info.order])).collect::<Vec<_>>()))?;
                done(Value::Array(orders))
            }
            "variants.setOrder" => {
                let name = field(arg, "name")?;
                let order = number(arg.get("order")).ok_or("No order.")?;
                self.with_mut(|ds| {
                    if let Some(info) = ds.variants.variants.get_mut(name) {
                        info.order = order;
                    }
                    Ok(None)
                })
            }
            "variants.compareFnOrders" => {
                let mut orders = self.with(|ds| Ok(ds.variants.compare_fns.keys().copied().collect::<Vec<_>>()))?;
                orders.sort_unstable();
                done(json!(orders))
            }
            "variants.moveCompareFns" => {
                let pairs: Vec<(i64, i64)> = match arg {
                    Value::Array(pairs) => pairs.iter().filter_map(|pair| Some((number(pair.get(0))?, number(pair.get(1))?))).collect(),
                    _ => Vec::new(),
                };
                self.with_mut(|ds| {
                    let old = std::mem::take(&mut ds.variants.compare_fns);
                    for (new_order, old_order) in pairs {
                        if let Some(compare_fn) = old.get(&old_order) {
                            ds.variants.compare_fns.insert(new_order, compare_fn.clone());
                        }
                    }
                    Ok(None)
                })
            }

            // The design system.
            "important" => self.with(|ds| Ok(ds.important)).and_then(|important| done(json!(important))),
            "setImportant" => {
                let important = arg.as_bool().unwrap_or(false);
                self.with_mut(|ds| {
                    ds.important = important;
                    Ok(None)
                })
            }
            "invalidCandidates.add" => {
                let candidate = text_arg(arg)?.to_string();
                self.with(|ds| {
                    ds.invalid_candidates.borrow_mut().insert(candidate);
                    Ok(None)
                })
            }
            "resolveThemeValue" => {
                let path = field(arg, "path")?;
                let force_inline = arg.get("forceInline").and_then(Value::as_bool).unwrap_or(true);
                let value = self.with(|ds| Ok(ds.resolve_theme_variable_value(path, force_inline)))?;
                match value {
                    Some(value) => done(json!(value)),
                    None => Ok(None),
                }
            }
            "hookResolveThemeValue" => {
                let runtime = self.this();
                self.with_mut(|ds| {
                    ds.resolve_theme_value_hook = Some(Rc::new(move |ds, path, force_inline| {
                        // Each JavaScript version of this function gives a
                        // theme variable to the function of the host.
                        if path.starts_with("--") {
                            return ds.resolve_theme_variable_value(path, force_inline);
                        }
                        let argument = json!({ "path": path, "forceInline": force_inline });
                        match runtime.call(ds, "__tw_resolve_theme_value", &argument) {
                            Ok(text) => text.and_then(|text| serde_json::from_str::<Value>(&text).ok()).and_then(|value| match value {
                                Value::String(text) => Some(text),
                                Value::Null => None,
                                other => Some(other.to_string()),
                            }),
                            Err(message) => {
                                runtime.set_error(message);
                                None
                            }
                        }
                    }));
                    Ok(None)
                })
            }
            "registerLegacyUtilities" => self.with_mut(|ds| {
                crate::compat::legacy_utilities::register_legacy_utilities(ds);
                Ok(None)
            }),
            "substituteAtApply" | "substituteFunctions" => {
                let mut ast = ast_from_json(arg, &self.locs)?;
                let features = self.with(|ds| {
                    if op == "substituteAtApply" { substitute_at_apply(&mut ast, ds, None) } else { substitute_functions(&mut ast, ds) }
                        .map_err(|error| error.message)
                })?;
                done(json!({ "ast": ast_to_json(&ast, &self.locs), "features": features }))
            }

            // The modules and the files.
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
                    return resolve::resolve(&*self.loader, name, &directory, kind).map(Some);
                }
                if kind == ResolveKind::Entry {
                    if let Some((path, base)) = self.loader.resolve_module(name, base) {
                        return done(json!({ "path": path, "base": base }));
                    }
                    let path = resolve::resolve(&*self.loader, name, base, kind)?;
                    let base = resolve::dirname(&path).to_string();
                    return done(json!({ "path": path, "base": base }));
                }
                // The base of `require` is a file.
                resolve::resolve(&*self.loader, name, resolve::dirname(base), kind).and_then(|path| done(json!(path)))
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
                done(json!({ "format": resolve::module_format(&*self.loader, path).name(), "source": source }))
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
                    "versions": { "node": "22.0.0", "tailwindcss": crate::index::VERSION },
                }))
            }
            "error.causes" => {
                *self.causes.borrow_mut() = text_list(Some(arg));
                Ok(None)
            }
            "cwd" => done(json!(self.loader.cwd())),
            "version" => done(json!(crate::index::VERSION)),

            _ => Err(format!("The host has no operation `{op}`.")),
        }
    }
}

/// Run `applyCompatibilityHooks` of Tailwind CSS on the tree: load the
/// modules of `@plugin` and `@config`, and register what they define.
pub fn apply_compatibility_hooks(
    design_system: &mut DesignSystem,
    ast: &mut Vec<AstNode>,
    sources: &mut Vec<Source>,
    base: &str,
    loader: Rc<dyn ModuleLoader>,
) -> Result<Features> {
    let runtime = JsRuntime::new(loader)?;

    // With source locations, JavaScript must give each object an identity.
    if design_system.track_sources.get() {
        runtime.engine.call("__tw_track", "true").map_err(to_error)?;
    }

    let source_list: Vec<Value> =
        sources.iter().map(|source| json!({ "base": source.base, "pattern": source.pattern, "negated": source.negated })).collect();
    let argument = json!({ "base": base, "ast": ast_to_json(ast, &runtime.locs), "sources": source_list }).to_string();

    let pointer: *mut DesignSystem = design_system;
    let result =
        runtime.scope(Access::Setup(pointer), || runtime.engine.call("__tw_compat", &argument)).map_err(|error| runtime.error_from(error))?;

    let value: Value = serde_json::from_str(&result.unwrap_or_default()).map_err(|error| Error::new(error.to_string()))?;
    *ast = ast_from_json(value.get("ast").unwrap_or(&Value::Null), &runtime.locs).map_err(Error::new)?;
    if let Some(Value::Array(items)) = value.get("sources") {
        *sources = items
            .iter()
            .map(|item| Source {
                base: item.get("base").and_then(Value::as_str).unwrap_or("").to_string(),
                pattern: item.get("pattern").and_then(Value::as_str).unwrap_or("").to_string(),
                negated: item.get("negated").and_then(Value::as_bool).unwrap_or(false),
            })
            .collect();
    }
    let features = number(value.get("features")).unwrap_or(0) as Features;

    design_system.js = Some(runtime);
    Ok(features)
}
