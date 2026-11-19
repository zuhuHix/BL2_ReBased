//! Owner: lane H-MATINEE. What could start a Matinee op on the loaded map: the Kismet graph's links read backwards
//! from each op, and the rule that an op nothing starts at load leaves its actors where they are.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-19-53-*`.
//!
//! **The graph.** Every export of the placed actors' map files whose class derives from `Engine.SequenceOp` is loaded
//! (events, actions, Matinee ops and the sequences themselves). An edge runs from an op to each op its `OutputLinks`
//! name (`Links[].LinkedOp`, read by property name). Two kinds of edge that no link holds are added:
//! - a remote event: `SeqAct_ActivateRemoteEvent` to every `SeqEvent_RemoteEvent` whose `EventName` is the action's;
//! - a subsequence: a `SeqEvent_SequenceActivated` runs when its sequence (its outer) is activated, so the sequence's
//!   own sources are its sources.
//!
//! **The start rule** ([`start_of`]). NATIVE_KISMET_MATINEE.md: an op is activated by an impulse on one of its inputs,
//! and only an activated op builds group instances and moves its actors. The op class declares no flag that plays it
//! by itself (it has `bLooping`, `bRewindOnPlay`, `bForceStartPos`, but no auto-play or play-on-load property). Walking
//! the graph backwards from the op gives every op an impulse could come from; the events among them with no source in
//! the graph are where its impulses start.
//! - A level-load event ([`LOAD_EVENTS`]) among them: [`Start::AtLoad`], naming it.
//! - Else, an event that something outside the graph fires (a mission remote event, a population event, a remote event
//!   no action in the graph names; any event class but [`GAMEPLAY_EVENTS`]): [`Start::Outside`], naming its class.
//!   Whether it fires while the map loads is a question for the system that fires it, not for the graph.
//! - Else: [`Start::NotAtLoad`]. Its impulses can only start at a touch or a console command, or nowhere (an action
//!   no link reaches never runs), so nothing starts it while the map loads and its actors stay where they are.
//!
//! UNVERIFIED in the running game: that no native or script code activates a Matinee op outside the graph (the record
//! names `SetPosition` and `Stop` as script calls, which act on an op's position, not on its start), and that no touch
//! fires while the map loads (a player spawned inside a trigger would; this world has no player pawn yet).

use ow_core::{DetMap, DetSet};
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

/// The base class of every Kismet event.
pub const SEQUENCE_EVENT_CLASS: &str = "Engine.SequenceEvent";

/// The event classes only gameplay input fires: an actor touching a trigger, a console command.
pub const GAMEPLAY_EVENTS: [&str; 2] = ["Engine.SeqEvent_Touch", "Engine.SeqEvent_Console"];

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
}

/// How a Matinee op can start (module notes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Start {
    /// No level-load event reaches it: it does not start while the map loads.
    NotAtLoad,
    /// These level-load events (class paths) reach it.
    AtLoad(Vec<String>),
    /// No level-load event reaches it, but events of these classes, fired from outside the graph, do.
    Outside(Vec<String>),
}

fn name_of(vm: &Vm, o: ObjRef, prop: &str) -> Option<String> {
    match read(&vm.space, o, prop)? {
        Value::Name(n) => Some(vm.space.names.text(n).into_owned()),
        _ => None,
    }
}

