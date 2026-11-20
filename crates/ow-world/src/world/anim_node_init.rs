//! Owner: lane IMPL-ANIM-NODE-INIT (decision 2026-11-19-193, number pending). The animation tree's per-node
//! initialisation (the copy's initialisation, section 2 step 4 of NATIVE_COPY_ANIM_TREE.md), in `ParityMode::Off`.
//! AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from `docs/verification/NATIVE_ANIM_NODE_INIT.md` alone (analyst lane
//! ANALYST-AIMOFFSET-REFRESH-AND-NODE-INIT, PR #566, accepted at 07b2dd68) and the hub's rulings in it; this lane did
//! not read the executable. **Every rule is UNVERIFIED in the running game.** Off only; Reference binds nothing.
//!
//! **The class table** ([`kind_of`], the note's section 2 and section 3): each node's **exact** class maps to one of
//! the note's eight implementations, A to H, and `AnimNodeSlot` to H's slot part. A class not in the table is
//! `NotYet { NotRead }` ([`unknown_class`]). The port cannot see C++ inheritance, so a subclass outside the table is
//! refused even if it would inherit an implementation.
//!
//! **The preflight** ([`plan`]) reads only, writes, loads and draws nothing. Over every node in tick order (the
//! children H initialises again are among them), the note's refusals 1 to 7 with its texts:
//! 1. `bCallScriptEventOnInit` set: [`ON_INIT`];
//! 2. D: `AnimSeq` set with `bIsIssuingNotifies` clear: [`SEQUENCE_CHANGE`]; `ActiveCameraAnimInstance` set:
//!    [`CAMERA`];
//! 3. C, or H's slot part, whose non-additive child weights sum to at most 1e-5 (H after its step 4):
//!    [`set_active_child`];
//! 4. E: the component's class must declare `bUpdateComposeSkeletonPasses` as a bool (`VmError::Script` otherwise);
//!    the write is the hub's ruling, **fitted, UNVERIFIED**;
//! 5. F: its weight builder's preflight (crate::world::per_bone_weights, NATIVE_PER_BONE_WEIGHTS.md, lane
//!    IMPL-PER-BONE-WEIGHTS): a null mesh skips the build; #569's layer refusals pass through; more than 256 bones is
//!    `NotYet { NotRead }`; a missing or mistyped field is a `VmError::Script`;
//! 6. H with an `AnimParameters` entry past the first naming a `CustomAnimDef` (after its step 7): [`ENTRY_WEIGHTS`];
//! 7. H with an owner: the owner's class must be known, without side effects, to implement
//!    `GearboxFramework.SpecialMoveInterface` ([`owner_implements`]: answered only when the owner class's interface
//!    entries are already resolved); otherwise [`OWNER_NOT_SMI`]. No owner leaves `SMI` as it is.
//!
//! Every declared field an implementation reads or writes is checked there too (a missing one is a `VmError::Script`).
//!
//! **The writes** ([`write_head`], [`write_tail`]) replay the planned [`Step`]s, iteratively: per node in tick order,
//! the copy's two writes (`SkelComponent`, `NodeTickTag`), then the note's section 2 steps for that node's
//! implementation; a slot's children past the first are initialised again between its head and its tail. The preflight
//! plans the same steps with the same tick rule, and checks every node they initialise (review r1 of #575).

use ow_core::NotYet;
use ow_object::ids::{ClassId, ObjRef};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::anim_tree_copy::{class_of, class_path, elements, object_of, read, truth, write};
use super::body_composition::{field, set_field};
use super::per_bone_weights::Build;

/// The analyst note.
pub const RECORD: &str = "docs/verification/NATIVE_ANIM_NODE_INIT.md";
const UNIT: &str = "the per-node initialisation";

/// The note's refusal texts (its section 3).
pub const ON_INIT: &str = "the node's OnInit script event (the event's name was not read)";
pub const SEQUENCE_CHANGE: &str = "AnimNodeSequence: the steps after its sequence changes";
pub const CAMERA: &str = "AnimNodeSequence: stopping its camera animation";
pub const ENTRY_WEIGHTS: &str = "AnimNodeSpecialMoveBlend: the per-bone weight build of an entry";
pub const OWNER_NOT_SMI: &str = "AnimNodeSpecialMoveBlend: an owner that does not implement SpecialMoveInterface";
/// The interface H's owner test names.
pub const SPECIAL_MOVE_INTERFACE: &str = "GearboxFramework.SpecialMoveInterface";

