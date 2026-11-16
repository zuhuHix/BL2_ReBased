//! Tests of the action tree on invented trees (AGENTS.md rule 9). AI-assisted (Claude Code).

use ow_core::{DetMap, Missing};
use ow_vm::error::VmError;

use super::*;

/// An invented AI: scripted draws, `CanRun` answers, evaluator answers, and a log of starts and stops.
#[derive(Default)]
struct Toy {
    draws: Vec<f32>,
    fails: DetMap<usize, String>,
    gates: DetMap<(usize, Gate), bool>,
    locked: Option<usize>,
    log: Vec<String>,
}

impl TreeWorld for Toy {
    fn rand(&mut self) -> f32 {
        if self.draws.is_empty() { 0.5 } else { self.draws.remove(0) }
    }
    fn evaluate(&mut self, node: usize, gate: Gate) -> Result<bool, VmError> {
        Ok(self.gates.get(&(node, gate)).copied().unwrap_or(true))
    }
    fn can_run(&mut self, node: usize) -> Result<Option<String>, VmError> {
        Ok(self.fails.get(&node).cloned())
    }
    fn start(&mut self, node: usize) -> Result<(), VmError> {
        self.log.push(format!("start {node}"));
        Ok(())
    }
    fn stop(&mut self, node: usize, failed: bool) -> Result<(), VmError> {
        self.log.push(format!("stop {node}{}", if failed { " failed" } else { "" }));
        Ok(())
    }
    fn locked(&mut self, node: usize) -> Result<bool, VmError> {
        Ok(self.locked == Some(node))
    }
}

impl PartialOrd for Gate {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Gate {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let key = |g: &Gate| match g {
            Gate::CanRunIf => 0,
            Gate::Condition(i) => i + 1,
        };
        key(self).cmp(&key(other))
    }
}

fn priority(children: Vec<usize>, interrupt: bool) -> Node {
    let mut n = Node::new(NodeKind::Priority, children);
    n.conditions = vec![false; n.children.len()];
    n.interrupt = interrupt;
    n
}

fn random(children: Vec<usize>, weights: Vec<f32>) -> Node {
    let mut n = Node::new(NodeKind::Random, children);
    n.weights = weights;
    n.loop_count_max = vec![0; n.weights.len()];
    n
}

fn action() -> Node {
    Node::new(NodeKind::Action, vec![])
}

/// The shape of a bullymong's tree, with invented values:
/// 0 root priority (interrupt) -> 1 scripted move, 2 combat priority (interrupt), 9 patrol;
/// 2 -> 3 perch (TimedStart 10..10, TimedReset 20..30), 4 throw, 5 random (6 melee 1.0, 7 sequence 0.5), 8 back up;
/// 7 sequence -> 10 roar, 11 leap.
fn beast() -> Tree {
    let mut nodes = vec![
        priority(vec![1, 2, 9], true),
        action(),
        priority(vec![3, 4, 5, 8], true),
        action(),
        action(),
        random(vec![6, 7], vec![1.0, 0.5]),
        action(),
        Node::new(NodeKind::Sequence, vec![10, 11]),
        action(),
        action(),
        action(),
        action(),
    ];
    nodes[3].timed_start = Range::new(10.0, 10.0);
    nodes[3].timed_reset = Range::new(20.0, 30.0);
    Tree::new(nodes).unwrap()
}

/// Without a target every attack fails its `CanRun`: the scripted move fails too, so patrol runs.
fn no_target(t: &mut Toy) {
    for n in [1, 3, 4, 6, 8, 10, 11] {
        t.fails.insert(n, "NoTarget".to_string());
    }
}

#[test]
fn a_priority_node_takes_the_first_eligible_child_in_order() {
    let mut tree = beast();
    let mut w = Toy::default();
    no_target(&mut w);
    assert_eq!(tree.select(&mut w, 1.0).unwrap(), Some(9), "patrol");
    assert_eq!(tree.nodes[4].action_fail_name.as_deref(), Some("NoTarget"));
    // The target appears: the scripted move still fails, the perch is first among the attacks.
    w.fails.clear();
    w.fails.insert(1, "NotScripted".to_string());
    assert_eq!(tree.select(&mut w, 2.0).unwrap(), Some(3));
    // A condition of the root's second child that does not hold skips it.
    tree.nodes[0].conditions[1] = true;
    w.gates.insert((0, Gate::Condition(1)), false);
    assert_eq!(tree.select(&mut w, 2.0).unwrap(), Some(9));
    // A CanRunIf that does not hold.
    w.gates.clear();
    tree.nodes[3].has_can_run_if = true;
    w.gates.insert((3, Gate::CanRunIf), false);
    assert_eq!(tree.select(&mut w, 2.0).unwrap(), Some(4));
}

