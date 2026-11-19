//! Owner: lane H-MATINEE. What could start a Matinee op on the loaded map: the Kismet graph's links read backwards
//! from each op, and what each op's start leaves the world to answer.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-19-53-*`.
//!
//! **The graph** ([`read_graph`]). Every export of the loaded levels' map files (the loader's level packages, streaming
//! levels with no placed actor included) whose class derives from `Engine.SequenceOp` is loaded: events, actions,
//! Matinee ops and the sequences themselves. An edge runs from an op to each op its `OutputLinks` name
//! (`Links[].LinkedOp`, read by property name), except, in `ParityMode::Off`, from a sequence (`Engine.Sequence` or a
//! subclass): a sequence's outputs fire only when a finish op inside it does, so its exits are added from those finish
//! ops alone (delta review of #436, finding 3). Three kinds of edge that no link holds are added:
//! - a remote event: every `SeqAct_ActivateRemoteEvent` (or subclass) to every `SeqEvent_RemoteEvent` (or subclass)
//!   whose `EventName` is the action's;
//! - into a subsequence: a `SeqEvent_SequenceActivated` runs when its sequence (its outer) is activated, and that is
//!   the only way an impulse into a sequence goes on inside it;
//! - out of a subsequence: a `SeqAct_FinishSequence` fires its sequence's output link whose `LinkDesc` is its
//!   `OutputLabel`, so each op that output names has the finish op as a source.
//!
//! **What makes the graph incomplete** ([`KismetGraph::incomplete`], by reason): an export class that does not resolve
//! (outside `super::NOT_OP_CLASSES`) or whose reference cannot be read, an op whose `OutputLinks`, a link's `Links` or a
//! link's `LinkedOp` is missing or of another shape (a decoded null `LinkedOp` is a link to nothing, not a fault), a
//! remote-event action whose `EventName` does not read, in Off a remote event whose `EventName` does not read (delta
//! review of #436, finding 4), and a
//! finish op whose output cannot be resolved. While the graph is incomplete, every op's start is
//! [`Start::Incomplete`].
//!
//! **The start rule** ([`start_of`]). NATIVE_KISMET_MATINEE.md: an op is activated by an impulse on one of its inputs;
//! the op class declares no flag that plays it by itself (it has `bLooping`, `bRewindOnPlay`, `bForceStartPos`, but no
//! auto-play or play-on-load property). Walking the graph backwards from the op gives every op an impulse could come
//! from. Among those:
//! - a level-load event ([`LOAD_EVENTS`]): [`Start::AtLoad`];
//! - else an event that something outside the graph can fire, whatever graph sources it also has: any event but a
//!   level-load event, a gameplay event ([`GAMEPLAY_EVENTS`]) or a subsequence's activation event. Mission and
//!   population events, a remote event (a script can fire it by name): [`Start::Outside`], naming the classes;
//! - else [`Start::StartupUnknown`]: through the graph only a touch, a console command or nothing reaches it, an op
//!   no link reaches included.
//!
//! **No op is answered "not started at load"** (delta review of #436, finding 1). The graph cannot rule out two startup
//! paths outside it: a script can call the op's `SetPosition`, which builds group instances and applies a position
//! with no impulse (the record), and an actor spawned overlapping a trigger can fire its touch while the map starts.
//! Proving neither happens needs the scripts' and the spawn's side, which this module does not read, so the start is
//! unknown and an actor that only such ops bind answers `NotYet`, naming both (`super::physics_rule`). A proven
//! exclusion would be a new variant with its proof.
//!
//! **Reference keeps the graph as it was before the delta review of #436** (its review r3, finding 1): there a
//! sequence's outputs are edges from the sequence itself, and a remote event whose name does not read is skipped. The
//! two delta rules are Off's, so an op Reference answers `AtLoad` (layer absent) through a sequence it enters stays so.

