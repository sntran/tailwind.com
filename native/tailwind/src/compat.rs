//! The compatibility layer: `compat/apply-compat-hooks.ts` of Tailwind CSS.
//!
//! There are two paths:
//!
//! - A stylesheet with no `@plugin` and no `@config` needs no JavaScript.
//!   The parts of the layer that upstream uses for each stylesheet are in
//!   Rust: the utilities of older versions (`legacy_utilities`), and
//!   `theme(…)` with a key path such as `colors.red.500` (`theme_fn`, with
//!   the default theme of `default_theme`).
//! - A stylesheet with `@plugin` or `@config` runs the JavaScript of
//!   Tailwind CSS itself in the engine of `js.rs`, when the program has the
//!   engine (the feature `js`) and the caller gives a module loader.

mod default_theme;
#[cfg(feature = "js")]
pub mod js;
pub mod legacy_utilities;
pub mod module_loader;
mod theme_fn;

use crate::ast::{css_parent, AstNode};
use crate::design_system::DesignSystem;
use crate::to_css::to_css;
use crate::walk::{walk, WalkAction};
use crate::{Error, Features, Result, FEATURES_NONE};
use std::cell::OnceCell;
use std::rc::Rc;

/// The error for a stylesheet that uses `@plugin`, in a build with no
/// JavaScript engine.
pub const PLUGIN_NEEDS_JAVASCRIPT: &str = "`@plugin` needs JavaScript. This build of the compiler has no JavaScript engine.";
/// The error for a stylesheet that uses `@config`, in a build with no
/// JavaScript engine.
pub const CONFIG_NEEDS_JAVASCRIPT: &str = "`@config` needs JavaScript. This build of the compiler has no JavaScript engine.";
/// The error for `@plugin` or `@config` when the caller gives no module
/// loader: the message of upstream.
pub const NO_LOAD_MODULE: &str = "No `loadModule` function provided to `compile`";

/// True when this build has the JavaScript engine.
pub const HAS_JAVASCRIPT: bool = cfg!(tw_js);

/// What the compatibility layer needs from the caller of `compile`.
pub struct CompatOptions<'a> {
    /// The directory of the stylesheet.
    pub base: &'a str,
    /// The file system for the modules of `@plugin` and `@config`.
    pub module_loader: Option<Rc<dyn module_loader::ModuleLoader>>,
}

/// True when the tree has an `@plugin` or `@config` rule.
#[cfg(tw_js)]
fn uses_javascript(ast: &[AstNode]) -> bool {
    ast.iter().any(|node| match node {
        AstNode::AtRule(rule) if rule.name == "@plugin" || rule.name == "@config" => true,
        node => node.children().is_some_and(|nodes| uses_javascript(nodes)),
    })
}

/// Apply the compatibility layer to the design system.
///
/// `sources` gets the entries of the `content` of a JavaScript config.
pub fn apply_compatibility_hooks(
    design_system: &mut DesignSystem,
    ast: &mut Vec<AstNode>,
    sources: &mut Vec<crate::index::Source>,
    options: CompatOptions,
) -> Result<Features> {
    #[cfg(tw_js)]
    if let Some(loader) = options.module_loader.clone() {
        if uses_javascript(ast) {
            return js::apply_compatibility_hooks(design_system, ast, sources, options.base, loader);
        }
    }
    let _ = (&sources, options.base);

    // Upstream collects the `@plugin` and `@config` rules, and then loads
    // their modules. With no JavaScript, the errors of the collection stay
    // the same, and the load is the error.
    let mut error: Option<Error> = None;
    let mut needs_javascript: Option<&'static str> = None;

    walk(ast, |node, ctx| {
        let AstNode::AtRule(rule) = &*node else { return WalkAction::Continue };

        if rule.name == "@plugin" {
            if css_parent(ctx.path).is_some() {
                error = Some(Error::new("`@plugin` cannot be nested."));
                return WalkAction::Stop;
            }

            // `params.slice(1, -1)` is the path.
            if rule.params.chars().count() <= 2 {
                error = Some(Error::new("`@plugin` must have a path."));
                return WalkAction::Stop;
            }

            for option in &rule.nodes {
                let AstNode::Declaration(declaration) = option else {
                    error = Some(Error::new(format!(
                        "Unexpected `@plugin` option:\n\n{}\n\n`@plugin` options must be a flat list of declarations.",
                        to_css(std::slice::from_ref(option))
                    )));
                    return WalkAction::Stop;
                };
                let Some(value) = &declaration.value else { continue };

                for part in crate::utils::segment::segment(value, b',') {
                    let part = crate::utils::js::js_trim(part);
                    if matches!(part, "null" | "true" | "false") || !crate::utils::js::js_number(part).is_nan() {
                        continue;
                    }
                    let quoted = part.len() >= 2
                        && ((part.starts_with('"') && part.ends_with('"')) || (part.starts_with('\'') && part.ends_with('\'')));
                    if !quoted && part.starts_with('{') && part.ends_with('}') {
                        error = Some(Error::new(format!(
                            "Unexpected `@plugin` option: Value of declaration `{}` is not supported.\n\nUsing an object as a plugin option is currently only supported in JavaScript configuration files.",
                            crate::utils::js::js_trim(&to_css(std::slice::from_ref(option)))
                        )));
                        return WalkAction::Stop;
                    }
                }
            }

            needs_javascript.get_or_insert(PLUGIN_NEEDS_JAVASCRIPT);
            return WalkAction::Replace(Vec::new());
        }

        if rule.name == "@config" {
            if !rule.nodes.is_empty() {
                error = Some(Error::new("`@config` cannot have a body."));
                return WalkAction::Stop;
            }

            if css_parent(ctx.path).is_some() {
                error = Some(Error::new("`@config` cannot be nested."));
                return WalkAction::Stop;
            }

            needs_javascript.get_or_insert(CONFIG_NEEDS_JAVASCRIPT);
            return WalkAction::Replace(Vec::new());
        }

        WalkAction::Continue
    });

    if let Some(error) = error {
        return Err(error);
    }

    legacy_utilities::register_legacy_utilities(design_system);

    // `theme(…)` with a key path. Upstream makes the full compatibility
    // configuration at the first use of such a path. The configuration is
    // the default theme with the values of the CSS theme.
    let compat_theme: OnceCell<theme_fn::CompatTheme> = OnceCell::new();
    design_system.resolve_theme_value_hook = Some(Rc::new(move |design_system, path, force_inline| {
        if path.starts_with("--") {
            return design_system.resolve_theme_variable_value(path, force_inline);
        }
        compat_theme.get_or_init(|| theme_fn::CompatTheme::new(design_system)).resolve_theme_value(design_system, path)
    }));

    if let Some(message) = needs_javascript {
        let message = if HAS_JAVASCRIPT && options.module_loader.is_none() { NO_LOAD_MODULE } else { message };
        return Err(Error::new(message));
    }

    Ok(FEATURES_NONE)
}