/// Reads the Kismet graph of the map files that hold the world's placed actors (module notes). Loads every op.
pub fn read_graph(vm: &mut Vm) -> Result<KismetGraph, String> {
    let mut graph = KismetGraph::default();
    let Ok(op_class) = vm.find_class(SEQUENCE_OP_CLASS) else {
        return Ok(graph);
    };
    let matinee_class = vm.find_class(MATINEE_OP_CLASS).ok();
    let load_classes: Vec<_> = LOAD_EVENTS.iter().filter_map(|c| vm.find_class(c).ok()).collect();
    let event_class = vm.find_class(SEQUENCE_EVENT_CLASS).ok();
    let gameplay_classes: Vec<_> = GAMEPLAY_EVENTS.iter().filter_map(|c| vm.find_class(c).ok()).collect();
    let packages = super::map_packages(vm)?;
    let mut is_op: DetMap<String, bool> = DetMap::new();
    for package in packages {
        let mut candidates = Vec::new();
        {
            let pkg = vm.defs.packages().get(package);
            for (i, entry) in pkg.exports.iter().enumerate() {
                if entry.class == 0 {
                    continue;
                }
                let Ok(class) = pkg.path_of(entry.class) else { continue };
                candidates.push((i32::try_from(i + 1).map_err(|e| e.to_string())?, class));
            }
        }
        for (index, class) in candidates {
            let op = match is_op.get(&class) {
                Some(&known) => known,
                None => {
                    let known = vm.find_class(&class).is_ok_and(|c| vm.space.classes.is_child_of(c, op_class));
                    is_op.insert(class, known);
                    known
                }
            };
            if !op {
                continue;
            }
            if let Some(o) = vm.defs.resolve_ref(&mut vm.space, package, index).map_err(|e| e.to_string())? {
                let class_id = vm.space.heap.get(o).map_err(|e| e.to_string())?.class;
                graph.ops.insert(o, ow_attr::eval::class_path(&vm.space, o));
                if matinee_class.is_some_and(|m| vm.space.classes.is_child_of(class_id, m)) {
                    graph.matinee.push(o);
                }
                if load_classes.iter().any(|&l| vm.space.classes.is_child_of(class_id, l)) {
                    graph.load_events.insert(o);
                }
                if event_class.is_some_and(|e| vm.space.classes.is_child_of(class_id, e)) {
                    graph.events.insert(o);
                }
                if gameplay_classes.iter().any(|&g| vm.space.classes.is_child_of(class_id, g)) {
                    graph.gameplay_events.insert(o);
                }
            }
        }
    }
    // Link edges, and the remote events by name.
    let mut remote_events: DetMap<String, Vec<ObjRef>> = DetMap::new();
    let mut remote_actions: Vec<(ObjRef, String)> = Vec::new();
    let ops: Vec<ObjRef> = graph.ops.keys().copied().collect();
    for &o in &ops {
        let class = graph.ops[&o].clone();
        if class.ends_with(".SeqEvent_RemoteEvent")
            && let Some(name) = name_of(vm, o, "EventName")
        {
            remote_events.entry(name).or_default().push(o);
        }
        if class.ends_with(".SeqAct_ActivateRemoteEvent")
            && let Some(name) = name_of(vm, o, "EventName")
        {
            remote_actions.push((o, name));
        }
        let Some(Value::Array(outputs)) = read(&vm.space, o, "OutputLinks") else { continue };
        for output in outputs.iter() {
            let Some(links) = field(&vm.space, output, "Links") else { continue };
            let Value::Array(links) = links else { continue };
            for link in links.iter() {
                if let Some(Value::Object(Some(target))) = field(&vm.space, link, "LinkedOp") {
                    graph.sources.entry(*target).or_default().insert(o);
                }
            }
        }
        // A subsequence's activation event runs when its sequence does.
        if class.ends_with(".SeqEvent_SequenceActivated")
            && let Some(outer) = vm.space.heap.get(o).map_err(|e| e.to_string())?.outer
        {
            graph.sources.entry(o).or_default().insert(outer);
        }
    }
    for (action, name) in remote_actions {
        for &event in remote_events.get(&name).map(Vec::as_slice).unwrap_or(&[]) {
            graph.sources.entry(event).or_default().insert(action);
        }
    }
    Ok(graph)
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
    let outside = classes(&|o| {
        graph.events.contains(&o)
            && !graph.gameplay_events.contains(&o)
            && graph.sources.get(&o).is_none_or(DetSet::is_empty)
    });
    if !outside.is_empty() {
        return Start::Outside(outside);
    }
    Start::NotAtLoad
}
