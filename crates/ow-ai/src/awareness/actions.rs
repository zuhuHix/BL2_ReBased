//! Owner: lane W7-AI (part 2). The action tree over the VM: the tree read from the AI definition, one action object
//! per action node with its script events run through the VM, the brain's tick in the recorded order, and the
//! script-facing natives of the actions. AI-assisted (Claude Code). Decision 2026-11-16-66.
//!
//! **The brain** ([`tick_brain`], one AI tick of a mind; NATIVE_AI_MELEE.md "AIComponent: cadence", L8 section 3.1):
//! 1. the first tick builds the tree and the action objects and raises `Init` on each. The mind's pawn, component
//!    and `AIDef` are checked first, on every tick until the build succeeds, and their errors are not kept (a mind
//!    ticked before possession, or before its `AIDef` is bound, builds once they are there). A build that fails
//!    after that is kept with the pawn and `AIDef` it was made for, and its error answered on later ticks without
//!    copying the archetypes or raising `Init` again, until either changes;
//! 2. part 1's tick ([`super::vm::tick_mind`]): the target update when due;
//! 3. on hold: the running action is stopped;
//! 4. the target events: from no target to one, the tree's timed starts are re-armed; `Target_Lost` and
//!    `Target_Acquired` go to the running action first, then every other action in node order (L8 section 3.4);
//! 5. the running action's `Update` (no arguments: the event declares none), then the tree's timed fail;
//! 6. when the state update is due: [`Tree::state_update`].
//!
//! `ActionSequence.Finish` is called from inside an action's own script, while the tree is in use: the native queues
//! the call and the brain applies it ([`Tree::finish`]) after the script calls of a step return: after the events of
//! step 4 (so an action finished by an event handler gets no `Update`), after `Update`, and after the state update.
//! The note reads the re-selection as happening inside `Finish`; here it happens right after the script returns
//! (UNVERIFIED either way). One application runs at most four `Finish` calls per tree node, then stops with an error
//! naming the action: a `Start` that calls `Finish` at once, with no `TimedReset`, would be selected again for ever
//! (a Rust-only bound).
//!
//! **Stopping.** After `Stop`, the engine puts the action back in its default state; that call is not read
//! (NATIVE_AI_AWARENESS.md, "Not read yet"). The rest state here is the action's state once built and initialised:
//! an action that `Stop` left there needs nothing more, and one left in another state is `NotYet`, with the running
//! state left as it was (the selection pass of a state update, or a failed `Finish`'s `LastFailTime`, may have run
//! before the stop).
//!
//! **Building the actions** (G26, "the AI initialisation hook ... creates one action object per action node, raises
//! `Init` on each"). Each action node's `Action` archetype is copied; on the copy `MyWillowPawn` is the pawn (live, L8:
//! "each `Action_AnimAttack` carries its pawn in `MyWillowPawn`") and `MyWillowMind` the mind (its own `Init` script
//! reads it, so the creator sets it; UNVERIFIED). The other context fields (`AI`, `MyGearboxMind`, `MyGearboxPawn`,
//! `WillowAI`) and the flags (`bInitialized`, `bDoWorkFinished`) are set by native code that is not read
//! (NATIVE_AI_ACTION_SEQUENCE.md, "Not read"): they stay at the archetype's values. The tree's run state starts as a
//! reset tree (every `ActiveChild` -1); what the first-run reset writes is not read.
//!
//! **What the actions reach** ([`Pack`]):
//! - `ActionSequence.Finish` (G26) and `TriggerBehavior` (G25: the event for the pawn's behavior consumer; sent through
//!   the behavior kernel's consumer routing, `BehaviorKernel.BroadcastBehaviorEventFromScript` with the pawn's
//!   handle, decision 2026-11-16-50);
//! - the AI component's holds (G26: `Hold` / `Release` / `OnHold` on `Holds`, and the same on `TargetHolds`).
//!   `WillowAIComponent` declares its own native `Hold`, `Release` and `OnHold` (other native keys, checked in the
//!   installed packages). In Off they are `NotYet { NotRead }`: the note's second reading (decision 2026-11-19-43)
//!   found they replace the base kind, and the replacement is not read. Under Reference they keep the base rule, as
//!   before (Willow's `Hold` and `Release` also send the pawn a head-look change, `SetLookAtTarget`, presentation that
//!   is not sent; Willow's `OnHold` is also true in one case reached through the pawn that is not decoded, G26:
//!   **UNVERIFIED**). Its other kinds are `AIComponent`'s own (`ow_combat::holds`);
//! - movement, through the host ([`Locomotion`]): path creation on the navigation handle, path validity, arrival,
//!   `CanMoveTo`, the movement speed and the nearest nav-mesh point. The latent ones (`FollowPath`, `WaitForPath`,
//!   `FinishPath`, `WaitForPawnToTurn`, `WaitForPawnToStop`) answer `NotYet`: the VM's latent actions are `Sleep`
//!   only, and a host-driven latent action is an `ow-vm` seam. Path creation, as the installed packages declare it:
//!   `GearboxNavigationHandle.CreatePathToLocation(out PathData, DestLocation, optional DestPoly, optional
//!   bAdjustStartForMotion)` and `WillowNavigationHandle.CreateConstrainedPath(out PathData, DestLocation,
//!   Constraints)`, both returning whether a path was made; the destination is argument 1 of each.
//!   `WillowActionSequencePawn.CreateVectorPath` (and `CreateActorPath`, `CreateBackupFlyerPath`) are script that
//!   reach `CreatePathToLocation`, so they run in the VM and need no binding. `PathData` is not written: the path is
//!   the host's, and what the engine stores in that struct is not read;
//! - `StartAtomic`, `StopAtomic`, `IsAtomic`, `IsRunning`, `IsActionReadyToFinish`, `RangeIsValid` and
//!   `GetRangeValue`, read by W7-AI part 2 as analyst (NATIVE_AI_ACTION_SEQUENCE.md) and implemented from that note
//!   alone by lane W7-AI-LOCK (decision 2026-11-16-72) in `ParityMode::Off`; under Reference they stay unbound (the
//!   stub policy: there is no C++ AI). See [`LOCK`] for their rules and the tree's "locked" question.

use std::cell::RefCell;
use std::rc::Rc;

use ow_combat::hit::access::{array, field, get, number, object, set, set_field, truth};
use ow_combat::hit::call_method;
use ow_core::rand::{CrtStream, Rand15};
use ow_core::{DetMap, NotYet, ParityMode};
use ow_object::ids::{NameId, ObjRef};
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, NativePack, Vm};
use ow_vm::error::VmError;

use super::targets::Effect;
use super::tick::TickReport;
use super::tree::{Change, Gate, Node, NodeKind, Range, Tree, TreeWorld};
use super::vm::{Host, state, tick_mind};
use super::{DECISION, RECORD_AWARENESS, RECORD_MELEE, Vec3};

#[cfg(test)]
mod tests;

/// The note of the natives this lane read as analyst.
pub const RECORD_ACTIONS: &str = "docs/verification/NATIVE_AI_ACTION_SEQUENCE.md";

