//! The CSS syntax tree: `ast.ts` of Tailwind CSS.

use crate::walk::Walkable;
use std::collections::BTreeMap;

/// A style rule: a selector with a block.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StyleRule {
    pub selector: String,
    pub nodes: Vec<AstNode>,
}

/// An at-rule. `name` has the `@`. `nodes` is empty for a statement.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AtRule {
    pub name: String,
    pub params: String,
    pub nodes: Vec<AstNode>,
}

/// A declaration. A `value` of `None` is a declaration that prints nothing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Declaration {
    pub property: String,
    pub value: Option<String>,
    pub important: bool,
}

/// A comment, without `/*` and `*/`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Comment {
    pub value: String,
}

/// A value of the context of a `Context` node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContextValue {
    Str(String),
    Bool(bool),
}

impl ContextValue {
    /// The JavaScript truth of the value.
    pub fn truthy(&self) -> bool {
        match self {
            ContextValue::Str(s) => !s.is_empty(),
            ContextValue::Bool(b) => *b,
        }
    }
    /// The text, when the value is text.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            ContextValue::Str(s) => Some(s),
            ContextValue::Bool(_) => None,
        }
    }
}

impl From<&str> for ContextValue {
    fn from(s: &str) -> Self {
        ContextValue::Str(s.to_string())
    }
}
impl From<String> for ContextValue {
    fn from(s: String) -> Self {
        ContextValue::Str(s)
    }
}
impl From<bool> for ContextValue {
    fn from(b: bool) -> Self {
        ContextValue::Bool(b)
    }
}

/// The keys and values of a `Context` node.
pub type ContextMap = BTreeMap<String, ContextValue>;

/// A node that prints nothing itself, and gives data to its subtree.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Context {
    pub context: ContextMap,
    pub nodes: Vec<AstNode>,
}

/// A node whose children go to the root of the output.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AtRoot {
    pub nodes: Vec<AstNode>,
}

/// A node of the CSS syntax tree.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AstNode {
    Rule(StyleRule),
    AtRule(AtRule),
    Declaration(Declaration),
    Comment(Comment),
    Context(Context),
    AtRoot(AtRoot),
}

impl AstNode {
    /// The children of a node that can have children.
    pub fn children(&self) -> Option<&Vec<AstNode>> {
        match self {
            AstNode::Rule(n) => Some(&n.nodes),
            AstNode::AtRule(n) => Some(&n.nodes),
            AstNode::Context(n) => Some(&n.nodes),
            AstNode::AtRoot(n) => Some(&n.nodes),
            AstNode::Declaration(_) | AstNode::Comment(_) => None,
        }
    }
    /// The children of a node that can have children, for a change.
    pub fn children_mut(&mut self) -> Option<&mut Vec<AstNode>> {
        match self {
            AstNode::Rule(n) => Some(&mut n.nodes),
            AstNode::AtRule(n) => Some(&mut n.nodes),
            AstNode::Context(n) => Some(&mut n.nodes),
            AstNode::AtRoot(n) => Some(&mut n.nodes),
            AstNode::Declaration(_) | AstNode::Comment(_) => None,
        }
    }
    pub fn as_rule(&self) -> Option<&StyleRule> {
        if let AstNode::Rule(n) = self { Some(n) } else { None }
    }
    pub fn as_at_rule(&self) -> Option<&AtRule> {
        if let AstNode::AtRule(n) = self { Some(n) } else { None }
    }
    pub fn as_declaration(&self) -> Option<&Declaration> {
        if let AstNode::Declaration(n) = self { Some(n) } else { None }
    }
    /// True for an at-rule with this name (the name has the `@`).
    pub fn is_at_rule(&self, name: &str) -> bool {
        matches!(self, AstNode::AtRule(n) if n.name == name)
    }
}

impl Walkable for AstNode {
    fn nodes(&self) -> Option<&Vec<Self>> {
        self.children()
    }
    fn nodes_mut(&mut self) -> Option<&mut Vec<Self>> {
        self.children_mut()
    }
    fn shallow(&self) -> Self {
        match self {
            AstNode::Rule(n) => AstNode::Rule(StyleRule { selector: n.selector.clone(), nodes: Vec::new() }),
            AstNode::AtRule(n) => AstNode::AtRule(AtRule {
                name: n.name.clone(),
                params: n.params.clone(),
                nodes: Vec::new(),
            }),
            AstNode::Context(n) => AstNode::Context(Context { context: n.context.clone(), nodes: Vec::new() }),
            AstNode::AtRoot(_) => AstNode::AtRoot(AtRoot::default()),
            AstNode::Declaration(_) | AstNode::Comment(_) => self.clone(),
        }
    }
}

/// A style rule node.
pub fn style_rule(selector: impl Into<String>, nodes: Vec<AstNode>) -> AstNode {
    AstNode::Rule(StyleRule { selector: selector.into(), nodes })
}

/// An at-rule node. `name` has the `@`.
pub fn at_rule(name: impl Into<String>, params: impl Into<String>, nodes: Vec<AstNode>) -> AstNode {
    AstNode::AtRule(AtRule { name: name.into(), params: params.into(), nodes })
}

/// A style rule, or an at-rule when the selector starts with `@`.
pub fn rule(selector: impl Into<String>, nodes: Vec<AstNode>) -> AstNode {
    let selector = selector.into();
    if selector.starts_with('@') {
        AstNode::AtRule(crate::css_parser::parse_at_rule(&selector, nodes))
    } else {
        style_rule(selector, nodes)
    }
}

/// A declaration node that is not important.
pub fn decl(property: impl Into<String>, value: impl Into<String>) -> AstNode {
    AstNode::Declaration(Declaration { property: property.into(), value: Some(value.into()), important: false })
}

/// A declaration node, with all its fields.
pub fn decl_full(property: impl Into<String>, value: Option<String>, important: bool) -> AstNode {
    AstNode::Declaration(Declaration { property: property.into(), value, important })
}

/// A comment node.
pub fn comment(value: impl Into<String>) -> AstNode {
    AstNode::Comment(Comment { value: value.into() })
}

/// A context node.
pub fn context(context: ContextMap, nodes: Vec<AstNode>) -> AstNode {
    AstNode::Context(Context { context, nodes })
}

/// An at-root node.
pub fn at_root(nodes: Vec<AstNode>) -> AstNode {
    AstNode::AtRoot(AtRoot { nodes })
}

/// The context of a node: the contexts of its ancestors, merged from the
/// root down. `path` is the path of a `VisitContext`.
pub fn css_context(path: &[AstNode]) -> ContextMap {
    let mut merged = ContextMap::new();
    for node in path {
        if let AstNode::Context(c) = node {
            for (k, v) in &c.context {
                merged.insert(k.clone(), v.clone());
            }
        }
    }
    merged
}

/// The parent of a node, without the context nodes.
pub fn css_parent(path: &[AstNode]) -> Option<&AstNode> {
    path.iter().rev().find(|n| !matches!(n, AstNode::Context(_)))
}
