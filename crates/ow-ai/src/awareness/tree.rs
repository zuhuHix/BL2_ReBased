//! Owner: lane W7-AI (part 2). The AI action tree: which action runs, when it may run again, and what stops it
//! (NATIVE_AI_AWARENESS.md, "AI tree natives"; NATIVE_AI_MELEE.md, "State selection"). AI-assisted (Claude Code).
//! Decision 2026-11-16-66.
//!
//! The tree is `AIDefinition.NodeList` (`AIComponent.AITreeData`); node 0 is the root. Node types: action, random,
//! priority, sequence. A node's `ActiveChild`, `LastRandomChild` and `NextSequentialChild` are indices into its own
//! `Children` (live, L9 section 7). Every random value is uniform in `[min, max]` from the engine's integer random
//! divided by 32768 ([`TreeWorld::rand`]).
//!
//! **What is live** (AI_AWARENESS_LIVE.md section 7, one regular bullymong in a 62 s fight): the `TimedStart` re-arm on
//! acquiring a target (+10.0003 for a 10 to 10 range), the `TimedReset` cooldown after a node finishes (+27.05 inside
//! 20 to 30), the random node's `LoopCount` and `LastRandomChild`, and `CanRun` failure names skipping a node in the
//! same update. **Not exercised live**: the `LoopCountMax` cap, the 1.0 s block after a failure, `Finish` with
//! `bFailed`, the timed fail. Everything is UNVERIFIED unless named here.
//!
//! **What the world answers** ([`TreeWorld`]): an action's script `CanRun` and its `Start` / `Stop` events, the
//! node's evaluators (`CanRunIf`, a priority node's `Conditions`), whether the running action is locked (running and
//! non-interruptible), and the random draws. A node with a `Resource` is `NotYet`: the AI resource registry's
//! virtuals are not read.

use ow_core::NotYet;
use ow_vm::error::VmError;

use super::RECORD_AWARENESS;

#[cfg(test)]
mod tests;

/// A node's type (`AITreeData.Type`: 1 action, 2 random, 3 priority, 4 sequence).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Action,
    Random,
    Priority,
    Sequence,
}

impl NodeKind {
    /// The kind of a `Type` byte, if it is one.
    pub fn of(byte: u8) -> Option<NodeKind> {
        match byte {
            1 => Some(NodeKind::Action),
            2 => Some(NodeKind::Random),
            3 => Some(NodeKind::Priority),
            4 => Some(NodeKind::Sequence),
            _ => None,
        }
    }
}

/// A `(min, max)` time range; valid when both are positive and `min <= max`.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Range {
    pub min: f32,
    pub max: f32,
}

impl Range {
    pub fn new(min: f32, max: f32) -> Self {
        Range { min, max }
    }

    pub fn valid(self) -> bool {
        self.min > 0.0 && self.max > 0.0 && self.min <= self.max
    }

    /// `min + (max - min) * u` for a draw `u` in `[0, 1)`.
    pub fn at(self, u: f32) -> f32 {
        self.min + (self.max - self.min) * u
    }
}

/// Which evaluator of a node the world is asked about.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    /// The node's `CanRunIf`.
    CanRunIf,
    /// The priority node's `Conditions[i]`, the condition of its `i`th child.
    Condition(usize),
}

/// One node: its data and its run state (the `AITreeData` fields).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    pub kind: NodeKind,
    pub children: Vec<usize>,
    pub has_resource: bool,
    pub has_can_run_if: bool,
    /// Per child of a priority node: whether its `Conditions` entry is set.
    pub conditions: Vec<bool>,
    pub timed_start: Range,
    pub timed_reset: Range,
    pub timed_fail: Range,
    pub stop_if_cannot_run: bool,
    pub interrupt: bool,
    /// Whether the action node has an action object.
    pub has_action: bool,
    pub weights: Vec<f32>,
    pub loop_count_max: Vec<i32>,
    // The run state.
    pub active_child: i32,
    pub next_active_child: i32,
    pub last_fail_time: f32,
    pub next_timed_run: f32,
    pub next_timed_fail: f32,
    pub loop_count: i32,
    pub last_random_child: i32,
    pub next_sequential_child: i32,
    /// `ActionFailName`: the last `CanRun` failure (`None` when it could run).
    pub action_fail_name: Option<String>,
}