/// Lane W7-AI-LOCK (decision 2026-11-16-72), the fresh implementer of NATIVE_AI_ACTION_SEQUENCE.md: the action-sequence
/// natives, in `ParityMode::Off` (Reference keeps the stub policy), and the tree's "locked" question, in both modes.
/// Every rule is the note's, UNVERIFIED in the running game:
///
/// - **`StartAtomic` / `StopAtomic` / `IsAtomic`:** on an action whose `bInitialized` is set, start and stop set and clear
///   `bInAtomicSection` (no nesting, no count), and `IsAtomic` is true when both flags are set. On an action whose
///   `bInitialized` is not set, the note's answer (nothing changes; false) is not given: who sets `bInitialized` when the
///   AI creates its actions is not read (the note's "Not read"), and this port never sets it, so an action the game
///   would have initialised cannot be told apart. That case is `NotYet { NotRead }`.
/// - **`IsRunning`:** requires the action's `AI` interface and that AI's component/current-state link. This port
///   does not represent that setup: owned actions answer `NotYet { NotRead }`, even if a caller assigns `AI`.
///   An action no brain made answers false when its `AI` is unset (the note's first condition) and `NotYet`
///   otherwise. A missing brains service is not evidence that an action is unowned (decision 2026-11-17-05).
/// - **`IsActionReadyToFinish`:** true when no latent action is pending in the action's state code and, up its
///   `ParentSequence` chain, each parent answers true. An action of `WillowGame.WillowActionSequencePawn` (or a
///   subclass), whose override the note did not read, is `NotYet { NotRead }`.
/// - **`RangeIsValid` / `GetRangeValue`** on the `Range` struct (`MinVal`, `MaxVal`): valid when both ends are above zero
///   and the minimum is not above the maximum; the value is 0 for an invalid range (no draw), else `min + (max - min) *
///   r / 32767` in single precision, `r` one 15-bit draw of the AI stream (`CrtStream::AiDecision`; which C `rand()`
///   stream the engine's call shares is UNVERIFIED).
/// - **The tree's "locked" question** (the periodic update leaves a running action alone when "both its running and its
///   non-interruptible flag" are set; the note reads the non-interruptible flag as `bInAtomicSection` and leaves which
///   flag is "running" open): with `bInAtomicSection` clear the action is not locked, whatever the other flag; with it
///   set, `NotYet { NotRead }` (the question stands in decision 72). In both modes: there is no C++ reading.
pub const LOCK: &str = "W7-AI-LOCK, decision 2026-11-16-72";

/// The class whose override of `IsActionReadyToFinish` was not read (the note).
pub const ACTION_SEQUENCE_PAWN: &str = "WillowGame.WillowActionSequencePawn";

/// The deepest `ParentSequence` chain followed (a Rust-only bound against a cycle in data).
const PARENT_DEPTH: usize = 32;

/// What a mind's tree is running, as the natives see it ([`LOCK`], `IsRunning`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Running {
    /// The action, its `Start` or `Stop` event in progress.
    Changing(ObjRef),
    Action(ObjRef),
}

/// What the host answers about movement (module documentation). The handle is the mind's navigation handle object;
/// the pawn is the action's pawn.
pub trait Locomotion {
    /// A path from the handle's pawn to `dest` (`CreatePathToLocation`, `CreateConstrainedPath`; module documentation):
    /// whether one was made. The natives' `out PathData` is left as the caller passed it.
    fn create_path(&mut self, vm: &mut Vm, handle: ObjRef, dest: Vec3) -> Result<bool, VmError>;
    /// `ClearPath(bStopMovement, ...)`.
    fn clear_path(&mut self, vm: &mut Vm, handle: ObjRef, stop: bool) -> Result<bool, VmError>;
    fn path_is_valid(&mut self, vm: &mut Vm, pawn: ObjRef) -> Result<bool, VmError>;
    fn invalidate_path(&mut self, vm: &mut Vm, pawn: ObjRef) -> Result<(), VmError>;
    fn reached_destination(&mut self, vm: &mut Vm, pawn: ObjRef) -> Result<bool, VmError>;
    fn can_move_to(&mut self, vm: &mut Vm, pawn: ObjRef, goal: Vec3) -> Result<bool, VmError>;
    /// `SetPawnMovementSpeed(stance)`, the stance byte as given.
    fn set_movement_speed(&mut self, vm: &mut Vm, pawn: ObjRef, speed: u8) -> Result<(), VmError>;
    /// The nearest point on the nav mesh to the pawn, if any.
    fn nearest_on_navmesh(&mut self, vm: &mut Vm, pawn: ObjRef) -> Result<Option<Vec3>, VmError>;
}

/// One mind's tree and its action objects.
#[non_exhaustive]
#[derive(Debug, Clone)]
pub struct Brain {
    pub tree: Tree,
    /// Per node: the action object (action nodes only).
    pub actions: Vec<Option<ObjRef>>,
    can_run_if: Vec<Option<ObjRef>>,
    conditions: Vec<Vec<Option<ObjRef>>>,
    /// Per node: the action object's state once built and initialised, the state `Stop` must leave it in (module
    /// documentation, "Stopping").
    rest_states: Vec<Option<NameId>>,
    pub pawn: ObjRef,
    pub mind: ObjRef,
}

/// The brains of every mind, the queued `Finish` calls, and the movement host (a VM service).
#[derive(Default)]
pub struct Brains {
    brains: DetMap<ObjRef, Brain>,
    /// `(action, bFailed)`, in call order.
    finishes: Vec<(ObjRef, bool)>,
    /// Minds whose brain failed to build, with the pawn and `AIDef` it was built for and the error: answered again
    /// without building again while both are the same.
    failed_builds: DetMap<ObjRef, (ObjRef, ObjRef, VmError)>,
    locomotion: Option<Rc<RefCell<dyn Locomotion>>>,
    /// Action object -> the mind whose brain made it.
    owners: DetMap<ObjRef, ObjRef>,
    /// Mind -> what its tree runs (absent: nothing).
    running: DetMap<ObjRef, Running>,
}

impl std::fmt::Debug for Brains {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Brains")
            .field("brains", &self.brains.keys().collect::<Vec<_>>())
            .field("finishes", &self.finishes)
            .field("failed_builds", &self.failed_builds)
            .field("locomotion", &self.locomotion.is_some())
            .field("owners", &self.owners)
            .field("running", &self.running)
            .finish()
    }
}

fn brains(vm: &mut Vm) -> Result<Rc<RefCell<Brains>>, VmError> {
    match vm.service::<Brains>() {
        Some(b) => Ok(b),
        None => vm.add_service(Brains::default()),
    }
}

fn borrow(b: &Rc<RefCell<Brains>>) -> Result<std::cell::RefMut<'_, Brains>, VmError> {
    b.try_borrow_mut().map_err(|_| VmError::ServiceBorrowed("ow_ai::awareness::actions::Brains"))
}

/// Sets the movement host the natives ask.
pub fn install_locomotion(vm: &mut Vm, host: Rc<RefCell<dyn Locomotion>>) -> Result<(), VmError> {
    let b = brains(vm)?;
    borrow(&b)?.locomotion = Some(host);
    Ok(())
}

/// `mind`'s brain, if built (a copy).
pub fn brain(vm: &Vm, mind: ObjRef) -> Option<Brain> {
    vm.service::<Brains>()?.try_borrow().ok()?.brains.get(&mind).cloned()
}

fn error(message: String) -> VmError {
    VmError::Script { function: "ai actions".to_string(), message }
}

fn object_of(v: Option<Value>) -> Option<ObjRef> {
    match v {
        Some(Value::Object(o)) => o,
        _ => None,
    }
}

fn range(vm: &Vm, node: &Value, name: &str) -> Range {
    let Some(r) = field(vm, node, name) else {
        return Range::default();
    };
    let at = |n: &str| field(vm, &r, n).map_or(0.0, |v| number(&v) as f32);
    Range::new(at("MinVal"), at("MaxVal"))
}

fn ints(v: Option<Value>) -> Vec<i64> {
    v.and_then(|v| v.elements().map(<[Value]>::to_vec))
        .unwrap_or_default()
        .iter()
        .map(ow_combat::hit::access::integer)
        .collect()
}

/// The tree of `mind`'s component, read from its `AIDef.NodeList` (`NotYet` while `AIDef` is unbound, as part 1).
pub fn read_tree(vm: &mut Vm, mind: ObjRef) -> Result<Brain, VmError> {
    let (pawn, aidef) = setup(vm, mind)?;
    read_tree_of(vm, mind, pawn, aidef)
}

