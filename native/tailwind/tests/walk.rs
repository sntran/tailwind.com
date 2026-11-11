//! Tests of the tree walk.
//!
//! The cases come from `walk.test.ts` of Tailwind CSS. Upstream has one
//! `walk` function with three forms of the visitor argument. Here, each
//! group of cases runs with each function of `walk.rs` that can do it.

use tailwind::ast::{decl, rule, AstNode};
use tailwind::to_css::to_css;
use tailwind::walk::{walk, walk_enter_exit, walk_siblings, VisitContext, WalkAction, Walkable};

/// A test node: a name, and children when `nodes` is there.
#[derive(Debug, Clone, PartialEq)]
struct Node {
    kind: &'static str,
    nodes: Option<Vec<Node>>,
}

impl Walkable for Node {
    fn nodes(&self) -> Option<&Vec<Self>> {
        self.nodes.as_ref()
    }
    fn nodes_mut(&mut self) -> Option<&mut Vec<Self>> {
        self.nodes.as_mut()
    }
    fn shallow(&self) -> Self {
        Node {
            kind: self.kind,
            nodes: self.nodes.as_ref().map(|_| Vec::new()),
        }
    }
}

fn leaf(kind: &'static str) -> Node {
    Node { kind, nodes: None }
}

fn branch(kind: &'static str, nodes: Vec<Node>) -> Node {
    Node {
        kind,
        nodes: Some(nodes),
    }
}

/// The tree that each upstream case uses.
fn tree() -> Vec<Node> {
    vec![
        branch(
            "a",
            vec![
                branch("b", vec![leaf("c")]),
                branch("d", vec![branch("e", vec![leaf("f")])]),
                branch("g", vec![leaf("h")]),
            ],
        ),
        leaf("i"),
    ]
}

fn two_branches(first: [&'static str; 2], second: [&'static str; 2]) -> Vec<Node> {
    vec![
        branch(first[0], vec![leaf(first[1])]),
        branch(second[0], vec![leaf(second[1])]),
    ]
}

/// The names of the nodes, in the sequence of a walk.
fn kinds(ast: &mut Vec<Node>) -> Vec<&'static str> {
    let mut visited = Vec::new();
    walk(ast, |node, _| {
        visited.push(node.kind);
        WalkAction::Continue
    });
    visited
}

/// The path to a node as text, as upstream prints it.
fn path_text(node: &Node, ctx: &VisitContext<Node>) -> String {
    let mut path: Vec<&str> = ctx.path.iter().map(|n| n.kind).collect();
    if path.is_empty() {
        path.push("ø");
    }
    path.push(node.kind);
    path.join(" → ")
}

type Visitor<'a> = &'a mut dyn FnMut(&mut Node, &VisitContext<Node>) -> WalkAction<Node>;

