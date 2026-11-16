//! Owner: lane H7-ANIMTREE (host track). Maya's AnimTrees evaluated to a pose, headless, in game units, on top of
//! the stage 1 sampler (`super::pose`).
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-56-*`.
//!
//! - [`data`]: a tree as plain data (nodes by export index, each node's kind and child links).
//! - [`load`]: builds it from the definition store, with each class's defaults applied.
//! - [`graph`]: the cycle guard and the depth budget.
//! - [`eval`]: the driver: named inputs, one-shot requests, a deterministic tick, and the pose.
//! - [`report`]: the installed-game check.
//!
//! **Evidence.** `src/` has no AnimTree code, and the AnimNode classes' functions are native (74 of the 78 in the
//! three code packages have no script body; the four that do are empty or start a clip at init). So the tree data
//! decides structure and parameters, one record decides the special-move blend times, and everything else is a
//! labelled stand-in ([`eval`] lists them). Classes whose own native part is unread are `NotYet` ([`load::NOT_YET`]).
//! Root motion and notifies are read and reported, never applied.

pub mod data;
pub mod eval;
pub mod graph;
pub mod load;
pub mod report;

pub use data::{Child, Kind, Node, NodeId, Selector, SequenceParams, TreeDef};
pub use eval::{ClipInfo, ClipLibrary, Driver, Ending, EvalError, Inputs, Play, Unsupported};
pub use load::{TreeError, load_tree};

#[cfg(test)]
mod tests;
