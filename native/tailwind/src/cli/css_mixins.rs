//! The lowering of CSS functions and mixins for the optimizer
//! (`docs/MIXINS.md`): `@function` and the calls `--name(…)`, `@mixin` and
//! `@apply --name(…)`, from CSS Custom Functions and Mixins Module Level 1
//! (the Editor's Draft of 2026-11-04).
//!
//! Tailwind CSS keeps these rules in its output, and Lightning CSS does not
//! know them. This step is a part of Lightning CSS in this program: it runs
//! before Lightning CSS, for `--minify` and `--optimize`, when a target
//! browser of the optimizer does not have the rule. The targets are those
//! of Tailwind CSS (`optimize_css::TARGETS`). When Lightning CSS has its own
//! lowering, this step goes.
//!
//! The step changes nothing in a stylesheet with no `@function` and no
//! `@mixin`.

use crate::ast::{AstNode, AtRule, Declaration, Loc};
use crate::css_parser::parse_with_source;
use crate::hash::{HashMap, HashSet};
use crate::source_maps::create_source_map;
use crate::to_css::{to_css, to_css_tracked};
use crate::utils::segment::segment;
use lightningcss::targets::Browsers;
use std::cell::RefCell;
use std::rc::Rc;

use super::source_maps::serialize_source_map;

/// The first version of each browser with a rule, in the encoding of
/// Lightning CSS (`major << 16 | minor << 8`). `None`: no version has it.
struct Support {
    android: Option<u32>,
    chrome: Option<u32>,
    edge: Option<u32>,
    firefox: Option<u32>,
    ie: Option<u32>,
    ios_saf: Option<u32>,
    opera: Option<u32>,
    safari: Option<u32>,
    samsung: Option<u32>,
}

const NO_BROWSER: Support = Support {
    android: None,
    chrome: None,
    edge: None,
    firefox: None,
    ie: None,
    ios_saf: None,
    opera: None,
    safari: None,
    samsung: None,
};

/// `@function`: Chrome and Edge 139 (the data of MDN).
const FUNCTIONS: Support =
    Support { android: Some(139 << 16), chrome: Some(139 << 16), edge: Some(139 << 16), opera: Some(123 << 16), ..NO_BROWSER };

/// `@mixin` and `@apply`: no browser yet.
const MIXINS: Support = NO_BROWSER;

/// True when a target browser does not have the rule.
fn needs_lowering(targets: &Browsers, support: &Support) -> bool {
    let pairs = [
        (targets.android, support.android),
        (targets.chrome, support.chrome),
        (targets.edge, support.edge),
        (targets.firefox, support.firefox),
        (targets.ie, support.ie),
        (targets.ios_saf, support.ios_saf),
        (targets.opera, support.opera),
        (targets.safari, support.safari),
        (targets.samsung, support.samsung),
    ];
    pairs.iter().any(|pair| match *pair {
        (Some(_), None) => true,
        (Some(target), Some(first)) => target < first,
        (None, _) => false,
    })
}

/// The result of the step: the CSS, and with a map, the source map from the
/// result to the input as JSON text.
pub struct Lowered {
    pub code: String,
    pub map: Option<String>,
}

/// Lower the functions and the mixins of `input` for the browsers
/// `targets`. `file` is the name of the input for the source map. `None`:
/// there is nothing to lower, and the input does not change.
pub fn lower(input: &str, file: &str, targets: &Browsers, with_map: bool) -> Option<Lowered> {
    let functions = input.contains("@function") && needs_lowering(targets, &FUNCTIONS);
    let mixins = input.contains("@mixin") && needs_lowering(targets, &MIXINS);
    if !functions && !mixins {
        return None;
    }
    let from = with_map.then_some(file).filter(|file| !file.is_empty());
    let mut ast = parse_with_source(input, from).ok()?;

    let mut lowering = Lowering::default();
    lowering.collect(&mut ast, functions, mixins, false);
    if lowering.functions.is_empty() && lowering.mixins.is_empty() {
        return None;
    }
    if !lowering.mixins.is_empty() {
        ast = lowering.expand(ast, false);
    }
    if !lowering.functions.is_empty() {
        lowering.lower_calls(&mut ast, false, false);
    }
    ast.append(&mut lowering.properties);

    if from.is_some() {
        let code = to_css_tracked(&mut ast);
        let map = serialize_source_map(&create_source_map(&ast));
        Some(Lowered { code, map: Some(map) })
    } else {
        Some(Lowered { code: to_css(&ast), map: None })
    }
}

/// A parameter of a function or a mixin: `--name <type>: default`.
#[derive(Debug, Clone)]
struct Param {
    name: String,
    /// The syntax of the type, as `<length>`: `type(…)` has no `type`.
    syntax: Option<String>,
    /// The default value. `None`: the guaranteed-invalid value.
    default: Option<String>,
}

/// One set of conditions of the body of a function: the declarations that
/// apply when the conditions are true.
#[derive(Debug, Clone)]
struct Case {
    /// The conditional rules, from the outside in: (`@media`, `(…)`).
    conditions: Vec<(String, String)>,
    /// The local variables: the last declaration of each name.
    locals: HashMap<String, String>,
    /// The last `result`. `None`: the guaranteed-invalid value.
    result: Option<String>,
}