#[test]
fn the_timed_start_holds_a_node_back_and_the_reset_is_its_cooldown() {
    let mut tree = beast();
    let mut w = Toy::default();
    w.fails.insert(1, "NotScripted".to_string());
    tree.rearm_timed_starts(&mut w, 5.0);
    assert_eq!(tree.nodes[3].next_timed_run, 15.0, "10 to 10");
    assert_eq!(tree.select(&mut w, 14.9).unwrap(), Some(4), "the perch waits");
    let leaf = tree.select(&mut w, 15.0).unwrap().unwrap();
    assert_eq!(leaf, 3);
    tree.start_state(&mut w, 15.0, leaf).unwrap();
    assert_eq!(tree.path(), vec![0, 2, 3]);
    // The perch finishes: a draw of 0.25 gives 20 + 10 x 0.25 = 22.5 s of cooldown; the throw is next.
    w.draws = vec![0.25];
    let change = tree.finish(&mut w, 16.0, 3, false).unwrap();
    assert_eq!(tree.nodes[3].next_timed_run, 38.5);
    assert_eq!(change.started, Some(4));
    assert_eq!(w.log, ["start 3", "stop 3", "start 4"]);
}

#[test]
fn a_random_node_draws_by_weight_and_counts_its_loops() {
    let mut tree = beast();
    let mut w = Toy::default();
    for n in [1, 3, 4] {
        w.fails.insert(n, "NotReachable".to_string());
    }
    // Weights 1.0 : 0.5, sum 1.5. A draw of 0.6 is 0.9: under 1.0, the first child (melee).
    w.draws = vec![0.6];
    assert_eq!(tree.select(&mut w, 1.0).unwrap(), Some(6));
    tree.start_state(&mut w, 1.0, 6).unwrap();
    tree.finish(&mut w, 2.0, 6, false).unwrap();
    // A draw of 0.7 is 1.05: the sequence, whose first child is the roar.
    assert_eq!((tree.nodes[5].loop_count, tree.nodes[5].last_random_child), (1, 0));
    let mut tree = beast();
    for n in [1, 3, 4] {
        w.fails.insert(n, "NotReachable".to_string());
    }
    w.draws = vec![0.7];
    assert_eq!(tree.select(&mut w, 1.0).unwrap(), Some(10));
    // The drawn child selects nothing: it leaves the list and the next draw takes another.
    let mut tree = beast();
    w.fails.insert(10, "NoTarget".to_string());
    w.draws = vec![0.7, 0.0];
    assert_eq!(tree.select(&mut w, 1.0).unwrap(), Some(6));
}

#[test]
fn the_same_random_child_again_counts_up_and_a_reached_maximum_excludes_it() {
    let mut tree = beast();
    let mut w = Toy::default();
    for n in [1, 3, 4] {
        w.fails.insert(n, "NotReachable".to_string());
    }
    tree.nodes[5].loop_count_max = vec![2, 0];
    for round in 1..=2 {
        w.draws = vec![0.0];
        let leaf = tree.select(&mut w, 1.0).unwrap().unwrap();
        assert_eq!(leaf, 6, "round {round}");
        tree.start_state(&mut w, 1.0, leaf).unwrap();
        tree.stop_state(&mut w, 1.0, false).unwrap();
        assert_eq!((tree.nodes[5].loop_count, tree.nodes[5].last_random_child), (round, 0));
    }
    // Melee ran twice in a row (its maximum is 2): it is out of the draw, so the sequence runs whatever the draw.
    w.draws = vec![0.0];
    let leaf = tree.select(&mut w, 9.0).unwrap();
    assert_eq!(leaf, Some(10));
}

#[test]
fn a_sequence_runs_its_children_in_turn() {
    let mut tree = beast();
    let mut w = Toy::default();
    for n in [1, 3, 4] {
        w.fails.insert(n, "NotReachable".to_string());
    }
    w.draws = vec![0.9];
    let leaf = tree.select(&mut w, 1.0).unwrap().unwrap();
    assert_eq!(leaf, 10, "the roar");
    tree.start_state(&mut w, 1.0, leaf).unwrap();
    // The roar finishes: the sequence advances and is not done; the random node keeps it as its active child.
    let change = tree.finish(&mut w, 2.0, 10, false).unwrap();
    assert_eq!(tree.nodes[7].next_sequential_child, 1);
    assert_eq!(change.started, Some(11), "the leap, without a new draw");
    // The leap finishes: the sequence is done and starts over; the random node counts the child.
    w.draws = vec![0.0];
    tree.finish(&mut w, 3.0, 11, false).unwrap();
    assert_eq!(tree.nodes[7].next_sequential_child, 0);
    assert_eq!((tree.nodes[5].loop_count, tree.nodes[5].last_random_child), (1, 1));
}