/// `"the per-node initialisation of <class>"`.
pub fn unknown_class(class: &str) -> String {
    format!("the per-node initialisation of {class}")
}

/// `"<class>: set active child during initialisation"`.
pub fn set_active_child(class: &str) -> String {
    format!("{class}: set active child during initialisation")
}


/// The note's eight implementations, and H's slot part on its own (`AnimNodeSlot`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    A,
    B,
    C,
    D,
    E,
    F,
    G,
    Slot,
    H,
}

/// The exact-class table (the note's section 2 table and section 3).
pub fn kind_of(path: &str) -> Option<Kind> {
    Some(match path {
        "Engine.AnimNode" => Kind::A,
        "Engine.AnimNodeBlendBase"
        | "Engine.AnimNodeScaleRateBySpeed"
        | "Engine.AnimNodeBlendDirectional"
        | "Engine.AnimNodeAimOffset"
        | "WillowGame.WillowAnimNodeAimOffset"
        | "WillowGame.WillowAnimNode_Condition"
        | "WillowGame.WillowAnimNode_ClimbLadder" => Kind::B,
        "Engine.AnimNodeBlendList"
        | "Engine.AnimNodeBlendBySpeed"
        | "Engine.AnimNodeBlendByPhysics"
        | "Engine.AnimNodeBlendByPosture"
        | "WillowGame.WillowAnimNodeBlendInjured"
        | "WillowGame.WillowAnimNodeBlendThirdPersonMenu" => Kind::C,
        "Engine.AnimNodeSequence" | "WillowGame.WillowAnimNodeSequenceAdditiveBlend" => Kind::D,
        "Engine.AnimTree" | "WillowGame.WillowAnimTree" => Kind::E,
        "Engine.AnimNodeBlendPerBone" | "WillowGame.WillowAnimNodeFeatherBlend" => Kind::F,
        "Engine.AnimNodeAdditiveBlending" => Kind::G,
        "Engine.AnimNodeSlot" => Kind::Slot,
        "GearboxFramework.AnimNodeSpecialMoveBlend" => Kind::H,
        _ => return None,
    })
}

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message }
}

fn not_read(what: impl Into<String>) -> VmError {
    NotYet::not_read(what.into(), RECORD).into()
}

fn float(v: &Value) -> f32 {
    match v {
        Value::Float(f) => *f,
        _ => 0.0,
    }
}

/// `node`'s implementation, by its exact class.
fn kind(vm: &Vm, node: ObjRef) -> Result<(Kind, String), VmError> {
    let path = class_path(vm, class_of(vm, node)?);
    kind_of(&path).map(|k| (k, path.clone())).ok_or_else(|| not_read(unknown_class(&path)))
}

/// Whether the implementation runs B (every one but A and D).
fn runs_b(k: Kind) -> bool {
    !matches!(k, Kind::A | Kind::D)
}

/// The child slots of `node` (its `Children`).
fn slots(vm: &Vm, node: ObjRef) -> Result<Vec<Value>, VmError> {
    elements(&read(vm, node, "Children")?, "Children")
}

/// The sum of the child weights of the slots not marked `bIsAdditive`, over `slots`.
fn non_additive_sum(vm: &Vm, slots: &[Value]) -> Result<f32, VmError> {
    let mut sum = 0.0;
    for s in slots {
        if !truth(&field(vm, s, "bIsAdditive", "AnimBlendChild")?) {
            sum += float(&field(vm, s, "Weight", "AnimBlendChild")?);
        }
    }
    Ok(sum)
}

/// H's step 4: `1 − clamp(sum of the other slots' non-additive weights, 0, 1)`.
fn first_weight(vm: &Vm, slots: &[Value]) -> Result<f32, VmError> {
    let others = non_additive_sum(vm, slots.get(1..).unwrap_or(&[]))?;
    Ok(1.0 - others.clamp(0.0, 1.0))
}

