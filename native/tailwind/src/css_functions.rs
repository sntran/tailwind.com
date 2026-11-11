//! The CSS functions of Tailwind CSS (`theme(…)`, `--theme(…)`,
//! `--spacing(…)`, `--alpha(…)`): `css-functions.ts` of Tailwind CSS.

use crate::ast::AstNode;
use crate::design_system::DesignSystem;
use crate::utilities::with_alpha;
use crate::utils::dimensions::dimensions;
use crate::utils::js::js_trim;
use crate::utils::segment::segment;
use crate::value_parser::{self as ValueParser, ValueAstNode};
use crate::walk::{walk, WalkAction};
use crate::{Error, Features, Result, FEATURES_NONE, FEATURES_THEME_FUNCTION};

fn alpha(args: &[&str]) -> Result<String> {
    let value = args.first().copied().unwrap_or("");
    let parts: Vec<&str> = segment(value, b'/').into_iter().map(js_trim).collect();
    let color = parts.first().copied().unwrap_or("");
    let alpha = parts.get(1).copied().unwrap_or("");
    let example = format!(
        "`--alpha({} / {})`",
        if color.is_empty() { "var(--my-color)" } else { color },
        if alpha.is_empty() { "50%" } else { alpha }
    );

    if color.is_empty() || alpha.is_empty() {
        return Err(Error::new(format!(
            "The --alpha(…) function requires a color and an alpha value, e.g.: {example}"
        )));
    }
    if args.len() > 1 {
        return Err(Error::new(format!(
            "The --alpha(…) function only accepts one argument, e.g.: {example}"
        )));
    }
    Ok(with_alpha(color, alpha))
}

fn spacing(design_system: &DesignSystem, args: &[&str]) -> Result<String> {
    let value = args.first().copied().unwrap_or("");
    if value.is_empty() {
        return Err(Error::new("The --spacing(…) function requires an argument, but received none."));
    }
    if args.len() > 1 {
        return Err(Error::new(format!(
            "The --spacing(…) function only accepts a single argument, but received {}.",
            args.len()
        )));
    }

    let multiplier = match design_system.theme.resolve(None, &["--spacing"]) {
        Some(m) if !m.is_empty() => m,
        _ => {
            return Err(Error::new(
                "The --spacing(…) function requires that the `--spacing` theme variable exists, but it was not found.",
            ))
        }
    };

    if let Some((number, _)) = dimensions(value).filter(|_| crate::switches::SPACING_FN_FOLD.on()) {
        if number == 0.0 {
            return Ok(if crate::switches::SPACING_ZERO_PX.on() { "0px" } else { "0" }.to_string());
        }
        if number == 1.0 {
            return Ok(multiplier);
        }
    }

    Ok(format!("calc({multiplier} * {value})"))
}

fn theme(design_system: &DesignSystem, source: &AstNode, args: &[&str]) -> Result<String> {
    let mut path = args.first().copied().unwrap_or("");
    let fallback = &args[args.len().min(1)..];

    if !path.starts_with("--") {
        return Err(Error::new("The --theme(…) function can only be used with CSS variables from your theme."));
    }

    let mut inline = false;

    // `--theme(--color-red-500 inline)` gives the value, not the variable.
    if path.ends_with(" inline") {
        inline = true;
        path = &path[..path.len() - 7];
    }

    // A variable does not work in the parameters of an at-rule.
    if matches!(source, AstNode::AtRule(_)) {
        inline = true;
    }

    let resolved = design_system.resolve_theme_value(path, inline).filter(|v| !v.is_empty());

    let Some(resolved) = resolved else {
        if !fallback.is_empty() {
            return Ok(fallback.join(", "));
        }
        return Err(Error::new(format!(
            "Could not resolve value for theme function: `theme({path})`. Consider checking if the variable name is correct or provide a fallback value to silence this error."
        )));
    };

    if fallback.is_empty() {
        return Ok(resolved);
    }

    let joined_fallback = fallback.join(", ");
    if joined_fallback == "initial" {
        return Ok(resolved);
    }

    // The value of the theme is `initial`: use the fallback.
    if resolved == "initial" {
        return Ok(joined_fallback);
    }

    // The value is a reference to another variable: give it the fallback.
    if resolved.starts_with("var(") || resolved.starts_with("theme(") || resolved.starts_with("--theme(") {
        let mut value_ast = ValueParser::parse(&resolved);
        inject_fallback_for_initial_fallback(&mut value_ast, &joined_fallback);
        return Ok(ValueParser::to_css(&value_ast));
    }

    Ok(resolved)
}

