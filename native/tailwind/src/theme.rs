//! The theme: `theme.ts` of Tailwind CSS.
//!
//! The theme holds the values of `@theme`. A read through `&self` can mark
//! a value as used, so the values are in a `RefCell`.

use crate::ast::AtRule;
use crate::utils::escape::{escape, unescape};
use indexmap::IndexMap;
use std::cell::RefCell;

/// The options of a theme value, as bits.
pub type ThemeOptions = u8;
pub const NONE: ThemeOptions = 0;
pub const INLINE: ThemeOptions = 1 << 0;
pub const REFERENCE: ThemeOptions = 1 << 1;
pub const DEFAULT: ThemeOptions = 1 << 2;
pub const STATIC: ThemeOptions = 1 << 3;
pub const USED: ThemeOptions = 1 << 4;

/// The namespaces that a shorter namespace must not match.
fn ignored_theme_keys(namespace: &str) -> &'static [&'static str] {
    match namespace {
        "--font" => &["--font-weight", "--font-size"],
        "--inset" => &["--inset-shadow", "--inset-ring"],
        "--text" => &[
            "--text-color",
            "--text-decoration-color",
            "--text-decoration-thickness",
            "--text-indent",
            "--text-shadow",
            "--text-underline-offset",
        ],
        // Before 4.1.14, `col-*` and `row-*` also read these keys.
        "--grid-column" if crate::switches::GRID_LINE_THEME_KEYS.on() => &["--grid-column-start", "--grid-column-end"],
        "--grid-row" if crate::switches::GRID_LINE_THEME_KEYS.on() => &["--grid-row-start", "--grid-row-end"],
        _ => &[],
    }
}

fn is_ignored_theme_key(theme_key: &str, namespace: &str) -> bool {
    ignored_theme_keys(namespace).iter().any(|ignored| {
        // Before 4.1.0, `--text-shadow-*` was a key of `--text`.
        if *ignored == "--text-shadow" && !crate::switches::TEXT_SHADOW.on() {
            return false;
        }
        theme_key == *ignored
            || (theme_key.starts_with(ignored) && theme_key.as_bytes().get(ignored.len()) == Some(&b'-'))
    })
}

/// One value of the theme.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeValue {
    pub value: String,
    pub options: ThemeOptions,
    /// The location of the declaration that gave the value.
    pub src: Option<crate::source_maps::SourceLocation>,
}

/// The values of `@theme`, in the sequence of their first definition.
#[derive(Debug, Default)]
pub struct Theme {
    pub prefix: Option<String>,
    values: RefCell<IndexMap<String, ThemeValue>>,
    keyframes: RefCell<Vec<AtRule>>,
}

impl Theme {
    pub fn new() -> Self {
        Self::default()
    }

    /// The number of values.
    pub fn size(&self) -> usize {
        self.values.borrow().len()
    }

    /// Add a value. A key that ends with `-*` clears a namespace: its value
    /// must be `initial`.
    pub fn add(&self, key: &str, value: &str, options: ThemeOptions) -> Result<(), crate::Error> {
        self.add_with_src(key, value, options, None)
    }

    /// `add`, with the location of the declaration that gives the value.
    pub fn add_with_src(
        &self,
        key: &str,
        value: &str,
        options: ThemeOptions,
        src: Option<crate::source_maps::SourceLocation>,
    ) -> Result<(), crate::Error> {
        if let Some(namespace) = key.strip_suffix("-*") {
            if value != "initial" {
                return Err(crate::Error::new(format!(
                    "Invalid theme value `{value}` for namespace `{key}`"
                )));
            }
            if key == "--*" {
                self.values.borrow_mut().clear();
            } else {
                self.clear_namespace(namespace, NONE);
            }
        }

        let mut values = self.values.borrow_mut();
        if options & DEFAULT != 0 {
            if let Some(existing) = values.get(key) {
                if existing.options & DEFAULT == 0 {
                    return Ok(());
                }
            }
        }

        if value == "initial" {
            values.shift_remove(key);
        } else {
            values.insert(key.to_string(), ThemeValue { value: value.to_string(), options, src });
        }
        Ok(())
    }