/// Rule 7's check, without side effects: `Some(true)` only when the owner's class is known to implement
/// [`SPECIAL_MOVE_INTERFACE`] by interface entries already resolved; `None` when that cannot be told without resolving
/// or loading (a class or the interface not in the code index, an entry not resolved yet).
pub fn owner_implements(vm: &Vm, owner: ObjRef) -> Result<Option<bool>, VmError> {
    #[cfg(test)]
    if let Some(answer) = OWNER_ANSWER.with(std::cell::Cell::get) {
        return Ok(Some(answer));
    }
    let class = class_of(vm, owner)?;
    let Some(iface) = vm.space.classes.by_path(SPECIAL_MOVE_INTERFACE) else {
        return Ok(None);
    };
    let code = vm.defs.code();
    Ok(match (code.key_of(class), code.key_of(iface)) {
        (Some(c), Some(i)) => code.implements_resolved(c, i),
        _ => None,
    })
}

#[cfg(test)]
thread_local! {
    static OWNER_ANSWER: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
}

/// For the tests only: answer [`owner_implements`] with `answer` on this thread (hand-built classes are not in the
/// code index, so the real query answers `None` for them).
#[cfg(test)]
pub(crate) fn answer_owner_for_tests(answer: Option<bool>) {
    OWNER_ANSWER.with(|c| c.set(answer));
}

/// The component's class declares `name` as a bool (refusal 4's check).
fn declares_bool(vm: &Vm, class: ClassId, name: &str) -> bool {
    vm.space.prop(class, name).and_then(|p| vm.space.classes.decl(p)).is_some_and(|d| matches!(d.kind, PropKind::Bool))
}

/// The preflight's check of one node (module documentation), reading `node` (a template node: the copy holds the same
/// values) and `component`.
fn check_node(vm: &Vm, component: ObjRef, node: ObjRef) -> Result<(), VmError> {
    let (k, path) = kind(vm, node)?;
    // A, the base every implementation runs first.
    for name in ["bRelevant", "bJustBecameRelevant", "NodeTotalWeight", "NodeTickTag", "bCallScriptEventOnInit"] {
        read(vm, node, name)?;
    }
    read(vm, component, "TickTag")?;
    if truth(&read(vm, node, "bCallScriptEventOnInit")?) {
        return Err(not_read(ON_INIT));
    }
    let children = if runs_b(k) { slots(vm, node)? } else { Vec::new() };
    if runs_b(k) {
        for s in &children {
            for f in ["AdditiveWeightScale", "Weight", "bIsAdditive"] {
                field(vm, s, f, "AnimBlendChild")?;
            }
        }
    }
    match k {
        Kind::A | Kind::B => {}
        Kind::C => {
            read(vm, node, "TargetWeight")?;
            read(vm, node, "ActiveChildIndex")?;
            if non_additive_sum(vm, &children)? <= 1e-5 {
                return Err(not_read(set_active_child(&path)));
            }
        }
        Kind::D => {
            for name in ["AnimSeqName", "AnimLinkupIndex", "CachedNumDesiredBones"] {
                read(vm, node, name)?;
            }
            let seq = object_of(&read(vm, node, "AnimSeq")?);
            let issuing = truth(&read(vm, node, "bIsIssuingNotifies")?);
            if seq.is_some() && !issuing {
                return Err(not_read(SEQUENCE_CHANGE));
            }
            if object_of(&read(vm, node, "ActiveCameraAnimInstance")?).is_some() {
                return Err(not_read(CAMERA));
            }
        }
        Kind::E => {
            let class = class_of(vm, component)?;
            if !declares_bool(vm, class, "bUpdateComposeSkeletonPasses") {
                return Err(script(format!(
                    "{} does not declare bUpdateComposeSkeletonPasses as a bool",
                    class_path(vm, class)
                )));
            }
        }
        // F's weight builder (NATIVE_PER_BONE_WEIGHTS.md, lane IMPL-PER-BONE-WEIGHTS): planned, refusals typed.
        // F's builder is planned with each of its heads (see `plan`).
        Kind::F => {}
        Kind::G => {}
        Kind::Slot | Kind::H => {
            read(vm, node, "TargetWeight")?;
            read(vm, node, "SynchNode")?;
            // After step 4 the first slot's weight is fixed; then step 5's test.
            let mut after = children.clone();
            if let Some(first) = after.first_mut() {
                let w = first_weight(vm, &children)?;
                set_field(vm, first, "Weight", Value::Float(w), "AnimBlendChild")?;
            }
            if non_additive_sum(vm, &after)? <= 1e-5 {
                return Err(not_read(set_active_child(&path)));
            }
            if k == Kind::H {
                // The element struct must be defined, its defaults readable through the checked API (review r1 of
                // #575, item 4), before any write uses it.
                parameters_element(vm, node)?;
                let params = elements(&read(vm, node, "AnimParameters")?, "AnimParameters")?;
                read(vm, node, "SMI")?;
                if params.len() == children.len() {
                    for p in params.iter().skip(1) {
                        if object_of(&field(vm, p, "CustomAnimDef", "SlotAnimParameters")?).is_some() {
                            return Err(not_read(ENTRY_WEIGHTS));
                        }
                    }
                }
                if let Some(owner) = object_of(&read(vm, component, "Owner")?)
                    && owner_implements(vm, owner)? != Some(true)
                {
                    return Err(not_read(OWNER_NOT_SMI));
                }
            }
        }
    }
    Ok(())
}

