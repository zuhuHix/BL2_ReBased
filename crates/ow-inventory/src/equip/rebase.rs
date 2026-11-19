//! Owner: lane IMPL-RECOMPUTE-ATTRIBUTE-BASE-VALUES. AI-assisted (Claude Code). `WillowWeapon.RecomputeAttributeBaseValues()`,
//! implemented from NATIVE_RECOMPUTE_ATTRIBUTE_BASE_VALUES.md alone (PR #539 at 31d529e5, accepted; this lane's decision
//! 2026-11-19-NNN). Every rule is the record's reading, **UNVERIFIED** in the running game.
//!
//! **What it does** (the record's section 3): it bakes each modified value into its base. For every stack-attribute
//! property of the weapon's runtime class, **inherited ones included** (the hub's ruling in the record: 41 triples on
//! a `WillowWeapon`, 34 own and 7 from `Engine.Weapon`; the reach comes from the live record and is UNVERIFIED):
//! 1. its modifier stack (`XModifierStack`) becomes the empty array (the modifiers are not destroyed);
//! 2. its base (`XBaseValue`) becomes the value's stored value, unchanged, as a plain property write: nothing is
//!    recomputed and no attribute-change notification runs. This is not `ow-attr`'s set-base, which recomputes,
//!    notifies and goes through an `f32` (the record's section 4, item 4).
//!
//! Then `WeaponAttributeModifiers` becomes the empty array. The properties are walked in `ClassTable::props` order
//! (inherited first, slot order); the writes touch disjoint fields, so their order cannot be observed.
//!
//! **No definition read, no evaluation, no random draw, no load** (section 3). The native has no `NotYet`: every rule
//! is read (section 4, item 6). A data shape the record does not cover is a `VmError::Script` naming the field, raised
//! by the preflight before the first write:
//! - the class declares no `WeaponAttributeModifiers`, or not as a dynamic array;
//! - a stack-attribute property with no linked companions, a stack companion that is not an array, or a base whose
//!   numeric kind (float, int or byte; an attribute kind counts as its plain kind) differs from the value's;
//! - a static-array attribute (the record reads scalars only).
//!
//! Only a `NotYet` takes the "stopped after partial writes: " mark (the hub's rule); this native raises none, and its
//! writes come only after the preflight, so no partial-writes mark arises here.
//!
//! **Only in `ParityMode::Off`.** The C++ reference has no stand-in, so under Reference the native stays unbound and
//! the stub policy answers it (architecture R3). `WillowItem.RecomputeAttributeBaseValues` is not bound: its body was
//! not reached (the record's section 3), and it is not on link 8's path.

use std::rc::Rc;

use ow_core::ParityMode;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use super::bad;

/// The record this native follows.
pub const RECORD: &str = "NATIVE_RECOMPUTE_ATTRIBUTE_BASE_VALUES.md";

/// The native key [`register`] binds in Off.
pub const RECOMPUTE_BASE_VALUES: &str = "WillowWeapon.RecomputeAttributeBaseValues()";

/// The applied-effect records the native empties after the attributes.
const APPLIED_EFFECTS: &str = "WeaponAttributeModifiers";

/// A numeric kind as the base copy compares it: an attribute kind counts as its plain kind (section 4, item 2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Num {
    Float,
    Int,
    Byte,
}

fn num(kind: &PropKind) -> Option<Num> {
    match kind {
        PropKind::Float | PropKind::FloatAttribute => Some(Num::Float),
        PropKind::Int | PropKind::IntAttribute => Some(Num::Int),
        PropKind::Byte { .. } | PropKind::ByteAttribute { .. } => Some(Num::Byte),
        _ => None,
    }
}

/// One planned write: the address and the value it takes.
type Write = (PropAddr, Value);

/// The preflight and the plan: every write, in order, read-only (nothing is written or loaded here).
fn plan(vm: &Vm, weapon: ObjRef) -> Result<Vec<Write>, VmError> {
    let what = RECOMPUTE_BASE_VALUES;
    let class = vm.space.heap.get(weapon)?.class;
    let classes = &vm.space.classes;
    let empty = Value::Array(Rc::new(Vec::new()));
    // The applied-effect records: a dynamic array the class declares.
    let applied = vm
        .space
        .prop(class, APPLIED_EFFECTS)
        .ok_or_else(|| bad(what, format!("the weapon's class declares no {APPLIED_EFFECTS}")))?;
    match classes.decl(applied).map(|d| &d.kind) {
        Some(PropKind::Array(_)) => {}
        _ => return Err(bad(what, format!("{APPLIED_EFFECTS} is not a dynamic array"))),
    }
    let mut writes = Vec::new();
    let props = classes.props(class).map_err(|e| bad(what, format!("the weapon's class layout: {e:?}")))?;
    for (value_prop, decl) in props {
        if !decl.is_stack_attribute() {
            continue;
        }
        let name = vm.space.names.text(decl.name).to_string();
        let Some(c) = decl.attr else {
            return Err(bad(what, format!("the stack attribute {name} has no linked companions")));
        };
        if decl.array_dim != 1 {
            return Err(bad(what, format!("the stack attribute {name} is a static array (the record reads scalars)")));
        }
        let (Some(base), Some(stack)) = (classes.decl(c.base), classes.decl(c.stack)) else {
            return Err(bad(what, format!("a companion of {name} is not a property of the class")));
        };
        if !matches!(stack.kind, PropKind::Array(_)) {
            return Err(bad(what, format!("the modifier stack of {name} is not an array")));
        }
        let (Some(v), Some(b)) = (num(&decl.kind), num(&base.kind)) else {
            return Err(bad(what, format!("{name} or its base is not a float, int or byte")));
        };
        if v != b {
            return Err(bad(what, format!("the base of {name} holds a different numeric kind than its value")));
        }
        let value = vm.space.read(weapon, PropAddr::scalar(value_prop))?.clone();
        writes.push((PropAddr::scalar(c.stack), empty.clone()));
        writes.push((PropAddr::scalar(c.base), value));
    }
    writes.push((PropAddr::scalar(applied), empty));
    // Every staged value against its destination's declaration, before the first write.
    for (a, v) in &writes {
        let kind = &classes.decl(a.prop).ok_or_else(|| bad(what, "a destination has no declaration"))?.kind;
        vm.space.heap.check_value(classes, kind, v)?;
    }
    Ok(writes)
}

/// The native's body: the preflight and plan, then the writes in order.
fn recompute(vm: &mut Vm, weapon: ObjRef) -> Result<(), VmError> {
    let writes = plan(vm, weapon)?;
    for (a, v) in writes {
        vm.space.write(weapon, a, v)?;
    }
    Ok(())
}

/// Binds the native in `ParityMode::Off`; Reference binds nothing (module documentation).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let f: NativeFn = Rc::new(|c: &mut NativeCall<'_>| {
        let weapon = c.this.ok_or_else(|| bad(RECOMPUTE_BASE_VALUES, "called without a weapon"))?;
        recompute(c.vm, weapon)?;
        Ok(Value::None)
    });
    vm.register_native(RECOMPUTE_BASE_VALUES, f)
}

#[cfg(test)]
#[path = "rebase_tests.rs"]
mod tests;