    /// The names of the values in these namespaces, without the namespace.
    pub fn keys_in_namespaces(&self, theme_keys: &[&str]) -> Vec<String> {
        let values = self.values.borrow();
        let mut keys = Vec::new();
        for namespace in theme_keys {
            let prefix = format!("{namespace}-");
            for key in values.keys() {
                if !key.starts_with(&prefix) {
                    continue;
                }
                if key.len() > 2 && key[2..].contains("--") {
                    continue;
                }
                if is_ignored_theme_key(key, namespace) {
                    continue;
                }
                keys.push(key[prefix.len()..].to_string());
            }
        }
        keys
    }

    /// The value of the first key that exists.
    pub fn get(&self, theme_keys: &[&str]) -> Option<String> {
        let values = self.values.borrow();
        theme_keys.iter().find_map(|key| values.get(*key).map(|v| v.value.clone()))
    }

    pub fn has_default(&self, key: &str) -> bool {
        self.get_options(key) & DEFAULT == DEFAULT
    }

    pub fn get_options(&self, key: &str) -> ThemeOptions {
        let key = self.unprefix_key(key);
        let key = unescape(&key);
        self.values.borrow().get(key.as_ref()).map_or(NONE, |v| v.options)
    }

    /// All the values, with the prefix in each key.
    pub fn entries(&self) -> Vec<(String, ThemeValue)> {
        self.values.borrow().iter().map(|(k, v)| (self.prefix_key(k), v.clone())).collect()
    }

    pub fn prefix_key(&self, key: &str) -> String {
        match &self.prefix {
            Some(prefix) if !prefix.is_empty() => format!("--{}-{}", prefix, key.get(2..).unwrap_or("")),
            _ => key.to_string(),
        }
    }

    fn unprefix_key(&self, key: &str) -> String {
        match &self.prefix {
            Some(prefix) if !prefix.is_empty() => {
                format!("--{}", js_slice_from(key, 3 + prefix.len()))
            }
            _ => key.to_string(),
        }
    }

