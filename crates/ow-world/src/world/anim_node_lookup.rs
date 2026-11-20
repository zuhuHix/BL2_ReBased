//! Owner: lanes IMPL-ANIM-NODE-LOOKUP and FIX-BODY-COMPOSITION-LOOKUP-STALE (decisions 2026-11-19-NNN, numbers
//! pending). The new `Mesh`'s animation-node and skeletal-control lookups and the slot-node scan of the pawn's
//! `PostInitBodyComposition`, in `ParityMode::Off`, over the **copied tree**. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from `docs/verification/NATIVE_ANIM_NODE_LOOKUP.md` alone (analyst lane
//! ANALYST-ANIM-NODE-LOOKUP, PR #543) and the hub's rulings; neither lane read the executable. **Every rule is
//! UNVERIFIED in the running game.** Reference is unchanged (the body composition is Off only).
//!
//! **What changed (lane FIX-BODY-COMPOSITION-LOOKUP-STALE).** The first lane ran the lookups on the template and
//! refused every answer that needed the instance's node ("needs an animation-tree instance ... not built yet"): the
//! port had no instanced tree. Since #561, #575 and #608 the attach copies and initialises the tree. So the lookups
//! now take the note's section 1 and 2 rules on that instance, planned from the copy's own preflight
//! (`anim_tree_copy::Plan`) before anything is written. The copy keeps each node's class and name (its slots), and the
//! plan's tick order and control order are the instance's, on template objects. The answers are template objects;
//! [`apply`] writes the copies through the copy's map.
//!
//! **The rules** (the note's section 1, with the instance's tick list filled by the initialisation):
//! - **an animation node by name:** the first node in the tick list whose `NodeName` equals the name, compared whole
//!   (a None name included: it matches the first node whose `NodeName` is None, the root when it has none);
//! - **a skeletal control by name:** the tree's `SkelControlLists` in order, each list's head then its `NextControl`
//!   chain, the first whose `ControlName` equals the name; a None name answers None with no search;
//! - **the five fields:** `ArmBoneBlend` (node `ArmBoneBlend`, kept as `AnimNodeBlendPerBone`), `AimOffset` (node
//!   `BodyClass.AimOffsetNodeName`, kept as `WillowAnimNodeAimOffset`), `LeftHandIK` (control `LeftHandGrip`, kept as
//!   `SkelControlLeftHandGripWeapon`), `HeadLookAt` (control `BodyClass.HeadLookAtName`, kept as
//!   `WillowSkelControl_LookAtActor`) and `bUsingFootIK` (whether a `WillowFootIK` control exists). A find of
//!   another class is None;
//! - **`SlotNodes`** (section 2): the tick list's `AnimNodeSlot` nodes (subclasses included), in tick order.
//!
//! **Refusals kept, each with nothing written or loaded:**
//! - an unloaded template component or body class is #529's refusal ([`refusal`]);
//! - a planned copy that skips the initialisation (the template component's `bAnimTreeInitialised` set) has no tick
//!   list, and the note's gather path for that case was not read: `NotYet { NotRead }` ([`NO_TICK_LIST`]);
//! - no body class: `NotYet { NotRead }` ([`NO_BODY_CLASS`]), as before;
//! - a tree with no planned copy (only when the attach's copy check is switched off in tests) is #529's refusal
//!   unchanged: no instance is planned, so none is there to search.
//!
//! No `AnimTreeTemplate` answers None for the four lookups, false for `bUsingFootIK` and an empty `SlotNodes` (the
//! note's rule 5).

use ow_core::{DetMap, NotYet};
use ow_object::ids::{ClassId, NameId, ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::body_composition::{ANIM_LAYER, ANIM_NODES};
use crate::world::anim_tree_copy::Plan;

/// The analyst note.
pub const RECORD: &str = "docs/verification/NATIVE_ANIM_NODE_LOOKUP.md";
/// The record of #529's refusal: the body composition's section 7, with this note (the note's rule 4).
pub const REFUSAL_RECORD: &str =
    "docs/verification/NATIVE_INITIALIZE_BODY_COMPOSITION.md (section 7) and docs/verification/NATIVE_ANIM_NODE_LOOKUP.md";
/// The AimOffset lookup with no body class: the note leaves it unread (unreachable from the pawn wrapper, whose gate
/// needs a loaded body class).
pub const NO_BODY_CLASS: &str = "InitializeBodyComposition: the AimOffset node name with no BodyClass";
/// A copy that skips the initialisation has no tick list; the lookup's other path (the node class's own gather) was
/// not read (the note's section 1 and open question 2).
pub const NO_TICK_LIST: &str =
    "InitializeBodyComposition: the animation-node lookup on a tree with no tick list (the node gather step)";

/// `"InitializeBodyComposition: the body class <path>, whose load failed"`: a failed body class handle (review r1 of
/// #557, item 3).
pub fn failed_body_class(path: &str) -> String {
    format!("InitializeBodyComposition: the body class {path}, whose load failed")
}

/// The lookups' answers, on template objects (the copy's sources): [`apply`] maps them to the copies. Not to be
/// dropped (review r1 of #618, item 1): a caller that keeps one field and writes the rest as None would give the pawn
/// a wrong answer, not a refusal.
#[must_use = "the lookups' answers must be written (anim_node_lookup::apply), not dropped"]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Settled {
    pub arm_bone_blend: Option<ObjRef>,
    pub aim_offset: Option<ObjRef>,
    pub left_hand_ik: Option<ObjRef>,
    pub head_look_at: Option<ObjRef>,
    /// `bUsingFootIK`: whether a `WillowFootIK` control exists.
    pub foot_ik: bool,
    /// `SlotNodes`, in tick order.
    pub slots: Vec<ObjRef>,
}