#[derive(Debug)]
struct Function {
    params: Vec<Param>,
    /// The case with no condition first, then one case for each
    /// conditional rule with a declaration, in the sequence of the body.
    cases: Vec<Case>,
}

#[derive(Debug)]
struct Mixin {
    params: Vec<Param>,
    body: Vec<AstNode>,
}

/// One possible result of a text: the text when the conditions are true.
/// The first variant of a list has no condition. `None` is the
/// guaranteed-invalid value.
#[derive(Debug, Clone, PartialEq)]
struct Variant {
    conditions: Vec<(String, String)>,
    text: Option<String>,
}

fn plain(text: impl Into<String>) -> Variant {
    Variant { conditions: Vec::new(), text: Some(text.into()) }
}

fn invalid() -> Variant {
    Variant { conditions: Vec::new(), text: None }
}

/// The variants of a sequence of parts. Each part has its variants. The
/// first variant is the first of each part. Then each other variant of a
/// part comes with the first variant of the other parts: two conditions
/// that are true together give the later variant only.
fn concat(parts: Vec<Vec<Variant>>) -> Vec<Variant> {
    let join = |pick: &dyn Fn(usize) -> Option<String>| -> Option<String> {
        let mut text = String::new();
        for index in 0..parts.len() {
            text.push_str(&pick(index)?);
        }
        Some(text)
    };
    let mut out = vec![Variant { conditions: Vec::new(), text: join(&|j| parts[j][0].text.clone()) }];
    for (index, part) in parts.iter().enumerate() {
        for variant in &part[1..] {
            let text = join(&|j| if j == index { variant.text.clone() } else { parts[j][0].text.clone() });
            out.push(Variant { conditions: variant.conditions.clone(), text });
        }
    }
    out
}

/// The value of a name in a scope.
enum Binding {
    /// Not a name of the scope: the `var()` stays.
    Keep,
    /// The guaranteed-invalid value.
    Invalid,
    /// Another name: the custom property of an argument of a mixin.
    Rename(String),
    Value(Vec<Variant>),
}

/// The scope of a call of a function.
struct Frame {
    params: HashMap<String, Option<String>>,
    locals: HashMap<String, String>,
    cache: RefCell<HashMap<String, Vec<Variant>>>,
    visiting: RefCell<HashSet<String>>,
}

/// The scope of one `@apply` of a mixin: the arguments and the private
/// properties.
struct MixinFrame {
    values: HashMap<String, Option<String>>,
    syntaxes: HashMap<String, String>,
    /// The custom properties for the nested rules of other elements: the
    /// name of the argument and the name of its property, in the sequence
    /// of first use.
    properties: RefCell<Vec<(String, String)>>,
    id: RefCell<Option<usize>>,
}