/// The cases of the enter phase. `$run` does a walk with an enter visitor.
macro_rules! enter_tests {
    ($run:path) => {
        use super::*;

        fn visit(
            ast: &mut Vec<Node>,
            on: &'static str,
            action: fn() -> WalkAction<Node>,
        ) -> Vec<&'static str> {
            let mut visited = Vec::new();
            $run(ast, &mut |node, _| {
                visited.push(node.kind);
                if node.kind == on {
                    action()
                } else {
                    WalkAction::Continue
                }
            });
            visited
        }

        #[test]
        fn visit_all_nodes() {
            let mut ast = tree();
            let visited = visit(&mut ast, "", || WalkAction::Continue);
            assert_eq!(visited, ["a", "b", "c", "d", "e", "f", "g", "h", "i"]);
            assert_eq!(ast, tree());
        }

        #[test]
        fn visit_all_nodes_and_give_their_path() {
            let mut ast = tree();
            let mut paths = Vec::new();
            $run(&mut ast, &mut |node, ctx| {
                assert_eq!(ctx.depth, ctx.path.len());
                assert_eq!(ctx.parent(), ctx.path.last());
                paths.push(path_text(node, ctx));
                WalkAction::Continue
            });
            assert_eq!(
                paths,
                [
                    "ø → a",
                    "a → b",
                    "a → b → c",
                    "a → d",
                    "a → d → e",
                    "a → d → e → f",
                    "a → g",
                    "a → g → h",
                    "ø → i",
                ]
            );
        }

        #[test]
        fn skip_the_children_of_the_first_node() {
            let visited = visit(&mut tree(), "b", || WalkAction::Skip);
            assert_eq!(visited, ["a", "b", "d", "e", "f", "g", "h", "i"]);
        }

        #[test]
        fn skip_the_children_of_a_middle_node() {
            let visited = visit(&mut tree(), "d", || WalkAction::Skip);
            assert_eq!(visited, ["a", "b", "c", "d", "g", "h", "i"]);
        }

        #[test]
        fn skip_the_children_of_the_last_node() {
            let visited = visit(&mut tree(), "g", || WalkAction::Skip);
            assert_eq!(visited, ["a", "b", "c", "d", "e", "f", "g", "i"]);
        }

        #[test]
        fn stop_the_walk() {
            let visited = visit(&mut tree(), "d", || WalkAction::Stop);
            assert_eq!(visited, ["a", "b", "c", "d"]);
        }

        #[test]
        fn replace_a_node_and_visit_the_replacements() {
            let mut ast = tree();
            let visited = visit(&mut ast, "d", || {
                WalkAction::Replace(two_branches(["e1", "f1"], ["e2", "f2"]))
            });
            assert_eq!(
                visited,
                ["a", "b", "c", "d", "e1", "f1", "e2", "f2", "g", "h", "i"]
            );
            assert_eq!(
                kinds(&mut ast),
                ["a", "b", "c", "e1", "f1", "e2", "f2", "g", "h", "i"]
            );
        }

        #[test]
        fn replace_a_node_and_do_not_visit_the_replacements() {
            let mut ast = tree();
            let visited = visit(&mut ast, "d", || {
                WalkAction::ReplaceSkip(two_branches(["e1", "f1"], ["e2", "f2"]))
            });
            assert_eq!(visited, ["a", "b", "c", "d", "g", "h", "i"]);
            // A second walk shows the new nodes.
            assert_eq!(
                kinds(&mut ast),
                ["a", "b", "c", "e1", "f1", "e2", "f2", "g", "h", "i"]
            );
        }

        #[test]
        fn replace_a_leaf_node_and_visit_the_replacements() {
            let mut ast = tree();
            let visited = visit(&mut ast, "f", || {
                WalkAction::Replace(two_branches(["foo1", "bar1"], ["foo2", "bar2"]))
            });
            assert_eq!(
                visited,
                ["a", "b", "c", "d", "e", "f", "foo1", "bar1", "foo2", "bar2", "g", "h", "i"]
            );
        }

        #[test]
        fn replace_a_leaf_node_and_do_not_visit_the_replacements() {
            let mut ast = tree();
            let visited = visit(&mut ast, "f", || {
                WalkAction::ReplaceSkip(two_branches(["foo1", "bar1"], ["foo2", "bar2"]))
            });
            assert_eq!(visited, ["a", "b", "c", "d", "e", "f", "g", "h", "i"]);
            // A second walk shows the new nodes.
            assert_eq!(
                kinds(&mut ast),
                ["a", "b", "c", "d", "e", "foo1", "bar1", "foo2", "bar2", "g", "h", "i"]
            );
        }

        #[test]
        fn replace_a_node_and_stop_the_walk() {
            let mut ast = tree();
            let visited = visit(&mut ast, "d", || {
                WalkAction::ReplaceStop(two_branches(["e1", "f1"], ["e2", "f2"]))
            });
            assert_eq!(visited, ["a", "b", "c", "d"]);
            // A second walk shows the new nodes.
            assert_eq!(
                kinds(&mut ast),
                ["a", "b", "c", "e1", "f1", "e2", "f2", "g", "h", "i"]
            );
        }
    };
}