impl Node {
    /// A node of `kind` over `children`, every flag clear and every time zero.
    pub fn new(kind: NodeKind, children: Vec<usize>) -> Self {
        Node {
            kind,
            children,
            has_resource: false,
            has_can_run_if: false,
            conditions: Vec::new(),
            timed_start: Range::default(),
            timed_reset: Range::default(),
            timed_fail: Range::default(),
            stop_if_cannot_run: false,
            interrupt: false,
            has_action: kind == NodeKind::Action,
            weights: Vec::new(),
            loop_count_max: Vec::new(),
            active_child: -1,
            next_active_child: -1,
            last_fail_time: 0.0,
            next_timed_run: 0.0,
            next_timed_fail: 0.0,
            loop_count: 0,
            last_random_child: -1,
            next_sequential_child: 0,
            action_fail_name: None,
        }
    }
}

/// What the tree asks of the AI and its actions (module documentation).
pub trait TreeWorld {
    /// The engine's integer random divided by 32768: a uniform draw in `[0, 1)`.
    fn rand(&mut self) -> f32;
    /// The evaluator `gate` of node `node`, for the AI's pawn.
    fn evaluate(&mut self, node: usize, gate: Gate) -> Result<bool, VmError>;
    /// Action node `node`'s script `CanRun`: `None` when it can run, else the failure name.
    fn can_run(&mut self, node: usize) -> Result<Option<String>, VmError>;
    /// Raises `Start` on action node `node`'s action (the start of its state).
    fn start(&mut self, node: usize) -> Result<(), VmError>;
    /// Raises `Stop` on action node `node`'s action, and puts it back to its default state.
    fn stop(&mut self, node: usize, failed: bool) -> Result<(), VmError>;
    /// Whether action node `node`'s action is running with its non-interruptible flag set.
    fn locked(&mut self, node: usize) -> Result<bool, VmError>;
}

/// The tree of one component: its nodes and the running leaf (`CurrentState`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Tree {
    pub nodes: Vec<Node>,
    /// The running action node, if any.
    pub current: Option<usize>,
}

/// What a state update did.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Change {
    /// The action node that stopped, and whether it stopped as failed.
    pub stopped: Option<(usize, bool)>,
    /// The action node that started.
    pub started: Option<usize>,
}

fn bad_tree(what: String) -> VmError {
    VmError::Script { function: "ai tree".to_string(), message: what }
}

fn child(node: &Node, index: i32) -> Option<usize> {
    usize::try_from(index).ok().and_then(|i| node.children.get(i).copied())
}

impl Tree {
    pub fn new(nodes: Vec<Node>) -> Result<Tree, VmError> {
        for (i, n) in nodes.iter().enumerate() {
            if let Some(&c) = n.children.iter().find(|&&c| c >= nodes.len()) {
                return Err(bad_tree(format!("node {i} names child {c}, outside the {} nodes", nodes.len())));
            }
        }
        if nodes.is_empty() {
            return Err(bad_tree("an AI tree with no nodes".to_string()));
        }
        Ok(Tree { nodes, current: None })
    }

    /// The running path, root first, following `ActiveChild` (bounded by the node count).
    pub fn path(&self) -> Vec<usize> {
        let mut path = vec![0];
        let mut at = 0;
        while path.len() <= self.nodes.len() {
            match child(&self.nodes[at], self.nodes[at].active_child) {
                Some(c) => {
                    path.push(c);
                    at = c;
                }
                None => break,
            }
        }
        path
    }

