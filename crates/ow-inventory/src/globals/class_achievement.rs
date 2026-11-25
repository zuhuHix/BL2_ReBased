//! Owner: lane IMPL-UNLOCKPLAYERCLASSACHIEVEMENT. AI-assisted (Claude Code).
//! `WillowPlayerController.UnlockPlayerClassAchievementIfConditionsMet`, the native slice link 9's Phaselock lift
//! reaches through `NotifyTargetedActionSkillImpact`.
//!
//! Coded from docs/verification/NATIVE_UNLOCK_CLASS_ACHIEVEMENT.md alone (analyst lane
//! ANALYST-UNLOCKPLAYERCLASSACHIEVEMENT, #662, decision 2026-11-25-01), its section 4 ("Proposed Off answer"). Clean
//! room: the analyst lane read the native, this lane did not. **Every rule is UNVERIFIED in the running game.** Off
//! only: under `ParityMode::Reference` nothing is bound, so the VM's stub policy answers.
//!
//! **The rule** (`UnlockPlayerClassAchievementIfConditionsMet(AchievementUnlockType, IntData, bCanBeGreater,
//! bCanBeLess)`, a bool). The preflight reads only (no write, no load, no draw); the first failure is the answer:
//! 1. **The receiver** resident: an unloaded or a failed handle is `NotYet { NotRead }` ([`receiver_not_loaded`]).
//! 2. **The arguments:** a byte, an int and two bools. An omitted argument, or one that arrives as `Value::None` (how
//!    the VM passes an omitted optional parameter; link 9's caller omits `bCanBeLess`), is its type's zero: 0 or false
//!    (the sibling's convention, [`super::achievement`]). Another shape is a `VmError::Script`.
//! 3. **The class field:** the receiver's class must declare `PlayerClass` as an object field (`VmError::Script`
//!    otherwise; the name is a fit, UNVERIFIED). None, or an object whose class is not [`PLAYER_CLASS_DEFINITION`] or a
//!    child of it, answers **false**. The class test needs only the handle's class, so it comes before the load test:
//!    a handle of that class the store has not loaded (or failed to load) is `NotYet { NotRead }`
//!    ([`class_not_loaded`]).
//! 4. **The array:** `PlayerClassAchievementData` declared as a dynamic array of a struct with a byte `UnlockType`, a
//!    byte `Achievement` and an int `IntData`, and each stored entry of that struct (`VmError::Script` naming the field
//!    otherwise). The note checks the struct by its fields only; its path is not recorded there.
//! 5. **The selection** ([`class_achievements_to_unlock`]): each entry, in order, whose `Achievement` is not 0, whose
//!    `UnlockType` equals the argument, whose `IntData` is at least 1, and whose `IntData` equals the argument, or is
//!    at most it with `bCanBeGreater`, or at least it with `bCanBeLess`. Duplicates are kept.
//! 6. **Nothing selected:** **false**, nothing written.
//! 7. **Something selected:** before each event the receiver's class (with its current state) must resolve
//!    [`CLIENT_UNLOCK_ACHIEVEMENT`] by name without a load (`Vm::find_loaded_method`, not `find_method`, which loads
//!    class code). Code on the chain not loaded yet is `NotYet { NotRead }` ([`code_not_loaded`]); resident code with
//!    no such function is a `VmError::Script`. The first lookup comes before any event, so both refusals are
//!    write-free there. The event is called once per selected achievement, in order, with the byte passed as an int
//!    (the sibling's reading of `AchievementId`); then **true**.
//! 8. **Partial writes:** #518's counters are taken at the native's entry; an error that leaves the native after they
//!    moved is marked "stopped after partial writes: " ([`ow_world::pools::after_partial_writes`]), one before is not.
//!
//! **Bound** by the inventory pack ([`super::register`]) and by the action-skill pack (link 9's world registers no
//! inventory pack), both through [`register`]: Off only, this key only, keep-existing, so whichever registers first
//! binds the one function and the other keeps it.

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{FuncId, ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use super::achievement::CLIENT_UNLOCK_ACHIEVEMENT;
use crate::equip::{bad, field};

/// The record this module follows.
pub const RECORD: &str = "docs/verification/NATIVE_UNLOCK_CLASS_ACHIEVEMENT.md";
/// The native's function path.
pub const PATH: &str = "WillowGame.WillowPlayerController.UnlockPlayerClassAchievementIfConditionsMet";
/// The native's key, the only signature [`register`] binds.
pub const KEY: &str = "WillowPlayerController.UnlockPlayerClassAchievementIfConditionsMet(byte,int,bool,bool)";
/// The class the player-class field must hold (by fit, UNVERIFIED).
pub const PLAYER_CLASS_DEFINITION: &str = "WillowGame.PlayerClassDefinition";

const WHAT: &str = "UnlockPlayerClassAchievementIfConditionsMet";

/// Step 1's text.
pub fn receiver_not_loaded(path: &str) -> String {
    format!("{WHAT}: the controller {path} is not loaded (a load is a write)")
}

/// Step 3's text.
pub fn class_not_loaded(path: &str) -> String {
    format!("{WHAT}: the player class {path} is not loaded (a load is a write)")
}

/// Step 7's text.
pub fn code_not_loaded(class: &str) -> String {
    format!("{WHAT}: {class}'s code is not loaded (a load is a write)")
}

/// One `PlayerClassAchievementData` entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClassUnlockData {
    pub unlock_type: u8,
    pub achievement: u8,
    pub int: i32,
}