/// What a brain is built from: the mind's pawn and its component's `AIDef`. These can stop being missing (a mind
/// before possession, an `AIDef` bound later), so [`tick_brain`] asks again each tick and keeps none of their errors.
fn setup(vm: &mut Vm, mind: ObjRef) -> Result<(ObjRef, ObjRef), VmError> {
    let pawn = object(vm, mind, "Pawn")?.ok_or_else(|| error("the mind has no Pawn".to_string()))?;
    let component = object(vm, mind, "AIComponent")?.ok_or_else(|| error("the mind has no AIComponent".to_string()))?;
    let aidef = object(vm, component, "AIDef")?.ok_or_else(|| -> VmError {
        NotYet::not_read("AIComponent.AIDef unset: the native that binds it when the AI starts is not read", DECISION)
            .into()
    })?;
    Ok((pawn, aidef))
}

fn read_tree_of(vm: &mut Vm, mind: ObjRef, pawn: ObjRef, aidef: ObjRef) -> Result<Brain, VmError> {
    let list = array(vm, aidef, "NodeList")?;
    let mut nodes = Vec::new();
    let (mut actions, mut can_run_if, mut conditions) = (Vec::new(), Vec::new(), Vec::new());
    for (i, data) in list.iter().enumerate() {
        let kind_byte = field(vm, data, "Type").map_or(0, |v| ow_combat::hit::access::integer(&v));
        let kind = u8::try_from(kind_byte)
            .ok()
            .and_then(NodeKind::of)
            .ok_or_else(|| error(format!("node {i} has the node type {kind_byte}")))?;
        let mut children = Vec::new();
        for c in ints(field(vm, data, "Children")) {
            children.push(usize::try_from(c).map_err(|_| error(format!("node {i} names child {c}")))?);
        }
        let mut n = Node::new(kind, children);
        n.has_resource = object_of(field(vm, data, "Resource")).is_some();
        let gate = object_of(field(vm, data, "CanRunIf"));
        n.has_can_run_if = gate.is_some();
        let conds: Vec<Option<ObjRef>> = field(vm, data, "Conditions")
            .and_then(|v| v.elements().map(<[Value]>::to_vec))
            .unwrap_or_default()
            .into_iter()
            .map(|v| object_of(Some(v)))
            .collect();
        n.conditions = conds.iter().map(Option::is_some).collect();
        n.timed_start = range(vm, data, "TimedStart");
        n.timed_reset = range(vm, data, "TimedReset");
        n.timed_fail = range(vm, data, "TimedFail");
        n.stop_if_cannot_run = field(vm, data, "bStopIfCannotRun").is_some_and(|v| truth(vm, &v));
        n.interrupt = field(vm, data, "bInterrupt").is_some_and(|v| truth(vm, &v));
        n.weights = field(vm, data, "Weights")
            .and_then(|v| v.elements().map(<[Value]>::to_vec))
            .unwrap_or_default()
            .iter()
            .map(|v| number(v) as f32)
            .collect();
        n.loop_count_max =
            ints(field(vm, data, "LoopCountMax")).into_iter().map(|x| i32::try_from(x).unwrap_or(i32::MAX)).collect();
        let archetype = object_of(field(vm, data, "Action"));
        n.has_action = archetype.is_some();
        let action = match archetype {
            Some(a) if kind == NodeKind::Action => {
                let made = ow_combat::hit::copy_of(vm, a)?;
                set(vm, made, "MyWillowPawn", Value::Object(Some(pawn)))?;
                set(vm, made, "MyWillowMind", Value::Object(Some(mind)))?;
                Some(made)
            }
            _ => None,
        };
        nodes.push(n);
        actions.push(action);
        can_run_if.push(gate);
        conditions.push(conds);
    }
    let rest_states = vec![None; actions.len()];
    Ok(Brain { tree: Tree::new(nodes)?, actions, can_run_if, conditions, rest_states, pawn, mind })
}

/// The tree's world over the VM ([`TreeWorld`]).
struct VmTree<'a> {
    vm: &'a mut Vm,
    brain: &'a Brain,
}

impl VmTree<'_> {
    fn action(&self, node: usize) -> Result<ObjRef, VmError> {
        self.brain
            .actions
            .get(node)
            .copied()
            .flatten()
            .ok_or_else(|| error(format!("node {node} has no action object")))
    }

    fn event(&mut self, node: usize, name: &str, args: Vec<Value>) -> Result<Option<Value>, VmError> {
        let action = self.action(node)?;
        event(self.vm, action, name, args).map(Some)
    }
}

impl TreeWorld for VmTree<'_> {
    fn rand(&mut self) -> f32 {
        // The engine's integer random divided by 32768 (G26).
        f32::from(self.vm.rng.crt(CrtStream::AiDecision).rand15()) / 32768.0
    }

    fn evaluate(&mut self, node: usize, gate: Gate) -> Result<bool, VmError> {
        let evaluator = match gate {
            Gate::CanRunIf => self.brain.can_run_if.get(node).copied().flatten(),
            Gate::Condition(i) => self.brain.conditions.get(node).and_then(|c| c.get(i)).copied().flatten(),
        };
        let Some(evaluator) = evaluator else {
            return Ok(true);
        };
        let evaluator = ow_combat::hit::access::loaded(self.vm, evaluator)?;
        match call_method(self.vm, evaluator, "Evaluate", vec![Value::Object(Some(self.brain.pawn))])? {
            Some(v) => Ok(truth(self.vm, &v)),
            None => Err(NotYet::not_read(
                format!("the evaluator {} has no Evaluate", ow_combat::hit::access::class_name(self.vm, evaluator)),
                RECORD_MELEE,
            )
            .into()),
        }
    }

    fn can_run(&mut self, node: usize) -> Result<Option<String>, VmError> {
        let answer = self.event(node, "CanRun", vec![])?;
        Ok(match answer {
            Some(Value::Name(n)) => {
                let text = self.vm.space.names.text(n).into_owned();
                (!text.eq_ignore_ascii_case("None")).then_some(text)
            }
            _ => None,
        })
    }

    fn start(&mut self, node: usize) -> Result<(), VmError> {
        let action = self.action(node)?;
        set_running(self.vm, self.brain.mind, Some(Running::Changing(action)))?;
        let result = self.event(node, "Start", vec![]).map(|_| ());
        set_running(self.vm, self.brain.mind, Some(Running::Action(action)))?;
        result
    }

    fn stop(&mut self, node: usize, _failed: bool) -> Result<(), VmError> {
        let action = self.action(node)?;
        let service = brains(self.vm)?;
        set_running(self.vm, self.brain.mind, Some(Running::Changing(action)))?;
        let result = (|| {
            self.event(node, "Stop", vec![])?;
            // The engine's "back to the default state" call after Stop is not read (NATIVE_AI_AWARENESS.md, "Not read
            // yet"): an action that Stop left in its rest state needs nothing more; any other is NotYet.
            let now_in = self.vm.space.heap.get(action)?.state;
            let rest = self.brain.rest_states.get(node).copied().flatten();
            if now_in != rest {
                let text =
                    |vm: &Vm, n: Option<NameId>| n.map_or("none".to_string(), |n| vm.space.names.text(n).into_owned());
                return Err(NotYet::not_read(
                    format!(
                        "the action default-state call at state stop: {} is in state {} after Stop, not {}",
                        ow_combat::hit::access::class_name(self.vm, action),
                        text(self.vm, now_in),
                        text(self.vm, rest)
                    ),
                    RECORD_AWARENESS,
                )
                .into());
            }
            Ok(())
        })();
        // No service borrow crosses a VM call. Restore only adapter bookkeeping, never script side effects.
        let running = result.as_ref().err().map(|_| Running::Action(action));
        match borrow(&service) {
            Ok(mut s) => {
                if let Some(running) = running {
                    s.running.insert(self.brain.mind, running);
                } else {
                    s.running.remove(&self.brain.mind);
                }
            }
            Err(e) => return result.and(Err(e)),
        }
        result
    }

    /// [`LOCK`]: not locked while `bInAtomicSection` is clear; `NotYet` while it is set.
    fn locked(&mut self, node: usize) -> Result<bool, VmError> {
        let action = self.action(node)?;
        match get(self.vm, action, "bInAtomicSection")? {
            Some(v) if !truth(self.vm, &v) => Ok(false),
            Some(_) => Err(NotYet::not_read(
                format!(
                    "whether the running action {} is locked: it is in an atomic section (the non-interruptible flag, \
                     the note's reading), and which flag is the update's \"running\" flag is not read",
                    ow_combat::hit::access::class_name(self.vm, action)
                ),
                RECORD_ACTIONS,
            )
            .into()),
            None => Err(NotYet::not_read(
                format!(
                    "whether the running action {} is locked: its class declares no bInAtomicSection",
                    ow_combat::hit::access::class_name(self.vm, action)
                ),
                RECORD_ACTIONS,
            )
            .into()),
        }
    }
}