/// The cases of the exit phase. `$run` does a walk with an exit visitor.
macro_rules! exit_tests {
    ($run:path) => {
        use super::*;

        fn visit(
            ast: &mut Vec<Node>,
            on: &'static str,
            action: fn() -> WalkAction<Node>,
        ) -> Vec<&'static str> {
            let mut visited = Vec::new();
            $run(ast, &mut |node, _| {
                visited.push(node.kind);
                if node.kind == on {
                    action()
                } else {
                    WalkAction::Continue
                }
            });
            visited
        }

        #[test]
        fn visit_all_nodes() {
            let mut ast = tree();
            let visited = visit(&mut ast, "", || WalkAction::Continue);
            assert_eq!(visited, ["c", "b", "f", "e", "d", "h", "g", "a", "i"]);
            assert_eq!(ast, tree());
        }

        #[test]
        fn visit_all_nodes_and_give_their_path() {
            let mut ast = tree();
            let mut paths = Vec::new();
            $run(&mut ast, &mut |node, ctx| {
                assert_eq!(ctx.depth, ctx.path.len());
                paths.push(path_text(node, ctx));
                WalkAction::Continue
            });
            assert_eq!(
                paths,
                [
                    "a → b → c",
                    "a → b",
                    "a → d → e → f",
                    "a → d → e",
                    "a → d",
                    "a → g → h",
                    "a → g",
                    "ø → a",
                    "ø → i",
                ]
            );
        }

        #[test]
        fn stop_the_walk() {
            let visited = visit(&mut tree(), "d", || WalkAction::Stop);
            assert_eq!(visited, ["c", "b", "f", "e", "d"]);
        }

        #[test]
        fn replace_a_node_and_do_not_visit_the_replacements() {
            let mut ast = tree();
            let visited = visit(&mut ast, "d", || {
                WalkAction::Replace(two_branches(["e1", "f1"], ["e2", "f2"]))
            });
            assert_eq!(visited, ["c", "b", "f", "e", "d", "h", "g", "a", "i"]);
            // A second walk shows the new nodes.
            assert_eq!(
                kinds(&mut ast),
                ["a", "b", "c", "e1", "f1", "e2", "f2", "g", "h", "i"]
            );
        }

        #[test]
        fn replace_skip_in_exit_is_the_same_as_replace() {
            let mut ast = tree();
            let visited = visit(&mut ast, "d", || {
                WalkAction::ReplaceSkip(two_branches(["e1", "f1"], ["e2", "f2"]))
            });
            assert_eq!(visited, ["c", "b", "f", "e", "d", "h", "g", "a", "i"]);
            assert_eq!(
                kinds(&mut ast),
                ["a", "b", "c", "e1", "f1", "e2", "f2", "g", "h", "i"]
            );
        }

        #[test]
        fn replace_a_leaf_node_and_do_not_visit_the_replacements() {
            let mut ast = tree();
            let visited = visit(&mut ast, "f", || {
                WalkAction::Replace(vec![leaf("f1"), leaf("f2")])
            });
            assert_eq!(visited, ["c", "b", "f", "e", "d", "h", "g", "a", "i"]);
            // A second walk shows the new nodes.
            assert_eq!(
                kinds(&mut ast),
                ["a", "b", "c", "d", "e", "f1", "f2", "g", "h", "i"]
            );
        }

        #[test]
        fn replace_a_node_and_stop_the_walk() {
            let mut ast = tree();
            let visited = visit(&mut ast, "d", || {
                WalkAction::ReplaceStop(two_branches(["e1", "f1"], ["e2", "f2"]))
            });
            assert_eq!(visited, ["c", "b", "f", "e", "d"]);
            // A second walk shows the new nodes.
            assert_eq!(
                kinds(&mut ast),
                ["a", "b", "c", "e1", "f1", "e2", "f2", "g", "h", "i"]
            );
        }
    };
}

fn run_walk(ast: &mut Vec<Node>, enter: Visitor) {
    walk(ast, enter);
}

fn run_enter_of_enter_exit(ast: &mut Vec<Node>, enter: Visitor) {
    walk_enter_exit(ast, enter, |_, _| WalkAction::Continue);
}

fn run_enter_of_siblings(ast: &mut Vec<Node>, enter: Visitor) {
    walk_siblings(
        ast,
        |nodes, ctx| enter(&mut nodes[ctx.index], ctx),
        |_, _| WalkAction::Continue,
    );
}

fn run_exit_of_enter_exit(ast: &mut Vec<Node>, exit: Visitor) {
    walk_enter_exit(ast, |_, _| WalkAction::Continue, exit);
}

fn run_exit_of_siblings(ast: &mut Vec<Node>, exit: Visitor) {
    walk_siblings(
        ast,
        |_, _| WalkAction::Continue,
        |nodes, ctx| exit(&mut nodes[ctx.index], ctx),
    );
}

// Upstream group: AST Enter (function).
mod enter_with_walk {
    enter_tests!(run_walk);
}