/// #529's refusal, text and layer unchanged (the note's rule 4).
pub fn refusal() -> VmError {
    NotYet::layer_absent(ANIM_NODES, ANIM_LAYER, REFUSAL_RECORD).into()
}

fn script(message: String) -> VmError {
    VmError::Script { function: super::body_composition::UNIT.to_string(), message }
}

/// Whether `o` can be read without a load: not an unloaded or failed store handle.
fn readable(vm: &Vm, o: ObjRef) -> bool {
    !vm.defs.is_unloaded(&vm.space, o) && !vm.defs.is_failed(&vm.space, o)
}

/// `o`, refused unless readable.
fn need(vm: &Vm, o: ObjRef) -> Result<ObjRef, VmError> {
    if readable(vm, o) { Ok(o) } else { Err(refusal()) }
}

/// Property `name` of `o` as it stands; its class must declare it.
fn get(vm: &Vm, o: ObjRef, name: &str) -> Result<Value, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{} has no {name}", class_text(vm, class))))?;
    Ok(vm.space.read(o, PropAddr::scalar(p))?.clone())
}

fn class_text(vm: &Vm, class: ClassId) -> String {
    vm.space.classes.get_layout(class).map_or_else(|| format!("{class:?}"), |l| l.path().as_str().to_string())
}

fn object(v: &Value) -> Option<ObjRef> {
    match v {
        Value::Object(o) => *o,
        _ => None,
    }
}

fn name_of(vm: &Vm, o: ObjRef, prop: &str) -> Result<NameId, VmError> {
    match get(vm, o, prop)? {
        Value::Name(n) => Ok(n),
        other => Err(script(format!("{prop} holds {}, not a name", other.kind_name()))),
    }
}

/// Whether `o`'s class is the class named `short`, or a subclass of it (by the class names up the chain, whatever
/// their package).
fn is_kind(vm: &Vm, o: ObjRef, short: &str) -> Result<bool, VmError> {
    let mut cursor = Some(vm.space.heap.get(o)?.class);
    while let Some(c) = cursor {
        let Some(layout) = vm.space.classes.get_layout(c) else {
            break;
        };
        if layout.path().name().eq_ignore_ascii_case(short) {
            return Ok(true);
        }
        cursor = layout.super_class();
    }
    Ok(false)
}

/// The first node named `name` (a None name included) in the tick list.
fn find_node(vm: &Vm, ticks: &[ObjRef], name: NameId) -> Result<Option<ObjRef>, VmError> {
    for &n in ticks {
        if name_of(vm, n, "NodeName")? == name {
            return Ok(Some(n));
        }
    }
    Ok(None)
}

/// The first control named `name` in list order; a None name answers None with no search.
fn find_control(vm: &Vm, controls: &[ObjRef], name: NameId) -> Result<Option<ObjRef>, VmError> {
    if name == NameId::NONE {
        return Ok(None);
    }
    for &c in controls {
        if name_of(vm, c, "ControlName")? == name {
            return Ok(Some(c));
        }
    }
    Ok(None)
}

/// A find kept only when it is of class `kept_as`.
fn kept(vm: &Vm, found: Option<ObjRef>, kept_as: &str) -> Result<Option<ObjRef>, VmError> {
    match found {
        Some(o) if is_kind(vm, o, kept_as)? => Ok(Some(o)),
        _ => Ok(None),
    }
}

/// The lookups (module documentation) for the made `Mesh` from `template`, with the pawn's `body` class and the
/// copy's preflight `plan` (None when no copy is planned). Reads only, writes and loads nothing.
pub fn check(vm: &Vm, template: ObjRef, body: Option<ObjRef>, plan: Option<&Plan>) -> Result<Settled, VmError> {
    let template = need(vm, template)?;
    if object(&get(vm, template, "AnimTreeTemplate")?).is_none() {
        // Rule 5: no tree.
        return Ok(Settled::default());
    }
    let Some(plan) = plan else {
        return Err(refusal());
    };
    lookups(vm, plan, body)
}