#[test]
fn a_failed_finish_blocks_the_node_for_one_second() {
    let mut tree = beast();
    let mut w = Toy::default();
    for n in [1, 3, 4] {
        w.fails.insert(n, "NotReachable".to_string());
    }
    w.draws = vec![0.0];
    let leaf = tree.select(&mut w, 1.0).unwrap().unwrap();
    tree.start_state(&mut w, 1.0, leaf).unwrap();
    assert_eq!(tree.current, Some(6));
    w.draws = vec![0.0, 0.0];
    let change = tree.finish(&mut w, 2.0, 6, true).unwrap();
    assert_eq!(tree.nodes[6].last_fail_time, 2.0);
    assert_eq!(change.stopped, Some((6, true)));
    assert_eq!(change.started, Some(10), "melee is blocked: the sequence");
    assert!(!tree.eligible(&mut w, 2.99, 6).unwrap());
    assert!(tree.eligible(&mut w, 3.0, 6).unwrap());
    // Finish on a node that is not the running state does nothing.
    assert_eq!(tree.finish(&mut w, 3.0, 4, true).unwrap(), Change::default());
}

#[test]
fn the_state_update_keeps_switches_and_stops() {
    let mut tree = beast();
    let mut w = Toy::default();
    no_target(&mut w);
    let change = tree.state_update(&mut w, 1.0).unwrap();
    assert_eq!(change.started, Some(9), "patrol");
    assert_eq!(tree.state_update(&mut w, 1.2).unwrap(), Change::default(), "patrol again: no change");
    // A target: the scripted move fails, the throw is free; the root is interruptible, so the throw takes over.
    w.fails.clear();
    w.fails.insert(1, "NotScripted".to_string());
    w.fails.insert(3, "NoPathToPerch".to_string());
    let change = tree.state_update(&mut w, 1.4).unwrap();
    assert_eq!((change.stopped, change.started), (Some((9, true)), Some(4)));
    // Locked (running and non-interruptible): nothing changes even when it could no longer run.
    w.locked = Some(4);
    w.fails.insert(4, "NotReachable".to_string());
    assert_eq!(tree.state_update(&mut w, 1.6).unwrap(), Change::default());
    // Unlocked: its CanRun fails, it stops as failed and the next leaf starts.
    w.locked = None;
    w.draws = vec![0.0];
    let change = tree.state_update(&mut w, 1.8).unwrap();
    assert_eq!((change.stopped, change.started), (Some((4, true)), Some(6)));
}

#[test]
fn a_node_that_must_stop_when_it_cannot_run() {
    let mut tree = beast();
    let mut w = Toy::default();
    w.fails.insert(1, "NotScripted".to_string());
    w.fails.insert(3, "NoPathToPerch".to_string());
    tree.nodes[4].stop_if_cannot_run = true;
    tree.nodes[4].has_can_run_if = true;
    tree.state_update(&mut w, 1.0).unwrap();
    assert_eq!(tree.current, Some(4));
    // Its CanRunIf turns false: with bStopIfCannotRun it is checked again while running, and stops.
    w.gates.insert((4, Gate::CanRunIf), false);
    w.draws = vec![0.0];
    let change = tree.state_update(&mut w, 1.2).unwrap();
    assert_eq!((change.stopped, change.started), (Some((4, true)), Some(6)));
}

#[test]
fn the_timed_fail_ends_the_action() {
    let mut tree = beast();
    let mut w = Toy::default();
    no_target(&mut w);
    tree.nodes[9].timed_fail = Range::new(4.0, 6.0);
    w.draws = vec![0.5];
    tree.state_update(&mut w, 1.0).unwrap();
    assert_eq!(tree.nodes[9].next_timed_fail, 6.0);
    assert_eq!(tree.timed_fail_tick(&mut w, 5.9).unwrap(), Change::default());
    let change = tree.timed_fail_tick(&mut w, 6.0).unwrap();
    assert_eq!(change.stopped, Some((9, true)));
}

#[test]
fn what_the_tree_cannot_answer() {
    let mut tree = beast();
    let mut w = Toy::default();
    w.fails.insert(1, "NotScripted".to_string());
    tree.nodes[3].has_resource = true;
    let e = tree.select(&mut w, 1.0).unwrap_err();
    assert!(matches!(e, VmError::NotYet(n) if n.missing == Missing::NotRead));
    assert!(Tree::new(vec![priority(vec![5], true)]).is_err(), "a child outside the tree");
    let mut looped = Tree::new(vec![priority(vec![0], true)]).unwrap();
    assert!(looped.select(&mut w, 1.0).is_err(), "a loop");
    // A priority node whose conditions do not match its children, or a random node whose weights do not: nothing.
    let mut odd = Tree::new(vec![Node::new(NodeKind::Priority, vec![1]), action()]).unwrap();
    assert_eq!(odd.select(&mut w, 1.0).unwrap(), None);
}
