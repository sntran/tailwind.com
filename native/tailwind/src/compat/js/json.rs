//! The tree, the candidates and the variants as JSON, in the shapes of
//! `ast.ts` and `candidate.ts` of Tailwind CSS.

use crate::ast::{AstNode, AtRoot, AtRule, Comment, Context, ContextMap, ContextValue, Declaration, Loc, StyleRule};
use crate::candidate::{Candidate, CandidateModifier, FunctionalCandidate, StaticCandidate, UtilityValue, ValueKind, Variant, VariantValue};
use crate::source_maps::SourceLocation;
use serde_json::{json, Map, Value};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// The key of the context node that is at the place of `@tailwind
/// utilities` (see `index.rs`). JavaScript gets the at-rule, as upstream.
pub const UTILITIES_KEY: &str = "__utilities";
/// The key that keeps the other context values of that node.
const UTILITIES_CONTEXT: &str = "__context";

/// The keys of the `src` and the `dst` of a node, as upstream. The value
/// is a number: the index of the location in the table. JavaScript only
/// copies these values (`node.src = src` in `compat/plugin-api.ts`,
/// `cloneAstNode`).
const SRC: &str = "src";
const DST: &str = "dst";
/// The key of the identity of a declaration or a comment: a number. Two
/// nodes with the same number are one object in JavaScript. An even number
/// is from this table, and an odd number is from JavaScript (`host.ts`
/// gives one to each object that it sends).
const ID: &str = "__id";

type SrcKey = (usize, usize, usize);

fn src_key(location: &SourceLocation) -> SrcKey {
    (Rc::as_ptr(&location.source) as usize, location.start, location.end)
}

/// The source locations of the nodes that are in JavaScript, for source
/// maps. A node in JSON has the index of its location. The table keeps
/// each location one time, and it keeps the sources alive.
///
/// With no source map, the nodes have no locations and the table stays
/// empty.
#[derive(Default)]
pub struct LocTable {
    locations: RefCell<(Vec<SourceLocation>, HashMap<SrcKey, usize>)>,
    /// The identities that this side gave: each one is in a `Loc`.
    identities: RefCell<(Vec<Loc>, HashMap<usize, usize>)>,
    /// The identities of the objects of JavaScript.
    objects: RefCell<HashMap<u64, Loc>>,
}

impl LocTable {
    fn location_index(&self, location: &SourceLocation) -> usize {
        let mut locations = self.locations.borrow_mut();
        let (list, index) = &mut *locations;
        *index.entry(src_key(location)).or_insert_with(|| {
            list.push(location.clone());
            list.len() - 1
        })
    }

    /// Put the locations of a node into its JSON object.
    fn write(&self, loc: &Loc, object: &mut Value) {
        let Value::Object(object) = object else { return };
        if let Some(src) = loc.src() {
            object.insert(SRC.into(), self.location_index(src).into());
        }
        if let Some(dst) = loc.dst() {
            object.insert(DST.into(), self.location_index(dst).into());
        }
        if let Some(identity) = loc.identity() {
            let mut identities = self.identities.borrow_mut();
            let (list, index) = &mut *identities;
            let id = *index.entry(identity as usize).or_insert_with(|| {
                let mut holder = Loc::default();
                holder.copy_identity(loc);
                list.push(holder);
                list.len() - 1
            });
            object.insert(ID.into(), (id * 2).into());
        }
    }

    /// The locations of a node from JavaScript.
    fn read(&self, object: &Map<String, Value>) -> Loc {
        let location = |key: &str| {
            let id = object.get(key).and_then(Value::as_u64)? as usize;
            self.locations.borrow().0.get(id).cloned()
        };
        let mut loc = Loc::from_src(location(SRC));
        loc.set_dst(location(DST));
        if let Some(id) = object.get(ID).and_then(Value::as_u64) {
            if id % 2 == 0 {
                if let Some(holder) = self.identities.borrow().0.get((id / 2) as usize) {
                    loc.copy_identity(holder);
                }
            } else {
                let mut objects = self.objects.borrow_mut();
                let holder = objects.entry(id).or_insert_with(|| {
                    let mut holder = Loc::default();
                    holder.set_identity();
                    holder
                });
                loc.copy_identity(holder);
            }
        }
        loc
    }
}

pub fn ast_to_json(nodes: &[AstNode], table: &LocTable) -> Value {
    Value::Array(nodes.iter().map(|node| node_to_json(node, table)).collect())
}

pub fn node_to_json(node: &AstNode, table: &LocTable) -> Value {
    let mut value = node_to_json_without_loc(node, table);
    if let Some(loc) = node.loc() {
        table.write(loc, &mut value);
    }
    value
}

