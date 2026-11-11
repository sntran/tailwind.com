//! The utilities of `@utility`: `createCssUtility` of `utilities.ts` of
//! Tailwind CSS.
//!
//! A static utility (`@utility foo`) gives its nodes as they are. A
//! functional utility (`@utility foo-*`) resolves each `--value(…)` and
//! `--modifier(…)` of its declarations for a candidate.
//!
//! Upstream keeps a reference to the `@utility` node of the stylesheet. The
//! compiler changes this node after the registration (`@apply`, `theme(…)`,
//! `--spacing(…)`, `--alpha(…)`), and the utility gives the changed nodes.
//! `create_css_utility_shared` gives the same behavior with a shared node.

use crate::ast::{AstNode, AtRule};
use crate::candidate::{CandidateModifier, FunctionalCandidate, UtilityValue, ValueKind};
use crate::design_system::DesignSystem;
use crate::utilities::Compiled;
use crate::utils::escape::unescape;
use crate::utils::infer_data_type::{infer_data_type, is_positive_integer, is_valid_spacing_multiplier, DataType};
use crate::utils::js::{is_js_line_terminator, is_js_whitespace, js_is_integer, js_number, js_trim};
use crate::utils::segment::segment;
use crate::value_parser::{self, ValueAstNode, ValueFunctionNode};
use std::cell::RefCell;
use std::rc::Rc;

/// The registration function of an `@utility` rule.
pub type RegisterUtility = Box<dyn Fn(&mut DesignSystem)>;

/// The data types that a bare value can have. Other types would make new
/// syntax, for example `text-#0088cc` for `--value(color)`.
pub const BARE_VALUE_DATA_TYPES: [&str; 4] = ["number", "integer", "ratio", "percentage"];

/// An `@utility` node that the compiler and the utility share.
pub type SharedAtRule = Rc<RefCell<AtRule>>;

/// The registration function of an `@utility` rule. `None` is an invalid
/// utility name. The utility has a copy of the node: a change of `node`
/// after this call has no effect on the utility.
pub fn create_css_utility(node: &AtRule) -> Option<RegisterUtility> {
    create_css_utility_shared(Rc::new(RefCell::new(node.clone())))
}

/// The registration function of an `@utility` rule, for a shared node.
/// `None` is an invalid utility name.
///
/// As upstream, the registration of a functional utility changes the node:
/// it makes the arguments of `--value(…)` and `--modifier(…)` regular. The
/// utility reads the node each time it compiles a candidate, thus it gives
/// each later change of the node. Do not hold a mutable borrow of the node
/// during the compilation of a candidate. The name of the utility is the
/// name at the time of this call.
pub fn create_css_utility_shared(node: SharedAtRule) -> Option<RegisterUtility> {
    // The name can have escapes: `@utility foo-1\/2` is `foo-1/2`.
    let name = unescape(&node.borrow().params).into_owned();

    if is_valid_functional_utility_name(&name) {
        return Some(Box::new(move |design_system: &mut DesignSystem| {
            normalize_declarations(&mut node.borrow_mut().nodes, &mut design_system.warnings.borrow_mut());
            let node = node.clone();
            design_system.utilities.functional_full(
                &name[..name.len() - 2],
                move |candidate, design_system| compile_functional(&node.borrow(), candidate, design_system),
                None,
            );
        }));
    }

    if is_valid_static_utility_name(&name) {
        return Some(Box::new(move |design_system: &mut DesignSystem| {
            let node = node.clone();
            design_system.utilities.static_full(name.clone(), move |_, _| Compiled::Nodes(node.borrow().nodes.clone()));
        }));
    }

    None
}

// ---------------------------------------------------------------------------
// The work at the registration.

