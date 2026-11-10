//! A walk of a tree, with replacement of nodes.
//!
//! This is `walk.ts` of Tailwind CSS. A visitor gets the siblings of a node
//! and the index of the node, so it can read the nodes that are near. The
//! ancestors of the node are in the context as shallow copies: copies with
//! no children.

/// A node of a tree that `walk` can visit.
pub trait Walkable: Sized {
    /// The children, when this kind of node can have children.
    fn nodes(&self) -> Option<&Vec<Self>>;
    /// The children, for a change.
    fn nodes_mut(&mut self) -> Option<&mut Vec<Self>>;
    /// A copy of the node with no children.
    fn shallow(&self) -> Self;
}

/// What the walk does after a visit.
#[derive(Debug)]
pub enum WalkAction<T> {
    /// Visit the children, then the next sibling.
    Continue,
    /// Do not visit the children.
    Skip,
    /// Stop the walk.
    Stop,
    /// Replace the node, and visit the replacement.
    Replace(Vec<T>),
    /// Replace the node, and do not visit the replacement.
    ReplaceSkip(Vec<T>),
    /// Replace the node, and stop the walk.
    ReplaceStop(Vec<T>),
}

/// Where the walk is.
pub struct VisitContext<'a, T> {
    /// The number of ancestors.
    pub depth: usize,
    /// The index of the node in its siblings.
    pub index: usize,
    /// The ancestors, from the root down, as shallow copies.
    pub path: &'a [T],
}

impl<T> VisitContext<'_, T> {
    /// The parent of the node, as a shallow copy.
    pub fn parent(&self) -> Option<&T> {
        self.path.last()
    }
}

/// Visit each node before its children.
pub fn walk<T: Walkable>(
    ast: &mut Vec<T>,
    mut enter: impl FnMut(&mut T, &VisitContext<T>) -> WalkAction<T>,
) {
    walk_siblings(
        ast,
        |nodes, ctx| enter(&mut nodes[ctx.index], ctx),
        |_, _| WalkAction::Continue,
    );
}

/// Visit each node before its children (`enter`) and after them (`exit`).
/// `exit` must not return `Skip`.
pub fn walk_enter_exit<T: Walkable>(
    ast: &mut Vec<T>,
    mut enter: impl FnMut(&mut T, &VisitContext<T>) -> WalkAction<T>,
    mut exit: impl FnMut(&mut T, &VisitContext<T>) -> WalkAction<T>,
) {
    walk_siblings(
        ast,
        |nodes, ctx| enter(&mut nodes[ctx.index], ctx),
        |nodes, ctx| exit(&mut nodes[ctx.index], ctx),
    );
}

/// The walk with the siblings of each node: the node is `nodes[ctx.index]`.
pub fn walk_siblings<T: Walkable>(
    ast: &mut Vec<T>,
    mut enter: impl FnMut(&mut Vec<T>, &VisitContext<T>) -> WalkAction<T>,
    mut exit: impl FnMut(&mut Vec<T>, &VisitContext<T>) -> WalkAction<T>,
) {
    let mut path = Vec::new();
    level(ast, &mut path, &mut enter, &mut exit);
}

/// Walk one list of siblings. The result is `false` when the walk stops.
fn level<T: Walkable>(
    nodes: &mut Vec<T>,
    path: &mut Vec<T>,
    enter: &mut impl FnMut(&mut Vec<T>, &VisitContext<T>) -> WalkAction<T>,
    exit: &mut impl FnMut(&mut Vec<T>, &VisitContext<T>) -> WalkAction<T>,
) -> bool {
    let mut index = 0;
    while index < nodes.len() {
        let action = enter(nodes, &VisitContext { depth: path.len(), index, path });
        match action {
            WalkAction::Continue => {
                let has_children = nodes[index].nodes().is_some_and(|n| !n.is_empty());
                if has_children {
                    path.push(nodes[index].shallow());
                    let go_on = level(nodes[index].nodes_mut().unwrap(), path, enter, exit);
                    path.pop();
                    if !go_on {
                        return false;
                    }
                }
            }
            WalkAction::Skip => {}
            WalkAction::Stop => return false,
            WalkAction::Replace(new) => {
                nodes.splice(index..=index, new);
                continue;
            }
            WalkAction::ReplaceSkip(new) => {
                let count = new.len();
                nodes.splice(index..=index, new);
                index += count;
                continue;
            }
            WalkAction::ReplaceStop(new) => {
                nodes.splice(index..=index, new);
                return false;
            }
        }
        let action = exit(nodes, &VisitContext { depth: path.len(), index, path });
        match action {
            WalkAction::Continue => index += 1,
            WalkAction::Skip => panic!("the exit visitor of walk must not return Skip"),
            WalkAction::Stop => return false,
            WalkAction::Replace(new) | WalkAction::ReplaceSkip(new) => {
                let count = new.len();
                nodes.splice(index..=index, new);
                index += count;
            }
            WalkAction::ReplaceStop(new) => {
                nodes.splice(index..=index, new);
                return false;
            }
        }
    }
    true
}
