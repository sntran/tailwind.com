//! The `theme(…)` function for key paths such as `colors.red.500`:
//! `compat/plugin-functions.ts`, `compat/config/resolve-config.ts`,
//! `compat/config/deep-merge.ts`, `compat/config/create-compat-config.ts`
//! and `keyPathToCssProperty` of `compat/apply-config-to-theme.ts` of
//! Tailwind CSS.
//!
//! Upstream has JavaScript values here: objects, arrays, and functions
//! that calculate a part of the theme. `Val` is a small model of these
//! values. The functions of the default theme have only two forms, so they
//! are data (`Lazy`).

use crate::design_system::DesignSystem;
use crate::switches;
use crate::theme;
use crate::utilities::with_alpha;
use crate::utils::escape::unescape;
use crate::utils::js::{js_number_to_string, js_trim};
use crate::utils::to_key_path::to_key_path;
use indexmap::IndexMap;
use std::cell::RefCell;
use std::rc::Rc;

/// The key of the object that has the theme options of the CSS values.
const CSS_VALUES: &str = "__CSS_VALUES__";

/// A JavaScript value.
#[derive(Clone)]
pub(super) enum Val {
    Undefined,
    Null,
    Str(String),
    Num(f64),
    Arr(Rc<Vec<Val>>),
    Obj(Rc<IndexMap<String, Val>>),
    /// A function of the configuration that calculates a theme value.
    Lazy(Rc<Lazy>),
    /// A function that this model does not call (`__BARE_VALUE__`).
    Opaque,
}