// Upstream groups: AST Enter (obj), and the enter phase of AST Enter & Exit.
mod enter_with_walk_enter_exit {
    enter_tests!(run_enter_of_enter_exit);
}

mod enter_with_walk_siblings {
    enter_tests!(run_enter_of_siblings);
}

// Upstream groups: AST Exit (obj), and the exit phase of AST Enter & Exit.
mod exit_with_walk_enter_exit {
    exit_tests!(run_exit_of_enter_exit);
}

mod exit_with_walk_siblings {
    exit_tests!(run_exit_of_siblings);
}

// Upstream group: AST Enter & Exit.

#[test]
fn enter_and_exit_visit_all_nodes() {
    let mut ast = tree();
    let visited = std::cell::RefCell::new(Vec::new());
    walk_siblings(
        &mut ast,
        |nodes, ctx| {
            let line = format!(
                "{} Enter({} @ {})",
                "  ".repeat(ctx.depth),
                nodes[ctx.index].kind,
                ctx.index
            );
            visited.borrow_mut().push(line);
            WalkAction::Continue
        },
        |nodes, ctx| {
            let line = format!(
                "{} Exit({} @ {})",
                "  ".repeat(ctx.depth),
                nodes[ctx.index].kind,
                ctx.index
            );
            visited.borrow_mut().push(line);
            WalkAction::Continue
        },
    );
    assert_eq!(
        visited.into_inner(),
        [
            " Enter(a @ 0)",
            "   Enter(b @ 0)",
            "     Enter(c @ 0)",
            "     Exit(c @ 0)",
            "   Exit(b @ 0)",
            "   Enter(d @ 1)",
            "     Enter(e @ 0)",
            "       Enter(f @ 0)",
            "       Exit(f @ 0)",
            "     Exit(e @ 0)",
            "   Exit(d @ 1)",
            "   Enter(g @ 2)",
            "     Enter(h @ 0)",
            "     Exit(h @ 0)",
            "   Exit(g @ 2)",
            " Exit(a @ 0)",
            " Enter(i @ 1)",
            " Exit(i @ 1)",
        ]
    );
}

#[test]
fn enter_and_exit_give_the_index_of_the_node_in_its_siblings() {
    // Each name is unique, so the visitor can find the node in the tree.
    fn siblings_of<'a>(nodes: &'a [Node], kind: &str) -> Option<&'a [Node]> {
        if nodes.iter().any(|n| n.kind == kind) {
            return Some(nodes);
        }
        nodes
            .iter()
            .filter_map(|n| n.nodes.as_deref())
            .find_map(|n| siblings_of(n, kind))
    }

    let original = tree();
    let check = |node: &mut Node, ctx: &VisitContext<Node>| {
        let siblings = siblings_of(&original, node.kind).unwrap();
        assert_eq!(
            siblings.iter().position(|n| n.kind == node.kind),
            Some(ctx.index)
        );
        WalkAction::Continue
    };
    walk_enter_exit(&mut tree(), check, check);
}

#[test]
fn enter_and_exit_visit_all_nodes_and_give_their_path() {
    let mut ast = tree();
    let paths = std::cell::RefCell::new(Vec::new());
    walk_enter_exit(
        &mut ast,
        |node, ctx| {
            paths
                .borrow_mut()
                .push(format!("Enter({})", path_text(node, ctx)));
            WalkAction::Continue
        },
        |node, ctx| {
            paths
                .borrow_mut()
                .push(format!("Exit({})", path_text(node, ctx)));
            WalkAction::Continue
        },
    );
    assert_eq!(
        paths.into_inner(),
        [
            "Enter(ø → a)",
            "Enter(a → b)",
            "Enter(a → b → c)",
            "Exit(a → b → c)",
            "Exit(a → b)",
            "Enter(a → d)",
            "Enter(a → d → e)",
            "Enter(a → d → e → f)",
            "Exit(a → d → e → f)",
            "Exit(a → d → e)",
            "Exit(a → d)",
            "Enter(a → g)",
            "Enter(a → g → h)",
            "Exit(a → g → h)",
            "Exit(a → g)",
            "Exit(ø → a)",
            "Enter(ø → i)",
            "Exit(ø → i)",
        ]
    );
}