/// Make the arguments of each `--value(…)` and `--modifier(…)` regular, in
/// each declaration of the tree. `warnings` gets the texts that upstream
/// gives to `console.warn` for a data type that a bare value cannot have.
fn normalize_declarations(nodes: &mut [AstNode], warnings: &mut Vec<String>) {
    for node in nodes {
        match node {
            AstNode::Declaration(declaration) => {
                let Some(value) = declaration.value.as_deref() else { continue };
                if !value.contains("--value(") && !value.contains("--modifier(") {
                    continue;
                }
                let mut value_ast = value_parser::parse(value);
                normalize_value_functions(&mut value_ast, warnings);
                declaration.value = Some(value_parser::to_css(&value_ast));
            }
            other => {
                if let Some(children) = other.children_mut() {
                    normalize_declarations(children, warnings);
                }
            }
        }
    }
}

fn normalize_value_functions(nodes: &mut [ValueAstNode], warnings: &mut Vec<String>) {
    for node in nodes {
        let ValueAstNode::Function(function) = node else { continue };
        if function.value == "--value" || function.value == "--modifier" {
            let css = value_parser::to_css(&function.nodes);
            let args: Vec<String> = segment(&css, b',').into_iter().map(normalize_argument).collect();
            function.nodes = value_parser::parse(&args.join(","));
            warn_about_bare_value_data_types(function, warnings);
        }
        normalize_value_functions(&mut function.nodes, warnings);
    }
}

/// The warning of upstream for a word of `--value(…)` or `--modifier(…)`
/// that is not a literal, not a theme key, not an arbitrary value, and not
/// a data type of a bare value: two calls of `console.warn`.
fn warn_about_bare_value_data_types(function: &ValueFunctionNode, warnings: &mut Vec<String>) {
    let utf16_len = |text: &str| text.encode_utf16().count();
    for (index, node) in function.nodes.iter().enumerate() {
        let ValueAstNode::Word(word) = node else { continue };
        let value = word.value.as_str();
        let first = value.chars().next();
        // A literal: the same quote at the two ends.
        if matches!(first, Some('"' | '\'')) && value.chars().next_back() == first {
            continue;
        }
        if value.starts_with("--") || (value.starts_with('[') && value.ends_with(']')) || BARE_VALUE_DATA_TYPES.contains(&value) {
            continue;
        }
        let types: Vec<String> = BARE_VALUE_DATA_TYPES.iter().map(|name| format!("\"{name}\"")).collect();
        warnings.push(format!("Unsupported bare value data type: \"{value}\".\nOnly valid data types are: {}.\n", types.join(", ")));
        let offset = utf16_len(&function.value) + 1 + utf16_len(&value_parser::to_css(&function.nodes[..index]));
        warnings.push(format!(
            "```css\n{}\n{}{}\n```",
            value_parser::to_css(std::slice::from_ref(&ValueAstNode::Function(function.clone()))),
            " ".repeat(offset),
            "^".repeat(utf16_len(value)),
        ));
    }
}

/// One argument of `--value(…)`:
/// - `--spacing` gives `--spacing-*`
/// - `--text- * --line-height` and `--text --line-height` give
///   `--text-*--line-height`
/// - `[ *]` gives `[*]`
fn normalize_argument(arg: &str) -> String {
    let arg = js_trim(arg);

    // The value of `--default(…)` goes to the output as it is.
    if arg.starts_with("--default(") {
        return arg.to_string();
    }

    // An escaped `\*` is `*`.
    let arg = arg.replace("\\*", "*");

    // `--foo --bar` is `--foo-*--bar`.
    let arg = join_theme_keys(&arg);

    // No whitespace.
    let arg: String = arg.chars().filter(|c| !is_js_whitespace(*c)).collect();

    // Two or more `-*` in sequence are one `-*`.
    let mut arg = collapse_wildcards(&arg);

    // A theme key ends in `-*`.
    if arg.starts_with("--") && !arg.contains('(') && !arg.contains("-*") {
        arg.push_str("-*");
    }

    arg
}