    /// Node eligibility (G26, in order; the first failure answers false).
    pub fn eligible(&mut self, w: &mut dyn TreeWorld, now: f32, node: usize) -> Result<bool, VmError> {
        let n = &self.nodes[node];
        // 1. not before NextTimedRun.
        if n.next_timed_run != 0.0 && now < n.next_timed_run {
            return Ok(false);
        }
        // 2. CanRunIf, unless this is the running action node without bStopIfCannotRun.
        let running_keep = self.current == Some(node) && !n.stop_if_cannot_run;
        if n.has_can_run_if && !running_keep && !w.evaluate(node, Gate::CanRunIf)? {
            return Ok(false);
        }
        // 3. 1.0 s after a failure.
        if n.last_fail_time != 0.0 && now < n.last_fail_time + 1.0 {
            return Ok(false);
        }
        // 4. an action node's script CanRun (skipped, as 2, for the running node without bStopIfCannotRun:
        // UNVERIFIED reading of "an action node that needs checking").
        if n.kind == NodeKind::Action && !running_keep {
            if !n.has_action {
                return Ok(false);
            }
            let fail = w.can_run(node)?;
            let ok = fail.is_none();
            self.nodes[node].action_fail_name = fail;
            if !ok {
                return Ok(false);
            }
        }
        // 5. the node's resource.
        if self.nodes[node].has_resource {
            return Err(NotYet::not_read(
                "an AI tree node with a Resource: the AI resource registry's virtuals",
                RECORD_AWARENESS,
            )
            .into());
        }
        Ok(true)
    }

    /// The selection pass: every node's next active child cleared, then the root evaluated. Returns the chosen leaf.
    pub fn select(&mut self, w: &mut dyn TreeWorld, now: f32) -> Result<Option<usize>, VmError> {
        for n in &mut self.nodes {
            n.next_active_child = -1;
        }
        self.select_in(w, now, 0, 0)
    }

    fn select_in(
        &mut self,
        w: &mut dyn TreeWorld,
        now: f32,
        node: usize,
        depth: usize,
    ) -> Result<Option<usize>, VmError> {
        if depth > self.nodes.len() {
            return Err(bad_tree(format!("the AI tree loops through node {node}")));
        }
        if !self.eligible(w, now, node)? {
            return Ok(None);
        }
        match self.nodes[node].kind {
            NodeKind::Action => Ok(Some(node)),
            NodeKind::Priority => self.select_priority(w, now, node, depth),
            NodeKind::Random => self.select_random(w, now, node, depth),
            NodeKind::Sequence => {
                let slot = self.nodes[node].next_sequential_child;
                let Some(c) = child(&self.nodes[node], slot) else {
                    return Ok(None);
                };
                let leaf = self.select_in(w, now, c, depth + 1)?;
                if leaf.is_some() {
                    self.nodes[node].next_active_child = slot;
                }
                Ok(leaf)
            }
        }
    }

    fn select_priority(
        &mut self,
        w: &mut dyn TreeWorld,
        now: f32,
        node: usize,
        depth: usize,
    ) -> Result<Option<usize>, VmError> {
        let n = &self.nodes[node];
        if n.children.len() != n.conditions.len() {
            return Ok(None);
        }
        // Not interruptible with an active child: only that child is evaluated again.
        if !n.interrupt
            && let Some(c) = child(n, n.active_child)
        {
            let slot = n.active_child;
            let leaf = self.select_in(w, now, c, depth + 1)?;
            if leaf.is_some() {
                self.nodes[node].next_active_child = slot;
            }
            return Ok(leaf);
        }
        for slot in 0..self.nodes[node].children.len() {
            let c = self.nodes[node].children[slot];
            if self.nodes[node].conditions[slot] && !w.evaluate(node, Gate::Condition(slot))? {
                continue;
            }
            if let Some(leaf) = self.select_in(w, now, c, depth + 1)? {
                self.nodes[node].next_active_child = i32::try_from(slot).unwrap_or(-1);
                return Ok(Some(leaf));
            }
        }
        Ok(None)
    }

