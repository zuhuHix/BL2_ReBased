//! Owner: lane IMPL-UPDATEANIMATIONS-TREE-TICKS. `UpdateAnimations`' per-node step on an animation tree, in
//! `ParityMode::Off`: what replaces `bone_transform`'s `TREE_TICKS` refusal. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_UPDATEANIMATIONS_TREE_TICKS.md alone (#747 with its r1
//! fix, lane ANALYST-UPDATEANIMATIONS-TREE-TICKS, decision 2026-11-26-06); this lane did not read the executable.
//! **Every rule is UNVERIFIED in the running game.** Reference binds nothing (`bone_transform` binds Off only).
//!
//! **The step is not a timed tick** (the note's section 2): the component's tag field := the tree's `NodeInitTag` + 1,
//! then one call per `AnimTickArray` node with no time argument. It does nothing on the 28 classes of [`EMPTY_STEP`];
//! the sequence family drops its sequence and keeps its name (section 3).
//!
//! **The preflight** ([`plan`]) reads only: no write, no store load, no class-code load, nothing interned (a name is
//! matched by its text). In order, the first refusal met is the answer:
//! 1. `Animations` declared as an object (checked by the caller); None is nothing at all;
//! 2. the tree root resident ([`NOT_LOADED`]), declaring `NodeInitTag` as an int; the component declares `InitTag` as
//!    an int (the tag field's name is the bone-transform note's fit, **UNVERIFIED**) and `AnimTickArray` as an array.
//!    The tick list is the component's `AnimTickArray` (the copy fills the component's and the root's alike);
//! 3. each tick-list entry in order: None skipped; a store handle not loaded is [`TICK_NODE_NOT_LOADED`]. By its
//!    **exact** class: one of [`EMPTY_STEP`], nothing; `Engine.AnimNodeSequence` or `WillowGame.WillowAnimNodeSequence`,
//!    the plain step; `WillowGame.WillowAnimNodeSequenceAdditiveBlend`, the plain step and the override; any other
//!    class, [`per_node_step`];
//! 4. per sequence node, the fields declared with their kinds (`AnimSeqName` a name, `AnimSeq` an object,
//!    `AnimLinkupIndex` an int, `bIsIssuingNotifies` a bool, `CachedNumDesiredBones` a byte), else `VmError::Script`.
//!    The reset is skipped when `bIsIssuingNotifies` is set **and** `AnimSeqName` is not None (the guard). When the
//!    reset runs, a set `AnimSeq` is [`SEQUENCE_CHANGES`] (the meta-data call, inside the reset; with the guard holding,
//!    the reset and so that call do not run);
//! 5. per additive node, whatever `bAutoGenerateAdditiveSeqName`: its declarations (that flag a bool, `AddAnimSeqName`
//!    a name, `AddAnimSeq` an object, `AddAnimLinkupIndex` an int, and `bReverseAnimSeqSearchOrder` a bool). With the
//!    flag set while the guard holds, [`AUTO_NAME_ISSUING`]. The name looked up is the text `Add_None` with the flag
//!    set, else the node's own `AddAnimSeqName`; None looks nothing up. Then the lookup's gate, read without a load:
//!    the node's `SkelComponent` set (and resident), its `SkeletalMesh` set, its `AnimSets` not empty. Then the lookup
//!    ([`lookup`]): a **match** is [`LINKUP`].
//!
//! **The writes** come after the whole preflight, so no refusal follows one: the component's `InitTag`; then per
//! sequence node in tick order: the reset (`AnimSeqName` None, `AnimSeq` None, `AnimLinkupIndex` -1) unless the guard
//! holds; for an additive node, with the auto-name flag `Add_None` interned and written to `AddAnimSeqName`, then
//! `AddAnimSeqName` := that name (or the node's own), `AddAnimSeq` None, `AddAnimLinkupIndex` -1; then `AnimSeqName` :=
//! the kept name and `CachedNumDesiredBones` := 0. A write that leaves a field at its value still counts, as in the
//! game.

