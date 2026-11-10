//! The tree, the candidates and the variants as JSON, in the shapes of
//! `ast.ts` and `candidate.ts` of Tailwind CSS.

use crate::ast::{AstNode, AtRoot, AtRule, Comment, Context, ContextMap, ContextValue, Declaration, StyleRule};
use crate::candidate::{Candidate, CandidateModifier, FunctionalCandidate, StaticCandidate, UtilityValue, ValueKind, Variant, VariantValue};
use serde_json::{json, Map, Value};
use std::rc::Rc;

/// The key of the context node that is at the place of `@tailwind
/// utilities` (see `index.rs`). JavaScript gets the at-rule, as upstream.
pub const UTILITIES_KEY: &str = "__utilities";
/// The key that keeps the other context values of that node.
const UTILITIES_CONTEXT: &str = "__context";

pub fn ast_to_json(nodes: &[AstNode]) -> Value {
    Value::Array(nodes.iter().map(node_to_json).collect())
}

pub fn node_to_json(node: &AstNode) -> Value {
    match node {
        AstNode::Rule(rule) => json!({ "kind": "rule", "selector": rule.selector, "nodes": ast_to_json(&rule.nodes) }),
        AstNode::AtRule(rule) => {
            json!({ "kind": "at-rule", "name": rule.name, "params": rule.params, "nodes": ast_to_json(&rule.nodes) })
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
                    "nodes": ast_to_json(&context.nodes),
                    UTILITIES_CONTEXT: context_to_json(&rest),
                });
            }
            json!({ "kind": "context", "context": context_to_json(&context.context), "nodes": ast_to_json(&context.nodes) })
        }
        AstNode::AtRoot(root) => json!({ "kind": "at-root", "nodes": ast_to_json(&root.nodes) }),
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

pub fn ast_from_json(value: &Value) -> Result<Vec<AstNode>, String> {
    match value {
        Value::Array(items) => items.iter().map(node_from_json).collect(),
        Value::Null => Ok(Vec::new()),
        _ => Err("The tree from JavaScript is not an array.".to_string()),
    }
}

pub fn node_from_json(value: &Value) -> Result<AstNode, String> {
    let Value::Object(object) = value else {
        return Err(format!("A node from JavaScript is not an object: {value}"));
    };
    let nodes = || ast_from_json(object.get("nodes").unwrap_or(&Value::Null));
    match object.get("kind").and_then(Value::as_str) {
        Some("rule") => Ok(AstNode::Rule(StyleRule { selector: text(object.get("selector")), nodes: nodes()? })),
        Some("at-rule") => {
            if let Some(rest) = object.get(UTILITIES_CONTEXT) {
                let mut context = context_from_json(Some(rest));
                context.insert(UTILITIES_KEY.to_string(), ContextValue::Str(text(object.get("params"))));
                return Ok(AstNode::Context(Context { context, nodes: nodes()? }));
            }
            Ok(AstNode::AtRule(AtRule { name: text(object.get("name")), params: text(object.get("params")), nodes: nodes()? }))
        }
        Some("declaration") => Ok(AstNode::Declaration(Declaration {
            property: text(object.get("property")),
            value: optional_text(object.get("value")),
            important: object.get("important").and_then(Value::as_bool).unwrap_or(false),
        })),
        Some("comment") => Ok(AstNode::Comment(Comment { value: text(object.get("value")) })),
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
