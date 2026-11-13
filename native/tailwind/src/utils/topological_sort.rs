//! A topological sort: `utils/topological-sort.ts` of Tailwind CSS.

use crate::hash::HashMap;
use std::hash::Hash;

/// A circular dependency. These are the arguments of the upstream callback
/// `onCircularDependency`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CircularDependency<K> {
    /// The nodes from the first node of the visit to the node before `start`.
    pub path: Vec<K>,
    /// The node that the visit finds again.
    pub start: K,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    New,
    Wip,
    Seen,
}

/// Sort the nodes of a graph: each node comes after its dependencies.
///
/// `graph` is the upstream `Map<Key, Set<Key>>` in insertion order: each key
/// one time, with its dependencies in insertion order. A dependency that is
/// not a key of the graph is ignored.
///
/// The upstream callback always throws, so the first cycle is the error.
pub fn topological_sort<K: Eq + Hash + Clone>(
    graph: &[(K, Vec<K>)],
) -> Result<Vec<K>, CircularDependency<K>> {
    let index: HashMap<&K, usize> = graph.iter().enumerate().map(|(i, (key, _))| (key, i)).collect();
    let mut state = vec![State::New; graph.len()];
    let mut sorted = Vec::with_capacity(graph.len());
    let mut path: Vec<usize> = Vec::new();

    // Each frame is a node and the position of its next dependency.
    let mut stack: Vec<(usize, usize)> = Vec::new();

    for root in 0..graph.len() {
        if state[root] != State::New {
            continue;
        }
        state[root] = State::Wip;
        stack.push((root, 0));

        while let Some(frame) = stack.last_mut() {
            let (node, position) = *frame;
            let dependencies = &graph[node].1;
            if position == dependencies.len() {
                state[node] = State::Seen;
                sorted.push(graph[node].0.clone());
                stack.pop();
                path.pop();
                continue;
            }
            frame.1 += 1;
            let Some(&dependency) = index.get(&dependencies[position]) else {
                continue;
            };
            match state[dependency] {
                State::Seen => {}
                State::Wip => {
                    path.push(node);
                    return Err(CircularDependency {
                        path: path.iter().map(|&i| graph[i].0.clone()).collect(),
                        start: graph[dependency].0.clone(),
                    });
                }
                State::New => {
                    path.push(node);
                    state[dependency] = State::Wip;
                    stack.push((dependency, 0));
                }
            }
        }
    }

    Ok(sorted)
}