use ow_core::NotYet;
use ow_object::ids::{NameId, ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::skeletal;

/// The analyst note.
pub const RECORD: &str = "docs/verification/NATIVE_UPDATEANIMATIONS_TREE_TICKS.md";
const UNIT: &str = "UpdateAnimations";

/// The refusal texts (the note's section 5).
pub const TICK_NODE_NOT_LOADED: &str = "UpdateAnimations: a tick node not loaded at set-up (a load is a write)";
pub const NOT_LOADED: &str = "UpdateAnimations: an object the store has not loaded (a load is a write)";
pub const AUTO_NAME_ISSUING: &str = "UpdateAnimations: the additive auto-name while issuing notifies";
pub const SEQUENCE_CHANGES: &str = "UpdateAnimations: the steps after a sequence node's sequence changes";
pub const LINKUP: &str = "UpdateAnimations: an additive sequence's mesh link-up";

/// `"UpdateAnimations: the per-node step of <class>"`.
pub fn per_node_step(class: &str) -> String {
    format!("UpdateAnimations: the per-node step of {class}")
}

/// The auto-built additive name (`"Add_"` and the asked name's text, None here).
pub const AUTO_NAME: &str = "Add_None";

/// The 28 classes whose step is empty, each checked in its own class table (the note's section 5).
pub const EMPTY_STEP: [&str; 28] = [
    "Engine.AnimNode",
    "Engine.AnimNodeBlendBase",
    "Engine.AnimTree",
    "WillowGame.WillowAnimTree",
    "Engine.AnimNodeBlend",
    "Engine.AnimNodeBlendList",
    "Engine.AnimNodeBlendBySpeed",
    "Engine.AnimNodeAdditiveBlending",
    "Engine.AnimNodeScaleRateBySpeed",
    "GearboxFramework.AnimNodeSpecialMoveBlend",
    "WillowGame.WillowAnimNodeFeatherBlend",
    "Engine.AnimNodeBlendDirectional",
    "Engine.AnimNodeBlendByPosture",
    "Engine.AnimNodeBlendByPhysics",
    "Engine.AnimNodeBlendPerBone",
    "Engine.AnimNodeAimOffset",
    "WillowGame.WillowAnimNodeAimOffset",
    "Engine.AnimNodeSlot",
    "WillowGame.WillowAnimNodeSlot",
    "WillowGame.WillowAnimNodeBlendDirectional",
    "WillowGame.WillowAnimNodeBlendInjured",
    "WillowGame.WillowAnimNodeBlendThirdPersonMenu",
    "WillowGame.WillowAnimNode_Falling",
    "WillowGame.WillowAnimNode_AddCameraBone",
    "WillowGame.WillowAnimNode_AimState",
    "WillowGame.WillowAnimNode_ClimbLadder",
    "WillowGame.WillowAnimNode_Condition",
    "WillowGame.WillowAnimNode_WeaponRecoil",
];
/// The sequence family's plain classes, and the additive one.
pub const PLAIN_SEQUENCE: [&str; 2] = ["Engine.AnimNodeSequence", "WillowGame.WillowAnimNodeSequence"];
pub const ADDITIVE_SEQUENCE: &str = "WillowGame.WillowAnimNodeSequenceAdditiveBlend";

fn script(message: String) -> VmError {
    VmError::Script { function: UNIT.to_string(), message: format!("{UNIT}: {message}") }
}

fn not_read(what: impl Into<String>) -> VmError {
    NotYet::not_read(what.into(), RECORD).into()
}

fn class_path(vm: &Vm, o: ObjRef) -> Result<String, VmError> {
    let class = vm.space.heap.get(o)?.class;
    Ok(vm.space.classes.get_layout(class).map_or_else(|| format!("{class:?}"), |l| l.path().as_str().to_string()))
}

/// The kinds the step's fields are declared with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Want {
    Name,
    Object,
    Int,
    Bool,
    Byte,
    Array,
}

/// Field `name` of `o`, declared with `want`, else a data error naming it; its value, read without a load.
fn declared(vm: &Vm, o: ObjRef, name: &str, want: Want) -> Result<Value, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{} declares no {name}", class_path(vm, o).unwrap_or_default())))?;
    let kind = vm.space.classes.decl(p).map(|d| &d.kind);
    let fits = matches!(
        (want, kind),
        (Want::Name, Some(PropKind::Name))
            | (Want::Object, Some(PropKind::Object { .. }))
            | (Want::Int, Some(PropKind::Int))
            | (Want::Bool, Some(PropKind::Bool))
            | (Want::Byte, Some(PropKind::Byte { .. }))
            | (Want::Array, Some(PropKind::Array(_)))
    );
    if !fits {
        return Err(script(format!("{}.{name} is not declared as {want:?}", class_path(vm, o)?)));
    }
    Ok(vm.space.read(o, PropAddr::scalar(p))?.clone())
}