use ow_core::{DetMap, DetSet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::Vm;

use super::{field, read};

pub mod track;

/// The base class of every Kismet op.
pub const SEQUENCE_OP_CLASS: &str = "Engine.SequenceOp";
/// A Matinee op.
pub const MATINEE_OP_CLASS: &str = "Engine.SeqAct_Interp";

/// The event classes that fire while a level loads or starts (stock UE3: "Level Loaded" and "Level Startup"). An
/// event of a class derived from one of these counts too.
pub const LOAD_EVENTS: [&str; 2] = ["Engine.SeqEvent_LevelLoaded", "Engine.SeqEvent_LevelStartup"];

/// A Kismet sequence (an op holding ops).
pub const SEQUENCE_CLASS: &str = "Engine.Sequence";

/// The base class of every Kismet event.
pub const SEQUENCE_EVENT_CLASS: &str = "Engine.SequenceEvent";

/// The event classes only gameplay input fires: an actor touching a trigger, a console command.
pub const GAMEPLAY_EVENTS: [&str; 2] = ["Engine.SeqEvent_Touch", "Engine.SeqEvent_Console"];

/// The touch event (a startup path when an actor spawns overlapping its trigger).
pub const TOUCH_EVENT: &str = "Engine.SeqEvent_Touch";
/// A subsequence's activation event: fired only when its sequence is activated, through the graph.
pub const SEQUENCE_ACTIVATED_EVENT: &str = "Engine.SeqEvent_SequenceActivated";
/// A remote event and the action that fires it by name.
pub const REMOTE_EVENT: &str = "Engine.SeqEvent_RemoteEvent";
pub const REMOTE_ACTION: &str = "Engine.SeqAct_ActivateRemoteEvent";
/// The action that ends a subsequence through one of its sequence's outputs.
pub const FINISH_SEQUENCE: &str = "Engine.SeqAct_FinishSequence";

/// The Kismet graph of the loaded map, read backwards.
#[derive(Debug, Clone, Default)]
pub struct KismetGraph {
    /// Every op, with its class path.
    pub ops: DetMap<ObjRef, String>,
    /// For each op, the ops an impulse into it can come from (links, remote events, the outer sequence).
    pub sources: DetMap<ObjRef, DetSet<ObjRef>>,
    /// The Matinee ops.
    pub matinee: Vec<ObjRef>,
    /// The ops that are level-load events.
    pub load_events: DetSet<ObjRef>,
    /// The ops that are events, and those of them only gameplay input fires.
    pub events: DetSet<ObjRef>,
    pub gameplay_events: DetSet<ObjRef>,
    /// The touch events, and the subsequence activation events (fired only through the graph).
    pub touch_events: DetSet<ObjRef>,
    pub activation_events: DetSet<ObjRef>,
    /// Why the graph may miss an edge or an op, with counts (module notes); empty when it is complete.
    pub incomplete: DetMap<String, usize>,
}

/// How a Matinee op can start (module notes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Start {
    /// Through the graph only a touch, a console command or nothing reaches it (`touch` when a touch does), and the
    /// startup paths outside the graph are not ruled out (module notes): its start is not known.
    StartupUnknown { touch: bool },
    /// The graph is incomplete (the reasons, joined): its start is not known.
    Incomplete(String),
    /// These level-load events (class paths) reach it.
    AtLoad(Vec<String>),
    /// No level-load event reaches it, but events of these classes, which something outside the graph can fire, do.
    Outside(Vec<String>),
}

fn text_of(vm: &Vm, v: &Value) -> Option<String> {
    match v {
        Value::Name(n) => Some(vm.space.names.text(*n).into_owned()),
        Value::Str(s) => Some(s.to_string()),
        _ => None,
    }
}