/// Upstream: `arg.replace(/--(.*?)\s--(.*?)/g, '--$1-*--$2')`.
fn join_theme_keys(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + 4);
    let mut pos = 0;
    while pos < input.len() {
        let rest = &input[pos..];
        if let Some(tail) = rest.strip_prefix("--") {
            if let Some(len) = theme_key_gap(tail) {
                // `tail[..len]` is the first group. One whitespace character
                // and `--` come after it.
                let group = &tail[..len];
                let space = tail[len..].chars().next().map_or(0, char::len_utf8);
                out.push_str("--");
                out.push_str(group);
                out.push_str("-*--");
                pos += 2 + len + space + 2;
                continue;
            }
        }
        let c = rest.chars().next().unwrap();
        out.push(c);
        pos += c.len_utf8();
    }
    out
}

/// The length of the shortest text with no line terminator that has a
/// whitespace character and `--` after it.
fn theme_key_gap(text: &str) -> Option<usize> {
    for (idx, c) in text.char_indices() {
        if is_js_whitespace(c) && text[idx + c.len_utf8()..].starts_with("--") {
            return Some(idx);
        }
        if is_js_line_terminator(c) {
            return None;
        }
    }
    None
}

/// Upstream: `arg.replace(/(-\*){2,}/g, '-*')`.
fn collapse_wildcards(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while !rest.is_empty() {
        let mut count = 0;
        while rest[count * 2..].starts_with("-*") {
            count += 1;
        }
        if count >= 2 {
            out.push_str("-*");
            rest = &rest[count * 2..];
        } else {
            let c = rest.chars().next().unwrap();
            out.push(c);
            rest = &rest[c.len_utf8()..];
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The compile function of a functional utility.

struct CompileState<'a> {
    value: Option<&'a UtilityValue>,
    modifier: Option<&'a CandidateModifier>,
    design_system: &'a DesignSystem,
    /// `--value(…)` is in a declaration.
    used_value_fn: bool,
    /// A `--value(…)` has a result.
    resolved_value_fn: bool,
    /// `--modifier(…)` is in a declaration.
    used_modifier_fn: bool,
    /// A `--modifier(…)` has a result.
    resolved_modifier_fn: bool,
    /// A `--value(ratio)` has a result.
    resolved_ratio_value: bool,
    /// For each declaration that stays in the tree, in the sequence of the
    /// tree: `true` when a `--value(…)` that is not a ratio has a result
    /// in it.
    resolved_declarations: Vec<bool>,
}

fn compile_functional(node: &AtRule, candidate: &FunctionalCandidate, design_system: &DesignSystem) -> Compiled {
    let mut at_rule = node.clone();
    let mut state = CompileState {
        value: candidate.value.as_ref(),
        modifier: candidate.modifier.as_ref(),
        design_system,
        used_value_fn: false,
        resolved_value_fn: false,
        used_modifier_fn: false,
        resolved_modifier_fn: false,
        resolved_ratio_value: false,
        resolved_declarations: Vec::new(),
    };

    resolve_declarations(&mut at_rule.nodes, true, &mut state);

    // One `--value(…)` must have a result.
    if !state.used_value_fn || !state.resolved_value_fn {
        return Compiled::Null;
    }

    // The candidate has a modifier, and no `--modifier(…)` has a result.
    if state.used_modifier_fn && !state.resolved_modifier_fn && state.modifier.is_some() {
        return Compiled::Null;
    }

    // A ratio and a modifier together are not valid.
    if state.resolved_ratio_value && state.resolved_modifier_fn {
        return Compiled::Null;
    }

    // The modifier of the candidate must go to a `--modifier(…)` or to a
    // `--value(ratio)`.
    if state.modifier.is_some() && !state.resolved_ratio_value && !state.resolved_modifier_fn {
        return Compiled::Null;
    }

    // A ratio has a result: remove the declarations that have a result
    // which is not a ratio. `--value(number)` also has a result for
    // `foo-1/2`.
    if state.resolved_ratio_value {
        let mut index = 0;
        remove_resolved_declarations(&mut at_rule.nodes, &state.resolved_declarations, &mut index);
    }

    Compiled::Nodes(at_rule.nodes)
}

/// Resolve the functions of each declaration whose parent is a rule or an
/// at-rule. A declaration with a function that has no result goes out.
fn resolve_declarations(nodes: &mut Vec<AstNode>, parent_is_rule: bool, state: &mut CompileState) {
    let mut index = 0;
    while index < nodes.len() {
        match &mut nodes[index] {
            AstNode::Declaration(declaration) => {
                let mut resolved = false;
                if parent_is_rule {
                    if let Some(value) = declaration.value.as_deref().filter(|v| !v.is_empty()) {
                        let mut value_ast = value_parser::parse(value);
                        if !resolve_value_functions(&mut value_ast, state, &mut resolved) {
                            nodes.remove(index);
                            continue;
                        }
                        declaration.value = Some(value_parser::to_css(&value_ast));
                    }
                }
                state.resolved_declarations.push(resolved);
            }
            node => {
                let is_rule = matches!(node, AstNode::Rule(_) | AstNode::AtRule(_));
                if let Some(children) = node.children_mut() {
                    resolve_declarations(children, is_rule, state);
                }
            }
        }
        index += 1;
    }
}

/// Replace each `--value(…)` and `--modifier(…)` with its result. The result
/// is `false` when a function has no result: the walk stops there.
fn resolve_value_functions(nodes: &mut Vec<ValueAstNode>, state: &mut CompileState, resolved_declaration: &mut bool) -> bool {
    let mut index = 0;
    while index < nodes.len() {
        let ValueAstNode::Function(function) = &mut nodes[index] else {
            index += 1;
            continue;
        };

        if function.value == "--value" {
            state.used_value_fn = true;
            let Some(resolved) = resolve_value_function(state.value.map(FunctionValue::from), function, state.design_system)
            else {
                return false;
            };
            state.resolved_value_fn = true;
            if resolved.ratio {
                state.resolved_ratio_value = true;
            } else {
                *resolved_declaration = true;
            }
            let count = resolved.nodes.len();
            nodes.splice(index..=index, resolved.nodes);
            index += count;
        } else if function.value == "--modifier" {
            state.used_modifier_fn = true;
            let Some(resolved) = resolve_value_function(state.modifier.map(FunctionValue::from), function, state.design_system)
            else {
                return false;
            };
            state.resolved_modifier_fn = true;
            let count = resolved.nodes.len();
            nodes.splice(index..=index, resolved.nodes);
            index += count;
        } else {
            if !resolve_value_functions(&mut function.nodes, state, resolved_declaration) {
                return false;
            }
            index += 1;
        }
    }
    true
}

/// Remove the declarations with a mark. The walk has the same sequence as
/// `resolve_declarations`.
fn remove_resolved_declarations(nodes: &mut Vec<AstNode>, marks: &[bool], index: &mut usize) {
    nodes.retain_mut(|node| match node {
        AstNode::Declaration(_) => {
            let remove = marks.get(*index).copied().unwrap_or(false);
            *index += 1;
            !remove
        }
        node => {
            if let Some(children) = node.children_mut() {
                remove_resolved_declarations(children, marks, index);
            }
            true
        }
    });
}

/// The value or the modifier of a candidate, as `resolve_value_function`
/// reads it.
struct FunctionValue<'a> {
    named: bool,
    value: &'a str,
    /// `Some` when the upstream object has the `fraction` key.
    fraction: Option<Option<&'a str>>,
    data_type: Option<&'a str>,
}

impl<'a> From<&'a UtilityValue> for FunctionValue<'a> {
    fn from(value: &'a UtilityValue) -> Self {
        match value {
            UtilityValue::Named { value, fraction } => {
                FunctionValue { named: true, value, fraction: Some(fraction.as_deref()), data_type: None }
            }
            UtilityValue::Arbitrary { data_type, value } => {
                FunctionValue { named: false, value, fraction: None, data_type: data_type.as_deref() }
            }
        }
    }
}