/// The zero `AnimParameters` entry of H node `node`: its declared element must be a defined struct (the checked
/// `struct_layout`, review r1 of #575, item 4), and the entry is that layout's zero value (review r2, item 1).
fn parameters_element(vm: &Vm, node: ObjRef) -> Result<Value, VmError> {
    let class = class_of(vm, node)?;
    let element = vm
        .space
        .prop(class, "AnimParameters")
        .and_then(|p| vm.space.classes.decl(p))
        .and_then(|d| match &d.kind {
            PropKind::Array(inner) => match **inner {
                PropKind::Struct(s) => Some(s),
                _ => None,
            },
            _ => None,
        })
        .ok_or_else(|| script(format!("{}.AnimParameters is not an array of structs", class_path(vm, class))))?;
    // H's step 7 makes zeroed entries: the struct's zero value, not its defaults, which may hold nonzero values or a
    // CustomAnimDef the preflight's step 6 check would then have to see (review r2 of #575, item 1).
    vm.space
        .classes
        .struct_layout(element)
        .map(|l| l.zero().clone())
        .ok_or_else(|| script(format!("{}.AnimParameters: its element struct is not defined", class_path(vm, class))))
}

/// One call of the initialisation, on a **template** node (the writes map it to its copy): the steps the writes run,
/// in order (review r1 of #575, items 1 and 3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// The copy's two writes on a node of the tick list (`SkelComponent`, `NodeTickTag`).
    Tick(ObjRef),
    /// H's step 2 on a child past the first: its `SkelComponent`.
    Child(ObjRef),
    /// A node's initialisation up to a slot's children (A, B, and the whole of every other implementation). For an F
    /// node, the index of its build in the plan's builds, planned in the preflight for this head on its own (review r1
    /// of #585, item 2: the writes replay it, never rerun the builder).
    Head(ObjRef, Option<usize>),
    /// A slot's steps after its children (H's steps 3 to 9).
    Tail(ObjRef),
}

/// The most steps one initialisation may plan (a guard against a tree whose slots re-initialise without end).
const MAX_STEPS: usize = 1 << 16;