/// Reads the Kismet graph of the map files `packages` (module notes). Loads every op.
pub fn read_graph(vm: &mut Vm, packages: &DetSet<ow_package::PackageId>) -> Result<KismetGraph, String> {
    let mut graph = KismetGraph::default();
    // The delta review's two graph rules are Off's (module notes).
    let off = vm.parity == ParityMode::Off;
    let Ok(op_class) = vm.find_class(SEQUENCE_OP_CLASS) else {
        return Ok(graph);
    };
    let class = |vm: &mut Vm, path: &str| vm.find_class(path).ok();
    let matinee_class = class(vm, MATINEE_OP_CLASS);
    let load_classes: Vec<_> = LOAD_EVENTS.iter().filter_map(|c| class(vm, c)).collect();
    let event_class = class(vm, SEQUENCE_EVENT_CLASS);
    let gameplay_classes: Vec<_> = GAMEPLAY_EVENTS.iter().filter_map(|c| class(vm, c)).collect();
    let (touch_class, activated_class) = (class(vm, TOUCH_EVENT), class(vm, SEQUENCE_ACTIVATED_EVENT));
    let (remote_event_class, remote_action_class) = (class(vm, REMOTE_EVENT), class(vm, REMOTE_ACTION));
    let finish_class = class(vm, FINISH_SEQUENCE);
    let sequence_class = class(vm, SEQUENCE_CLASS);
    let note = |graph: &mut KismetGraph, why: String| *graph.incomplete.entry(why).or_insert(0) += 1;
    let mut is_op: DetMap<String, Option<bool>> = DetMap::new();
    let (mut remote_events, mut remote_actions, mut finishes) = (Vec::new(), Vec::new(), Vec::new());
    let mut sequences = DetSet::new();
    for &package in packages {
        let mut candidates = Vec::new();
        {
            let pkg = vm.defs.packages().get(package);
            for (i, entry) in pkg.exports.iter().enumerate() {
                if entry.class == 0 {
                    continue;
                }
                let index = i32::try_from(i + 1).map_err(|e| e.to_string())?;
                match pkg.path_of(entry.class) {
                    Ok(class) => candidates.push((index, class)),
                    Err(e) => note(&mut graph, format!("an export class reference that cannot be read ({e})")),
                }
            }
        }
        for (index, class_path) in candidates {
            let op = match is_op.get(&class_path) {
                Some(&known) => known,
                None => {
                    let known = vm.find_class(&class_path).ok().map(|c| vm.space.classes.is_child_of(c, op_class));
                    is_op.insert(class_path.clone(), known);
                    known
                }
            };
            match op {
                Some(true) => {}
                Some(false) => continue,
                None if super::NOT_OP_CLASSES.contains(&class_path.as_str()) => continue,
                None => {
                    note(&mut graph, format!("the export class {class_path} does not resolve"));
                    continue;
                }
            }
            let Some(o) = vm.defs.resolve_ref(&mut vm.space, package, index).map_err(|e| e.to_string())? else {
                continue;
            };
            let class_id = vm.space.heap.get(o).map_err(|e| e.to_string())?.class;
            let is = |c: Option<ow_object::ids::ClassId>| c.is_some_and(|c| vm.space.classes.is_child_of(class_id, c));
            graph.ops.insert(o, ow_attr::eval::class_path(&vm.space, o));
            if is(matinee_class) {
                graph.matinee.push(o);
            }
            if load_classes.iter().any(|&l| is(Some(l))) {
                graph.load_events.insert(o);
            }
            if is(event_class) {
                graph.events.insert(o);
            }
            if gameplay_classes.iter().any(|&g| is(Some(g))) {
                graph.gameplay_events.insert(o);
            }
            if is(touch_class) {
                graph.touch_events.insert(o);
            }
            if is(activated_class) {
                graph.activation_events.insert(o);
            }
            if is(remote_event_class) {
                remote_events.push(o);
            }
            if is(remote_action_class) {
                remote_actions.push(o);
            }
            if is(finish_class) {
                finishes.push(o);
            }
            if is(sequence_class) {
                sequences.insert(o);
            }
        }
    }
    let ops: Vec<ObjRef> = graph.ops.keys().copied().collect();
    for &o in &ops {
        let class = graph.ops[&o].clone();
        let targets = output_targets(vm, o, &mut |why| note(&mut graph, format!("{class}: {why}")))?;
        // A sequence's outputs fire through its finish ops only (below): an impulse into it is not one out of it.
        if !(off && sequences.contains(&o)) {
            for (target, _) in targets {
                graph.sources.entry(target).or_default().insert(o);
            }
        }
        // A subsequence's activation event runs when its sequence does.
        if graph.activation_events.contains(&o)
            && let Some(outer) = vm.space.heap.get(o).map_err(|e| e.to_string())?.outer
        {
            graph.sources.entry(o).or_default().insert(outer);
        }
    }
    // Remote events by name, both sides by inheritance; a name that does not read on either side is a fault.
    let named = |vm: &Vm, o: ObjRef| read(&vm.space, o, "EventName").and_then(|v| text_of(vm, &v));
    let mut events_named: Vec<(ObjRef, String)> = Vec::new();
    for &event in &remote_events {
        match named(vm, event) {
            Some(name) => events_named.push((event, name)),
            None if off => note(&mut graph, "a remote event with no EventName".into()),
            None => {}
        }
    }
    for &action in &remote_actions {
        let Some(name) = named(vm, action) else {
            note(&mut graph, "a remote-event action with no EventName".into());
            continue;
        };
        for (event, event_name) in &events_named {
            if *event_name == name {
                graph.sources.entry(*event).or_default().insert(action);
            }
        }
    }
    // Out of a subsequence: the finish op's sequence output whose description is its label.
    for &finish in &finishes {
        let label = read(&vm.space, finish, "OutputLabel").and_then(|v| text_of(vm, &v));
        let sequence = vm.space.heap.get(finish).map_err(|e| e.to_string())?.outer;
        let (Some(label), Some(sequence)) = (label, sequence) else {
            note(&mut graph, "a finish op with no OutputLabel or no sequence".into());
            continue;
        };
        // The sequence is an op too: its faults were noted in the pass above.
        let targets: Vec<ObjRef> = output_targets(vm, sequence, &mut |_| {})?
            .into_iter()
            .filter(|(_, desc)| desc.as_deref() == Some(label.as_str()))
            .map(|(t, _)| t)
            .collect();
        let exposed = output_descs(vm, sequence).iter().any(|d| d.as_deref() == Some(label.as_str()));
        if !exposed {
            note(&mut graph, format!("a finish op whose label {label} names no output of its sequence"));
            continue;
        }
        for target in targets {
            graph.sources.entry(target).or_default().insert(finish);
        }
    }
    Ok(graph)
}