    fn select_random(
        &mut self,
        w: &mut dyn TreeWorld,
        now: f32,
        node: usize,
        depth: usize,
    ) -> Result<Option<usize>, VmError> {
        let n = &self.nodes[node];
        if n.children.len() != n.weights.len() {
            return Ok(None);
        }
        if let Some(c) = child(n, n.active_child) {
            let slot = n.active_child;
            let leaf = self.select_in(w, now, c, depth + 1)?;
            if leaf.is_some() {
                self.nodes[node].next_active_child = slot;
            }
            return Ok(leaf);
        }
        // The candidates: every child but the last random child when its non-zero LoopCountMax is reached.
        let mut candidates: Vec<usize> = (0..n.children.len())
            .filter(|&slot| {
                let capped = n.loop_count_max.get(slot).copied().unwrap_or(0);
                !(i32::try_from(slot).ok() == Some(n.last_random_child) && capped != 0 && n.loop_count >= capped)
            })
            .collect();
        while !candidates.is_empty() {
            let total: f32 = candidates.iter().map(|&s| self.nodes[node].weights[s]).sum();
            // A uniform draw in [0, 1) times the weight sum; the first candidate whose running sum passes it.
            let draw = w.rand() * total;
            let mut sum = 0.0f32;
            let mut pick = candidates.len() - 1;
            for (i, &s) in candidates.iter().enumerate() {
                sum += self.nodes[node].weights[s];
                if draw < sum {
                    pick = i;
                    break;
                }
            }
            let slot = candidates.remove(pick);
            let c = self.nodes[node].children[slot];
            if let Some(leaf) = self.select_in(w, now, c, depth + 1)? {
                self.nodes[node].next_active_child = i32::try_from(slot).unwrap_or(-1);
                return Ok(Some(leaf));
            }
        }
        Ok(None)
    }

    /// Activation of the chosen path: `ActiveChild = NextActiveChild` along it, and each node's `TimedFail` armed.
    fn activate(&mut self, w: &mut dyn TreeWorld, now: f32) {
        let mut at = 0;
        for _ in 0..=self.nodes.len() {
            let n = &mut self.nodes[at];
            n.active_child = n.next_active_child;
            if n.timed_fail.valid() {
                let r = n.timed_fail;
                self.nodes[at].next_timed_fail = now + r.at(w.rand());
            }
            match child(&self.nodes[at], self.nodes[at].active_child) {
                Some(c) => at = c,
                None => break,
            }
        }
    }

    /// The completion walk of the running path (G26, "Completion"): leaf first, each finishing node arms its
    /// `TimedReset` cooldown; a sequence that has more children advances and is not done (its parents stay); a random
    /// node counts loops of the same child.
    fn complete(&mut self, w: &mut dyn TreeWorld, now: f32, failed: bool) {
        let path = self.path();
        let mut done = true;
        for &at in path.iter().rev() {
            if !done {
                break;
            }
            let finished_slot = self.nodes[at].active_child;
            let n = &mut self.nodes[at];
            match n.kind {
                NodeKind::Sequence => {
                    let last = i32::try_from(n.children.len()).unwrap_or(0) - 1;
                    if !failed && n.next_sequential_child < last {
                        n.next_sequential_child += 1;
                        n.active_child = -1;
                        done = false;
                        continue;
                    }
                    n.next_sequential_child = 0;
                }
                NodeKind::Random => {
                    if finished_slot == n.last_random_child {
                        n.loop_count = n.loop_count.saturating_add(1);
                    } else {
                        n.loop_count = 1;
                        n.last_random_child = finished_slot;
                    }
                }
                NodeKind::Action | NodeKind::Priority => {}
            }
            n.active_child = -1;
            if n.timed_reset.valid() {
                let r = n.timed_reset;
                self.nodes[at].next_timed_run = now + r.at(w.rand());
            }
        }
    }

    /// Starting a state: the leaf becomes `CurrentState`, its path is activated, and `Start` is raised.
    pub fn start_state(&mut self, w: &mut dyn TreeWorld, now: f32, leaf: usize) -> Result<(), VmError> {
        self.current = Some(leaf);
        self.activate(w, now);
        w.start(leaf)
    }