/// What one brain tick did.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BrainReport {
    /// Part 1's tick.
    pub awareness: TickReport,
    /// The `Target_*` events sent, with the action nodes that got them in order.
    pub events: Vec<(String, Vec<usize>)>,
    /// Starts and stops, in order.
    pub changes: Vec<Change>,
    /// Whether the brain was built in this tick.
    pub built: bool,
}

/// What [`dry_select`] found: the leaf picked, if any, and each action node's `CanRun` failure name by node index.
pub type DrySelection = (Option<usize>, Vec<(usize, String)>);

/// The leaf the selection pass picks for `mind` now, and each action node's `CanRun` failure name, without starting
/// anything (for a report: the state update selects only once the running action is not locked, [`LOCK`]). The
/// selection's own writes (the next active children, the failure names) stay, as a selection pass leaves them.
pub fn dry_select(vm: &mut Vm, mind: ObjRef) -> Result<DrySelection, VmError> {
    let Some(mut brain) = take(vm, mind)? else {
        return Err(error("dry_select: the mind's brain is not built".to_string()));
    };
    let now = vm.clock.time_seconds();
    let snapshot = brain.clone();
    let picked = brain.tree.select(&mut VmTree { vm, brain: &snapshot }, now);
    let fails =
        brain.tree.nodes.iter().enumerate().filter_map(|(i, n)| n.action_fail_name.clone().map(|f| (i, f))).collect();
    put(vm, mind, brain)?;
    Ok((picked?, fails))
}

/// Raises the script event `name` on `action` with `args`. An action class without the event is an error (every
/// `ActionSequence` declares its events), and so is an event that declares other parameters than `args` fills: no
/// parameter is left to default silently. In the installed packages `Init`, `Start`, `Stop`, `Update` and `CanRun`
/// take none (`ActionSequence` and every override; checked with `research/script_disasm.py`) and `ActivateEvent` takes
/// the event name. Script that needs the frame time reads `WorldInfo.DeltaSeconds`, the world's to set, not this
/// adapter's. Nothing writes it yet (no map load), so that script reads the class default: the frame time action
/// script sees is UNVERIFIED until the world sets it.
fn event(vm: &mut Vm, action: ObjRef, name: &str, args: Vec<Value>) -> Result<Value, VmError> {
    let class = vm.space.heap.get(action)?.class;
    let state = vm.space.heap.get(action)?.state;
    let Some(f) = vm.find_method(class, name, state)? else {
        return Err(error(format!("the action {} has no {name}", ow_combat::hit::access::class_name(vm, action))));
    };
    let declared = vm.function(f).map_or(0, |d| d.params.len());
    if declared != args.len() {
        return Err(error(format!(
            "the action {}: {name} declares {declared} parameters and the adapter passes {}",
            ow_combat::hit::access::class_name(vm, action),
            args.len()
        )));
    }
    Ok(vm.call(f, Some(action), args)?.ret)
}

fn take(vm: &mut Vm, mind: ObjRef) -> Result<Option<Brain>, VmError> {
    let b = brains(vm)?;
    Ok(borrow(&b)?.brains.remove(&mind))
}

fn put(vm: &mut Vm, mind: ObjRef, brain: Brain) -> Result<(), VmError> {
    let b = brains(vm)?;
    borrow(&b)?.brains.insert(mind, brain);
    Ok(())
}

/// How many queued `Finish` calls one application may run per node of the tree (a Rust-only bound: an action whose
/// `Start` calls `Finish` at once, with no `TimedReset`, is selected and finishes again for ever).
const FINISHES_PER_NODE: usize = 4;

/// Applies the queued `Finish` calls of `brain`'s actions, at most [`FINISHES_PER_NODE`] per node of the tree; past
/// that, an error naming the action, and the queue of `brain`'s actions is dropped.
fn apply_finishes(vm: &mut Vm, brain: &mut Brain, now: f32, changes: &mut Vec<Change>) -> Result<(), VmError> {
    let limit = brain.tree.nodes.len().saturating_mul(FINISHES_PER_NODE).max(FINISHES_PER_NODE);
    let mut applied = 0usize;
    loop {
        let next = {
            let b = brains(vm)?;
            let mut b = borrow(&b)?;
            let at = b.finishes.iter().position(|(a, _)| brain.actions.contains(&Some(*a)));
            at.map(|i| b.finishes.remove(i))
        };
        let Some((action, failed)) = next else {
            return Ok(());
        };
        let Some(node) = brain.actions.iter().position(|a| *a == Some(action)) else {
            continue;
        };
        applied += 1;
        if applied > limit {
            let b = brains(vm)?;
            borrow(&b)?.finishes.retain(|(a, _)| !brain.actions.contains(&Some(*a)));
            return Err(error(format!(
                "more than {limit} Finish calls of this brain's actions in one tick, the last from {} (node {node}): an \
                 action whose script calls Finish at once and that the tree selects again (no TimedReset) would never \
                 settle",
                ow_combat::hit::access::class_name(vm, action)
            )));
        }
        let snapshot = brain.clone();
        let change = brain.tree.finish(&mut VmTree { vm, brain: &snapshot }, now, node, failed)?;
        changes.push(change);
    }
}

/// One AI tick of `mind` at the VM's world time (module documentation).
pub fn tick_brain(vm: &mut Vm, host: &mut dyn Host, mind: ObjRef, globals: ObjRef) -> Result<BrainReport, VmError> {
    let mut report = BrainReport::default();
    let now = vm.clock.time_seconds();
    let mut brain = match take(vm, mind)? {
        Some(b) => b,
        None => {
            // Not kept: these can stop being true (module documentation, step 1).
            let (pawn, aidef) = setup(vm, mind)?;
            let known = borrow(&brains(vm)?)?
                .failed_builds
                .get(&mind)
                .filter(|(p, d, _)| (*p, *d) == (pawn, aidef))
                .map(|(_, _, e)| e.clone());
            if let Some(e) = known {
                return Err(e);
            }
            match build(vm, mind, pawn, aidef) {
                Ok(b) => {
                    borrow(&brains(vm)?)?.failed_builds.remove(&mind);
                    report.built = true;
                    b
                }
                Err(e) => {
                    borrow(&brains(vm)?)?.failed_builds.insert(mind, (pawn, aidef, e.clone()));
                    return Err(e);
                }
            }
        }
    };
    let result = tick_with(vm, host, mind, globals, now, &mut brain, &mut report);
    put(vm, mind, brain)?;
    result.map(|()| report)
}