/// The `LinkDesc` of each of `op`'s outputs (`None` where it does not read as text).
fn output_descs(vm: &Vm, op: ObjRef) -> Vec<Option<String>> {
    match read(&vm.space, op, "OutputLinks") {
        Some(Value::Array(outputs)) => {
            outputs.iter().map(|o| field(&vm.space, o, "LinkDesc").and_then(|v| text_of(vm, v))).collect()
        }
        _ => Vec::new(),
    }
}

/// The ops `op`'s outputs link to, each with its output's `LinkDesc`. A missing or misshapen `OutputLinks`, `Links` or
/// `LinkedOp` is reported to `fault` (a decoded null `LinkedOp` is not a fault: it links to nothing).
fn output_targets(
    vm: &Vm,
    op: ObjRef,
    fault: &mut dyn FnMut(String),
) -> Result<Vec<(ObjRef, Option<String>)>, String> {
    let mut out = Vec::new();
    let outputs = match read(&vm.space, op, "OutputLinks") {
        Some(Value::Array(outputs)) => outputs,
        Some(other) => {
            fault(format!("OutputLinks is {}, not an array", other.kind_name()));
            return Ok(out);
        }
        None => {
            fault("no OutputLinks".into());
            return Ok(out);
        }
    };
    for output in outputs.iter() {
        let desc = field(&vm.space, output, "LinkDesc").and_then(|v| text_of(vm, v));
        let links = match field(&vm.space, output, "Links") {
            Some(Value::Array(links)) => links,
            _ => {
                fault("an output's Links is missing or not an array".into());
                continue;
            }
        };
        for link in links.iter() {
            match field(&vm.space, link, "LinkedOp") {
                Some(Value::Object(Some(target))) => out.push((*target, desc.clone())),
                Some(Value::Object(None)) => {}
                _ => fault("a link's LinkedOp is missing or not an object".into()),
            }
        }
    }
    Ok(out)
}

/// Every op an impulse into `op` can come from, directly or not.
pub fn upstream(graph: &KismetGraph, op: ObjRef) -> DetSet<ObjRef> {
    let mut seen = DetSet::new();
    let mut todo = vec![op];
    while let Some(o) = todo.pop() {
        for &s in graph.sources.get(&o).into_iter().flatten() {
            if seen.insert(s) {
                todo.push(s);
            }
        }
    }
    seen
}

/// How `op` can start (module notes).
pub fn start_of(graph: &KismetGraph, op: ObjRef) -> Start {
    if !graph.incomplete.is_empty() {
        let mut reasons: Vec<String> = graph.incomplete.iter().map(|(why, n)| format!("{why} (x{n})")).collect();
        reasons.sort();
        return Start::Incomplete(reasons.join("; "));
    }
    let up = upstream(graph, op);
    let classes = |pick: &dyn Fn(ObjRef) -> bool| {
        let mut out: Vec<String> =
            up.iter().copied().filter(|&o| pick(o)).map(|o| graph.ops.get(&o).cloned().unwrap_or_default()).collect();
        out.sort();
        out.dedup();
        out
    };
    let load = classes(&|o| graph.load_events.contains(&o));
    if !load.is_empty() {
        return Start::AtLoad(load);
    }
    // Eligible to be fired from outside the graph whatever sources it has in it (review r1 of #436).
    let outside = classes(&|o| {
        graph.events.contains(&o) && !graph.gameplay_events.contains(&o) && !graph.activation_events.contains(&o)
    });
    if !outside.is_empty() {
        return Start::Outside(outside);
    }
    Start::StartupUnknown { touch: up.iter().any(|o| graph.touch_events.contains(o)) }
}
