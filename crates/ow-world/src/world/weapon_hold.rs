//! Owner: lane IMPL-GETWEAPONHOLDDEF. `BodyClassDefinition.GetWeaponHoldDef(HoldName, EnvironmentTag,
//! bSearchParentClass, bFallbackToDefaultDef)` in `ParityMode::Off`: link 1's stop in `maya:8`
//! (`WillowPawn.UpdateAnimSets` through `GetWeaponHoldAnimSets`), and links 3, 4, 5, 13 and 14's. AI-assisted (Claude
//! Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_GET_WEAPON_HOLD_DEF.md section 3 alone (lane
//! ANALYST-GETWEAPONHOLDDEF, #728 at its review fix 7e0ed56c); this lane did not read the executable.
//! **Every rule is UNVERIFIED in the running game.** Reference keeps the stub policy.
//!
//! **A pure read:** no load, no class-code load, nothing interned, nothing written. On body class B with name N, tag
//! T, search-parent P and fallback F:
//! 1. **the receiver** is resident: a store handle not loaded yet (`DefStore::is_unloaded` / `is_failed`, or a slotless
//!    stand-in) is [`NOT_LOADED`] for a body class (`NotRead`), before any field read. Its class declares
//!    `WeaponHoldDefs` (an array of object references), `DefaultWeaponHoldDef` and `WeaponActionParentClass` (object
//!    references); otherwise `VmError::Script` naming the field;
//! 2. **the own list**, in order, None entries skipped. Each entry is resident (the same refusal, for "a weapon hold
//!    definition", before any field of it is read) and declares `HoldName` (a name) and `EnvironmentTag` (an object
//!    reference). Among entries named N: one whose tag is T is the answer at once (the exact match). Otherwise the
//!    first name match is the candidate, replaced by a later name match only when that entry's tag is None. Names
//!    compare by id, objects by identity;
//! 3. **the parent**, when P is true and `WeaponActionParentClass` is set (the receiver of the parent lookup is that
//!    field, by fit): the same lookup on the parent (with the same residency check), with N, T, P, and F passed on
//!    only when B has no `DefaultWeaponHoldDef`. With a candidate, the parent's answer stands only when it is an exact
//!    match, otherwise (or when it is None) the candidate. With no candidate, the parent's answer when it is not None.
//!    **"An exact match" is a field read** (the note's fix, #728 at 7e0ed56c): only when this class has a candidate
//!    and the parent answers non-None, the returned hold's own `HoldName` and `EnvironmentTag` are re-read and compared
//!    with N and T; the recursion does not report how it matched. So a parent default whose name and tag equal N and T
//!    stands over the candidate. The returned hold gets the same residency check before its fields are read (it can be
//!    the parent's `DefaultWeaponHoldDef`, returned unread);
//! 4. **the candidate**, when there is one;
//! 5. **the default**: `DefaultWeaponHoldDef` when F is true, else None.
//!
//! A parent chain that comes back to a class already visited is `VmError::Script`: a port-only guard (the note: the
//! game's routine has none).

use std::rc::Rc;

use ow_core::NotYet;
use ow_object::ids::{NameId, ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

/// The record of the rule.
pub const RECORD: &str = "docs/verification/NATIVE_GET_WEAPON_HOLD_DEF.md";
/// The native key.
pub const KEY: &str = "BodyClassDefinition.GetWeaponHoldDef(name,object,bool,bool)";
/// `GetFirstPersonWeaponHoldDef`'s key: the same rule on the first-person fields (NATIVE_LINK1_RBPHYSICS_AND_AFTER.md
/// 2.C; lane IMPL-LINK1-RBPHYSICS).
pub const FIRST_PERSON_KEY: &str = "BodyClassDefinition.GetFirstPersonWeaponHoldDef(name,object,bool,bool)";
/// A body class the store has not loaded.
pub const NOT_LOADED: &str = "GetWeaponHoldDef on a body class not loaded at set-up (a load is a write)";
/// A weapon hold definition the store has not loaded.
pub const HOLD_NOT_LOADED: &str =
    "GetWeaponHoldDef over a weapon hold definition not loaded at set-up (a load is a write)";
const WHAT: &str = "BodyClassDefinition.GetWeaponHoldDef";

fn script(message: String) -> VmError {
    VmError::Script { function: WHAT.to_string(), message }
}

/// `o` is resident (neither a handle the store has not loaded nor a slotless stand-in), else `what`'s `NotRead`.
fn resident(vm: &Vm, o: ObjRef, what: &'static str) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let slotless = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || slotless {
        return Err(NotYet::not_read(what, RECORD).into());
    }
    Ok(())
}