fn object(v: &Value) -> Option<ObjRef> {
    match v {
        Value::Object(o) | Value::Interface(o) => *o,
        _ => None,
    }
}

fn name_of(v: &Value) -> NameId {
    match v {
        Value::Name(n) => *n,
        _ => NameId::NONE,
    }
}

fn int_of(v: &Value) -> i32 {
    match v {
        Value::Int(i) => *i,
        _ => 0,
    }
}

fn elements(v: &Value, what: &str) -> Result<Vec<Value>, VmError> {
    v.elements().map(<[Value]>::to_vec).ok_or_else(|| script(format!("{what} holds {}, not an array", v.kind_name())))
}

fn resident(vm: &Vm, o: ObjRef, refusal: &'static str) -> Result<(), VmError> {
    if !skeletal::resident(vm, o)? || vm.defs.is_failed(&vm.space, o) {
        return Err(not_read(refusal));
    }
    Ok(())
}

/// One sequence node's planned writes.
#[derive(Debug, Clone, PartialEq)]
struct SequenceStep {
    node: ObjRef,
    kept: Value,
    /// The reset runs (the guard does not hold).
    reset: bool,
    /// For an additive node: whether the auto-name flag is set.
    additive: Option<bool>,
}

/// What the step writes, every check passed.
#[derive(Debug, Clone, PartialEq)]
pub struct Plan {
    tag: i32,
    steps: Vec<SequenceStep>,
}

/// The component's sequence lookup of the name `text` for the asking `node` (the note's section 4), read only: the
/// component's `AnimSets` last to first (first to last with the node's `bReverseAnimSeqSearchOrder` set), None entries
/// skipped; in each set, its `Sequences` in order, None skipped, each resident, matched by `SequenceName` text as names
/// compare (case-insensitively). The set's native name map is replaced by this walk (**UNVERIFIED**: the map's
/// agreement with the list is a fit). The first set that answers gives the answer.
fn lookup(vm: &Vm, component: ObjRef, reverse: bool, text: &str) -> Result<Option<ObjRef>, VmError> {
    let mut sets = elements(&declared(vm, component, "AnimSets", Want::Array)?, "AnimSets")?;
    if !reverse {
        sets.reverse();
    }
    for set in sets.iter().filter_map(object) {
        resident(vm, set, NOT_LOADED)?;
        for sequence in elements(&declared(vm, set, "Sequences", Want::Array)?, "Sequences")?.iter().filter_map(object) {
            resident(vm, sequence, NOT_LOADED)?;
            let name = name_of(&declared(vm, sequence, "SequenceName", Want::Name)?);
            if name != NameId::NONE && vm.space.names.text(name).eq_ignore_ascii_case(text) {
                return Ok(Some(sequence));
            }
        }
    }
    Ok(None)
}

/// The additive override's preflight (module documentation, step 5).
fn additive(vm: &Vm, node: ObjRef, guard: bool) -> Result<bool, VmError> {
    let auto = matches!(declared(vm, node, "bAutoGenerateAdditiveSeqName", Want::Bool)?, Value::Bool(true));
    let own = name_of(&declared(vm, node, "AddAnimSeqName", Want::Name)?);
    declared(vm, node, "AddAnimSeq", Want::Object)?;
    declared(vm, node, "AddAnimLinkupIndex", Want::Int)?;
    let reverse = matches!(declared(vm, node, "bReverseAnimSeqSearchOrder", Want::Bool)?, Value::Bool(true));
    if auto && guard {
        return Err(not_read(AUTO_NAME_ISSUING));
    }
    let text = if auto {
        AUTO_NAME.to_string()
    } else if own == NameId::NONE {
        return Ok(auto);
    } else {
        vm.space.names.text(own).into_owned()
    };
    // The gate, read without a load.
    let Some(component) = object(&declared(vm, node, "SkelComponent", Want::Object)?) else {
        return Ok(auto);
    };
    resident(vm, component, NOT_LOADED)?;
    if object(&declared(vm, component, "SkeletalMesh", Want::Object)?).is_none() {
        return Ok(auto);
    }
    if elements(&declared(vm, component, "AnimSets", Want::Array)?, "AnimSets")?.is_empty() {
        return Ok(auto);
    }
    if lookup(vm, component, reverse, &text)?.is_some() {
        return Err(not_read(LINKUP));
    }
    Ok(auto)
}