fn legacy_theme(design_system: &DesignSystem, args: &[&str]) -> Result<String> {
    let path = eventually_unquote(args.first().copied().unwrap_or(""));
    let fallback = &args[args.len().min(1)..];

    let resolved = design_system.resolve_theme_value(&path, true).filter(|v| !v.is_empty());

    match resolved {
        Some(value) => Ok(value),
        None if !fallback.is_empty() => Ok(fallback.join(", ")),
        None => Err(Error::new(format!(
            "Could not resolve value for theme function: `theme({path})`. Consider checking if the path is correct or provide a fallback value to silence this error."
        ))),
    }
}

fn is_css_function(name: &str) -> bool {
    matches!(name, "--alpha" | "--spacing" | "--theme" | "theme")
}

/// True when the text can have a call of a function of Tailwind CSS.
pub fn has_theme_function_invocation(value: &str) -> bool {
    // `theme(` also matches `--theme(`.
    value.contains("theme(") || value.contains("--alpha(") || value.contains("--spacing(")
}

/// Replace each function of Tailwind CSS in the tree with its value.
pub fn substitute_functions(ast: &mut Vec<AstNode>, design_system: &DesignSystem) -> Result<Features> {
    let mut features = FEATURES_NONE;
    let mut error: Option<Error> = None;

    walk(ast, |node, _| {
        let new_value = match &*node {
            // A declaration: `color: theme(--color-red-500)`.
            AstNode::Declaration(d) => match &d.value {
                Some(value) if has_theme_function_invocation(value) => {
                    Some(substitute_functions_in_value(value, node, design_system))
                }
                _ => None,
            },
            // Some at-rules: `@media (min-width: theme(--breakpoint-md))`.
            AstNode::AtRule(r)
                if matches!(r.name.as_str(), "@media" | "@custom-media" | "@container" | "@supports")
                    && has_theme_function_invocation(&r.params) =>
            {
                Some(substitute_functions_in_value(&r.params, node, design_system))
            }
            _ => None,
        };

        match new_value {
            None => WalkAction::Continue,
            Some(Err(e)) => {
                error = Some(e);
                WalkAction::Stop
            }
            Some(Ok(value)) => {
                features |= FEATURES_THEME_FUNCTION;
                match node {
                    AstNode::Declaration(d) => d.value = Some(value),
                    AstNode::AtRule(r) => r.params = value,
                    _ => {}
                }
                WalkAction::Continue
            }
        }
    });

    match error {
        Some(e) => Err(e),
        None => Ok(features),
    }
}

/// Replace each function of Tailwind CSS in one value.
pub fn substitute_functions_in_value(value: &str, source: &AstNode, design_system: &DesignSystem) -> Result<String> {
    let mut ast = ValueParser::parse(value);
    let mut error: Option<Error> = None;

    walk(&mut ast, |node, _| {
        let ValueAstNode::Function(f) = &*node else { return WalkAction::Continue };
        if !is_css_function(&f.value) {
            return WalkAction::Continue;
        }
        let css = ValueParser::to_css(&f.nodes);
        let args: Vec<&str> = segment(js_trim(&css), b',').into_iter().map(js_trim).collect();
        let result = match f.value.as_str() {
            "--alpha" => alpha(&args),
            "--spacing" => spacing(design_system, &args),
            "--theme" => theme(design_system, source, &args),
            _ => legacy_theme(design_system, &args),
        };
        match result {
            Ok(result) => WalkAction::Replace(ValueParser::parse(&result)),
            Err(e) => {
                error = Some(e);
                WalkAction::Stop
            }
        }
    });

    match error {
        Some(e) => Err(e),
        None => Ok(ValueParser::to_css(&ast)),
    }
}

fn eventually_unquote(value: &str) -> String {
    let mut chars = value.chars();
    let quote = match chars.next() {
        Some(c @ ('\'' | '"')) => c,
        _ => return value.to_string(),
    };

    // The characters between the first one and the last one.
    let inner: Vec<char> = value.chars().collect();
    let mut unquoted = String::new();
    let mut i = 1;
    while i + 1 < inner.len() {
        let current = inner[i];
        let next = inner[i + 1];
        if current == '\\' && (next == quote || next == '\\') {
            unquoted.push(next);
            i += 1;
        } else {
            unquoted.push(current);
        }
        i += 1;
    }
    unquoted
}

fn inject_fallback_for_initial_fallback(ast: &mut Vec<ValueAstNode>, fallback: &str) {
    walk(ast, |node, _| {
        let ValueAstNode::Function(f) = node else { return WalkAction::Continue };
        if f.value != "var" && f.value != "theme" && f.value != "--theme" {
            return WalkAction::Continue;
        }
        if f.nodes.len() == 1 {
            f.nodes.push(ValueParser::word(format!(", {fallback}")));
        } else if let Some(ValueAstNode::Word(last)) = f.nodes.last_mut() {
            if last.value == "initial" {
                last.value = fallback.to_string();
            }
        }
        WalkAction::Continue
    });
}