/// The brain of `mind` for `pawn` and `aidef`: the tree read, `Init` raised on each action, each action's rest state
/// noted. A failure is kept by [`tick_brain`] with the pawn and `AIDef`, so the archetypes are not copied again each
/// tick.
fn build(vm: &mut Vm, mind: ObjRef, pawn: ObjRef, aidef: ObjRef) -> Result<Brain, VmError> {
    let mut b = read_tree_of(vm, mind, pawn, aidef)?;
    {
        let service = brains(vm)?;
        let mut s = borrow(&service)?;
        // Init may query this action or another created action. Register all of them first;
        // release the service borrow before invoking any script callback.
        for action in b.actions.iter().flatten() {
            s.owners.insert(*action, mind);
        }
    }
    for action in b.actions.iter().flatten() {
        event(vm, *action, "Init", vec![])?;
    }
    for (i, action) in b.actions.iter().enumerate() {
        if let Some(a) = action {
            b.rest_states[i] = vm.space.heap.get(*a)?.state;
        }
    }
    Ok(b)
}

/// What `mind`'s tree runs, as the natives see it ([`LOCK`]); `None` for nothing.
fn set_running(vm: &mut Vm, mind: ObjRef, what: Option<Running>) -> Result<(), VmError> {
    let service = brains(vm)?;
    let mut s = borrow(&service)?;
    match what {
        Some(r) => s.running.insert(mind, r),
        None => s.running.remove(&mind),
    };
    Ok(())
}

fn tick_with(
    vm: &mut Vm,
    host: &mut dyn Host,
    mind: ObjRef,
    globals: ObjRef,
    now: f32,
    brain: &mut Brain,
    report: &mut BrainReport,
) -> Result<(), VmError> {
    report.awareness = tick_mind(vm, host, mind, globals)?;
    let on_hold = state(vm, mind).is_some_and(|s| s.on_hold);
    if on_hold {
        let snapshot = brain.clone();
        if let Some(leaf) = brain.tree.stop_state(&mut VmTree { vm, brain: &snapshot }, now, false)? {
            report.changes.push(Change { stopped: Some((leaf, false)), ..Change::default() });
        }
        return Ok(());
    }
    for effect in report.awareness.effects.clone() {
        let (name, target) = match effect {
            Effect::TimedStartsReArmed => {
                let snapshot = brain.clone();
                brain.tree.rearm_timed_starts(&mut VmTree { vm, brain: &snapshot }, now);
                continue;
            }
            Effect::TargetLost(t) => ("Target_Lost", t),
            Effect::TargetAcquired(t) => ("Target_Acquired", t),
            _ => continue,
        };
        let _ = target;
        // The running action first, then every other action in node order (L8).
        let mut order: Vec<usize> = brain.tree.current.into_iter().collect();
        order.extend((0..brain.actions.len()).filter(|&i| brain.actions[i].is_some() && Some(i) != brain.tree.current));
        let name_value = Value::Name(vm.space.names.intern(name));
        for &node in &order {
            if let Some(action) = brain.actions[node] {
                event(vm, action, "ActivateEvent", vec![name_value.clone()])?;
            }
        }
        report.events.push((name.to_string(), order));
    }
    // A Finish called from an event handler takes effect before the running action's Update (review r1 L6).
    apply_finishes(vm, brain, now, &mut report.changes)?;
    if let Some(leaf) = brain.tree.current
        && let Some(action) = brain.actions[leaf]
    {
        event(vm, action, "Update", vec![])?;
    }
    apply_finishes(vm, brain, now, &mut report.changes)?;
    let snapshot = brain.clone();
    let timed = brain.tree.timed_fail_tick(&mut VmTree { vm, brain: &snapshot }, now)?;
    if timed != Change::default() {
        report.changes.push(timed);
    }
    if report.awareness.state_update {
        let snapshot = brain.clone();
        let change = brain.tree.state_update(&mut VmTree { vm, brain: &snapshot }, now)?;
        if change != Change::default() {
            report.changes.push(change);
        }
        apply_finishes(vm, brain, now, &mut report.changes)?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------------------------
// The natives
// ---------------------------------------------------------------------------------------------------------------

pub const FINISH: &str = "GearboxFramework.ActionSequence.Finish";
pub const TRIGGER_BEHAVIOR: &str = "GearboxFramework.ActionSequence.TriggerBehavior";
pub const READ_AS_ANALYST: [&str; 7] = [
    "GearboxFramework.ActionSequence.StartAtomic",
    "GearboxFramework.ActionSequence.StopAtomic",
    "GearboxFramework.ActionSequence.IsAtomic",
    "GearboxFramework.ActionSequence.IsRunning",
    "GearboxFramework.ActionSequence.IsActionReadyToFinish",
    "GearboxFramework.ActionSequence.RangeIsValid",
    "GearboxFramework.ActionSequence.GetRangeValue",
];
pub const LATENT_MOVES: [&str; 5] = [
    "WillowGame.WillowActionSequencePawn.FollowPath",
    "WillowGame.WillowActionSequencePawn.WaitForPath",
    "WillowGame.WillowActionSequencePawn.FinishPath",
    "WillowGame.WillowActionSequencePawn.WaitForPawnToTurn",
    "WillowGame.WillowActionSequencePawn.WaitForPawnToStop",
];

/// `WillowAIComponent`'s replacements of the base hold kind (`NotYet { NotRead }` in Off; decision 2026-11-19-43).
pub const WILLOW_BASE_KIND: [&str; 3] = [
    "WillowGame.WillowAIComponent.Hold",
    "WillowGame.WillowAIComponent.Release",
    "WillowGame.WillowAIComponent.OnHold",
];

fn willow_base_kind(path: &'static str) -> NativeFn {
    Rc::new(move |_c: &mut NativeCall<'_>| -> Result<Value, VmError> {
        Err(NotYet::not_read(
            format!(
                "{path}: WillowAIComponent replaces the base hold kind, and the replacement is not read \
                 (NATIVE_AI_HOLDS.md)"
            ),
            "docs/verification/NATIVE_AI_HOLDS.md",
        )
        .into())
    })
}

/// Binds the actions' natives the VM declares (module documentation, "What the actions reach").
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

fn bind(vm: &mut Vm, path: &str, f: NativeFn) -> Result<(), VmError> {
    let Ok(id) = vm.find_function(path) else {
        return Ok(());
    };
    let Some(key) = vm.function(id).and_then(|d| d.native_key.clone()) else {
        return Ok(());
    };
    if !vm.has_native(&key) {
        vm.register_native(&key, f)?;
    }
    Ok(())
}

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        brains(vm)?;
        bind(vm, FINISH, Rc::new(n_finish))?;
        register_activate_event(vm)?;
        bind(vm, TRIGGER_BEHAVIOR, Rc::new(n_trigger_behavior))?;
        // The Willow component's own Hold, Release and OnHold (other native keys): in Off `NotYet { NotRead }`, because
        // the note's second reading (NATIVE_AI_HOLDS.md, read 2026-11-19; decision 2026-11-19-43) found that
        // `WillowAIComponent` replaces the base kind and the replacement was not read. Reference keeps the base rule
        // these had before (module documentation; Willow's OnHold extra case is not decoded: UNVERIFIED).
        if vm.parity == ParityMode::Off {
            for path in WILLOW_BASE_KIND {
                bind(vm, path, willow_base_kind(path))?;
            }
        }
        for (list, path) in [
            ("Holds", "GearboxFramework.AIComponent.Hold"),
            ("TargetHolds", "GearboxFramework.AIComponent.HoldTarget"),
            ("Holds", WILLOW_BASE_KIND[0]),
        ] {
            bind(vm, path, hold_body(list, true))?;
        }
        for (list, path) in [
            ("Holds", "GearboxFramework.AIComponent.Release"),
            ("TargetHolds", "GearboxFramework.AIComponent.ReleaseTarget"),
            ("Holds", WILLOW_BASE_KIND[1]),
        ] {
            bind(vm, path, hold_body(list, false))?;
        }
        for (list, path) in [
            ("Holds", "GearboxFramework.AIComponent.OnHold"),
            ("TargetHolds", "GearboxFramework.AIComponent.TargetOnHold"),
            ("Holds", WILLOW_BASE_KIND[2]),
        ] {
            bind(
                vm,
                path,
                Rc::new(move |c: &mut NativeCall<'_>| {
                    let me = this(c)?;
                    Ok(Value::Bool(!array(c.vm, me, list)?.is_empty()))
                }),
            )?;
        }
        bind_locomotion(vm)?;
        // W7-AI part 3 (decision 2026-11-19-16): the waits, target records and mind queries of NATIVE_AI_MOVEMENT.md,
        // Off only; the latent moves it does not bind keep the NotYet below.
        super::movement::register(vm, bind)?;
        for path in LATENT_MOVES {
            bind(
                vm,
                path,
                Rc::new(move |_c: &mut NativeCall<'_>| -> Result<Value, VmError> {
                    Err(NotYet::layer_absent(
                        format!("{path}: a latent movement action"),
                        "ow-vm (latent actions other than Sleep; a host-driven latent action is a seam)",
                        RECORD_AWARENESS,
                    )
                    .into())
                }),
            )?;
        }
        // The action-sequence natives ([`LOCK`]): Off only; under Reference they stay unbound (the stub policy).
        if vm.parity == ParityMode::Off {
            let bodies: [(&str, NativeFn); 7] = [
                (READ_AS_ANALYST[0], Rc::new(|c| n_atomic(c, Some(true)))),
                (READ_AS_ANALYST[1], Rc::new(|c| n_atomic(c, Some(false)))),
                (READ_AS_ANALYST[2], Rc::new(|c| n_atomic(c, None))),
                (READ_AS_ANALYST[3], Rc::new(n_is_running)),
                (READ_AS_ANALYST[4], Rc::new(n_ready_to_finish)),
                (READ_AS_ANALYST[5], Rc::new(|c| Ok(Value::Bool(arg_range(c)?.valid())))),
                (READ_AS_ANALYST[6], Rc::new(n_range_value)),
            ];
            for (path, body) in bodies {
                bind(vm, path, body)?;
            }
        }
        Ok(())
    }
}