/// The value of `o`'s field `name`, whose declaration must satisfy `kind_ok` (else `VmError::Script` naming it).
fn field<'a>(vm: &'a Vm, o: ObjRef, name: &str, kind: &str, kind_ok: fn(&PropKind) -> bool) -> Result<&'a Value, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let path = || crate::world::pawn_meshes::handle_path(vm, o);
    let p = vm.space.prop(class, name).ok_or_else(|| script(format!("{}'s class declares no {name}", path())))?;
    if !vm.space.classes.decl(p).is_some_and(|d| kind_ok(&d.kind)) {
        return Err(script(format!("{}'s class declares {name} as something other than {kind}", path())));
    }
    Ok(vm.space.read(o, PropAddr::scalar(p))?)
}

fn is_object(k: &PropKind) -> bool {
    matches!(k, PropKind::Object { .. })
}

fn object(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    match field(vm, o, name, "an object reference", is_object)? {
        Value::Object(x) => Ok(*x),
        other => Err(script(format!("{name} holds {}, not an object", other.kind_name()))),
    }
}

/// A hold definition's `HoldName` and `EnvironmentTag`, after its residency check.
fn hold_key(vm: &Vm, hold: ObjRef, not_loaded: &'static str) -> Result<(NameId, Option<ObjRef>), VmError> {
    resident(vm, hold, not_loaded)?;
    let name = match field(vm, hold, "HoldName", "a name", |k| matches!(k, PropKind::Name))? {
        Value::Name(n) => *n,
        other => return Err(script(format!("HoldName holds {}, not a name", other.kind_name()))),
    };
    Ok((name, object(vm, hold, "EnvironmentTag")?))
}

/// The two fields a lookup reads (the parent is `WeaponActionParentClass` for both) and its refusal texts.
#[derive(Clone, Copy)]
struct Lists {
    holds: &'static str,
    default: &'static str,
    not_loaded: &'static str,
    hold_not_loaded: &'static str,
}

/// `GetWeaponHoldDef`'s fields.
const THIRD_PERSON: Lists =
    Lists { holds: "WeaponHoldDefs", default: "DefaultWeaponHoldDef", not_loaded: NOT_LOADED, hold_not_loaded: HOLD_NOT_LOADED };
/// `GetFirstPersonWeaponHoldDef`'s fields (2.C: the two swapped, the parent the same).
const FIRST_PERSON: Lists = Lists {
    holds: "FirstPersonWeaponHoldDefs",
    default: "DefaultFirstPersonWeaponHoldDef",
    not_loaded: FIRST_PERSON_NOT_LOADED,
    hold_not_loaded: FIRST_PERSON_HOLD_NOT_LOADED,
};
/// 2.C's refusal texts.
pub const FIRST_PERSON_NOT_LOADED: &str =
    "GetFirstPersonWeaponHoldDef on a body class not loaded at set-up (a load is a write)";
pub const FIRST_PERSON_HOLD_NOT_LOADED: &str =
    "GetFirstPersonWeaponHoldDef over a weapon hold definition not loaded at set-up (a load is a write)";