/// A function of the theme configuration, as data.
pub(super) enum Lazy {
    /// `({ theme }) => theme('key')`
    Theme(&'static str),
    /// `({ theme }) => theme('key', {})`
    ThemeOrEmpty(&'static str),
    /// `({ theme }) => ({ ...theme('key'), name: value })`
    Object(Vec<Part>),
    /// The merge of a theme key with its `extend` values.
    Merge(Vec<Val>),
}

/// A part of an object literal.
pub(super) enum Part {
    /// `...theme('key')`
    Spread(&'static str),
    /// `...theme('key', {})`
    SpreadOrEmpty(&'static str),
    Entry(&'static str, Val),
}

/// A string value.
pub(super) fn s(value: &str) -> Val {
    Val::Str(value.to_string())
}

/// An array value.
pub(super) fn a(values: Vec<Val>) -> Val {
    Val::Arr(Rc::new(values))
}

/// An object value.
pub(super) fn o(entries: Vec<(&'static str, Val)>) -> Val {
    Val::Obj(Rc::new(entries.into_iter().map(|(k, v)| (k.to_string(), v)).collect()))
}

type Object = IndexMap<String, Val>;

impl Val {
    fn object(map: Object) -> Val {
        Val::Obj(Rc::new(map))
    }

    /// `value === null || value === undefined`
    fn is_nullish(&self) -> bool {
        matches!(self, Val::Undefined | Val::Null)
    }

    /// `value ?? other`
    fn or(self, other: Val) -> Val {
        if self.is_nullish() {
            other
        } else {
            self
        }
    }

    /// `isPlainObject(value)`
    fn is_plain_object(&self) -> bool {
        matches!(self, Val::Obj(_))
    }

    /// The operand of `value & flags`.
    fn bits(&self) -> u8 {
        match self {
            Val::Num(n) => *n as u8,
            _ => 0,
        }
    }

    /// `Object.hasOwn(value, key)` and `value[key]`, for an object or an array.
    fn get_own(&self, key: &str) -> Option<Val> {
        match self {
            Val::Obj(map) => map.get(key).cloned(),
            Val::Arr(items) => array_index(key).and_then(|index| items.get(index).cloned()),
            _ => None,
        }
    }

    /// `value[key]`
    fn get(&self, key: &str) -> Val {
        self.get_own(key).unwrap_or(Val::Undefined)
    }

    /// The own keys and values that a spread or a `for … in` gives.
    fn entries(&self) -> Vec<(String, Val)> {
        match self {
            Val::Obj(map) => map.iter().map(|(k, v)| (k.clone(), v.clone())).collect(),
            Val::Arr(items) => items.iter().enumerate().map(|(i, v)| (i.to_string(), v.clone())).collect(),
            Val::Str(text) => text.chars().enumerate().map(|(i, c)| (i.to_string(), Val::Str(c.to_string()))).collect(),
            _ => Vec::new(),
        }
    }

    /// `String(value)`, for the items of `Array.prototype.join`.
    fn to_js_string(&self) -> String {
        match self {
            Val::Undefined | Val::Null => String::new(),
            Val::Str(text) => text.clone(),
            Val::Num(n) => js_number_to_string(*n),
            Val::Arr(items) => items.iter().map(|v| v.to_js_string()).collect::<Vec<_>>().join(","),
            Val::Obj(_) => "[object Object]".to_string(),
            Val::Lazy(_) | Val::Opaque => String::new(),
        }
    }
}

fn array_index(key: &str) -> Option<usize> {
    if key.is_empty() || !key.bytes().all(|b| b.is_ascii_digit()) || (key.len() > 1 && key.starts_with('0')) {
        return None;
    }
    key.parse().ok()
}

/// The resolved theme of the compatibility configuration, for a design
/// system that has no JavaScript configuration and no plugin.
pub(super) struct CompatTheme {
    theme: Object,
}

/// The state of `mergeTheme`: the theme, with keys that are not yet
/// calculated.
struct Resolution<'a> {
    design_system: &'a DesignSystem,
    theme: RefCell<Object>,
}

impl CompatTheme {
    /// `resolveConfig` for the compatibility configuration and no user
    /// configuration.
    pub(super) fn new(design_system: &DesignSystem) -> Self {
        let css_theme = &design_system.theme;

        // `createCompatConfig`: the default theme with the `extend` object.
        let mut theme: Object = super::default_theme::default_theme().into_iter().map(|(k, v)| (k.to_string(), v)).collect();

        // Before 4.3.3, the sans-serif fonts of the default theme started
        // with `ui-sans-serif, system-ui`.
        if !switches::DEFAULT_THEME_SYSTEM_FONTS.on() {
            if let Some(Val::Obj(fonts)) = theme.get("fontFamily") {
                let mut fonts: Object = (**fonts).clone();
                let old = ["ui-sans-serif", "system-ui", "sans-serif", "\"Apple Color Emoji\"", "\"Segoe UI Emoji\"", "\"Segoe UI Symbol\"", "\"Noto Color Emoji\""];
                fonts.insert("sans".to_string(), a(old.into_iter().map(s).collect()));
                theme.insert("fontFamily".to_string(), Val::object(fonts));
            }
        }

        // Before 4.1.2, the default colors of the border and of the ring
        // were `currentColor`.
        if !crate::switches::CURRENTCOLOR_LOWERCASE.on() {
            for key in ["borderColor", "ringColor"] {
                let parts = vec![Part::Entry("DEFAULT", s("currentColor")), Part::Spread("colors")];
                theme.insert(key.to_string(), Val::Lazy(Rc::new(Lazy::Object(parts))));
            }
        }

        let spread = |key: &'static str| Val::Lazy(Rc::new(Lazy::Object(vec![Part::SpreadOrEmpty(key)])));
        let default_of = |key: &str| {
            let mut map = Object::new();
            map.insert("DEFAULT".to_string(), css_theme.get(&[key]).map_or(Val::Null, Val::Str));
            Val::object(map)
        };
        let extend: Vec<(&str, Val)> = vec![
            ("fontSize", spread("text")),
            ("boxShadow", spread("shadow")),
            ("animation", spread("animate")),
            ("aspectRatio", spread("aspect")),
            ("borderRadius", spread("radius")),
            ("screens", spread("breakpoint")),
            ("letterSpacing", spread("tracking")),
            ("lineHeight", spread("leading")),
            ("transitionDuration", default_of("--default-transition-duration")),
            ("transitionTimingFunction", default_of("--default-transition-timing-function")),
            ("maxWidth", spread("container")),
        ];

        // `mergeTheme`: each key with an extension becomes a function that
        // merges the values.
        for (key, extension) in extend {
            let base = theme.get(key).cloned().unwrap_or(Val::Undefined);
            theme.insert(key.to_string(), Val::Lazy(Rc::new(Lazy::Merge(vec![base, extension]))));
        }

        let resolution = Resolution { design_system, theme: RefCell::new(theme) };
        let keys: Vec<String> = resolution.theme.borrow().keys().cloned().collect();
        for key in keys {
            let value = resolution.theme.borrow().get(&key).cloned().unwrap_or(Val::Undefined);
            let value = resolution.resolve_value(value);
            resolution.theme.borrow_mut().insert(key, value);
        }

        // In `screens`, `{ min: '123px' }` becomes `'123px'`.
        let mut theme = resolution.theme.into_inner();
        if let Some(Val::Obj(screens)) = theme.get("screens").cloned() {
            let mut screens = (*screens).clone();
            for (_, screen) in screens.iter_mut() {
                let Val::Obj(map) = screen else { continue };
                if map.contains_key("raw") || map.contains_key("max") || !map.contains_key("min") {
                    continue;
                }
                *screen = map.get("min").cloned().unwrap_or(Val::Undefined);
            }
            theme.insert("screens".to_string(), Val::object(screens));
        }

        Self { theme }
    }

    /// The replacement of `resolveThemeValue` of `upgradeToFullPluginSupport`,
    /// for a path that does not start with `--`.
    pub(super) fn resolve_theme_value(&self, design_system: &DesignSystem, path: &str) -> Option<String> {
        let mut resolved = theme_fn(design_system, &self.theme, &|value| value, path);

        // An object with `DEFAULT` gives that value: a namespace root with a
        // bare value (`--shadow` for `theme('shadow')`), or `colors.foo`
        // with `--color-foo` and `--color-foo-bar`.
        // (Before 4.1.15, such an object was no value.)
        if let Val::Obj(map) = &resolved {
            if let Some(default) = map.get("DEFAULT").cloned().filter(|_| switches::THEME_FN_OBJECT_DEFAULT.on()) {
                resolved = default;
            }
        }

        match &resolved {
            // A tuple gives its first item.
            Val::Arr(items) if items.len() == 2 => match &items[0] {
                Val::Str(text) => Some(text.clone()),
                Val::Undefined | Val::Null => None,
                other => Some(other.to_js_string()),
            },
            // An array becomes a list with commas.
            Val::Arr(items) => Some(items.iter().map(|v| v.to_js_string()).collect::<Vec<_>>().join(", ")),
            Val::Str(text) => Some(text.clone()),
            _ => None,
        }
    }
}

impl Resolution<'_> {
    /// The `theme` function that the functions of the configuration get.
    fn theme(&self, path: &str, default_value: Val) -> Val {
        let config = self.theme.borrow().clone();
        theme_fn(self.design_system, &config, &|value| self.resolve_value(value), path).or(default_value)
    }

    /// `resolveValue` of `mergeTheme`: call a function, and `undefined`
    /// becomes `null`.
    fn resolve_value(&self, value: Val) -> Val {
        match value {
            Val::Lazy(lazy) => self.call(&lazy).or(Val::Null),
            // The model does not call this function.
            Val::Opaque => Val::Null,
            value => value.or(Val::Null),
        }
    }

    fn call(&self, lazy: &Lazy) -> Val {
        match lazy {
            Lazy::Theme(key) => self.theme(key, Val::Undefined),
            Lazy::ThemeOrEmpty(key) => self.theme(key, Val::object(Object::new())),
            Lazy::Object(parts) => {
                let mut map = Object::new();
                for part in parts {
                    match part {
                        Part::Spread(key) => map.extend(self.theme(key, Val::Undefined).entries()),
                        Part::SpreadOrEmpty(key) => map.extend(self.theme(key, Val::object(Object::new())).entries()),
                        Part::Entry(key, value) => {
                            map.insert(key.to_string(), value.clone());
                        }
                    }
                }
                Val::object(map)
            }
            Lazy::Merge(values) => {
                let values: Vec<Val> = values.iter().map(|value| self.resolve_value(value.clone())).collect();
                Val::object(deep_merge(Object::new(), &values, &merge_theme_extension))
            }
        }
    }
}

/// `mergeThemeExtension`
fn merge_theme_extension(theme_value: &Val, extension_value: &Val) -> Val {
    // An array of objects: add the extension.
    if let Val::Arr(items) = theme_value {
        if items.first().is_some_and(|first| first.is_plain_object()) {
            let mut items = (**items).clone();
            match extension_value {
                Val::Arr(more) => items.extend(more.iter().cloned()),
                other => items.push(other.clone()),
            }
            return Val::Arr(Rc::new(items));
        }
    }

    if let Val::Arr(items) = extension_value {
        // An array of objects after an object: the object comes first.
        if items.first().is_some_and(|first| first.is_plain_object()) && theme_value.is_plain_object() {
            let mut merged = vec![theme_value.clone()];
            merged.extend(items.iter().cloned());
            return Val::Arr(Rc::new(merged));
        }

        // An array replaces the value (font families, box shadows).
        return extension_value.clone();
    }

    Val::Undefined
}

/// `deepMerge`
fn deep_merge(mut target: Object, sources: &[Val], customizer: &dyn Fn(&Val, &Val) -> Val) -> Object {
    for source in sources {
        if source.is_nullish() {
            continue;
        }

        for (key, source_value) in source.entries() {
            let target_value = target.get(&key).cloned().unwrap_or(Val::Undefined);
            let merged = customizer(&target_value, &source_value);

            let value = if !matches!(merged, Val::Undefined) {
                merged
            } else if let (Val::Obj(_), Val::Obj(_)) = (&target_value, &source_value) {
                Val::object(deep_merge(Object::new(), &[target_value, source_value], customizer))
            } else {
                source_value
            };
            target.insert(key, value);
        }
    }

    target
}

/// `createThemeFn`: the value of a path, from the CSS theme and from the
/// theme of the configuration.
fn theme_fn(design_system: &DesignSystem, config_theme: &Object, resolve_value: &dyn Fn(Val) -> Val, path: &str) -> Val {
    // A modifier is after the last slash: `colors.red.500 / 50%`.
    let (path, modifier) = match path.rfind('/') {
        Some(index) => (js_trim(&path[..index]), Some(js_trim(&path[index + 1..]))),
        None => (path, None),
    };

    let mut keypath: Vec<String> = to_key_path(path).into_iter().map(str::to_string).collect();
    let (mut css_value, options) = read_from_css(design_system, &keypath);

    let config_root = Val::Obj(Rc::new(config_theme.clone()));
    let mut config_value = resolve_value(get(&config_root, &mut keypath).or(Val::Null));

    if let Val::Str(text) = &config_value {
        config_value = Val::Str(text.replacen("<alpha-value>", "1", 1));
    }

    let resolved = 'value: {
        // The CSS value is a primitive value.
        if let Val::Str(_) | Val::Num(_) | Val::Undefined = css_value {
            if !matches!(options, Val::Obj(_) | Val::Arr(_)) && options.bits() & theme::DEFAULT != 0 {
                break 'value config_value.or(css_value);
            }
            break 'value css_value;
        }

        if let Val::Obj(config_map) = &config_value {
            // A copy: the value of the configuration must not change.
            let mut copy: Object = (**config_map).clone();
            let config_css_values = config_map.get(CSS_VALUES).cloned();

            // No CSS value: use the CSS values that the configuration has.
            if matches!(css_value, Val::Null) {
                if let Some(css_values) = &config_css_values {
                    let mut local = Object::new();
                    for (key, _) in css_values.entries() {
                        local.insert(key.clone(), config_map.get(&key).cloned().unwrap_or(Val::Undefined));
                        copy.shift_remove(&key);
                    }
                    css_value = Val::object(local);
                }
            }

            for (key, value) in css_value.entries() {
                if key == CSS_VALUES {
                    continue;
                }

                // A default value of the CSS (`@theme default`) does not
                // replace a value of the configuration.
                let is_default = config_css_values.as_ref().map_or(0, |values| values.get(&key).bits()) & theme::DEFAULT != 0;
                if is_default {
                    let mut parts: Vec<String> = key.split('-').map(str::to_string).collect();
                    if !matches!(get(&Val::Obj(Rc::new(copy.clone())), &mut parts), Val::Undefined) {
                        continue;
                    }
                }

                // A value of `@theme` wins over a value of the configuration.
                copy.insert(unescape(&key).into_owned(), value);
            }

            break 'value Val::object(copy);
        }

        // A tuple.
        if let (Val::Arr(css_items), Val::Arr(option_items), Val::Arr(config_items)) = (&css_value, &options, &config_value) {
            let mut base = css_items.first().cloned().unwrap_or(Val::Undefined);
            let mut extra = css_items.get(1).cloned().unwrap_or(Val::Undefined);

            // A value of the configuration replaces a default CSS value.
            if option_items.first().map_or(0, Val::bits) & theme::DEFAULT != 0 {
                base = config_items.first().cloned().unwrap_or(Val::Undefined).or(base);
            }

            if let Val::Obj(extra_map) = &extra {
                let mut extra_map = (**extra_map).clone();
                let extra_options = option_items.get(1).cloned().unwrap_or(Val::Undefined);
                let config_extra = config_items.get(1).cloned().unwrap_or(Val::Undefined);
                for (key, value) in extra_map.iter_mut() {
                    if extra_options.get(key).bits() & theme::DEFAULT != 0 {
                        *value = config_extra.get(key).or(value.clone());
                    }
                }
                extra = Val::object(extra_map);
            }

            break 'value Val::Arr(Rc::new(vec![base, extra]));
        }

        // `--color-foo` and `--color-foo-bar` together give an object for
        // `colors.foo`. Then the result is the value of `DEFAULT`. A
        // namespace root (`theme('shadow')`) stays an object.
        // (Before 4.3.3, the result was the object. In 4.3.3, a namespace
        // root also gave its `DEFAULT`.)
        if switches::THEME_FN_PREFERS_DEFAULT.on() && (keypath.len() > 1 || !switches::THEME_FN_NAMESPACE_ROOT.on()) {
            if let Val::Obj(map) = &css_value {
                if let Some(default) = map.get("DEFAULT") {
                    break 'value default.clone();
                }
            }
        }

        // A CSS value wins over a value of the configuration.
        css_value.or(config_value)
    };

    // Apply the opacity modifier.
    match (&resolved, modifier) {
        (Val::Str(value), Some(modifier)) if !modifier.is_empty() => Val::Str(with_alpha(value, modifier)),
        _ => resolved,
    }
}

/// `readFromCss`: the value of a key path in the CSS theme, and its theme
/// options.
fn read_from_css(design_system: &DesignSystem, path: &[String]) -> (Val, Val) {
    let css_theme = &design_system.theme;
    let none = || Val::Num(f64::from(theme::NONE));
    let options_of = |key: &str| Val::Num(f64::from(css_theme.get_options(key)));

    // `--color-red-500` is the theme variable itself.
    if path.len() == 1 && path[0].starts_with("--") {
        return (css_theme.get(&[&path[0]]).map_or(Val::Null, Val::Str), options_of(&path[0]));
    }

    let Some(theme_key) = key_path_to_css_property(path) else { return (Val::Null, none()) };

    let namespace = css_theme.namespace(&format!("--{theme_key}"));
    if namespace.is_empty() {
        return (Val::Null, none());
    }

    let mut map: IndexMap<Option<String>, Val> = IndexMap::new();
    let mut options: IndexMap<Option<String>, Val> = IndexMap::new();
    let mut nested: IndexMap<Option<String>, IndexMap<String, (String, Val)>> = IndexMap::new();

    for (key, value) in &namespace {
        // A value that is not nested.
        let Some(nested_index) = key.as_deref().and_then(|key| key.find("--")) else {
            let option_key = match key.as_deref() {
                Some(key) if !key.is_empty() => format!("--{theme_key}-{key}"),
                _ => format!("--{theme_key}"),
            };
            map.insert(key.clone(), Val::Str(value.clone()));
            options.insert(key.clone(), options_of(&option_key));
            continue;
        };

        // A nested value, for example `--text-sm--line-height`.
        let key = key.as_deref().unwrap_or("");
        let main_key = &key[..nested_index];
        let nested_key = camel_case(&key[nested_index + 2..]);

        nested
            .entry(if main_key.is_empty() { None } else { Some(main_key.to_string()) })
            .or_default()
            .insert(nested_key, (value.clone(), options_of(&format!("--{theme_key}{key}"))));
    }

    let base_options = options_of(&format!("--{theme_key}"));
    for (key, extra) in nested {
        let Some(Val::Str(value)) = map.get(&key).cloned() else { continue };

        let mut extra_object = Object::new();
        let mut extra_options = Object::new();
        for (nested_key, (nested_value, nested_options)) in extra {
            extra_object.insert(nested_key.clone(), Val::Str(nested_value));
            extra_options.insert(nested_key, nested_options);
        }

        map.insert(key.clone(), Val::Arr(Rc::new(vec![Val::Str(value), Val::object(extra_object)])));
        options.insert(key, Val::Arr(Rc::new(vec![base_options.clone(), Val::object(extra_options)])));
    }

    // An object, as in Tailwind CSS v3.
    let to_object = |source: IndexMap<Option<String>, Val>| -> Object {
        let mut object = Object::new();
        for (key, value) in source {
            object.insert(key.unwrap_or_else(|| "DEFAULT".to_string()), value);
        }
        object
    };
    let mut object = to_object(map);
    let options_object = to_object(options);

    // `theme('animation.DEFAULT')` reads `--animation-*`: the result is the
    // value of the key `DEFAULT`.
    if path.last().is_some_and(|last| last == "DEFAULT") {
        return (
            object.get("DEFAULT").cloned().unwrap_or(Val::Null).or(Val::Null),
            options_object.get("DEFAULT").cloned().unwrap_or_else(none),
        );
    }

    // `theme('animation.spin')` reads `--animation-spin-*`, and there is
    // only the value itself. A namespace root keeps the object.
    // (Before 4.3.4, a namespace root with only a bare value also gave
    // the value itself.)
    if (path.len() > 1 || !switches::THEME_FN_NAMESPACE_ROOT.on()) && object.len() == 1 {
        if let Some(default) = object.get("DEFAULT") {
            return (default.clone(), options_object.get("DEFAULT").cloned().unwrap_or_else(none));
        }
    }

    object.insert(CSS_VALUES.to_string(), Val::object(options_object.clone()));
    (Val::object(object), Val::object(options_object))
}

/// `value.replace(/-([a-z])/g, (_, a) => a.toUpperCase())`
fn camel_case(value: &str) -> String {
    let mut result = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match chars.peek() {
            Some(next) if c == '-' && next.is_ascii_lowercase() => {
                result.push(next.to_ascii_uppercase());
                chars.next();
            }
            _ => result.push(c),
        }
    }
    result
}

/// `get` of `plugin-functions.ts`: read a path in an object. When a key
/// does not exist, the key and the next key become one key with a dash.
fn get(object: &Val, path: &mut [String]) -> Val {
    let mut current = object.clone();
    // Before 4.1.15, a key was also an index into a text.
    let index_strings = !switches::THEME_FN_NO_STRING_INDEX.on();
    for i in 0..path.len() {
        let value = match &current {
            Val::Str(text) if index_strings => string_index(text, &path[i]),
            _ => current.get_own(&path[i]),
        };
        match value {
            Some(value) => current = value,
            None => {
                if i + 1 >= path.len() {
                    return Val::Undefined;
                }
                path[i + 1] = format!("{}-{}", path[i], path[i + 1]);
            }
        }
    }
    current
}

/// `text[key]` of JavaScript, for an index and for `length`. The units
/// are UTF-16 code units. (The functions of a text, such as `trim`, are
/// not in this model.)
fn string_index(text: &str, key: &str) -> Option<Val> {
    if key == "length" {
        return Some(Val::Num(text.encode_utf16().count() as f64));
    }
    let unit = text.encode_utf16().nth(array_index(key)?)?;
    Some(Val::Str(String::from_utf16_lossy(&[unit])))
}

/// `/^[a-zA-Z0-9-_%/.]+$/`
fn is_valid_key(part: &str) -> bool {
    !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'%' | b'/' | b'.'))
}

