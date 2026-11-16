//! Owner: lane H7-ANIMTREE. The graph checks of a [`TreeDef`]: every link lands on a node of the tree, no node is its
//! own ancestor (a cycle), and the walk's path from the root stays within [`MAX_DEPTH`]. One iterative depth-first
//! walk with an explicit stack, each node expanded once; nodes reached by two parents (the installed trees share a
//! few) are fine. The evaluator recurses along the same links, so this bound is also its recursion budget
//! (`super::eval` checks its own depth again).

use std::fmt;

use ow_core::DetMap;

use super::data::{NodeId, TreeDef};

/// Deepest path allowed from the root (the walk reaches 11 in the installed hands tree and 18 in the body tree).
pub const MAX_DEPTH: usize = 64;

#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// The root is not a node of the tree.
    NoRoot(NodeId),
    /// A child link names a node the tree does not have.
    Dangling { from: NodeId, to: NodeId },
    /// A node is reached again from one of its own descendants.
    Cycle { at: NodeId },
    /// A path from the root is deeper than [`MAX_DEPTH`].
    TooDeep { at: NodeId },
}

impl fmt::Display for GraphError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GraphError::NoRoot(r) => write!(f, "root {r} is not a node of the tree"),
            GraphError::Dangling { from, to } => {
                write!(f, "node {from} links to {to}, which is not a node")
            }
            GraphError::Cycle { at } => write!(f, "node {at} is its own ancestor (a cycle)"),
            GraphError::TooDeep { at } => {
                write!(f, "node {at} is deeper than {MAX_DEPTH} below the root")
            }
        }
    }
}

impl std::error::Error for GraphError {}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Mark {
    OnPath,
    Done,
}

/// Checks the tree; returns the deepest path the walk took (the root alone is 1).
pub fn check(tree: &TreeDef) -> Result<usize, GraphError> {
    if !tree.nodes.contains_key(&tree.root) {
        return Err(GraphError::NoRoot(tree.root));
    }
    let mut marks: DetMap<NodeId, Mark> = DetMap::new();
    // (node, index of the next child to visit)
    let mut stack: Vec<(NodeId, usize)> = vec![(tree.root, 0)];
    marks.insert(tree.root, Mark::OnPath);
    let mut deepest = 1;
    while let Some(&(id, next)) = stack.last() {
        let node = tree.nodes.get(&id).ok_or(GraphError::NoRoot(id))?;
        let found = node
            .children
            .iter()
            .enumerate()
            .skip(next)
            .find_map(|(k, c)| c.node.map(|n| (k, n)));
        match found {
            Some((k, c)) => {
                if let Some(top) = stack.last_mut() {
                    top.1 = k + 1;
                }
                if !tree.nodes.contains_key(&c) {
                    return Err(GraphError::Dangling { from: id, to: c });
                }
                match marks.get(&c) {
                    Some(Mark::OnPath) => return Err(GraphError::Cycle { at: c }),
                    Some(Mark::Done) => {}
                    None => {
                        if stack.len() == MAX_DEPTH {
                            return Err(GraphError::TooDeep { at: c });
                        }
                        marks.insert(c, Mark::OnPath);
                        stack.push((c, 0));
                        deepest = deepest.max(stack.len());
                    }
                }
            }
            None => {
                marks.insert(id, Mark::Done);
                stack.pop();
            }
        }
    }
    Ok(deepest)
}