/// `bInitialized` of an action, `NotYet` when it is not set ([`LOCK`]: who sets it is not read).
fn initialized(c: &mut NativeCall<'_>, action: ObjRef, what: &str) -> Result<(), VmError> {
    let set = get(c.vm, action, "bInitialized")?.is_some_and(|v| truth(c.vm, &v));
    if set {
        return Ok(());
    }
    Err(NotYet::not_read(
        format!(
            "{what} on {}, whose bInitialized is not set: who sets it when the AI creates its actions is not read \
             (this port never sets it)",
            ow_combat::hit::access::class_name(c.vm, action)
        ),
        RECORD_ACTIONS,
    )
    .into())
}

// ActionSequence.StartAtomic (Some(true)), StopAtomic (Some(false)), IsAtomic (None) ([`LOCK`]).
fn n_atomic(c: &mut NativeCall<'_>, write: Option<bool>) -> Result<Value, VmError> {
    let action = this(c)?;
    let what = match write {
        Some(true) => "StartAtomic",
        Some(false) => "StopAtomic",
        None => "IsAtomic",
    };
    initialized(c, action, what)?;
    match write {
        Some(on) => {
            if !set(c.vm, action, "bInAtomicSection", Value::Bool(on))? {
                return Err(error(format!("{what}: the action's class has no bInAtomicSection")));
            }
            Ok(Value::None)
        }
        None => Ok(Value::Bool(get(c.vm, action, "bInAtomicSection")?.is_some_and(|v| truth(c.vm, &v)))),
    }
}

// ActionSequence.IsRunning ([`LOCK`]).
fn n_is_running(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let action = this(c)?;
    let service = c.vm.service::<Brains>().ok_or_else(|| -> VmError {
        NotYet::not_read("IsRunning: no brains service represents the action's ownership or AI setup", RECORD_ACTIONS)
            .into()
    })?;
    let owner = {
        let s = borrow(&service)?;
        s.owners.get(&action).copied()
    };
    if owner.is_none() {
        let ai = match get(c.vm, action, "AI")? {
            Some(Value::Interface(o)) | Some(Value::Object(o)) => o,
            _ => None,
        };
        if ai.is_none() {
            return Ok(Value::Bool(false));
        }
        return Err(NotYet::not_read(
            "IsRunning on an action this port's brains did not make, whose AI is set (its tree is not known here)",
            RECORD_ACTIONS,
        )
        .into());
    }
    Err(NotYet::not_read(
        "IsRunning on an owned action: its AI interface/component/current-state setup is not represented; \
         Rust tree ownership does not establish that setup",
        RECORD_ACTIONS,
    )
    .into())
}

// ActionSequence.IsActionReadyToFinish ([`LOCK`]).
fn n_ready_to_finish(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let action = this(c)?;
    ready_to_finish(c.vm, action, &mut ow_vm::states::latent_pending).map(Value::Bool)
}

/// [`LOCK`]'s `IsActionReadyToFinish` rule up the `ParentSequence` chain, with `pending` the "latent action pending in
/// its state frame" question (a parameter so the tests can answer it without running state code).
fn ready_to_finish(
    vm: &mut Vm,
    action: ObjRef,
    pending: &mut dyn FnMut(&mut Vm, ObjRef) -> Result<bool, VmError>,
) -> Result<bool, VmError> {
    let mut at = action;
    let mut seen = Vec::new();
    loop {
        if ow_combat::hit::access::is_a(vm, at, ACTION_SEQUENCE_PAWN)? {
            return Err(NotYet::not_read(
                format!(
                    "IsActionReadyToFinish on {}: the {ACTION_SEQUENCE_PAWN} override is not read",
                    ow_combat::hit::access::class_name(vm, at)
                ),
                RECORD_ACTIONS,
            )
            .into());
        }
        if pending(vm, at)? {
            return Ok(false);
        }
        seen.push(at);
        match object(vm, at, "ParentSequence")? {
            None => return Ok(true),
            Some(parent) => {
                if seen.contains(&parent) || seen.len() >= PARENT_DEPTH {
                    return Err(error(format!(
                        "IsActionReadyToFinish: the ParentSequence chain loops or is deeper than {PARENT_DEPTH}"
                    )));
                }
                at = parent;
            }
        }
    }
}

/// The `Range` argument (`MinVal`, `MaxVal`).
fn arg_range(c: &NativeCall<'_>) -> Result<Range, VmError> {
    let v = c.arg(0)?;
    let end = |name: &str| -> Result<f32, VmError> {
        field(c.vm, v, name)
            .map(|x| ow_combat::hit::access::to_f32(number(&x)))
            .ok_or_else(|| error(format!("the range argument has no {name}")))
    };
    Ok(Range::new(end("MinVal")?, end("MaxVal")?))
}

// ActionSequence.GetRangeValue ([`LOCK`]).
fn n_range_value(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let range = arg_range(c)?;
    if !range.valid() {
        return Ok(Value::Float(0.0));
    }
    let r = f32::from(c.vm.rng.crt(CrtStream::AiDecision).rand15());
    Ok(Value::Float(range.min + (range.max - range.min) * r / 32767.0))
}

fn this(c: &NativeCall<'_>) -> Result<ObjRef, VmError> {
    c.this.ok_or_else(|| error("an action native called without an object".to_string()))
}