/// `value.replace(/([a-z])([A-Z])/g, (_, a, b) => `${a}-${b.toLowerCase()}`)`
fn kebab_case(value: &str) -> String {
    let mut result = String::with_capacity(value.len() + 4);
    let mut previous_is_lower = false;
    for c in value.chars() {
        if previous_is_lower && c.is_ascii_uppercase() {
            result.push('-');
            result.push(c.to_ascii_lowercase());
        } else {
            result.push(c);
        }
        previous_is_lower = c.is_ascii_lowercase();
    }
    result
}

/// `keyPathToCssProperty`: the name of the theme variable of a key path,
/// without the `--`. `None` is a path with no theme variable.
pub(super) fn key_path_to_css_property(path: &[String]) -> Option<String> {
    let first = path.first().map(String::as_str).unwrap_or("");

    // The `DEFAULT` of these keys was a default for more than one utility.
    // Such a value is in the `--default-*` namespace.
    let special_default = match first {
        "borderWidth" => Some("border-width"),
        "outlineWidth" => Some("outline-width"),
        "ringColor" => Some("ring-color"),
        "ringWidth" => Some("ring-width"),
        "transitionDuration" => Some("transition-duration"),
        "transitionTimingFunction" => Some("transition-timing-function"),
        _ => None,
    };
    // (Before 4.1.18, it was in the namespace of the key.)
    if let Some(special_default) = special_default.filter(|_| switches::THEME_FN_DEFAULT_NAMESPACE.on()) {
        if path.get(1).is_some_and(|key| key == "DEFAULT") {
            return Some(format!("default-{special_default}"));
        }
    }

    // The configuration of the container component is not in the theme.
    if first == "container" {
        return None;
    }

    if !path.iter().all(|part| is_valid_key(part)) {
        return None;
    }

    // The namespaces of Tailwind CSS v3 have new names.
    let namespace = match first {
        "animation" => "animate",
        "aspectRatio" => "aspect",
        "borderRadius" => "radius",
        "boxShadow" => "shadow",
        "colors" => "color",
        "containers" => "container",
        "fontFamily" => "font",
        "fontSize" => "text",
        "letterSpacing" => "tracking",
        "lineHeight" => "leading",
        "maxWidth" => "container",
        "screens" => "breakpoint",
        "transitionTimingFunction" => "ease",
        other => other,
    };

    let last = path.len() - 1;
    let mut parts: Vec<String> = Vec::with_capacity(path.len());
    for (index, part) in path.iter().enumerate() {
        let part = if index == 0 { namespace } else { part.as_str() };

        // `1` is the object of a tuple: an empty part gives two dashes.
        // `fontSize.xs.1.lineHeight` is `text-xs--line-height`.
        let part = if part == "1" && index != last { "" } else { part };
        let mut part = part.replace('.', "_");

        // The namespace, a nested key of a tuple, and `lineHeight` are in
        // kebab case.
        // (Before 4.1.18, each part was in kebab case.)
        if index == 0 || part.starts_with('-') || part == "lineHeight" || !switches::THEME_FN_KEEPS_KEY_CASE.on() {
            part = kebab_case(&part);
        }

        // No `DEFAULT` at the end of the path.
        if part == "DEFAULT" && index == last {
            continue;
        }
        parts.push(part);
    }

    Some(parts.join("-"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn property(path: &[&str]) -> Option<String> {
        key_path_to_css_property(&path.iter().map(|p| p.to_string()).collect::<Vec<_>>())
    }

    #[test]
    fn key_paths_become_theme_variables() {
        assert_eq!(property(&["colors", "red", "500"]).as_deref(), Some("color-red-500"));
        assert_eq!(property(&["spacing", "2.5"]).as_deref(), Some("spacing-2_5"));
        assert_eq!(property(&["fontSize", "xs", "1", "lineHeight"]).as_deref(), Some("text-xs--line-height"));
        assert_eq!(property(&["spacing", "1"]).as_deref(), Some("spacing-1"));
        assert_eq!(property(&["blur", "DEFAULT"]).as_deref(), Some("blur"));
        assert_eq!(property(&["ringColor", "DEFAULT"]).as_deref(), Some("default-ring-color"));
        assert_eq!(property(&["transitionTimingFunction", "in-out"]).as_deref(), Some("ease-in-out"));
        assert_eq!(property(&["backgroundColor", "red", "500"]).as_deref(), Some("background-color-red-500"));
        assert_eq!(property(&["container", "center"]), None);
        assert_eq!(property(&["colors", "a b"]), None);
    }

    #[test]
    fn get_joins_missing_keys() {
        let object = o(vec![("a", o(vec![("b-c", s("x"))]))]);
        let mut path = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        assert!(matches!(get(&object, &mut path), Val::Str(v) if v == "x"));
        let mut path = vec!["a".to_string(), "z".to_string()];
        assert!(matches!(get(&object, &mut path), Val::Undefined));
    }

    #[test]
    fn case_changes() {
        assert_eq!(camel_case("line-height"), "lineHeight");
        assert_eq!(camel_case("font-feature-settings"), "fontFeatureSettings");
        assert_eq!(kebab_case("backgroundColor"), "background-color");
        assert_eq!(kebab_case("aBC"), "a-bC");
    }
}