/// The steps of `node`'s initialisation (after its tick-list writes, or after H's child step), iteratively: a slot's
/// children past the first are initialised again, in slot order, between its head and its tail. A child already on
/// the active slot call path is a loop the writes would never leave: a `VmError::Script` (review r1 of #575, item 3).
fn expand(vm: &Vm, node: ObjRef, out: &mut Vec<Step>) -> Result<(), VmError> {
    enum Frame {
        Enter(ObjRef),
        Child(ObjRef),
        Leave(ObjRef),
    }
    let mut stack = vec![Frame::Enter(node)];
    let mut path: Vec<ObjRef> = Vec::new();
    while let Some(frame) = stack.pop() {
        if out.len() > MAX_STEPS {
            return Err(script(format!("the initialisation plans more than {MAX_STEPS} steps")));
        }
        match frame {
            Frame::Child(c) => {
                out.push(Step::Child(c));
                stack.push(Frame::Enter(c));
            }
            Frame::Enter(n) => {
                out.push(Step::Head(n, None));
                let (k, path_text) = kind(vm, n)?;
                if matches!(k, Kind::Slot | Kind::H) {
                    if path.contains(&n) {
                        return Err(script(format!(
                            "{path_text}: a slot's children lead back to it (its initialisation would not end)"
                        )));
                    }
                    path.push(n);
                    stack.push(Frame::Leave(n));
                    let again: Vec<ObjRef> = slots(vm, n)?
                        .iter()
                        .skip(1)
                        .map(|s| field(vm, s, "Anim", "AnimBlendChild").map(|v| object_of(&v)))
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .flatten()
                        .collect();
                    for c in again.into_iter().rev() {
                        stack.push(Frame::Child(c));
                    }
                }
            }
            Frame::Leave(n) => {
                path.pop();
                out.push(Step::Tail(n));
            }
        }
    }
    Ok(())
}

/// The preflight (module documentation): the steps for `order` (every node of the tick list, in order) and their
/// checks. Every node a step initialises is checked, H's re-initialised children included, whether or not it is on the
/// tick list (review r1 of #575, item 1).
pub fn plan(vm: &Vm, component: ObjRef, order: &[ObjRef]) -> Result<(Vec<Step>, Vec<Build>), VmError> {
    let mut steps = Vec::new();
    for &n in order {
        steps.push(Step::Tick(n));
        expand(vm, n, &mut steps)?;
    }
    let mut checked = ow_core::DetSet::new();
    let mut builds = Vec::new();
    for step in &mut steps {
        if let Step::Head(n, slot) = step {
            if checked.insert(*n) {
                check_node(vm, component, *n)?;
            }
            // Every F head's build, planned on its own (the note's section 6, "Where it runs").
            if kind(vm, *n)?.0 == Kind::F {
                builds.push(super::per_bone_weights::plan(vm, component, *n)?);
                *slot = Some(builds.len() - 1);
            }
        }
    }
    Ok((steps, builds))
}

/// Writes `v` into the integer property `name` of `o` by its declared kind (an int, or a byte).
fn write_int(vm: &mut Vm, o: ObjRef, name: &str, v: i32) -> Result<(), VmError> {
    let class = class_of(vm, o)?;
    let kind = vm.space.prop(class, name).and_then(|p| vm.space.classes.decl(p)).map(|d| d.kind.clone());
    let value = match kind {
        Some(PropKind::Byte { .. }) => Value::Byte(u8::try_from(v).unwrap_or(0)),
        _ => Value::Int(v),
    };
    write(vm, o, name, value)
}

/// A `TargetWeight` with one entry per child, the first 1.0 (C's and H's step), unless it already matches.
fn fix_target_weight(vm: &mut Vm, node: ObjRef, children: usize) -> Result<(), VmError> {
    let current = elements(&read(vm, node, "TargetWeight")?, "TargetWeight")?;
    if current.len() == children {
        return Ok(());
    }
    let mut fresh = vec![Value::Float(0.0); children];
    if let Some(first) = fresh.first_mut() {
        *first = Value::Float(1.0);
    }
    write(vm, node, "TargetWeight", Value::array(fresh))
}