/// The lookup on `body` over `lists` (module documentation); `visited` is the port's loop guard.
#[allow(clippy::too_many_arguments)]
fn lookup(
    vm: &Vm,
    lists: Lists,
    body: ObjRef,
    name: NameId,
    tag: Option<ObjRef>,
    parent_search: bool,
    fallback: bool,
    visited: &mut Vec<ObjRef>,
) -> Result<Option<ObjRef>, VmError> {
    if visited.contains(&body) {
        return Err(script("the WeaponActionParentClass chain comes back to a class it already visited".to_string()));
    }
    visited.push(body);
    resident(vm, body, lists.not_loaded)?;
    let list = field(vm, body, lists.holds, "an array of object references", |k| {
        matches!(k, PropKind::Array(inner) if is_object(inner))
    })?;
    let holds = list
        .elements()
        .map(<[Value]>::to_vec)
        .ok_or_else(|| script(format!("{} holds {}, not an array", lists.holds, list.kind_name())))?;
    let default = object(vm, body, lists.default)?;
    let parent = object(vm, body, "WeaponActionParentClass")?;
    let mut candidate = None;
    for entry in holds.iter() {
        let hold = match entry {
            Value::Object(Some(h)) => *h,
            Value::Object(None) => continue,
            other => return Err(script(format!("a {} entry holds {}, not an object", lists.holds, other.kind_name()))),
        };
        let (n, t) = hold_key(vm, hold, lists.hold_not_loaded)?;
        if n != name {
            continue;
        }
        if t == tag {
            return Ok(Some(hold));
        }
        if candidate.is_none() || t.is_none() {
            candidate = Some(hold);
        }
    }
    if parent_search && let Some(parent) = parent {
        let answer = lookup(vm, lists, parent, name, tag, parent_search, fallback && default.is_none(), visited)?;
        match (candidate, answer) {
            // The re-read: the returned hold's own name and tag (residency checked first by `hold_key`).
            (Some(c), Some(a)) => return Ok(Some(if hold_key(vm, a, lists.hold_not_loaded)? == (name, tag) { a } else { c })),
            (Some(c), None) => return Ok(Some(c)),
            (None, Some(a)) => return Ok(Some(a)),
            (None, None) => {}
        }
    }
    if candidate.is_some() {
        return Ok(candidate);
    }
    Ok(if fallback { default } else { None })
}

/// The native (module documentation).
pub fn get_weapon_hold_def(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    hold_def(c, THIRD_PERSON)
}

/// `GetFirstPersonWeaponHoldDef` (2.C): the same rule on the first-person fields.
pub fn get_first_person_weapon_hold_def(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    hold_def(c, FIRST_PERSON)
}

fn hold_def(c: &mut NativeCall<'_>, lists: Lists) -> Result<Value, VmError> {
    let body = c.this.ok_or_else(|| script("called without a body class".to_string()))?;
    let name = match c.arg(0)? {
        Value::Name(n) => *n,
        other => return Err(script(format!("HoldName holds {}, not a name", other.kind_name()))),
    };
    let tag = match c.arg(1)? {
        Value::Object(o) => *o,
        Value::None => None,
        other => return Err(script(format!("EnvironmentTag holds {}, not an object", other.kind_name()))),
    };
    let flag = |c: &NativeCall<'_>, i: usize, what: &str| match c.arg(i)? {
        Value::Bool(b) => Ok(*b),
        other => Err(script(format!("{what} holds {}, not a bool", other.kind_name()))),
    };
    let parent_search = flag(c, 2, "bSearchParentClass")?;
    let fallback = flag(c, 3, "bFallbackToDefaultDef")?;
    let answer = lookup(c.vm, lists, body, name, tag, parent_search, fallback, &mut Vec::new())?;
    Ok(Value::Object(answer))
}

/// Binds the key (Off only; the caller checks the mode). A key another pack bound first keeps its binding.
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    if !vm.has_native(KEY) {
        vm.register_native(KEY, Rc::new(get_weapon_hold_def))?;
    }
    if !vm.has_native(FIRST_PERSON_KEY) {
        vm.register_native(FIRST_PERSON_KEY, Rc::new(get_first_person_weapon_hold_def))?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "weapon_hold_tests.rs"]
mod tests;
