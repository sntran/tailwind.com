//! The CSS syntax tree: `ast.ts` of Tailwind CSS.

use crate::source_maps::{Locations, SourceLocation};
use crate::walk::Walkable;
use std::collections::BTreeMap;

/// The source locations of a node (`src` and `dst` upstream), for source
/// maps. A node has none when the compiler makes no source map: the field
/// is then one null pointer.
///
/// Two values are always equal: a comparison of nodes ignores the
/// locations. Thus a tree from the parser with `from` is equal to the tree
/// without it.
#[derive(Debug, Clone, Default)]
pub struct Loc(Option<Box<Locations>>);

impl PartialEq for Loc {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl Eq for Loc {}

impl Loc {
    /// The locations of a node from a source: `src` and `dst` are the same.
    pub fn new(location: SourceLocation) -> Self {
        Loc(Some(Box::new(Locations { src: Some(location.clone()), dst: Some(location), identity: None })))
    }
    /// The locations with only a `src`.
    pub fn from_src(src: Option<SourceLocation>) -> Self {
        let mut loc = Loc::default();
        loc.set_src(src);
        loc
    }
    /// The text that made the node.
    pub fn src(&self) -> Option<&SourceLocation> {
        self.0.as_ref()?.src.as_ref()
    }
    /// The text of the node in the output.
    pub fn dst(&self) -> Option<&SourceLocation> {
        self.0.as_ref()?.dst.as_ref()
    }
    pub fn set_src(&mut self, src: Option<SourceLocation>) {
        match &mut self.0 {
            Some(locations) => locations.src = src,
            None if src.is_some() => self.0 = Some(Box::new(Locations { src, dst: None, identity: None })),
            None => {}
        }
    }
    pub fn set_dst(&mut self, dst: Option<SourceLocation>) {
        match &mut self.0 {
            Some(locations) => locations.dst = dst,
            None if dst.is_some() => self.0 = Some(Box::new(Locations { src: None, dst, identity: None })),
            None => {}
        }
    }
    /// The identity of the node, when it has one. See `share_identity`.
    pub fn identity(&self) -> Option<*const ()> {
        self.0.as_ref()?.identity.as_ref().map(std::rc::Rc::as_ptr)
    }
    /// Give the node an identity, when it has none.
    pub fn set_identity(&mut self) {
        let locations = self.0.get_or_insert_with(Default::default);
        if locations.identity.is_none() {
            locations.identity = Some(std::rc::Rc::new(()));
        }
    }
    /// Remove the identity: the node is a new object.
    pub fn clear_identity(&mut self) {
        if let Some(locations) = &mut self.0 {
            locations.identity = None;
        }
    }
    /// True when the node has a `src` or a `dst`.
    pub fn is_some(&self) -> bool {
        self.0.as_ref().is_some_and(|locations| locations.src.is_some() || locations.dst.is_some())
    }
}

/// A style rule: a selector with a block.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StyleRule {
    pub selector: String,
    pub nodes: Vec<AstNode>,
    /// The source locations, for source maps.
    pub loc: Loc,
}

/// An at-rule. `name` has the `@`. `nodes` is empty for a statement.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AtRule {
    pub name: String,
    pub params: String,
    pub nodes: Vec<AstNode>,
    /// The source locations, for source maps.
    pub loc: Loc,
}

/// A declaration. A `value` of `None` is a declaration that prints nothing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Declaration {
    pub property: String,
    pub value: Option<String>,
    pub important: bool,
    /// The source locations, for source maps.
    pub loc: Loc,
}