fn node_to_json_without_loc(node: &AstNode, table: &LocTable) -> Value {
    match node {
        AstNode::Rule(rule) => json!({ "kind": "rule", "selector": rule.selector, "nodes": ast_to_json(&rule.nodes, table) }),
        AstNode::AtRule(rule) => {
            json!({ "kind": "at-rule", "name": rule.name, "params": rule.params, "nodes": ast_to_json(&rule.nodes, table) })
        }
        AstNode::Declaration(declaration) => {
            let mut object = Map::new();
            object.insert("kind".into(), "declaration".into());
            object.insert("property".into(), declaration.property.clone().into());
            // `undefined` upstream: no key.
            if let Some(value) = &declaration.value {
                object.insert("value".into(), value.clone().into());
            }
            object.insert("important".into(), declaration.important.into());
            Value::Object(object)
        }
        AstNode::Comment(comment) => json!({ "kind": "comment", "value": comment.value }),
        AstNode::Context(context) => {
            if let Some(params) = context.context.get(UTILITIES_KEY) {
                let mut rest = context.context.clone();
                rest.remove(UTILITIES_KEY);
                return json!({
                    "kind": "at-rule",
                    "name": "@tailwind",
                    "params": params.as_str().unwrap_or("utilities"),
                    "nodes": ast_to_json(&context.nodes, table),
                    UTILITIES_CONTEXT: context_to_json(&rest),
                });
            }
            json!({ "kind": "context", "context": context_to_json(&context.context), "nodes": ast_to_json(&context.nodes, table) })
        }
        AstNode::AtRoot(root) => json!({ "kind": "at-root", "nodes": ast_to_json(&root.nodes, table) }),
    }
}

fn context_to_json(context: &ContextMap) -> Value {
    let mut object = Map::new();
    for (key, value) in context {
        let value = match value {
            ContextValue::Str(text) => Value::String(text.clone()),
            ContextValue::Bool(flag) => Value::Bool(*flag),
        };
        object.insert(key.clone(), value);
    }
    Value::Object(object)
}

fn context_from_json(value: Option<&Value>) -> ContextMap {
    let mut context = ContextMap::new();
    if let Some(Value::Object(object)) = value {
        for (key, value) in object {
            match value {
                Value::String(text) => {
                    context.insert(key.clone(), ContextValue::Str(text.clone()));
                }
                Value::Bool(flag) => {
                    context.insert(key.clone(), ContextValue::Bool(*flag));
                }
                _ => {}
            }
        }
    }
    context
}

/// Text of a field. A number or a boolean becomes its JSON text.
fn text(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => text.clone(),
        Some(Value::Null) | None => String::new(),
        Some(other) => other.to_string(),
    }
}

fn optional_text(value: Option<&Value>) -> Option<String> {
    match value {
        Some(Value::Null) | None => None,
        value => Some(text(value)),
    }
}

pub fn ast_from_json(value: &Value, table: &LocTable) -> Result<Vec<AstNode>, String> {
    match value {
        Value::Array(items) => items.iter().map(|item| node_from_json(item, table)).collect(),
        Value::Null => Ok(Vec::new()),
        _ => Err("The tree from JavaScript is not an array.".to_string()),
    }
}

pub fn node_from_json(value: &Value, table: &LocTable) -> Result<AstNode, String> {
    let Value::Object(object) = value else {
        return Err(format!("A node from JavaScript is not an object: {value}"));
    };
    let nodes = || ast_from_json(object.get("nodes").unwrap_or(&Value::Null), table);
    let loc = || table.read(object);
    match object.get("kind").and_then(Value::as_str) {
        Some("rule") => Ok(AstNode::Rule(StyleRule { selector: text(object.get("selector")), nodes: nodes()?, loc: loc() })),
        Some("at-rule") => {
            if let Some(rest) = object.get(UTILITIES_CONTEXT) {
                let mut context = context_from_json(Some(rest));
                context.insert(UTILITIES_KEY.to_string(), ContextValue::Str(text(object.get("params"))));
                return Ok(AstNode::Context(Context { context, nodes: nodes()? }));
            }
            Ok(AstNode::AtRule(AtRule {
                name: text(object.get("name")),
                params: text(object.get("params")),
                nodes: nodes()?,
                loc: loc(),
            }))
        }
        Some("declaration") => Ok(AstNode::Declaration(Declaration {
            property: text(object.get("property")),
            value: optional_text(object.get("value")),
            important: object.get("important").and_then(Value::as_bool).unwrap_or(false),
            loc: loc(),
        })),
        Some("comment") => Ok(AstNode::Comment(Comment { value: text(object.get("value")), loc: loc() })),
        Some("context") => Ok(AstNode::Context(Context { context: context_from_json(object.get("context")), nodes: nodes()? })),
        Some("at-root") => Ok(AstNode::AtRoot(AtRoot { nodes: nodes()? })),
        kind => Err(format!("A node from JavaScript has the unknown kind {kind:?}.")),
    }
}