/// The preflight (module documentation): `Ok(None)` with no tree. Reads only.
pub fn plan(vm: &Vm, component: ObjRef) -> Result<Option<Plan>, VmError> {
    let Some(root) = object(&declared(vm, component, "Animations", Want::Object)?) else {
        return Ok(None);
    };
    resident(vm, root, NOT_LOADED)?;
    let tag = int_of(&declared(vm, root, "NodeInitTag", Want::Int)?);
    declared(vm, component, "InitTag", Want::Int)?;
    let ticks = elements(&declared(vm, component, "AnimTickArray", Want::Array)?, "AnimTickArray")?;
    let mut steps = Vec::new();
    for node in ticks.iter().filter_map(object) {
        if !skeletal::resident(vm, node)? || vm.defs.is_failed(&vm.space, node) {
            return Err(not_read(TICK_NODE_NOT_LOADED));
        }
        let path = class_path(vm, node)?;
        if EMPTY_STEP.contains(&path.as_str()) {
            continue;
        }
        let is_additive = path == ADDITIVE_SEQUENCE;
        if !is_additive && !PLAIN_SEQUENCE.contains(&path.as_str()) {
            return Err(not_read(per_node_step(&path)));
        }
        let kept = declared(vm, node, "AnimSeqName", Want::Name)?;
        let sequence = object(&declared(vm, node, "AnimSeq", Want::Object)?);
        declared(vm, node, "AnimLinkupIndex", Want::Int)?;
        let issuing = matches!(declared(vm, node, "bIsIssuingNotifies", Want::Bool)?, Value::Bool(true));
        declared(vm, node, "CachedNumDesiredBones", Want::Byte)?;
        let guard = issuing && name_of(&kept) != NameId::NONE;
        if !guard && sequence.is_some() {
            return Err(not_read(SEQUENCE_CHANGES));
        }
        let additive = if is_additive { Some(additive(vm, node, guard)?) } else { None };
        steps.push(SequenceStep { node, kept, reset: !guard, additive });
    }
    Ok(Some(Plan { tag, steps }))
}

fn write(vm: &mut Vm, o: ObjRef, name: &str, v: Value) -> Result<(), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("no {name} to write")))?;
    vm.space.write(o, PropAddr::scalar(p), v)?;
    Ok(())
}

/// The writes of a planned step (module documentation), on `component`.
pub fn commit(vm: &mut Vm, component: ObjRef, plan: Plan) -> Result<(), VmError> {
    write(vm, component, "InitTag", Value::Int(plan.tag.wrapping_add(1)))?;
    for step in plan.steps {
        let node = step.node;
        if step.reset {
            write(vm, node, "AnimSeqName", Value::Name(NameId::NONE))?;
            write(vm, node, "AnimSeq", Value::Object(None))?;
            write(vm, node, "AnimLinkupIndex", Value::Int(-1))?;
        }
        if let Some(auto) = step.additive {
            if auto {
                let name = vm.space.names.intern(AUTO_NAME);
                write(vm, node, "AddAnimSeqName", Value::Name(name))?;
            }
            let name = vm.space.read(node, PropAddr::scalar(prop(vm, node, "AddAnimSeqName")?))?.clone();
            write(vm, node, "AddAnimSeqName", name)?;
            write(vm, node, "AddAnimSeq", Value::Object(None))?;
            write(vm, node, "AddAnimLinkupIndex", Value::Int(-1))?;
        }
        write(vm, node, "AnimSeqName", step.kept)?;
        write(vm, node, "CachedNumDesiredBones", Value::Byte(0))?;
    }
    Ok(())
}

fn prop(vm: &Vm, o: ObjRef, name: &str) -> Result<ow_object::ids::PropId, VmError> {
    let class = vm.space.heap.get(o)?.class;
    vm.space.prop(class, name).ok_or_else(|| script(format!("no {name}")))
}

#[cfg(test)]
#[path = "tree_ticks_tests.rs"]
mod tests;