/// The call's four inputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ClassConditions {
    pub unlock_type: u8,
    pub int: i32,
    pub can_be_greater: bool,
    pub can_be_less: bool,
}

/// The achievements `entries` select for `call`, in entry order, duplicates kept (module documentation, step 5).
pub fn class_achievements_to_unlock(entries: &[ClassUnlockData], call: &ClassConditions) -> Vec<u8> {
    entries.iter().filter(|e| class_selected(e, call)).map(|e| e.achievement).collect()
}

/// Whether `entry` is selected for `call` (module documentation, step 5).
pub fn class_selected(entry: &ClassUnlockData, call: &ClassConditions) -> bool {
    entry.achievement != 0
        && entry.unlock_type == call.unlock_type
        && entry.int >= 1
        && (call.int == entry.int
            || (call.can_be_greater && call.int >= entry.int)
            || (call.can_be_less && call.int <= entry.int))
}

/// #518's counters at one moment.
#[derive(PartialEq, Eq)]
struct Marks {
    writes: u64,
    draws: u64,
    loads: u64,
    /// The VM's code loads: a load is a write (lane RUNTIME-MARK-CODE-LOADS).
    code_loads: u64,
}

fn marks(vm: &Vm) -> Marks {
    Marks { writes: vm.space.heap.committed_writes(), draws: vm.rng.draws(), loads: vm.defs.load_transitions(), code_loads: vm.class_code_loads() }
}

fn not_loaded(vm: &Vm, o: ObjRef) -> bool {
    vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o)
}

/// Step 2.
fn conditions(c: &NativeCall<'_>) -> Result<ClassConditions, VmError> {
    let given = |i: usize| -> Result<Option<&Value>, VmError> {
        if !c.has(i) {
            return Ok(None);
        }
        Ok(match c.arg(i)? {
            Value::None => None,
            v => Some(v),
        })
    };
    let flag = |i: usize, name: &str| match given(i)? {
        None => Ok(false),
        Some(Value::Bool(b)) => Ok(*b),
        Some(other) => Err(bad(WHAT, format!("{name} is {}, not a bool", other.kind_name()))),
    };
    Ok(ClassConditions {
        unlock_type: match given(0)? {
            None => 0,
            Some(Value::Byte(b)) => *b,
            Some(other) => return Err(bad(WHAT, format!("AchievementUnlockType is {}, not a byte", other.kind_name()))),
        },
        int: match given(1)? {
            None => 0,
            Some(Value::Int(i)) => *i,
            Some(other) => return Err(bad(WHAT, format!("IntData is {}, not an int", other.kind_name()))),
        },
        can_be_greater: flag(2, "bCanBeGreater")?,
        can_be_less: flag(3, "bCanBeLess")?,
    })
}

/// Step 3: the player class definition, or `None` for the answer false.
fn player_class(vm: &Vm, this: ObjRef) -> Result<Option<ObjRef>, VmError> {
    let class = vm.space.heap.get(this)?.class;
    let path = || ow_attr::eval::class_path(&vm.space, this);
    let p = vm.space.prop(class, "PlayerClass").ok_or_else(|| bad(WHAT, format!("{} declares no PlayerClass", path())))?;
    if !matches!(vm.space.classes.decl(p).map(|d| &d.kind), Some(PropKind::Object { .. })) {
        return Err(bad(WHAT, format!("{}.PlayerClass is not an object field", path())));
    }
    let o = match vm.space.read(this, PropAddr::scalar(p))? {
        Value::Object(None) => return Ok(None),
        Value::Object(Some(o)) => *o,
        other => return Err(bad(WHAT, format!("{}.PlayerClass holds {}", path(), other.kind_name()))),
    };
    let Some(definition) = vm.space.classes.by_path(PLAYER_CLASS_DEFINITION) else {
        return Ok(None);
    };
    if !vm.space.classes.is_child_of(vm.space.heap.get(o)?.class, definition) {
        return Ok(None);
    }
    if not_loaded(vm, o) {
        return Err(NotYet::not_read(class_not_loaded(&ow_attr::eval::object_path(&vm.space, o)), RECORD).into());
    }
    Ok(Some(o))
}