/// The head of `node`'s initialisation for `component` ([`Step::Head`]): A, B, and every implementation's steps but a
/// slot's (whose steps after its children are [`write_tail`]).
pub fn write_head(vm: &mut Vm, component: ObjRef, node: ObjRef, build: Option<&Build>) -> Result<(), VmError> {
    let (k, _) = kind(vm, node)?;
    // A.
    write(vm, node, "bRelevant", Value::Bool(false))?;
    write(vm, node, "bJustBecameRelevant", Value::Bool(false))?;
    let tag = match read(vm, component, "TickTag")? {
        Value::Int(i) => i,
        _ => 0,
    };
    write(vm, node, "NodeTickTag", Value::Int(tag))?;
    write(vm, node, "NodeTotalWeight", Value::Float(0.0))?;
    // B.
    if runs_b(k) {
        let mut list = slots(vm, node)?;
        let mut changed = false;
        for s in &mut list {
            if float(&field(vm, s, "AdditiveWeightScale", "AnimBlendChild")?) == 0.0 {
                set_field(vm, s, "AdditiveWeightScale", Value::Float(1.0), "AnimBlendChild")?;
                changed = true;
            }
        }
        if changed {
            write(vm, node, "Children", Value::array(list))?;
        }
    }
    match k {
        Kind::A | Kind::B => {}
        Kind::C => {
            let n = slots(vm, node)?.len();
            fix_target_weight(vm, node, n)?;
        }
        Kind::D => {
            let kept = read(vm, node, "AnimSeqName")?;
            if !truth(&read(vm, node, "bIsIssuingNotifies")?) {
                write(vm, node, "AnimSeqName", Value::Name(ow_object::ids::NameId::NONE))?;
                write(vm, node, "AnimSeq", Value::Object(None))?;
                write_int(vm, node, "AnimLinkupIndex", -1)?;
            }
            write_int(vm, node, "CachedNumDesiredBones", 0)?;
            write(vm, node, "AnimSeqName", kept)?;
        }
        Kind::E => write(vm, component, "bUpdateComposeSkeletonPasses", Value::Bool(true))?,
        Kind::F | Kind::G => {
            // F: the weight builder's two writes, after B and before the first child's weight (lane
            // IMPL-PER-BONE-WEIGHTS); a null mesh builds nothing. F and G: the first child's weight 1.0.
            if k == Kind::F {
                match build {
                    Some(Build::Write(w, l)) => {
                        write(vm, node, "Child2PerBoneWeight", Value::array(w.iter().copied().map(Value::Float).collect()))?;
                        write(vm, node, "LocalToCompReqBones", Value::array(l.iter().copied().map(Value::Byte).collect()))?;
                    }
                    Some(Build::Skip) => {}
                    None => return Err(script("an F head with no planned build".into())),
                }
            }
            let mut list = slots(vm, node)?;
            if let Some(first) = list.first_mut()
                && float(&field(vm, first, "Weight", "AnimBlendChild")?) != 1.0
            {
                set_field(vm, first, "Weight", Value::Float(1.0), "AnimBlendChild")?;
                write(vm, node, "Children", Value::array(list))?;
            }
        }
        // A slot's step 2 (its children past the first, initialised again) is the planned steps between its head and
        // its tail.
        Kind::Slot | Kind::H => {}
    }
    Ok(())
}

/// The tail of slot node `node`'s initialisation for `component` ([`Step::Tail`]): H's steps 3 to 9, after its
/// children's.
pub fn write_tail(vm: &mut Vm, component: ObjRef, node: ObjRef) -> Result<(), VmError> {
    let (k, _) = kind(vm, node)?;
    match k {
        Kind::Slot | Kind::H => {
            let n = slots(vm, node)?.len();
            fix_target_weight(vm, node, n)?;
            let mut list = slots(vm, node)?;
            let w = first_weight(vm, &list)?;
            if let Some(first) = list.first_mut() {
                set_field(vm, first, "Weight", Value::Float(w), "AnimBlendChild")?;
                write(vm, node, "Children", Value::array(list))?;
            }
            write(vm, node, "SynchNode", Value::Object(None))?;
            if k == Kind::H {
                let params = elements(&read(vm, node, "AnimParameters")?, "AnimParameters")?;
                if params.len() != n {
                    let zero = parameters_element(vm, node)?;
                    write(vm, node, "AnimParameters", Value::array(vec![zero; n]))?;
                }
                if let Some(owner) = object_of(&read(vm, component, "Owner")?) {
                    // The preflight answered yes (rule 7).
                    let class = class_of(vm, node)?;
                    let interface = vm
                        .space
                        .prop(class, "SMI")
                        .and_then(|p| vm.space.classes.decl(p))
                        .is_some_and(|d| matches!(d.kind, PropKind::Interface { .. }));
                    let v = if interface { Value::Interface(Some(owner)) } else { Value::Object(Some(owner)) };
                    write(vm, node, "SMI", v)?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

// Tested with the copy's fixture (anim_tree_copy_tests.rs, "The per-node initialisation").