enum Env<'a> {
    Root,
    Function(&'a Frame),
    /// The frame, and true when the rule is for the element of the `@apply`.
    Mixin(&'a MixinFrame, bool),
}

/// The most calls of functions or mixins in one stack: more is a cycle.
const MAX_DEPTH: usize = 32;

#[derive(Default)]
struct Lowering {
    functions: HashMap<String, Rc<Function>>,
    mixins: HashMap<String, Rc<Mixin>>,
    /// The `@property` rules of the typed arguments of mixins.
    properties: Vec<AstNode>,
    /// The names of the functions and mixins of the current stack.
    stack: Vec<String>,
    next_id: usize,
}

impl Lowering {
    /// Take each `@function` and `@mixin` out of the tree. A rule of a
    /// later place replaces a rule with the same name.
    fn collect(&mut self, nodes: &mut Vec<AstNode>, functions: bool, mixins: bool, in_style: bool) {
        nodes.retain_mut(|node| {
            match node {
                AstNode::AtRule(rule) if !in_style && functions && rule.name.eq_ignore_ascii_case("@function") => {
                    if let Some((name, function)) = parse_function(rule) {
                        self.functions.insert(name, Rc::new(function));
                        return false;
                    }
                }
                AstNode::AtRule(rule) if !in_style && mixins && rule.name.eq_ignore_ascii_case("@mixin") => {
                    if let Some((name, params, _)) = parse_prelude(&rule.params, true) {
                        self.mixins.insert(name, Rc::new(Mixin { params, body: std::mem::take(&mut rule.nodes) }));
                        return false;
                    }
                }
                AstNode::Rule(rule) => self.collect(&mut rule.nodes, functions, mixins, true),
                AstNode::AtRule(rule) => self.collect(&mut rule.nodes, functions, mixins, in_style),
                _ => {}
            }
            true
        });
    }

    // The mixins.

    /// Replace each `@apply` of a known mixin in a style rule.
    fn expand(&mut self, nodes: Vec<AstNode>, in_style: bool) -> Vec<AstNode> {
        let mut out = Vec::with_capacity(nodes.len());
        for node in nodes {
            match node {
                AstNode::AtRule(rule) if in_style && rule.name.eq_ignore_ascii_case("@apply") => match self.apply(&rule, None) {
                    Some(nodes) => out.extend(nodes),
                    None => out.push(AstNode::AtRule(rule)),
                },
                AstNode::Rule(mut rule) => {
                    rule.nodes = self.expand(std::mem::take(&mut rule.nodes), true);
                    out.push(AstNode::Rule(rule));
                }
                AstNode::AtRule(mut rule) => {
                    rule.nodes = self.expand(std::mem::take(&mut rule.nodes), in_style);
                    out.push(AstNode::AtRule(rule));
                }
                other => out.push(other),
            }
        }
        out
    }

    /// The nodes of one `@apply`. `caller` is the scope of the mixin with
    /// this `@apply` in its body. `None`: not a known mixin, and the rule
    /// stays.
    fn apply(&mut self, rule: &AtRule, caller: Option<(&MixinFrame, bool)>) -> Option<Vec<AstNode>> {
        let (name, args) = parse_call(&rule.params)?;
        let mixin = self.mixins.get(name)?.clone();
        // A cycle, or too many arguments: the `@apply` does nothing.
        if self.stack.iter().any(|entry| entry == name) || self.stack.len() >= MAX_DEPTH || args.len() > mixin.params.len() {
            return Some(Vec::new());
        }
        let args: Vec<Option<String>> = args
            .iter()
            .map(|arg| match caller {
                Some((frame, same)) => self.scan(arg, &Env::Mixin(frame, same), false).swap_remove(0).text,
                None => Some(arg.to_string()),
            })
            .collect();

        // The block of the `@apply` is in the scope of the caller. An empty
        // block is no block: Tailwind CSS prints `@apply --m {}` as
        // `@apply --m;`.
        let contents = (!rule.nodes.is_empty()).then(|| match caller {
            Some((frame, same)) => self.instantiate(rule.nodes.clone(), frame, same, &None),
            None => self.expand(rule.nodes.clone(), true),
        });

        let mut frame = MixinFrame {
            values: HashMap::default(),
            syntaxes: HashMap::default(),
            properties: RefCell::new(Vec::new()),
            id: RefCell::new(None),
        };
        for (index, param) in mixin.params.iter().enumerate() {
            let value = if index < args.len() { args[index].clone() } else { param.default.clone() };
            frame.values.insert(param.name.clone(), value);
            if let Some(syntax) = &param.syntax {
                frame.syntaxes.insert(param.name.clone(), syntax.clone());
            }
        }
        // The private properties: in the sequence of the body, each one
        // sees the arguments and the private properties before it.
        for node in &mixin.body {
            let AstNode::AtRule(private) = node else { continue };
            if !private.name.eq_ignore_ascii_case("@private") {
                continue;
            }
            for child in &private.nodes {
                let AstNode::Declaration(declaration) = child else { continue };
                if !declaration.property.starts_with("--") {
                    continue;
                }
                let value = declaration.value.as_deref().unwrap_or("");
                let value = self.scan(value, &Env::Mixin(&frame, true), false).swap_remove(0).text;
                frame.values.insert(declaration.property.clone(), value);
                frame.syntaxes.remove(&declaration.property);
            }
        }

        self.stack.push(name.to_string());
        let body = self.instantiate(mixin.body.clone(), &frame, true, &contents);
        self.stack.pop();

        let mut out = Vec::new();
        for (param, property) in frame.properties.borrow().iter() {
            let Some(Some(value)) = frame.values.get(param) else { continue };
            out.push(AstNode::Declaration(Declaration {
                property: property.clone(),
                value: Some(value.clone()),
                important: false,
                loc: rule.loc.clone(),
            }));
            if let Some(initial) = frame.syntaxes.get(param).and_then(|syntax| initial_value(syntax).map(|v| (syntax, v))) {
                self.properties.push(registration(property, initial.0, initial.1, &rule.loc));
            }
        }
        out.extend(body);
        Some(out)
    }

    /// The body of a mixin in the scope `frame`. `same`: the nodes are for
    /// the element of the `@apply`.
    fn instantiate(&mut self, nodes: Vec<AstNode>, frame: &MixinFrame, same: bool, contents: &Option<Vec<AstNode>>) -> Vec<AstNode> {
        let mut out = Vec::with_capacity(nodes.len());
        for node in nodes {
            match node {
                AstNode::Declaration(mut declaration) => {
                    if let Some(value) = &declaration.value {
                        let text = self.scan(value, &Env::Mixin(frame, same), false).swap_remove(0).text;
                        declaration.value = Some(text.unwrap_or_else(|| invalid_at_computed_value_time(&declaration.property).to_string()));
                    }
                    out.push(AstNode::Declaration(declaration));
                }
                AstNode::Rule(mut rule) => {
                    let same = same && is_same_element(&rule.selector);
                    rule.nodes = self.instantiate(std::mem::take(&mut rule.nodes), frame, same, contents);
                    out.push(AstNode::Rule(rule));
                }
                AstNode::AtRule(mut rule) => {
                    let name = rule.name.to_ascii_lowercase();
                    match name.as_str() {
                        // Not valid in the body of a mixin, or done.
                        "@private" | "@layer" | "@mixin" | "@function" => {}
                        "@contents" => match contents {
                            Some(nodes) => out.extend(nodes.iter().cloned()),
                            None => out.extend(self.instantiate(std::mem::take(&mut rule.nodes), frame, same, &None)),
                        },
                        "@apply" => match self.apply(&rule, Some((frame, same))) {
                            Some(nodes) => out.extend(nodes),
                            None => out.push(AstNode::AtRule(rule)),
                        },
                        _ => {
                            rule.nodes = self.instantiate(std::mem::take(&mut rule.nodes), frame, same, contents);
                            out.push(AstNode::AtRule(rule));
                        }
                    }
                }
                other => out.push(other),
            }
        }
        out
    }

    /// The custom property for an argument of a mixin, in a rule for
    /// another element.
    fn property_name(&mut self, frame: &MixinFrame, name: &str) -> String {
        if let Some((_, property)) = frame.properties.borrow().iter().find(|(param, _)| param == name) {
            return property.clone();
        }
        let id = *frame.id.borrow_mut().get_or_insert_with(|| {
            self.next_id += 1;
            self.next_id
        });
        let property = format!("--mixin-{id}-{}", name.trim_start_matches("--"));
        frame.properties.borrow_mut().push((name.to_string(), property.clone()));
        property
    }

    // The functions.

    /// Replace each call of a known function in the declarations of the
    /// style rules. `in_style`: the nodes are in a style rule. `in_keyframes`:
    /// a nested conditional rule is not valid there.
    fn lower_calls(&mut self, nodes: &mut Vec<AstNode>, in_style: bool, in_keyframes: bool) {
        let mut index = 0;
        while index < nodes.len() {
            let mut added = Vec::new();
            match &mut nodes[index] {
                AstNode::Declaration(declaration) if in_style => {
                    if let Some(value) = &declaration.value {
                        if value.contains("--") && value.contains('(') {
                            let variants = self.scan(value, &Env::Root, true);
                            if variants.len() > 1 || variants[0].text.as_deref() != Some(value.as_str()) {
                                let text = |variant: &Variant| {
                                    variant.text.clone().unwrap_or_else(|| invalid_at_computed_value_time(&declaration.property).to_string())
                                };
                                let mut variants = variants.into_iter();
                                let first = variants.next().map(|variant| text(&variant));
                                if !in_keyframes {
                                    for variant in variants {
                                        let mut node = AstNode::Declaration(Declaration {
                                            property: declaration.property.clone(),
                                            value: Some(text(&variant)),
                                            important: declaration.important,
                                            loc: declaration.loc.clone(),
                                        });
                                        for (name, params) in variant.conditions.iter().rev() {
                                            node = AstNode::AtRule(AtRule {
                                                name: name.clone(),
                                                params: params.clone(),
                                                nodes: vec![node],
                                                loc: declaration.loc.clone(),
                                            });
                                        }
                                        added.push(node);
                                    }
                                }
                                declaration.value = first;
                            }
                        }
                    }
                }
                AstNode::Rule(rule) => self.lower_calls(&mut rule.nodes, true, in_keyframes),
                AstNode::AtRule(rule) => {
                    let keyframes = in_keyframes || rule.name.to_ascii_lowercase().ends_with("keyframes");
                    self.lower_calls(&mut rule.nodes, in_style, keyframes);
                }
                _ => {}
            }
            index += 1;
            let count = added.len();
            nodes.splice(index..index, added);
            index += count;
        }
    }

    /// The variants of one call `--name(args)` of a known function.
    fn call(&mut self, name: &str, args: &str, env: &Env) -> Vec<Variant> {
        let Some(function) = self.functions.get(name).cloned() else { return vec![invalid()] };
        if self.stack.iter().any(|entry| entry == name) || self.stack.len() >= MAX_DEPTH {
            return vec![invalid()];
        }
        let args = split_args(args);
        if args.len() > function.params.len() {
            return vec![invalid()];
        }
        let args: Vec<Vec<Variant>> = args.iter().map(|arg| self.scan(arg, env, true)).collect();
        let first: Vec<Option<String>> = args.iter().map(|variants| variants[0].text.clone()).collect();

        self.stack.push(name.to_string());
        let mut out = self.evaluate(&function, &first);
        for (index, variants) in args.iter().enumerate() {
            for variant in &variants[1..] {
                let mut with = first.clone();
                with[index] = variant.text.clone();
                for mut result in self.evaluate(&function, &with) {
                    result.conditions.splice(0..0, variant.conditions.iter().cloned());
                    out.push(result);
                }
            }
        }
        self.stack.pop();
        out
    }

    /// The variants of the result of a function with these arguments.
    fn evaluate(&mut self, function: &Function, args: &[Option<String>]) -> Vec<Variant> {
        let mut params = HashMap::default();
        for (index, param) in function.params.iter().enumerate() {
            let value = if index < args.len() { args[index].clone() } else { param.default.clone() };
            params.insert(param.name.clone(), value);
        }
        let mut out: Vec<Variant> = Vec::new();
        for (index, case) in function.cases.iter().enumerate() {
            let frame = Frame {
                params: params.clone(),
                locals: case.locals.clone(),
                cache: RefCell::new(HashMap::default()),
                visiting: RefCell::new(HashSet::default()),
            };
            let variants = match &case.result {
                Some(result) => self.scan(result, &Env::Function(&frame), true),
                None => vec![invalid()],
            };
            if index == 0 {
                out = variants;
                continue;
            }
            for mut variant in variants {
                // The same result as with no condition, and nothing before
                // it to replace: no rule.
                if out.len() == 1 && variant.text == out[0].text {
                    continue;
                }
                variant.conditions.splice(0..0, case.conditions.iter().cloned());
                out.push(variant);
            }
        }
        if out.is_empty() {
            out.push(invalid());
        }
        out
    }

    // The text of values.

    /// The value of the name of a `var()` in a scope.
    fn lookup(&mut self, env: &Env, name: &str) -> Binding {
        match env {
            Env::Root => Binding::Keep,
            Env::Function(frame) => {
                if let Some(text) = frame.locals.get(name) {
                    if let Some(variants) = frame.cache.borrow().get(name) {
                        return Binding::Value(variants.clone());
                    }
                    // A local variable that uses itself is not valid.
                    if !frame.visiting.borrow_mut().insert(name.to_string()) {
                        return Binding::Invalid;
                    }
                    let variants = self.scan(text, env, true);
                    frame.visiting.borrow_mut().remove(name);
                    frame.cache.borrow_mut().insert(name.to_string(), variants.clone());
                    return Binding::Value(variants);
                }
                match frame.params.get(name) {
                    Some(Some(text)) => Binding::Value(vec![plain(text.clone())]),
                    Some(None) => Binding::Invalid,
                    None => Binding::Keep,
                }
            }
            Env::Mixin(frame, same) => match frame.values.get(name) {
                Some(Some(_)) if !same => Binding::Rename(self.property_name(frame, name)),
                Some(Some(text)) => Binding::Value(vec![plain(text.clone())]),
                Some(None) => Binding::Invalid,
                None => Binding::Keep,
            },
        }
    }

    /// The variants of a text: each `var()` of a name of the scope gets
    /// its value, and with `calls`, each call of a known function gets its
    /// result. The text of a value from the scope is not read again.
    fn scan(&mut self, text: &str, env: &Env, calls: bool) -> Vec<Variant> {
        let mut parts: Vec<Vec<Variant>> = Vec::new();
        let mut literal = String::new();
        let bytes = text.as_bytes();
        let mut index = 0;
        while index < text.len() {
            let byte = bytes[index];
            if byte == b'"' || byte == b'\'' {
                let end = string_end(text, index);
                literal.push_str(&text[index..end]);
                index = end;
                continue;
            }
            if byte == b'\\' {
                let end = (index + 1 + text[index + 1..].chars().next().map_or(0, char::len_utf8)).min(text.len());
                literal.push_str(&text[index..end]);
                index = end;
                continue;
            }
            let c = text[index..].chars().next().unwrap_or(' ');
            if !is_ident_char(c) {
                literal.push(c);
                index += c.len_utf8();
                continue;
            }
            let end = ident_end(text, index);
            let name = &text[index..end];
            if bytes.get(end) == Some(&b'(') {
                if let Some(close) = matching_paren(text, end) {
                    let inner = &text[end + 1..close];
                    let call = if name.eq_ignore_ascii_case("var") {
                        Some(self.var(inner, env, calls))
                    } else if calls && name.starts_with("--") && self.functions.contains_key(name) {
                        Some(self.call(name, inner, env))
                    } else {
                        None
                    };
                    if let Some(variants) = call {
                        if !literal.is_empty() {
                            parts.push(vec![plain(std::mem::take(&mut literal))]);
                        }
                        parts.push(variants);
                        index = close + 1;
                        continue;
                    }
                    if name.eq_ignore_ascii_case("url") {
                        literal.push_str(&text[index..=close]);
                        index = close + 1;
                        continue;
                    }
                }
            }
            literal.push_str(name);
            index = end;
        }
        if !literal.is_empty() || parts.is_empty() {
            parts.push(vec![plain(literal)]);
        }
        if parts.len() == 1 {
            return parts.pop().unwrap_or_else(|| vec![plain("")]);
        }
        concat(parts)
    }

    /// The variants of `var(inner)`.
    fn var(&mut self, inner: &str, env: &Env, calls: bool) -> Vec<Variant> {
        let (name_part, fallback) = match segment(inner, b',').first() {
            Some(first) if first.len() < inner.len() => (*first, Some(&inner[first.len() + 1..])),
            _ => (inner, None),
        };
        let name = name_part.trim();
        let wrap = |this: &mut Self, name: &str| -> Vec<Variant> {
            match fallback {
                None => vec![plain(format!("var({name})"))],
                Some(fallback) => this
                    .scan(fallback, env, calls)
                    .into_iter()
                    .map(|variant| Variant {
                        // A fallback that is not valid is no fallback.
                        text: Some(match variant.text {
                            Some(text) => format!("var({name},{text})"),
                            None => format!("var({name})"),
                        }),
                        conditions: variant.conditions,
                    })
                    .collect(),
            }
        };
        match self.lookup(env, name) {
            Binding::Keep => wrap(self, name_part),
            Binding::Rename(property) => wrap(self, &property),
            Binding::Value(variants) => variants,
            Binding::Invalid => match fallback {
                Some(fallback) => self.scan(fallback.trim(), env, calls),
                None => vec![invalid()],
            },
        }
    }
}

// The syntax.

/// The value of a property that is invalid at computed-value time.
fn invalid_at_computed_value_time(property: &str) -> &'static str {
    if property.starts_with("--") {
        "initial"
    } else {
        "unset"
    }
}