/// The lookups over the copy `plan` (its tick order and control order, on template objects), with the pawn's `body`
/// class. Reads only.
pub fn lookups(vm: &Vm, plan: &Plan, body: Option<ObjRef>) -> Result<Settled, VmError> {
    if !plan.initialise {
        return Err(NotYet::not_read(NO_TICK_LIST, RECORD).into());
    }
    let ticks = &plan.order;
    let controls = &plan.controls;
    let body = match body {
        // A body class whose load failed is named, as a failed template is (review r1 of #557, item 3).
        Some(b) if vm.defs.is_failed(&vm.space, b) => {
            return Err(NotYet::not_read(failed_body_class(&super::pawn_meshes::handle_path(vm, b)), RECORD).into());
        }
        Some(b) => Some(need(vm, b)?),
        None => None,
    };
    let fixed = |text: &str| vm.space.names.find(text);
    let mut out = Settled::default();
    if let Some(n) = fixed("ArmBoneBlend") {
        out.arm_bone_blend = kept(vm, find_node(vm, ticks, n)?, "AnimNodeBlendPerBone")?;
    }
    let Some(b) = body else {
        return Err(NotYet::not_read(NO_BODY_CLASS, RECORD).into());
    };
    let aim = name_of(vm, b, "AimOffsetNodeName")?;
    out.aim_offset = kept(vm, find_node(vm, ticks, aim)?, "WillowAnimNodeAimOffset")?;
    if let Some(n) = fixed("LeftHandGrip") {
        out.left_hand_ik = kept(vm, find_control(vm, controls, n)?, "SkelControlLeftHandGripWeapon")?;
    }
    let head = name_of(vm, b, "HeadLookAtName")?;
    out.head_look_at = kept(vm, find_control(vm, controls, head)?, "WillowSkelControl_LookAtActor")?;
    out.foot_ik = match fixed("WillowFootIK") {
        Some(n) => find_control(vm, controls, n)?.is_some(),
        None => false,
    };
    for &n in ticks {
        if is_kind(vm, n, "AnimNodeSlot")? {
            out.slots.push(n);
        }
    }
    Ok(out)
}

/// The pawn fields the lookups write, in the hook's order (the note's section 1, then `SlotNodes`).
pub const FIELDS: [&str; 6] = ["ArmBoneBlend", "AimOffset", "LeftHandIK", "HeadLookAt", "bUsingFootIK", "SlotNodes"];

/// The lookups' writes on `owner` (the pawn), with `map` the copy's map from each template object to its copy
/// (`anim_tree_copy::instance_with_map`): the four objects, `bUsingFootIK` and `SlotNodes`. Called after the attach
/// made the copy. An answer the map has no copy of is a `VmError::Script`, checked before the first write (review r1
/// of #618, item 2): writing the template object instead would put a template node in a pawn field.
pub fn apply(vm: &mut Vm, owner: ObjRef, s: &Settled, map: &DetMap<ObjRef, ObjRef>) -> Result<(), VmError> {
    use crate::world::anim_tree_copy::write;
    let copy = |o: ObjRef| -> Result<ObjRef, VmError> {
        map.get(&o).copied().ok_or_else(|| {
            script(format!("the copy's map has no copy of {} (the plan and the copy disagree)", class_text_of(vm, o)))
        })
    };
    let obj = |o: Option<ObjRef>| -> Result<Value, VmError> { Ok(Value::Object(o.map(copy).transpose()?)) };
    let values = [obj(s.arm_bone_blend)?, obj(s.aim_offset)?, obj(s.left_hand_ik)?, obj(s.head_look_at)?];
    let slots: Vec<Value> = s.slots.iter().map(|&n| copy(n).map(|c| Value::Object(Some(c)))).collect::<Result<_, _>>()?;
    for (name, v) in FIELDS[..4].iter().zip(values) {
        write(vm, owner, name, v)?;
    }
    write(vm, owner, FIELDS[4], Value::Bool(s.foot_ik))?;
    write(vm, owner, FIELDS[5], Value::array(slots))
}

fn class_text_of(vm: &Vm, o: ObjRef) -> String {
    vm.space.heap.get(o).map_or_else(|_| format!("{o:?}"), |x| class_text(vm, x.class))
}

// Tested with the copy's fixture (anim_tree_copy_tests.rs, "the body composition's lookups over the copy") and the
// body composition's (body_composition_tests.rs, "the animation-node lookups").