/// Step 4: `PlayerClassAchievementData` of `definition`, its declaration checked before any value is read.
pub fn class_achievement_data(vm: &Vm, definition: ObjRef) -> Result<Vec<ClassUnlockData>, VmError> {
    const ARRAY: &str = "PlayerClassAchievementData";
    let class = vm.space.heap.get(definition)?.class;
    let p = vm.space.prop(class, ARRAY).ok_or_else(|| bad(WHAT, format!("the player class declares no {ARRAY}")))?;
    let ty = match vm.space.classes.decl(p).map(|d| (&d.kind, d.array_dim)) {
        Some((PropKind::Array(inner), 1)) => match **inner {
            PropKind::Struct(ty) => ty,
            ref k => return Err(bad(WHAT, format!("{ARRAY} holds {}, not a struct", k.kind_name()))),
        },
        _ => return Err(bad(WHAT, format!("{ARRAY} is not declared as a dynamic array"))),
    };
    let layout = vm.space.classes.struct_layout(ty).ok_or_else(|| bad(WHAT, format!("{ARRAY}'s struct has no layout")))?;
    for name in ["UnlockType", "Achievement", "IntData"] {
        let kind = vm.space.names.find(name).and_then(|id| layout.field(id)).map(|(_, d)| (d.kind.clone(), d.array_dim));
        let fits = matches!(
            (name, &kind),
            ("UnlockType" | "Achievement", Some((PropKind::Byte { .. }, 1))) | ("IntData", Some((PropKind::Int, 1)))
        );
        if !fits {
            return Err(bad(WHAT, format!("{ARRAY}'s {name} is not declared as the rule reads it")));
        }
    }
    let entries = match vm.space.read(definition, PropAddr::scalar(p))? {
        Value::Array(a) => a.as_ref().clone(),
        other => return Err(bad(WHAT, format!("{ARRAY} holds {}, not an array", other.kind_name()))),
    };
    entries
        .iter()
        .map(|e| {
            if e.as_struct().map(|s| s.ty) != Some(ty) {
                return Err(bad(WHAT, format!("{ARRAY} holds an entry that is not its declared struct")));
            }
            let byte = |name: &str| match field(vm, e, name)? {
                Value::Byte(b) => Ok(b),
                other => Err(bad(WHAT, format!("{ARRAY}'s {name} holds {}", other.kind_name()))),
            };
            let int = match field(vm, e, "IntData")? {
                Value::Int(i) => i,
                other => return Err(bad(WHAT, format!("{ARRAY}'s IntData holds {}", other.kind_name()))),
            };
            Ok(ClassUnlockData { unlock_type: byte("UnlockType")?, achievement: byte("Achievement")?, int })
        })
        .collect()
}

/// Step 7's lookup, without a load.
fn client_unlock(vm: &Vm, this: ObjRef) -> Result<FuncId, VmError> {
    let o = vm.space.heap.get(this)?;
    let class = ow_attr::eval::class_path(&vm.space, this);
    match vm.find_loaded_method(o.class, CLIENT_UNLOCK_ACHIEVEMENT, o.state) {
        None => Err(NotYet::not_read(code_not_loaded(&class), RECORD).into()),
        Some(None) => Err(bad(WHAT, format!("{class} declares no {CLIENT_UNLOCK_ACHIEVEMENT}"))),
        Some(Some(f)) => Ok(f),
    }
}

/// The native (module documentation).
pub fn unlock_player_class_achievement_if_conditions_met(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let entry = marks(c.vm);
    match run(c) {
        Err(e) if marks(c.vm) != entry => Err(ow_world::pools::after_partial_writes(e)),
        other => other,
    }
}

fn run(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let this = c.this.ok_or_else(|| bad(WHAT, "called with no receiver"))?;
    // Step 1.
    if not_loaded(c.vm, this) {
        return Err(NotYet::not_read(receiver_not_loaded(&ow_attr::eval::object_path(&c.vm.space, this)), RECORD).into());
    }
    // Steps 2 to 5, reads only.
    let call = conditions(c)?;
    let Some(definition) = player_class(c.vm, this)? else {
        return Ok(Value::Bool(false));
    };
    let ids = class_achievements_to_unlock(&class_achievement_data(c.vm, definition)?, &call);
    // Step 6.
    if ids.is_empty() {
        return Ok(Value::Bool(false));
    }
    // Step 7: the event by name, looked up without a load before each call.
    for id in ids {
        let f = client_unlock(c.vm, this)?;
        c.call_script(f, Some(this), vec![Value::Int(i32::from(id))])?;
    }
    Ok(Value::Bool(true))
}

/// Binds the native in `ParityMode::Off` for [`PATH`]'s function declared under [`KEY`], keep-existing: a function
/// already bound (in its package or VM-wide) is left as it is. Shared by the inventory and the action-skill packs.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let Ok(id) = vm.find_function(PATH) else { return Ok(()) };
    let ours = vm.function(id).is_some_and(|d| d.is_native() && d.native_key.as_deref() == Some(KEY));
    if !ours || vm.is_bound(id) {
        return Ok(());
    }
    let f: NativeFn = Rc::new(unlock_player_class_achievement_if_conditions_met);
    vm.register_native_for(id, f)
}

#[cfg(test)]
#[path = "class_achievement_tests.rs"]
mod tests;