/// The initial value for the registration of a typed argument. `None`: the
/// type has no simple initial value, and the property is not registered.
fn initial_value(syntax: &str) -> Option<&'static str> {
    Some(match syntax.trim() {
        "<length>" | "<length-percentage>" => "0px",
        "<percentage>" => "0%",
        "<number>" | "<integer>" => "0",
        "<angle>" => "0deg",
        "<time>" => "0s",
        "<color>" => "transparent",
        _ => return None,
    })
}

/// `@property NAME { syntax: …; inherits: true; initial-value: …; }`.
fn registration(property: &str, syntax: &str, initial: &str, loc: &Loc) -> AstNode {
    let declaration = |property: &str, value: String| {
        AstNode::Declaration(Declaration { property: property.to_string(), value: Some(value), important: false, loc: loc.clone() })
    };
    AstNode::AtRule(AtRule {
        name: "@property".to_string(),
        params: property.to_string(),
        nodes: vec![
            declaration("syntax", format!("\"{}\"", syntax.trim())),
            declaration("inherits", "true".to_string()),
            declaration("initial-value", initial.to_string()),
        ],
        loc: loc.clone(),
    })
}

/// True when each selector of a nested rule is for the element of its
/// parent: `&` with only pseudo-classes, classes, IDs and attributes
/// (`&:hover`, `&.on`), and no combinator and no pseudo-element.
fn is_same_element(selector: &str) -> bool {
    segment(selector, b',').iter().all(|part| {
        let part = part.trim();
        let Some(rest) = part.strip_prefix('&') else { return false };
        let mut depth = 0usize;
        let mut previous = ' ';
        for c in rest.chars() {
            match c {
                '(' | '[' => depth += 1,
                ')' | ']' => depth = depth.saturating_sub(1),
                _ if depth > 0 => {}
                ' ' | '\t' | '\n' | '>' | '+' | '~' | '&' => return false,
                ':' if previous == ':' => return false,
                _ => {}
            }
            previous = c;
        }
        true
    })
}

