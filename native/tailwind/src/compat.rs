//! The compatibility layer: `compat/apply-compat-hooks.ts` of Tailwind CSS.
//!
//! `@plugin` and `@config` need JavaScript. This compiler does not support
//! them: `apply_compatibility_hooks` gives an error for them. The parts of
//! the layer that upstream uses for each stylesheet are here:
//!
//! - the utilities of older versions (`legacy_utilities`),
//! - `theme(…)` with a key path such as `colors.red.500` (`theme_fn`, with
//!   the default theme of `default_theme`).

mod default_theme;
pub mod legacy_utilities;
mod theme_fn;

use crate::ast::{css_parent, AstNode};
use crate::design_system::DesignSystem;
use crate::to_css::to_css;
use crate::walk::{walk, WalkAction};
use crate::{Error, Features, Result, FEATURES_NONE};
use std::cell::OnceCell;
use std::rc::Rc;

/// The error for a stylesheet that uses `@plugin`.
pub const PLUGIN_NEEDS_JAVASCRIPT: &str = "`@plugin` needs JavaScript. This compiler does not support `@plugin` and `@config`.";
/// The error for a stylesheet that uses `@config`.
pub const CONFIG_NEEDS_JAVASCRIPT: &str = "`@config` needs JavaScript. This compiler does not support `@plugin` and `@config`.";

/// Apply the compatibility layer to the design system.
///
/// The result is always `FEATURES_NONE`, as upstream for a stylesheet with
/// no `@plugin` and no `@config`.
pub fn apply_compatibility_hooks(design_system: &mut DesignSystem, ast: &mut Vec<AstNode>) -> Result<Features> {
    // Upstream collects the `@plugin` and `@config` rules, and then loads
    // their modules. The errors of the collection stay the same. The load
    // is the error here.
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
        return Err(Error::new(message));
    }

    Ok(FEATURES_NONE)
}