impl<'a> From<&'a CandidateModifier> for FunctionValue<'a> {
    fn from(modifier: &'a CandidateModifier) -> Self {
        FunctionValue { named: modifier.kind == ValueKind::Named, value: &modifier.value, fraction: None, data_type: None }
    }
}

struct Resolved {
    nodes: Vec<ValueAstNode>,
    ratio: bool,
}

impl Resolved {
    fn parse(value: &str) -> Option<Self> {
        Some(Resolved { nodes: value_parser::parse(value), ratio: false })
    }
}

/// The text between the quotes of a word such as `'foo'` or `"foo"`.
fn quoted(word: &str) -> Option<&str> {
    let first = word.chars().next()?;
    if (first != '\'' && first != '"') || !word.ends_with(first) {
        return None;
    }
    // A word of one quote character gives the empty text.
    Some(if word.len() < 2 { "" } else { &word[1..word.len() - 1] })
}

/// The result of one `--value(…)` or `--modifier(…)` for a value. `None` is
/// "no result".
fn resolve_value_function(
    value: Option<FunctionValue>,
    function: &ValueFunctionNode,
    design_system: &DesignSystem,
) -> Option<Resolved> {
    // No value: use `--default(…)`.
    let Some(value) = value else {
        for arg in &function.nodes {
            if let ValueAstNode::Function(default) = arg {
                if default.value == "--default" {
                    return Some(Resolved { nodes: default.nodes.clone(), ratio: false });
                }
            }
        }
        return None;
    };

    for arg in &function.nodes {
        let ValueAstNode::Word(word) = arg else { continue };
        let arg = word.value.as_str();

        if value.named {
            // A literal: `--modifier('closest-side')`.
            if quoted(arg) == Some(value.value) {
                return Resolved::parse(value.value);
            }

            // A theme key: `--value(--color-*)`.
            if arg.starts_with("--") {
                if let Some(theme_key) = arg.strip_suffix("-*") {
                    if let Some(resolved) =
                        design_system.theme.resolve(Some(value.value), &[theme_key]).filter(|r| !r.is_empty())
                    {
                        return Resolved::parse(&resolved);
                    }
                } else {
                    // `--text-*--line-height` is `--text` and `--line-height`.
                    let mut nested_keys: Vec<&str> = arg.split("-*").collect();
                    if nested_keys.len() <= 1 {
                        continue;
                    }
                    let theme_key = nested_keys.remove(0);
                    if let Some((_, options)) = design_system.theme.resolve_with(value.value, &[theme_key], &nested_keys) {
                        let last = nested_keys[nested_keys.len() - 1];
                        if let Some((_, resolved)) =
                            options.iter().rev().find(|(name, resolved)| name == last && !resolved.is_empty())
                        {
                            return Resolved::parse(resolved);
                        }
                    }
                }
                continue;
            }

            // A bare value: `--value(integer)`.
            if !BARE_VALUE_DATA_TYPES.contains(&arg) {
                continue;
            }

            let resolved = match (arg, value.fraction) {
                ("ratio", Some(fraction)) => fraction,
                _ => Some(value.value),
            };
            let Some(resolved) = resolved.filter(|r| !r.is_empty()) else { continue };

            let Some(data_type) = DataType::from_name(arg).and_then(|ty| infer_data_type(resolved, &[ty])) else {
                continue;
            };

            match data_type {
                // A ratio is `<integer>/<integer>`.
                DataType::Ratio => {
                    let parts = segment(resolved, b'/');
                    let is_integer = |part: Option<&&str>| {
                        part.is_some_and(|p| {
                            let number = js_number(p);
                            js_is_integer(number) && number >= 0.0
                        })
                    };
                    if !is_integer(parts.first()) || !is_integer(parts.get(1)) {
                        continue;
                    }
                    let ratio = format!("{} / {}", js_trim(parts[0]), js_trim(parts[1]));
                    return Some(Resolved { nodes: value_parser::parse(&ratio), ratio: true });
                }
                // A number is a multiple of 0.25: `1.5`.
                DataType::Number if !is_valid_spacing_multiplier(resolved) => continue,
                // A percentage is an integer: `50%`.
                DataType::Percentage => {
                    let mut number = resolved.chars();
                    number.next_back();
                    if !is_positive_integer(number.as_str()) {
                        continue;
                    }
                }
                _ => {}
            }

            return Resolved::parse(resolved);
        }

        // An arbitrary value: `--value([integer])`.
        if arg.len() >= 2 && arg.starts_with('[') && arg.ends_with(']') {
            let data_type = &arg[1..arg.len() - 1];

            // Each data type is permitted: `--value([*])`.
            if data_type == "*" {
                return Resolved::parse(value.value);
            }

            match value.data_type.filter(|hint| !hint.is_empty()) {
                // The hint of the candidate must be the type of the argument:
                // `tab-[color:var(--x)]` does not match `--value([integer])`.
                Some(hint) if hint != data_type => continue,
                Some(_) => return Resolved::parse(value.value),
                None => {}
            }

            // No hint: find the type.
            if DataType::from_name(data_type).and_then(|ty| infer_data_type(value.value, &[ty])).is_some() {
                return Resolved::parse(value.value);
            }
        }
    }

    None
}