/// `AIComponent.ActivateEvent(EventName)`, the AI component's event broadcast (lane FIX-ACTIVATE-EVENT, decision
/// 2026-11-19-42). Implemented from `docs/verification/NATIVE_SLICE_OFF_STOPS.md` alone (clean room: the analyst was
/// lane FIX-SLICE-OFF-1); every rule is **UNVERIFIED** in the running game. `ParityMode::Off` only
/// ([`register_activate_event`]); under Reference the key stays unbound, so the slice's C++ stub serves it.
pub const ACTIVATE_EVENT: &str = "GearboxFramework.AIComponent.ActivateEvent";

/// The analyst note of [`ACTIVATE_EVENT`].
pub const RECORD_SLICE_STOPS: &str = "docs/verification/NATIVE_SLICE_OFF_STOPS.md";

/// The note's rule, in order:
/// - the component's `AI` interface or its `AIDef` unset: nothing happens, the answer is false;
/// - a running node (`CurrentState` set): the running action gets the event first through a routine the note did not
///   open;
/// - otherwise every `AIDef.NodeList` node whose `Action` is set gets the script event `ActivateEvent(EventName)`, in
///   list order;
/// - the answer is always false.
///
/// What Off answers itself is only the case with no recipient: `AI` or `AIDef` unset with no brain for this
/// component, or a list with no `Action` set, answers false. Every other case is `NotYet { NotRead }`:
/// - a running node: the running action's first delivery (the note's "Not read" item);
/// - a brain part 2 built for this component (found through `AI`, or through its mind's `AIComponent` while `AI` is
///   still unset), running or not, or any set `Action` (reviews r1 and r2 of #429): the note leaves open which objects
///   the game's list holds, per-AI copies or the archetypes. The Rust `AIDef` is the shared package definition, part
///   2's brain keeps its own copies, and who owns a copy does not say which AI the game delivers to, so no receiver is
///   picked until that is read.
///
/// So Off delivers nothing. That also keeps the event all-or-nothing: no handler runs, so none can write state before
/// a later one fails. Every field is read and its kind checked before any answer; a component, definition or node
/// without a field read, or with a field of the wrong kind, is bad data, never a default.
fn n_activate_event(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const PER_AI_COPIES: &str = "AIComponent.ActivateEvent: which objects AIDef.NodeList holds, per-AI copies or the                                  archetypes, and so which AI receives the event (the note's open item \"which                                  objects the game's list holds\"); the Rust AIDef is the shared definition and part                                  2's brain keeps its own per-AI copies";
    let me = this(c)?;
    let vm = &mut *c.vm;
    let unit = "AIComponent.ActivateEvent";
    let required = |vm: &mut Vm, o: ObjRef, f: &str| -> Result<Value, VmError> {
        get(vm, o, f)?.ok_or_else(|| {
            error(format!("{unit}: {} declares no {f}", ow_combat::hit::access::class_name(vm, o)))
        })
    };
    let ai = match required(vm, me, "AI")? {
        Value::Interface(o) | Value::Object(o) => o,
        other => return Err(error(format!("{unit}: AI holds {}, not an interface", other.kind_name()))),
    };
    let definition = match required(vm, me, "AIDef")? {
        Value::Object(o) => o,
        other => return Err(error(format!("{unit}: AIDef holds {}, not an object", other.kind_name()))),
    };
    // A brain part 2 built for this component, even while the component's `AI` is not filled in (review r2).
    let mut has_brain = ai.is_some_and(|ai| brain(vm, ai).is_some());
    if !has_brain {
        let minds: Vec<ObjRef> = vm
            .service::<Brains>()
            .and_then(|s| s.try_borrow().ok().map(|s| s.brains.keys().copied().collect()))
            .unwrap_or_default();
        for mind in minds {
            if object(vm, mind, "AIComponent")? == Some(me) {
                has_brain = true;
                break;
            }
        }
    }
    if has_brain {
        return Err(NotYet::not_read(PER_AI_COPIES, RECORD_SLICE_STOPS).into());
    }
    let (Some(_), Some(definition)) = (ai, definition) else {
        return Ok(Value::Bool(false));
    };
    // The running node: the component's own pointer.
    let state = required(vm, me, "CurrentState")?;
    let pointer = match field(vm, &state, "Dummy") {
        Some(Value::Int(p)) => p,
        Some(other) => return Err(error(format!("{unit}: CurrentState.Dummy holds {}, not an int", other.kind_name()))),
        None => return Err(error(format!("{unit}: the component's CurrentState declares no Dummy"))),
    };
    if pointer != 0 {
        return Err(NotYet::not_read(
            "AIComponent.ActivateEvent with a running node: the running action's first delivery (the note's              \"What the running action's first delivery in ActivateEvent does beyond the event\")",
            RECORD_SLICE_STOPS,
        )
        .into());
    }
    // Read and check every node.
    let nodes = match required(vm, definition, "NodeList")? {
        Value::Array(nodes) => nodes,
        other => return Err(error(format!("{unit}: AIDef.NodeList holds {}, not an array", other.kind_name()))),
    };
    let mut recipients = 0usize;
    for node in nodes.iter() {
        match field(vm, node, "Action") {
            Some(Value::Object(Some(_))) => recipients += 1,
            Some(Value::Object(None)) => {}
            Some(other) => return Err(error(format!("{unit}: a node's Action holds {}", other.kind_name()))),
            None => return Err(error(format!("{unit}: the AIDef's nodes declare no Action"))),
        }
    }
    if recipients > 0 {
        return Err(NotYet::not_read(PER_AI_COPIES, RECORD_SLICE_STOPS).into());
    }
    Ok(Value::Bool(false))
}

/// Binds [`ACTIVATE_EVENT`] in `ParityMode::Off` (nothing under Reference). A key another pack bound first keeps its
/// binding. Worlds that do not register [`Pack`] (the mission and slice runs) call this directly.
pub fn register_activate_event(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    bind(vm, ACTIVATE_EVENT, Rc::new(n_activate_event))
}

fn arg_name(c: &NativeCall<'_>, i: usize) -> Result<Option<ow_object::ids::NameId>, VmError> {
    if i >= c.arg_count() {
        return Ok(None);
    }
    Ok(match c.arg(i)? {
        Value::Name(n) => Some(*n),
        _ => None,
    })
}

// ActionSequence.Finish(optional bool bFailed): queued (module documentation).
fn n_finish(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let action = this(c)?;
    let failed = c.arg_count() > 0 && c.has(0) && truth(c.vm, c.arg(0)?);
    let b = brains(c.vm)?;
    borrow(&b)?.finishes.push((action, failed));
    Ok(Value::None)
}

// ActionSequence.TriggerBehavior(OutputName): the event for the pawn's behavior consumer, through the kernel's
// consumer routing (`BroadcastBehaviorEventFromScript` with the pawn's handle).
fn n_trigger_behavior(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let action = this(c)?;
    let Some(name) = arg_name(c, 0)? else {
        return Ok(Value::None);
    };
    let Some(pawn) = object(c.vm, action, "MyWillowPawn")? else {
        return Ok(Value::None);
    };
    let Some(handle) = get(c.vm, pawn, "ConsumerHandle")? else {
        return Ok(Value::None);
    };
    let f = c.vm.find_function(ow_behavior::kernel::natives::BROADCAST_FROM_SCRIPT)?;
    c.vm.call(f, None, vec![handle, Value::Name(name)])?;
    Ok(Value::None)
}

