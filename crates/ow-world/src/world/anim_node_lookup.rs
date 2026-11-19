//! Owner: lane IMPL-ANIM-NODE-LOOKUP (decision 2026-11-19-NNN, number pending). The new `Mesh`'s animation-node and
//! skeletal-control lookups and the slot-node scan of the pawn's `PostInitBodyComposition`, as a check of the body
//! composition's write-free preflight (crate::world::body_composition), in `ParityMode::Off`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from `docs/verification/NATIVE_ANIM_NODE_LOOKUP.md` alone (analyst lane
//! ANALYST-ANIM-NODE-LOOKUP, PR #543 at b09720f6) and the hub's rulings; this lane did not read the executable. **Every
//! rule is UNVERIFIED in the running game.** Reference is unchanged (the body composition is Off only).
//!
//! It replaces #529's bullet "identifier `Mesh` with a made `SkeletalMeshComponent` is `LayerAbsent`" (section 7, step
//! 4) and runs after that step's release check for the old `Pawn.Mesh`. The port has no instanced animation tree, so
//! the lookups run against the **template** and only settle the answers that need no instance (the note's section 5):
//! 1. What is read: the attachment's template component (`Data.ComponentData.Component`; the new component does not
//!    exist yet), its `AnimTreeTemplate`, the tree's nodes (the closure of the root's `Children`, each child's `Anim`)
//!    and controls (`SkelControlLists` in order, each list's `ControlHead`, then `NextControl`). Values are read as
//!    they stand, so a template the store loaded has its class defaults applied.
//! 2. **Nothing is loaded** (hub ruling, review r1 of #543: a lazy load is a write under #518). An unloaded or failed
//!    template component, tree, node or control, or an unloaded body class whose names a lookup needs, is #529's
//!    refusal ([`refusal`]), raised with nothing loaded.
//! 3. Node lookups take the first match in the saved `TickArrayIndex` order. Saved values that are not a full, distinct
//!    order 0 to n - 1 over the nodes are `NotYet { NotRead }` ([`TICK_ORDER`]). A None node name matches the first
//!    node whose `NodeName` is None (the root, when it has none). A None control name answers None with no search.
//!    The names are compared whole (name and number). By fit, the copy the game makes keeps each node's class and
//!    name, so the template's names stand for the instance's (the note's open question 3).
//! 4. A lookup that finds nothing, or an object not of its "kept only as" class, settles its field as None;
//!    `bUsingFootIK` settles as whether a `WillowFootIK` control exists; `SlotNodes` settles as empty when no node is an
//!    `AnimNodeSlot`. Anything else needs the instance's node and is #529's refusal. No `AnimTreeTemplate` settles all
//!    four lookups as None, `bUsingFootIK` false and `SlotNodes` empty.
//!
//! The check writes nothing. Its answer is [`Settled`]; the rest of the made-`Mesh` step is the caller's.

use ow_core::NotYet;
use ow_object::ids::{ClassId, NameId, ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::body_composition::{ANIM_LAYER, ANIM_NODES, field};

/// The analyst note.
pub const RECORD: &str = "docs/verification/NATIVE_ANIM_NODE_LOOKUP.md";
/// The record of #529's refusal: the body composition's section 7, with this note (the note's rule 4).
pub const REFUSAL_RECORD: &str =
    "docs/verification/NATIVE_INITIALIZE_BODY_COMPOSITION.md (section 7) and docs/verification/NATIVE_ANIM_NODE_LOOKUP.md";
/// The saved tick order is not a full, distinct order (rule 3).
pub const TICK_ORDER: &str = "InitializeBodyComposition: the template's saved tick order";
/// The AimOffset lookup with no body class: the note leaves it unread (unreachable from the pawn wrapper, whose gate
/// needs a loaded body class).
pub const NO_BODY_CLASS: &str = "InitializeBodyComposition: the AimOffset node name with no BodyClass";

/// The answers the template settles (rules 4 and 5): the four lookups are None and `SlotNodes` is empty.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Settled {
    /// `bUsingFootIK`: whether a `WillowFootIK` control exists.
    pub foot_ik: bool,
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

/// `o`, refused unless readable (rule 2).
fn need(vm: &Vm, o: ObjRef) -> Result<ObjRef, VmError> {
    if readable(vm, o) { Ok(o) } else { Err(refusal()) }
}

/// Property `name` of `o` as it stands; its class must declare it.
fn get(vm: &Vm, o: ObjRef, name: &str) -> Result<Value, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{} has no {name}", class_text(vm, class))))?;
    Ok(vm.space.read(o, PropAddr::scalar(p))?.clone())
}