/// The name and the parameters of `@function` or `@mixin`, and the text
/// after the parentheses. A mixin can have no parentheses.
fn parse_prelude(params: &str, mixin: bool) -> Option<(String, Vec<Param>, &str)> {
    let params = params.trim();
    let (name, inner, rest) = match params.find('(') {
        Some(open) => {
            let close = matching_paren(params, open)?;
            (params[..open].trim(), &params[open + 1..close], &params[close + 1..])
        }
        None if mixin => (params, "", ""),
        None => return None,
    };
    if !name.starts_with("--") || name.contains(char::is_whitespace) {
        return None;
    }
    let mut list = Vec::new();
    if !inner.trim().is_empty() {
        for param in segment(inner, b',') {
            let (left, default) = match segment(param, b':').first() {
                Some(first) if first.len() < param.len() => (*first, Some(param[first.len() + 1..].trim().to_string())),
                _ => (param, None),
            };
            let left = left.trim();
            let (name, syntax) = match left.find(char::is_whitespace) {
                Some(space) => (&left[..space], Some(left[space..].trim())),
                None => (left, None),
            };
            if !name.starts_with("--") {
                return None;
            }
            let syntax = syntax.map(|syntax| match syntax.strip_prefix("type(").and_then(|s| s.strip_suffix(')')) {
                Some(inner) => inner.trim().to_string(),
                None => syntax.to_string(),
            });
            list.push(Param { name: name.to_string(), syntax, default });
        }
    }
    Some((name.to_string(), list, rest))
}