    /// Stopping a state: `CurrentState` cleared, `Stop` raised on the action, and the completion walk of the path.
    pub fn stop_state(&mut self, w: &mut dyn TreeWorld, now: f32, failed: bool) -> Result<Option<usize>, VmError> {
        let Some(leaf) = self.current.take() else {
            return Ok(None);
        };
        w.stop(leaf, failed)?;
        self.complete(w, now, failed);
        Ok(Some(leaf))
    }

    /// "Action finished": the state stops, a new leaf is selected at once, and it starts.
    pub fn action_finished(&mut self, w: &mut dyn TreeWorld, now: f32, failed: bool) -> Result<Change, VmError> {
        let mut change = Change::default();
        if let Some(leaf) = self.stop_state(w, now, failed)? {
            change.stopped = Some((leaf, failed));
        }
        if let Some(leaf) = self.select(w, now)? {
            self.start_state(w, now, leaf)?;
            change.started = Some(leaf);
        }
        Ok(change)
    }

    /// `ActionSequence.Finish(bFailed)` on action node `node`: only when it is the running state; a failure stamps
    /// `LastFailTime`; then the action finishes (a new leaf is selected and started at once).
    pub fn finish(&mut self, w: &mut dyn TreeWorld, now: f32, node: usize, failed: bool) -> Result<Change, VmError> {
        if self.current != Some(node) {
            return Ok(Change::default());
        }
        if failed {
            self.nodes[node].last_fail_time = now;
        }
        self.action_finished(w, now, failed)
    }

    /// From no target to one (`TargetChanged`): every node with a valid `TimedStart` gets `NextTimedRun` armed.
    pub fn rearm_timed_starts(&mut self, w: &mut dyn TreeWorld, now: f32) {
        for i in 0..self.nodes.len() {
            if self.nodes[i].timed_start.valid() {
                let r = self.nodes[i].timed_start;
                self.nodes[i].next_timed_run = now + r.at(w.rand());
            }
        }
    }

    /// The timed fail (each frame): a node on the running path whose `NextTimedFail` has passed fails the action.
    pub fn timed_fail_tick(&mut self, w: &mut dyn TreeWorld, now: f32) -> Result<Change, VmError> {
        if self.current.is_none() {
            return Ok(Change::default());
        }
        let due =
            self.path().iter().any(|&i| self.nodes[i].next_timed_fail != 0.0 && now >= self.nodes[i].next_timed_fail);
        if due {
            return self.action_finished(w, now, true);
        }
        Ok(Change::default())
    }

    /// The periodic state update (G26, "exact interruption rules"; the caller gates it by `UpdateRate` and holds).
    pub fn state_update(&mut self, w: &mut dyn TreeWorld, now: f32) -> Result<Change, VmError> {
        let mut change = Change::default();
        if let Some(leaf) = self.current {
            if !self.nodes[leaf].has_action {
                return Ok(change);
            }
            if w.locked(leaf)? {
                return Ok(change);
            }
            let fail = w.can_run(leaf)?;
            let stop = if fail.is_some() {
                true
            } else if self.nodes[leaf].stop_if_cannot_run {
                !self.eligible(w, now, leaf)?
            } else {
                false
            };
            self.nodes[leaf].action_fail_name = fail;
            if stop {
                self.stop_state(w, now, true)?;
                change.stopped = Some((leaf, true));
            } else {
                let timed = self.timed_fail_tick(w, now)?;
                if timed != Change::default() {
                    return Ok(timed);
                }
            }
        }
        let Some(leaf) = self.select(w, now)? else {
            return Ok(change);
        };
        if !self.nodes[leaf].has_action || self.current == Some(leaf) {
            return Ok(change);
        }
        if let Some(old) = self.current {
            self.stop_state(w, now, true)?;
            change.stopped = Some((old, true));
        }
        self.start_state(w, now, leaf)?;
        change.started = Some(leaf);
        Ok(change)
    }
}