fn has(vm: &Vm, o: ObjRef, name: &str) -> Result<bool, VmError> {
    let class = vm.space.heap.get(o)?.class;
    Ok(vm.space.prop(class, name).is_some())
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

/// The tree as the template holds it: nodes in saved tick order, controls in list order.
struct Tree {
    nodes: Vec<ObjRef>,
    controls: Vec<ObjRef>,
}

fn read_tree(vm: &Vm, root: ObjRef) -> Result<Tree, VmError> {
    // The closure of the root's Children (each child's Anim, then its own Children).
    let mut nodes = vec![need(vm, root)?];
    let mut next = 0;
    while next < nodes.len() {
        let node = nodes[next];
        next += 1;
        if !has(vm, node, "Children")? {
            continue;
        }
        let children = get(vm, node, "Children")?;
        let children = children.elements().ok_or_else(|| script("Children is not an array".into()))?.to_vec();
        for child in &children {
            if let Some(anim) = object(&field(vm, child, "Anim", "AnimBlendChild")?)
                && !nodes.contains(&anim)
            {
                nodes.push(need(vm, anim)?);
            }
        }
    }
    // Rule 3: the saved TickArrayIndex values are a full, distinct order 0 to n - 1.
    let mut order: Vec<Option<ObjRef>> = vec![None; nodes.len()];
    for &node in &nodes {
        let i = match get(vm, node, "TickArrayIndex")? {
            Value::Int(i) => i,
            other => return Err(script(format!("TickArrayIndex holds {}, not an int", other.kind_name()))),
        };
        match usize::try_from(i).ok().and_then(|i| order.get_mut(i)) {
            Some(slot @ None) => *slot = Some(node),
            _ => return Err(NotYet::not_read(TICK_ORDER, RECORD).into()),
        }
    }
    let nodes = order.into_iter().map(|n| n.expect("a full order fills every slot")).collect();
    // The controls: only an AnimTree has control lists.
    let mut controls = Vec::new();
    if is_kind(vm, root, "AnimTree")? {
        let lists = get(vm, root, "SkelControlLists")?;
        let lists = lists.elements().ok_or_else(|| script("SkelControlLists is not an array".into()))?.to_vec();
        for list in &lists {
            let mut cursor = object(&field(vm, list, "ControlHead", "SkelControlListHead")?);
            while let Some(control) = cursor {
                if controls.contains(&control) {
                    return Err(script("a skeletal control list that loops back on itself".into()));
                }
                controls.push(need(vm, control)?);
                cursor = object(&get(vm, control, "NextControl")?);
            }
        }
    }
    Ok(Tree { nodes, controls })
}

/// The first node named `name` (a None name included), in saved tick order.
fn find_node(vm: &Vm, tree: &Tree, name: NameId) -> Result<Option<ObjRef>, VmError> {
    for &n in &tree.nodes {
        if name_of(vm, n, "NodeName")? == name {
            return Ok(Some(n));
        }
    }
    Ok(None)
}

/// The first control named `name`; a None name answers None with no search.
fn find_control(vm: &Vm, tree: &Tree, name: NameId) -> Result<Option<ObjRef>, VmError> {
    if name == NameId::NONE {
        return Ok(None);
    }
    for &c in &tree.controls {
        if name_of(vm, c, "ControlName")? == name {
            return Ok(Some(c));
        }
    }
    Ok(None)
}

/// The id of a fixed name, or `None` when no such name exists (then nothing can match it).
fn fixed(vm: &Vm, text: &str) -> Option<NameId> {
    vm.space.names.find(text)
}

/// A lookup that settles only when it finds nothing kept (rule 4).
fn settle(vm: &Vm, found: Option<ObjRef>, kept_as: &str) -> Result<(), VmError> {
    match found {
        Some(o) if is_kind(vm, o, kept_as)? => Err(refusal()),
        _ => Ok(()),
    }
}

/// The check (module documentation) for the made `Mesh` from `template`, with the pawn's `body` class: reads only,
/// writes and loads nothing.
pub fn check(vm: &Vm, template: ObjRef, body: Option<ObjRef>) -> Result<Settled, VmError> {
    let template = need(vm, template)?;
    let Some(root) = object(&get(vm, template, "AnimTreeTemplate")?) else {
        // Rule 5: no tree.
        return Ok(Settled { foot_ik: false });
    };
    let tree = read_tree(vm, root)?;
    let body = match body {
        Some(b) => Some(need(vm, b)?),
        None => None,
    };
    if let Some(n) = fixed(vm, "ArmBoneBlend") {
        settle(vm, find_node(vm, &tree, n)?, "AnimNodeBlendPerBone")?;
    }
    let Some(b) = body else {
        return Err(NotYet::not_read(NO_BODY_CLASS, RECORD).into());
    };
    let aim = name_of(vm, b, "AimOffsetNodeName")?;
    settle(vm, find_node(vm, &tree, aim)?, "WillowAnimNodeAimOffset")?;
    if let Some(n) = fixed(vm, "LeftHandGrip") {
        settle(vm, find_control(vm, &tree, n)?, "SkelControlLeftHandGripWeapon")?;
    }
    let head = name_of(vm, b, "HeadLookAtName")?;
    settle(vm, find_control(vm, &tree, head)?, "WillowSkelControl_LookAtActor")?;
    let foot_ik = match fixed(vm, "WillowFootIK") {
        Some(n) => find_control(vm, &tree, n)?.is_some(),
        None => false,
    };
    for &n in &tree.nodes {
        if is_kind(vm, n, "AnimNodeSlot")? {
            return Err(refusal());
        }
    }
    Ok(Settled { foot_ik })
}

// Tested with the body composition's fixture (body_composition_tests.rs, "the animation-node lookups").