    /// Remove the values of a namespace. With `clear_options`, remove only
    /// the values that have all these options.
    pub fn clear_namespace(&self, namespace: &str, clear_options: ThemeOptions) {
        let ignored = ignored_theme_keys(namespace);
        let keys: Vec<String> = self.values.borrow().keys().cloned().collect();
        'outer: for key in keys {
            if key.starts_with(namespace) {
                if clear_options != NONE {
                    let options = self.get_options(&key);
                    if options & clear_options != clear_options {
                        continue;
                    }
                }
                for ignored_namespace in ignored {
                    if key.starts_with(ignored_namespace) {
                        continue 'outer;
                    }
                }
                self.values.borrow_mut().shift_remove(&key);
            }
        }
    }

    fn resolve_key(&self, candidate_value: Option<&str>, theme_keys: &[&str]) -> Option<String> {
        let values = self.values.borrow();
        // Before 4.0.2, the keys of the theme were the text of the
        // stylesheet, with its escapes. The key of a value was the escaped
        // name, with `_` for each `.`.
        if !crate::switches::THEME_KEYS_UNESCAPED.on() {
            for namespace in theme_keys {
                let theme_key = match candidate_value {
                    Some(v) => escape(&format!("{namespace}-{}", v.replace('.', "_"))).into_owned(),
                    None => namespace.to_string(),
                };
                if !values.contains_key(&theme_key) || is_ignored_theme_key(&theme_key, namespace) {
                    continue;
                }
                return Some(theme_key);
            }
            return None;
        }
        for namespace in theme_keys {
            let mut theme_key = match candidate_value {
                Some(v) => format!("{namespace}-{v}"),
                None => namespace.to_string(),
            };
            if !values.contains_key(&theme_key) {
                // A key with a dot can be in the theme with an underscore.
                match candidate_value {
                    Some(v) if v.contains('.') => {
                        theme_key = format!("{namespace}-{}", v.replace('.', "_"));
                        if !values.contains_key(&theme_key) {
                            continue;
                        }
                    }
                    _ => continue,
                }
            }
            if is_ignored_theme_key(&theme_key, namespace) {
                continue;
            }
            return Some(theme_key);
        }
        None
    }

    fn var(&self, theme_key: &str) -> Option<String> {
        let values = self.values.borrow();
        let value = values.get(theme_key)?;
        // A reference theme does not emit its variables, so the `var()`
        // needs a fallback.
        let prefixed = self.prefix_key(theme_key);
        // (Before 4.0.2, the key had its escapes.)
        let name = if crate::switches::THEME_KEYS_UNESCAPED.on() { escape(&prefixed) } else { prefixed.as_str().into() };
        // (Before 4.0.9, no fallback.)
        if value.options & REFERENCE != 0 && !value.value.is_empty() && crate::switches::REFERENCE_VAR_FALLBACK.on() {
            Some(format!("var({name}, {})", value.value))
        } else {
            Some(format!("var({name})"))
        }
    }

    /// Mark a variable as used. The result is true when this is the first use.
    pub fn mark_used_variable(&self, theme_key: &str) -> bool {
        let key = self.unprefix_key(theme_key);
        let key = unescape(&key);
        let mut values = self.values.borrow_mut();
        match values.get_mut(key.as_ref()) {
            None => false,
            Some(value) => {
                let is_used = value.options & USED != 0;
                value.options |= USED;
                !is_used
            }
        }
    }

    /// The value as `var(--key)`, or the raw value for an inline value.
    pub fn resolve(&self, candidate_value: Option<&str>, theme_keys: &[&str]) -> Option<String> {
        self.resolve_with_options(candidate_value, theme_keys, NONE)
    }

    pub fn resolve_with_options(
        &self,
        candidate_value: Option<&str>,
        theme_keys: &[&str],
        options: ThemeOptions,
    ) -> Option<String> {
        let theme_key = self.resolve_key(candidate_value, theme_keys)?;
        let (value, value_options) = {
            let values = self.values.borrow();
            let v = values.get(&theme_key)?;
            (v.value.clone(), v.options)
        };
        if (options | value_options) & INLINE != 0 {
            return Some(value);
        }
        self.var(&theme_key)
    }

    /// The raw value.
    pub fn resolve_value(&self, candidate_value: Option<&str>, theme_keys: &[&str]) -> Option<String> {
        let theme_key = self.resolve_key(candidate_value, theme_keys)?;
        self.values.borrow().get(&theme_key).map(|v| v.value.clone())
    }

    /// The value, with the values of the nested keys (for example
    /// `--line-height` of a `--text-*` key), as pairs of name and value.
    pub fn resolve_with(
        &self,
        candidate_value: &str,
        theme_keys: &[&str],
        nested_keys: &[&str],
    ) -> Option<(String, Vec<(String, String)>)> {
        let theme_key = self.resolve_key(Some(candidate_value), theme_keys)?;
        let mut extra = Vec::new();
        for name in nested_keys {
            let nested_key = format!("{theme_key}{name}");
            let nested = match self.values.borrow().get(&nested_key) {
                Some(v) => v.clone(),
                None => continue,
            };
            if nested.options & INLINE != 0 {
                extra.push((name.to_string(), nested.value));
            } else {
                extra.push((name.to_string(), self.var(&nested_key).unwrap()));
            }
        }
        let value = self.values.borrow().get(&theme_key).cloned()?;
        if value.options & INLINE != 0 {
            return Some((value.value, extra));
        }
        Some((self.var(&theme_key).unwrap(), extra))
    }

    /// The values of a namespace. The key `None` is the value of the
    /// namespace itself.
    pub fn namespace(&self, namespace: &str) -> IndexMap<Option<String>, String> {
        let mut result = IndexMap::new();
        let prefix = format!("{namespace}-");
        let nested_prefix = format!("{prefix}-");
        for (key, value) in self.values.borrow().iter() {
            if key == namespace {
                result.insert(None, value.value.clone());
            } else if key.starts_with(&nested_prefix) {
                // Keep the `--` of a nested key, as in `--text-sm--line-height`.
                result.insert(Some(key[namespace.len()..].to_string()), value.value.clone());
            } else if key.starts_with(&prefix) {
                result.insert(Some(key[prefix.len()..].to_string()), value.value.clone());
            }
        }
        result
    }

    pub fn add_keyframes(&self, value: AtRule) {
        self.keyframes.borrow_mut().push(value);
    }

    pub fn get_keyframes(&self) -> Vec<AtRule> {
        self.keyframes.borrow().clone()
    }
}

/// `key.slice(start)` of JavaScript for text where the cut is at ASCII.
fn js_slice_from(key: &str, start: usize) -> &str {
    key.get(start..).unwrap_or("")
}
