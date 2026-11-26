//! Owner: lane ADDITIVE-OVERRIDE-HELPER (a refactor, no behaviour change). The pieces of
//! `WillowAnimNodeSequenceAdditiveBlend`'s override that `super::tree_ticks` (`UpdateAnimations`' per-node step,
//! #751) and `super::anim_node_init` (kind D's corrections, #752) both run, and the one residency test they and
//! `super::aim_offset` share. AI-assisted (Claude Code). **Every rule is UNVERIFIED in the running game**, as in the
//! notes behind its callers (NATIVE_UPDATEANIMATIONS_TREE_TICKS.md sections 3 to 5, NATIVE_ANIM_NODE_INIT.md D).
//!
//! Each caller keeps its own refusal texts, field checks and the order in which it reads its fields: the lookup takes
//! them as [`Reads`], and its search order as a flag the caller has read.

use ow_object::ids::{NameId, ObjRef};
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

/// Whether `o` is resident: neither a handle the store has not loaded (or whose load failed) nor a stand-in with fewer
/// slots than its class lays out. Reads only.
pub(crate) fn resident(vm: &Vm, o: ObjRef) -> Result<bool, VmError> {
    let x = vm.space.heap.get(o)?;
    let slotless = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    Ok(!(vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || slotless))
}

/// How a caller reads the lookup's fields and refuses an object not resident, with its own texts.
pub(crate) struct Reads<'a> {
    /// Array field `name` of `o`, its elements.
    pub array: &'a dyn Fn(&Vm, ObjRef, &str) -> Result<Vec<Value>, VmError>,
    /// Name field `name` of `o`.
    pub name: &'a dyn Fn(&Vm, ObjRef, &str) -> Result<NameId, VmError>,
    /// The refusal of `o`, not resident, in the `role` named ("the anim set", "the anim sequence").
    pub not_loaded: &'a dyn Fn(&Vm, ObjRef, &str) -> VmError,
}

fn object(v: &Value) -> Option<ObjRef> {
    match v {
        Value::Object(o) | Value::Interface(o) => *o,
        _ => None,
    }
}

/// The component's sequence lookup of the name text `target` over the anim sets `sets` (the component's `AnimSets`,
/// as the caller read them), read only (NATIVE_UPDATEANIMATIONS_TREE_TICKS.md section 4): last to first, first to
/// last when `in_order` (the node's `bReverseAnimSeqSearchOrder`); None entries skipped; each set resident, then its
/// `Sequences` in order, None skipped, each resident, matched by `SequenceName` text as names compare
/// (case-insensitively; the name None never matches). The set's native name map is replaced by this walk
/// (**UNVERIFIED**). The first match is the answer.
pub(crate) fn find_sequence(
    vm: &Vm,
    reads: &Reads<'_>,
    mut sets: Vec<Value>,
    in_order: bool,
    target: &str,
) -> Result<Option<ObjRef>, VmError> {
    if !in_order {
        sets.reverse();
    }
    for set in sets.iter().filter_map(object) {
        if !resident(vm, set)? {
            return Err((reads.not_loaded)(vm, set, "the anim set"));
        }
        for sequence in (reads.array)(vm, set, "Sequences")?.iter().filter_map(object) {
            if !resident(vm, sequence)? {
                return Err((reads.not_loaded)(vm, sequence, "the anim sequence"));
            }
            let name = (reads.name)(vm, sequence, "SequenceName")?;
            if name != NameId::NONE && vm.space.names.text(name).eq_ignore_ascii_case(target) {
                return Ok(Some(sequence));
            }
        }
    }
    Ok(None)
}

/// The override's writes on `node` after its base step (NATIVE_UPDATEANIMATIONS_TREE_TICKS.md section 3): with the
/// auto-name flag `auto`, `auto_name` interned (only here) and written to `AddAnimSeqName`; then the re-set:
/// `AddAnimSeqName` := that name (or the node's own), `AddAnimSeq` := None, `AddAnimLinkupIndex` := -1. Through the
/// caller's `write` and `read`, in that order.
pub(crate) fn write_override(
    vm: &mut Vm,
    node: ObjRef,
    auto: bool,
    auto_name: &str,
    read: &dyn Fn(&Vm, ObjRef, &str) -> Result<Value, VmError>,
    write: &dyn Fn(&mut Vm, ObjRef, &str, Value) -> Result<(), VmError>,
) -> Result<(), VmError> {
    let name = if auto {
        let n = Value::Name(vm.space.names.intern(auto_name));
        write(vm, node, "AddAnimSeqName", n.clone())?;
        n
    } else {
        read(vm, node, "AddAnimSeqName")?
    };
    write(vm, node, "AddAnimSeqName", name)?;
    write(vm, node, "AddAnimSeq", Value::Object(None))?;
    write(vm, node, "AddAnimLinkupIndex", Value::Int(-1))
}