/// A `@function` rule: its name, and its parameters and cases.
fn parse_function(rule: &AtRule) -> Option<(String, Function)> {
    let (name, params, _returns) = parse_prelude(&rule.params, false)?;

    // Each declaration with the indices of its conditional rules, and each
    // conditional rule with its conditions and the indices of the rules
    // around it and of itself.
    struct Item {
        chain: Vec<usize>,
        property: String,
        value: String,
    }
    struct Block {
        conditions: Vec<(String, String)>,
        chain: Vec<usize>,
        has_items: bool,
    }
    fn walk(nodes: &[AstNode], chain: &mut Vec<usize>, conditions: &mut Vec<(String, String)>, items: &mut Vec<Item>, blocks: &mut Vec<Block>) {
        for node in nodes {
            match node {
                AstNode::Declaration(declaration) => {
                    if let Some(&last) = chain.last() {
                        blocks[last].has_items = true;
                    }
                    items.push(Item {
                        chain: chain.clone(),
                        property: declaration.property.clone(),
                        value: declaration.value.clone().unwrap_or_default(),
                    });
                }
                AstNode::AtRule(rule) => {
                    let name = rule.name.to_ascii_lowercase();
                    if !matches!(name.as_str(), "@media" | "@supports" | "@container") {
                        continue;
                    }
                    conditions.push((rule.name.clone(), rule.params.clone()));
                    chain.push(blocks.len());
                    blocks.push(Block { conditions: conditions.clone(), chain: chain.clone(), has_items: false });
                    walk(&rule.nodes, chain, conditions, items, blocks);
                    chain.pop();
                    conditions.pop();
                }
                _ => {}
            }
        }
    }
    let mut items = Vec::new();
    let mut blocks = Vec::new();
    walk(&rule.nodes, &mut Vec::new(), &mut Vec::new(), &mut items, &mut blocks);

    let case = |conditions: Vec<(String, String)>, chain: &[usize]| {
        let mut locals = HashMap::default();
        let mut result = None;
        for item in items.iter().filter(|item| chain.starts_with(&item.chain)) {
            if item.property.eq_ignore_ascii_case("result") {
                result = Some(item.value.clone());
            } else if item.property.starts_with("--") {
                locals.insert(item.property.clone(), item.value.clone());
            }
        }
        Case { conditions, locals, result }
    };
    let mut cases = vec![case(Vec::new(), &[])];
    for block in blocks.iter().filter(|block| block.has_items) {
        cases.push(case(block.conditions.clone(), &block.chain));
    }
    Some((name, Function { params, cases }))
}