fn kind_name(kind: ValueKind) -> &'static str {
    match kind {
        ValueKind::Arbitrary => "arbitrary",
        ValueKind::Named => "named",
    }
}

fn modifier_to_json(modifier: Option<&CandidateModifier>) -> Value {
    match modifier {
        Some(modifier) => json!({ "kind": kind_name(modifier.kind), "value": modifier.value }),
        None => Value::Null,
    }
}

fn variants_to_json(variants: &[Rc<Variant>]) -> Value {
    Value::Array(variants.iter().map(|variant| variant_to_json(variant)).collect())
}

pub fn variant_to_json(variant: &Variant) -> Value {
    match variant {
        Variant::Arbitrary { selector, relative } => json!({ "kind": "arbitrary", "selector": selector, "relative": relative }),
        Variant::Static { root } => json!({ "kind": "static", "root": root }),
        Variant::Functional { root, value, modifier } => json!({
            "kind": "functional",
            "root": root,
            "value": match value {
                Some(value) => json!({ "kind": kind_name(value.kind), "value": value.value }),
                None => Value::Null,
            },
            "modifier": modifier_to_json(modifier.as_ref()),
        }),
        Variant::Compound { root, modifier, variant } => json!({
            "kind": "compound",
            "root": root,
            "modifier": modifier_to_json(modifier.as_ref()),
            "variant": variant_to_json(variant),
        }),
    }
}

fn kind_from_json(value: &Value) -> ValueKind {
    match value.get("kind").and_then(Value::as_str) {
        Some("arbitrary") => ValueKind::Arbitrary,
        _ => ValueKind::Named,
    }
}

fn modifier_from_json(value: Option<&Value>) -> Option<CandidateModifier> {
    match value {
        Some(value @ Value::Object(_)) => Some(CandidateModifier { kind: kind_from_json(value), value: text(value.get("value")) }),
        _ => None,
    }
}

pub fn variant_from_json(value: &Value) -> Result<Variant, String> {
    match value.get("kind").and_then(Value::as_str) {
        Some("arbitrary") => Ok(Variant::Arbitrary {
            selector: text(value.get("selector")),
            relative: value.get("relative").and_then(Value::as_bool).unwrap_or(false),
        }),
        Some("static") => Ok(Variant::Static { root: text(value.get("root")) }),
        Some("functional") => Ok(Variant::Functional {
            root: text(value.get("root")),
            value: match value.get("value") {
                Some(inner @ Value::Object(_)) => Some(VariantValue { kind: kind_from_json(inner), value: text(inner.get("value")) }),
                _ => None,
            },
            modifier: modifier_from_json(value.get("modifier")),
        }),
        Some("compound") => Ok(Variant::Compound {
            root: text(value.get("root")),
            modifier: modifier_from_json(value.get("modifier")),
            variant: Rc::new(variant_from_json(value.get("variant").unwrap_or(&Value::Null))?),
        }),
        kind => Err(format!("A variant from JavaScript has the unknown kind {kind:?}.")),
    }
}

pub fn static_candidate_to_json(candidate: &StaticCandidate) -> Value {
    json!({
        "kind": "static",
        "root": candidate.root,
        "variants": variants_to_json(&candidate.variants),
        "important": candidate.important,
        "raw": candidate.raw,
    })
}

pub fn functional_candidate_to_json(candidate: &FunctionalCandidate) -> Value {
    json!({
        "kind": "functional",
        "root": candidate.root,
        "value": match &candidate.value {
            Some(UtilityValue::Arbitrary { data_type, value }) => json!({ "kind": "arbitrary", "dataType": data_type, "value": value }),
            Some(UtilityValue::Named { value, fraction }) => json!({ "kind": "named", "value": value, "fraction": fraction }),
            None => Value::Null,
        },
        "modifier": modifier_to_json(candidate.modifier.as_ref()),
        "variants": variants_to_json(&candidate.variants),
        "important": candidate.important,
        "raw": candidate.raw,
    })
}

pub fn candidate_to_json(candidate: &Candidate) -> Value {
    match candidate {
        Candidate::Arbitrary(candidate) => json!({
            "kind": "arbitrary",
            "property": candidate.property,
            "value": candidate.value,
            "modifier": modifier_to_json(candidate.modifier.as_ref()),
            "variants": variants_to_json(&candidate.variants),
            "important": candidate.important,
            "raw": candidate.raw,
        }),
        Candidate::Static(candidate) => static_candidate_to_json(candidate),
        Candidate::Functional(candidate) => functional_candidate_to_json(candidate),
    }
}