#[test]
fn change_a_css_tree_in_enter_and_exit() {
    let mut ast = vec![rule(
        ".example",
        vec![
            decl("margin-top", "12px"),
            decl("padding", "8px"),
            decl("margin", "16px 18px"),
            decl("colors", "red"),
        ],
    )];

    walk_enter_exit(
        &mut ast,
        |node, _| {
            let AstNode::Declaration(declaration) = node else {
                return WalkAction::Continue;
            };
            let value = declaration.value.clone().unwrap_or_default();
            match declaration.property.as_str() {
                // Expand the `margin` shorthand. The walk visits the new nodes.
                "margin" if !value.is_empty() => {
                    let (y, x) = value.split_once(' ').unwrap();
                    WalkAction::Replace(vec![
                        decl("margin-top", y),
                        decl("margin-bottom", y),
                        decl("margin-left", x),
                        decl("margin-right", x),
                    ])
                }
                // The walk does not visit these new nodes.
                "colors" if !value.is_empty() => WalkAction::ReplaceSkip(vec![
                    decl("color", value.as_str()),
                    decl("background-color", value.as_str()),
                    decl("border-color", value.as_str()),
                ]),
                // Each visited property becomes upper case.
                _ => {
                    declaration.property = declaration.property.to_uppercase();
                    WalkAction::Continue
                }
            }
        },
        |node, _| {
            // Sort the declarations of a rule. Exit sees the nodes after
            // the changes of the enter phase. Upstream uses
            // `localeCompare`: here, a compare with no case gives the same
            // sequence.
            if let AstNode::Rule(rule) = node {
                rule.nodes
                    .sort_by_key(|n| n.as_declaration().map(|d| d.property.to_lowercase()));
            }
            WalkAction::Continue
        },
    );

    assert_eq!(
        to_css(&ast),
        "\
.example {
  background-color: red;
  border-color: red;
  color: red;
  MARGIN-BOTTOM: 16px;
  MARGIN-LEFT: 18px;
  MARGIN-RIGHT: 18px;
  MARGIN-TOP: 12px;
  MARGIN-TOP: 16px;
  PADDING: 8px;
}
"
    );
}

// Cases that are not in the upstream test file.

#[test]
fn replace_a_node_with_no_nodes() {
    let mut ast = tree();
    walk(&mut ast, |node, _| {
        if node.kind == "d" || node.kind == "i" {
            WalkAction::Replace(vec![])
        } else {
            WalkAction::Continue
        }
    });
    assert_eq!(kinds(&mut ast), ["a", "b", "c", "g", "h"]);
}

#[test]
fn exit_runs_for_a_node_that_enter_skips() {
    let mut ast = tree();
    let mut exited = Vec::new();
    walk_enter_exit(
        &mut ast,
        |node, _| {
            if node.kind == "d" {
                WalkAction::Skip
            } else {
                WalkAction::Continue
            }
        },
        |node, _| {
            exited.push(node.kind);
            WalkAction::Continue
        },
    );
    assert_eq!(exited, ["c", "b", "d", "h", "g", "a", "i"]);
}

#[test]
fn exit_does_not_run_for_a_node_that_enter_replaces_and_skips() {
    let mut ast = tree();
    let mut exited = Vec::new();
    walk_enter_exit(
        &mut ast,
        |node, _| {
            if node.kind == "d" {
                WalkAction::ReplaceSkip(vec![leaf("x"), leaf("y")])
            } else {
                WalkAction::Continue
            }
        },
        |node, _| {
            exited.push(node.kind);
            WalkAction::Continue
        },
    );
    assert_eq!(exited, ["c", "b", "h", "g", "a", "i"]);
}

#[test]
fn the_siblings_visitor_can_read_the_nodes_near_the_node() {
    let mut ast = tree();
    let mut next = Vec::new();
    walk_siblings(
        &mut ast,
        |nodes, ctx| {
            next.push((
                nodes[ctx.index].kind,
                nodes.get(ctx.index + 1).map(|n| n.kind),
            ));
            WalkAction::Continue
        },
        |_, _| WalkAction::Continue,
    );
    assert_eq!(next[0], ("a", Some("i")));
    assert_eq!(next[1], ("b", Some("d")));
    assert_eq!(next[2], ("c", None));
}

#[test]
#[should_panic(expected = "must not return Skip")]
fn exit_must_not_return_skip() {
    walk_enter_exit(
        &mut tree(),
        |_, _| WalkAction::Continue,
        |_, _| WalkAction::Skip,
    );
}