/// The name and the arguments of `--name` or `--name(…)`.
fn parse_call(params: &str) -> Option<(&str, Vec<&str>)> {
    let params = params.trim();
    match params.find('(') {
        Some(open) => {
            let close = matching_paren(params, open)?;
            if !params[close + 1..].trim().is_empty() {
                return None;
            }
            let name = &params[..open];
            name.starts_with("--").then(|| (name, split_args(&params[open + 1..close])))
        }
        None => (params.starts_with("--") && !params.contains(char::is_whitespace)).then(|| (params, Vec::new())),
    }
}

/// The arguments of a call: the parts at the commas, with no space at
/// their ends. An argument in braces can have commas: `{ 1px, 2px }`.
fn split_args(text: &str) -> Vec<&str> {
    if text.trim().is_empty() {
        return Vec::new();
    }
    segment(text, b',')
        .into_iter()
        .map(|arg| {
            let arg = arg.trim();
            match arg.strip_prefix('{').and_then(|arg| arg.strip_suffix('}')) {
                Some(inner) => inner.trim(),
                None => arg,
            }
        })
        .collect()
}

fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_' || !c.is_ascii()
}

fn ident_end(text: &str, start: usize) -> usize {
    text[start..].char_indices().find(|(_, c)| !is_ident_char(*c)).map_or(text.len(), |(offset, _)| start + offset)
}

/// The end of the string that starts at `start`, after its last quote.
fn string_end(text: &str, start: usize) -> usize {
    let bytes = text.as_bytes();
    let quote = bytes[start];
    let mut index = start + 1;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index += 2,
            byte if byte == quote => return index + 1,
            _ => index += 1,
        }
    }
    text.len()
}