/// A comment, without `/*` and `*/`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Comment {
    pub value: String,
    /// The source locations, for source maps.
    pub loc: Loc,
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
    /// The source locations. A context node and an at-root node have none.
    pub fn loc(&self) -> Option<&Loc> {
        match self {
            AstNode::Rule(n) => Some(&n.loc),
            AstNode::AtRule(n) => Some(&n.loc),
            AstNode::Declaration(n) => Some(&n.loc),
            AstNode::Comment(n) => Some(&n.loc),
            AstNode::Context(_) | AstNode::AtRoot(_) => None,
        }
    }
    /// The source locations, for a change.
    pub fn loc_mut(&mut self) -> Option<&mut Loc> {
        match self {
            AstNode::Rule(n) => Some(&mut n.loc),
            AstNode::AtRule(n) => Some(&mut n.loc),
            AstNode::Declaration(n) => Some(&mut n.loc),
            AstNode::Comment(n) => Some(&mut n.loc),
            AstNode::Context(_) | AstNode::AtRoot(_) => None,
        }
    }
    /// The text that made the node.
    pub fn src(&self) -> Option<&SourceLocation> {
        self.loc()?.src()
    }
    /// The text of the node in the output.
    pub fn dst(&self) -> Option<&SourceLocation> {
        self.loc()?.dst()
    }
    /// Set the `src` of the node. A context node and an at-root node keep
    /// no location: no step reads it.
    pub fn set_src(&mut self, src: Option<SourceLocation>) {
        if let Some(loc) = self.loc_mut() {
            loc.set_src(src);
        }
    }
    /// The node with this `src`.
    pub fn with_src(mut self, src: Option<&SourceLocation>) -> Self {
        if src.is_some() {
            self.set_src(src.cloned());
        }
        self
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
            AstNode::Rule(n) => AstNode::Rule(StyleRule { selector: n.selector.clone(), nodes: Vec::new(), loc: n.loc.clone() }),
            AstNode::AtRule(n) => AstNode::AtRule(AtRule {
                name: n.name.clone(),
                params: n.params.clone(),
                nodes: Vec::new(),
                loc: n.loc.clone(),
            }),
            AstNode::Context(n) => AstNode::Context(Context { context: n.context.clone(), nodes: Vec::new() }),
            AstNode::AtRoot(_) => AstNode::AtRoot(AtRoot::default()),
            AstNode::Declaration(_) | AstNode::Comment(_) => self.clone(),
        }
    }
}

/// Mark the declarations and the comments of the trees: the caller puts
/// copies of the trees at more than one place, where upstream puts the
/// same objects. Upstream gives such an object one `dst`: the last place
/// where the printer prints it. `to_css_tracked` does the same for the
/// copies. A style rule and an at-rule need no mark: `optimize_ast` of
/// upstream makes a new object for each place.
///
/// Call this only when the compiler keeps the source locations.
pub fn share_identity(nodes: &mut [AstNode]) {
    for node in nodes {
        match node {
            AstNode::Declaration(n) => n.loc.set_identity(),
            AstNode::Comment(n) => n.loc.set_identity(),
            _ => {}
        }
        if let Some(children) = node.children_mut() {
            share_identity(children);
        }
    }
}

/// A style rule node.
pub fn style_rule(selector: impl Into<String>, nodes: Vec<AstNode>) -> AstNode {
    AstNode::Rule(StyleRule { selector: selector.into(), nodes, loc: Loc::default() })
}

/// An at-rule node. `name` has the `@`.
pub fn at_rule(name: impl Into<String>, params: impl Into<String>, nodes: Vec<AstNode>) -> AstNode {
    AstNode::AtRule(AtRule { name: name.into(), params: params.into(), nodes, loc: Loc::default() })
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
    AstNode::Declaration(Declaration { property: property.into(), value: Some(value.into()), important: false, loc: Loc::default() })
}

/// A declaration node, with all its fields.
pub fn decl_full(property: impl Into<String>, value: Option<String>, important: bool) -> AstNode {
    AstNode::Declaration(Declaration { property: property.into(), value, important, loc: Loc::default() })
}

/// A comment node.
pub fn comment(value: impl Into<String>) -> AstNode {
    AstNode::Comment(Comment { value: value.into(), loc: Loc::default() })
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