// ---------------------------------------------------------------------------
// The names.

/// The length of the match of `/^-?[a-z][a-zA-Z0-9_-]*/`.
fn utility_root_length(name: &str) -> Option<usize> {
    let bytes = name.as_bytes();
    let mut pos = usize::from(bytes.first() == Some(&b'-'));
    if !bytes.get(pos).is_some_and(u8::is_ascii_lowercase) {
        return None;
    }
    pos += 1;
    while bytes.get(pos).is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_' || *b == b'-') {
        pos += 1;
    }
    Some(pos)
}

/// `true` for the name of a static utility: `tab-size`, `p-1.5`, `w-50%`,
/// `text-xs/8`.
pub fn is_valid_static_utility_name(name: &str) -> bool {
    let Some(root_length) = utility_root_length(name) else { return false };
    let (root, value) = name.as_bytes().split_at(root_length);

    if value.is_empty() {
        // A root with no value must not end in `-`.
        return !root.ends_with(b"-");
    }

    let mut seen_slash = false;
    for (i, byte) in value.iter().enumerate() {
        // The root has one character or more.
        let previous = if i > 0 { value[i - 1] } else { root[root.len() - 1] };
        match byte {
            b'%' => {
                // Only at the end, and after a digit.
                if i != value.len() - 1 || !previous.is_ascii_digit() {
                    return false;
                }
            }
            b'/' => {
                // One slash only, with a character after it.
                if i == value.len() - 1 || seen_slash {
                    return false;
                }
                seen_slash = true;
            }
            b'.' => {
                // After a digit. Upstream does not reject a dot at the end.
                if !previous.is_ascii_digit() {
                    return false;
                }
                if value.get(i + 1).is_some_and(|next| !next.is_ascii_digit()) {
                    return false;
                }
            }
            b'_' | b'-' => {}
            other => {
                if !other.is_ascii_alphanumeric() {
                    return false;
                }
            }
        }
    }

    true
}

/// `true` for the name of a functional utility: `tab-size-*`.
pub fn is_valid_functional_utility_name(name: &str) -> bool {
    let Some(name) = name.strip_suffix("-*") else { return false };
    // The root is the full name. A root that ends in `-` is valid, so a
    // candidate such as `foo--bar` is possible.
    utility_root_length(name) == Some(name.len())
}