/// The index of the `)` for the `(` at `open`.
fn matching_paren(text: &str, open: usize) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0usize;
    let mut index = open;
    while index < bytes.len() {
        match bytes[index] {
            b'"' | b'\'' => {
                index = string_end(text, index);
                continue;
            }
            b'\\' => index += 1,
            b'(' | b'[' | b'{' => depth += 1,
            b')' | b']' | b'}' => {
                depth -= 1;
                if depth == 0 {
                    return (bytes[index] == b')').then_some(index);
                }
            }
            _ => {}
        }
        index += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn targets() -> Browsers {
        super::super::optimize_css::TARGETS
    }

    fn run(css: &str) -> String {
        lower(css, "input.css", &targets(), false).map_or_else(|| css.to_string(), |lowered| lowered.code)
    }

    #[test]
    fn the_targets_of_tailwind_need_both() {
        assert!(needs_lowering(&targets(), &FUNCTIONS));
        assert!(needs_lowering(&targets(), &MIXINS));
        let new = Browsers { chrome: Some(140 << 16), ..Browsers::default() };
        assert!(!needs_lowering(&new, &FUNCTIONS));
        assert!(needs_lowering(&new, &MIXINS));
    }

    #[test]
    fn no_rule_gives_no_change() {
        assert!(lower(".a { color: red; }", "input.css", &targets(), false).is_none());
        assert!(lower(".a { gap: --gap(1); }", "input.css", &targets(), false).is_none());
    }

    #[test]
    fn a_function() {
        assert_eq!(
            run("@function --double(--n) { result: calc(var(--n) * 2); }\n.a { width: --double(4px); }"),
            ".a {\n  width: calc(4px * 2);\n}\n"
        );
    }

    #[test]
    fn locals_defaults_and_the_call_site() {
        let css = "@function --f(--a, --b: 2) { --c: var(--b); result: var(--a) var(--c) var(--d) var(--e, x); }\n.a { margin: --f(1); }";
        assert_eq!(run(css), ".a {\n  margin: 1 2 var(--d) var(--e, x);\n}\n");
    }

    #[test]
    fn a_missing_argument_is_not_valid() {
        let css = "@function --f(--a) { result: var(--a, 7); }\n@function --g(--a) { result: var(--a); }\n.a { top: --f(); left: --g(); --x: --g(); }";
        assert_eq!(run(css), ".a {\n  top: 7;\n  left: unset;\n  --x: initial;\n}\n");
    }

    #[test]
    fn conditions_give_nested_rules() {
        let css = "@function --gap(--n) {\n  --step: 4px;\n  @media (width >= 40rem) { --step: 8px; }\n  result: calc(var(--n) * var(--step));\n}\n.a { gap: --gap(2); }";
        assert_eq!(
            run(css),
            ".a {\n  gap: calc(2 * 4px);\n  @media (width >= 40rem) {\n    gap: calc(2 * 8px);\n  }\n}\n"
        );
    }

    #[test]
    fn functions_call_functions_and_a_cycle_is_not_valid() {
        let css = "@function --inner(--x) { result: calc(var(--x) + 1px); }\n@function --outer(--x) { result: --inner(var(--x)); }\n@function --loop() { result: --loop(); }\n.a { top: --outer(1px); left: --loop(); }";
        assert_eq!(run(css), ".a {\n  top: calc(1px + 1px);\n  left: unset;\n}\n");
    }

    #[test]
    fn arguments_in_braces_and_too_many_arguments() {
        let css = "@function --f(--a, --b) { result: max(var(--a)) var(--b); }\n.a { top: --f({ 1px, 2px }, 3px); left: --f(1, 2, 3); }";
        assert_eq!(run(css), ".a {\n  top: max(1px, 2px) 3px;\n  left: unset;\n}\n");
    }

    #[test]
    fn a_mixin() {
        let css = "@mixin --card(--bg: white, --pad) {\n  background: var(--bg);\n  padding: var(--pad, 1rem);\n  &:hover { color: var(--bg); }\n}\n.a { @apply --card; }\n.b { @apply --card(red, 2rem); }";
        assert_eq!(
            run(css),
            ".a {\n  background: white;\n  padding: 1rem;\n  &:hover {\n    color: white;\n  }\n}\n.b {\n  background: red;\n  padding: 2rem;\n  &:hover {\n    color: red;\n  }\n}\n"
        );
    }

    #[test]
    fn contents_and_its_fallback() {
        let css = "@mixin --wide { @media (width >= 40rem) { @contents { color: red; } } }\n.a { @apply --wide; }\n.b { @apply --wide { color: blue; } }";
        assert_eq!(
            run(css),
            ".a {\n  @media (width >= 40rem) {\n    color: red;\n  }\n}\n.b {\n  @media (width >= 40rem) {\n    color: blue;\n  }\n}\n"
        );
    }

    #[test]
    fn other_elements_get_a_custom_property() {
        // A descendant inherits the argument; a sibling does not see it and
        // uses the fallback, as the spec says.
        let css = "@mixin --m(--c <color>) { & > p { color: var(--c); } & + * { color: var(--c, red); } }\n.a { @apply --m(green); }";
        assert_eq!(
            run(css),
            ".a {\n  --mixin-1-c: green;\n  & > p {\n    color: var(--mixin-1-c);\n  }\n  & + * {\n    color: var(--mixin-1-c, red);\n  }\n}\n@property --mixin-1-c {\n  syntax: \"<color>\";\n  inherits: true;\n  initial-value: transparent;\n}\n"
        );
    }

    #[test]
    fn private_properties_and_nested_mixins() {
        let css = "@mixin --inner(--x) { width: var(--x); }\n@mixin --outer(--y) { @private { --z: calc(var(--y) * 2); } @apply --inner(var(--z)); @layer a { top: 0; } }\n.a { @apply --outer(3px); }";
        assert_eq!(run(css), ".a {\n  width: calc(3px * 2);\n}\n");
    }

    #[test]
    fn unknown_mixins_stay_and_cycles_do_nothing() {
        let css = "@mixin --a { @apply --a; color: red; }\n.a { @apply --a; @apply --nothing; }";
        assert_eq!(run(css), ".a {\n  color: red;\n  @apply --nothing;\n}\n");
    }

    #[test]
    fn a_mixin_with_a_function() {
        let css = "@function --twice(--n) { result: calc(var(--n) * 2); }\n@mixin --m(--n) { width: --twice(var(--n)); }\n.a { @apply --m(3px); }";
        assert_eq!(run(css), ".a {\n  width: calc(3px * 2);\n}\n");
    }

    #[test]
    fn the_source_map_points_to_the_mixin() {
        let css = "@mixin --m {\n  color: red;\n}\n.a {\n  @apply --m;\n}\n";
        let lowered = lower(css, "input.css", &targets(), true).unwrap();
        assert_eq!(lowered.code, ".a {\n  color: red;\n}\n");
        let map = lowered.map.unwrap();
        assert!(map.contains("\"sources\":[\"input.css\"]"), "{map}");
        // `.a {` (line 1 of the result) comes from line 4 of the input
        // (`AAGA`), and `color: red;` (line 2) from line 2, column 2
        // (`EAFD`): the body of the mixin.
        let mappings = map.split("\"mappings\":\"").nth(1).unwrap();
        assert!(mappings.starts_with("AAGA,GAAG;EAFD"), "{mappings}");
    }

    #[test]
    fn the_same_element() {
        assert!(is_same_element("&:hover, &.on, &[data-x], &:is(.a .b)"));
        assert!(!is_same_element("& > p"));
        assert!(!is_same_element("&::before"));
        assert!(!is_same_element(".child"));
    }
}