/// `Hold` / `Release` on the component's list `list` of (`HoldName`, `Tag`) pairs (G26): a hold is added unless the same
/// pair is there or the tag is None; a release removes the pair. The Willow head-look side effects
/// (`SetLookAtTarget`) are presentation and not sent; the Willow `OnHold` extra case is not decoded (the base rule
/// answers it: UNVERIFIED).
fn hold_body(list: &'static str, add: bool) -> NativeFn {
    Rc::new(move |c: &mut NativeCall<'_>| {
        let me = this(c)?;
        let name = c.arg(0)?.clone();
        let tag = match c.arg(1)? {
            Value::Object(o) => *o,
            _ => None,
        };
        let mut items = array(c.vm, me, list)?;
        let same = |vm: &Vm, v: &Value| {
            field(vm, v, "HoldName").as_ref() == Some(&name) && object_of(field(vm, v, "Tag")) == tag
        };
        let at = items.iter().position(|v| same(c.vm, v));
        if add {
            if tag.is_none() || at.is_some() {
                return Ok(Value::None);
            }
            let class = c.vm.space.heap.get(me)?.class;
            let entry_kind =
                c.vm.space
                    .classes
                    .props(class)
                    .map_err(ow_object::heap::HeapError::from)?
                    .into_iter()
                    .find(|(_, d)| c.vm.space.names.text(d.name).eq_ignore_ascii_case(list))
                    .and_then(|(_, d)| match &d.kind {
                        ow_object::layout::PropKind::Array(inner) => Some((**inner).clone()),
                        _ => None,
                    })
                    .ok_or_else(|| error(format!("the component has no {list} array")))?;
            let mut entry = c.vm.space.classes.default_value(&entry_kind).map_err(ow_object::heap::HeapError::from)?;
            set_field(c.vm, &mut entry, "HoldName", name.clone());
            set_field(c.vm, &mut entry, "Tag", Value::Object(tag));
            items.push(entry);
        } else if let Some(i) = at {
            items.remove(i);
        } else {
            return Ok(Value::None);
        }
        set(c.vm, me, list, Value::Array(Rc::new(items)))?;
        Ok(Value::None)
    })
}

/// The installed movement host, if any.
pub(super) fn locomotion_host(vm: &mut Vm) -> Result<Option<Rc<RefCell<dyn Locomotion>>>, VmError> {
    let b = brains(vm)?;
    let b = borrow(&b)?;
    Ok(b.locomotion.clone())
}

fn locomotion(vm: &mut Vm) -> Result<Rc<RefCell<dyn Locomotion>>, VmError> {
    let b = brains(vm)?;
    let b = borrow(&b)?;
    b.locomotion.clone().ok_or_else(|| {
        NotYet::layer_absent("the AI's movement", "the host (ow_ai::awareness::actions::install_locomotion)", DECISION)
            .into()
    })
}

/// A vector argument's X, Y and Z; a value without them is an error (a wrong argument would otherwise send the pawn
/// to the origin).
fn vector(vm: &Vm, v: &Value) -> Result<Vec3, VmError> {
    let axis = |n: &str| {
        field(vm, v, n)
            .map(|x| number(&x) as f32)
            .ok_or_else(|| error(format!("a movement native's vector argument has no {n}")))
    };
    Ok([axis("X")?, axis("Y")?, axis("Z")?])
}

/// The pawn of an action native's object (an action: its `MyWillowPawn`).
fn action_pawn(c: &mut NativeCall<'_>) -> Result<ObjRef, VmError> {
    let me = this(c)?;
    object(c.vm, me, "MyWillowPawn")?.ok_or_else(|| error("the action has no MyWillowPawn".to_string()))
}

/// The movement natives answered by [`Locomotion`] for the action's pawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Move {
    PathIsValid,
    Invalidate,
    Reached,
    CanMoveTo,
    Speed,
}

fn bind_locomotion(vm: &mut Vm) -> Result<(), VmError> {
    let path_body: NativeFn = Rc::new(|c: &mut NativeCall<'_>| {
        let handle = this(c)?;
        let dest = vector(c.vm, c.arg(1)?)?;
        let host = locomotion(c.vm)?;
        let ok = host
            .try_borrow_mut()
            .map_err(|_| VmError::ServiceBorrowed("the AI's movement host"))?
            .create_path(c.vm, handle, dest)?;
        Ok(Value::Bool(ok))
    });
    bind(vm, "GearboxFramework.GearboxNavigationHandle.CreatePathToLocation", path_body.clone())?;
    bind(vm, "WillowGame.WillowNavigationHandle.CreateConstrainedPath", path_body)?;
    let clear: NativeFn = Rc::new(|c: &mut NativeCall<'_>| {
        let handle = this(c)?;
        let stop = c.arg_count() > 0 && truth(c.vm, c.arg(0)?);
        let host = locomotion(c.vm)?;
        let ok = host
            .try_borrow_mut()
            .map_err(|_| VmError::ServiceBorrowed("the AI's movement host"))?
            .clear_path(c.vm, handle, stop)?;
        Ok(Value::Bool(ok))
    });
    bind(vm, "GearboxFramework.GearboxNavigationHandle.ClearPath", clear.clone())?;
    bind(vm, "WillowGame.WillowNavigationHandle.ClearPath", clear)?;
    for (path, op) in [
        ("WillowGame.WillowActionSequencePawn.PathIsValid", Move::PathIsValid),
        ("WillowGame.WillowActionSequencePawn.InvalidatePath", Move::Invalidate),
        ("WillowGame.WillowActionSequencePawn.ReachedDestination", Move::Reached),
        ("WillowGame.WillowActionSequencePawn.CanMoveTo", Move::CanMoveTo),
        ("WillowGame.WillowActionSequencePawn.SetPawnMovementSpeed", Move::Speed),
    ] {
        bind(
            vm,
            path,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let pawn = action_pawn(c)?;
                let goal = match op {
                    Move::CanMoveTo => vector(c.vm, c.arg(0)?)?,
                    _ => [0.0; 3],
                };
                let speed = match op {
                    Move::Speed => u8::try_from(ow_combat::hit::access::integer(c.arg(0)?)).unwrap_or(0),
                    _ => 0,
                };
                let host = locomotion(c.vm)?;
                let mut h = host.try_borrow_mut().map_err(|_| VmError::ServiceBorrowed("the AI's movement host"))?;
                Ok(match op {
                    Move::PathIsValid => Value::Bool(h.path_is_valid(c.vm, pawn)?),
                    Move::Invalidate => {
                        h.invalidate_path(c.vm, pawn)?;
                        Value::None
                    }
                    Move::Reached => Value::Bool(h.reached_destination(c.vm, pawn)?),
                    Move::CanMoveTo => Value::Bool(h.can_move_to(c.vm, pawn, goal)?),
                    Move::Speed => {
                        h.set_movement_speed(c.vm, pawn, speed)?;
                        Value::None
                    }
                })
            }),
        )?;
    }
    bind(
        vm,
        "WillowGame.WillowPawn.GetNearestPositionOnNavMesh",
        Rc::new(|c: &mut NativeCall<'_>| {
            let pawn = this(c)?;
            let host = locomotion(c.vm)?;
            let point = host
                .try_borrow_mut()
                .map_err(|_| VmError::ServiceBorrowed("the AI's movement host"))?
                .nearest_on_navmesh(c.vm, pawn)?;
            let kind =
                c.vm.function(c.function)
                    .and_then(|d| d.result.as_ref())
                    .map(|r| r.kind.clone())
                    .ok_or_else(|| error("GetNearestPositionOnNavMesh declares no result".to_string()))?;
            let mut based = c.vm.space.classes.default_value(&kind).map_err(ow_object::heap::HeapError::from)?;
            if let Some(p) = point
                && let Some(mut position) = field(c.vm, &based, "Position")
            {
                for (axis, x) in ["X", "Y", "Z"].iter().zip(p) {
                    set_field(c.vm, &mut position, axis, Value::Float(x));
                }
                set_field(c.vm, &mut based, "Position", position);
            }
            Ok(based)
        }),
    )?;
    Ok(())
}
