//! Owner: lane IMPL-LINK12-UNLOCK-ACHIEVEMENT (decision 2026-11-19-157, number pending). AI-assisted (Claude Code).
//! `WillowPlayerController.UnlockAchievementIfConditionsMet`, the native slice link 12's turn-in reaches.
//!
//! Coded from NATIVE_UNLOCK_ACHIEVEMENT_IF_CONDITIONS_MET.md alone (analyst lane ANALYST-LINK12-UNLOCK-ACHIEVEMENT,
//! #541), its section 4 in full by the hub's ruling (not the note's smaller alternative). No native code or analysis
//! output was opened; the declared types were read from the installed package. **Every rule is UNVERIFIED in the
//! running game.** Off only: under `ParityMode::Reference` nothing is bound, so the VM's stub policy answers.
//!
//! **The rule** (`UnlockAchievementIfConditionsMet(AchievementUnlockType, StringData, IntData, bCanBeGreater,
//! bCheckStringAndIntData, bCanBeLess)`, a bool):
//! 1. **The globals definition** (the hub's ruling on #541's r1 review): the world's globals
//!    ([`super::world_globals`]); no globals object recorded is that lookup's `NotYet { LayerAbsent }`, unmarked and
//!    before any write: a port gap, not a game answer (review r1 of #546, item 1; the note's section 4, rule 1). Then `TheGlobalsDefinition` through the
//!    existing [`super::globals_definition`] helper, as it is: a None field is its `NotYet { NotRead }`, "the globals
//!    definition's first load, which numbers the challenge list"; a handle the store has not loaded is its lazy-load
//!    `NotYet`. Both are raised before any write, unmarked. A world that wants the definition read preloads it at
//!    set-up, as the pickup world does for links 6 and 7 and the Fire driver now does for link 12 (`ow-cli`
//!    `cmd/slice.rs`, a labelled harness input). The first load's challenge numbering stays unimplemented
//!    (NATIVE_WILLOW_GLOBALS.md section 6.1).
//! 2. **The selection** (reads only; [`achievements_to_unlock`], one function with the six inputs, shaped so that
//!    `AchievementUtility.ShouldUnlockAchievementFromUnlockType`, which takes the same inputs and returns the ids,
//!    could share it; that native is not bound here). The declared field types are checked first, after the
//!    definition refusal and before any value is read. Each entry of the definition's `AchievementData`, in order, is
//!    selected when its
//!    `Achievement` is not 0, its `UnlockType` equals the call's type, and the string match or the int match holds
//!    (both, with `bCheckStringAndIntData` set):
//!    - **the string match:** the entry's `StringData` is not empty and equals the call's, compared without case (an
//!      ASCII case fold: UNVERIFIED for text outside ASCII);
//!    - **the int match:** the entry's `IntData` is at least 1 and the call's equals it, or is above it with
//!      `bCanBeGreater`, or below it with `bCanBeLess`.
//! 3. **None selected:** false; nothing is written.
//! 4. **Some selected:** the receiver's script function `ClientUnlockAchievement(AchievementId)` (found by name on the
//!    receiver's class and state, as a script event) is called through the VM once per selected entry, in order, with
//!    the entry's `Achievement` (a byte) passed as an int: the installed package declares `AchievementId` an
//!    `IntProperty` (checked for review r1 of #546, item 3); then true. Its refusals propagate. A `NotYet` that leaves the native after any write
//!    committed since entry (#518's counters: the heap's committed writes, the generators' draws, the store's load
//!    transitions; the counter files were copied byte-identical from `fix/rust-483-followup` 1f1a2bc, as #518 is not
//!    on main yet) is marked "stopped after partial writes: " with its kind and record
//!    kept ([`ow_world::pools::after_partial_writes`]); one before any write is unmarked. Nothing is rolled back.
//!
//! **Data errors** (`VmError::Script`, naming the field): `AchievementData` not declared as a dynamic array of
//! `WillowGame.AchievementUtility.AchievementUnlockData` with a byte `UnlockType` and `Achievement`, a string
//! `StringData` and an int `IntData`; a receiver that is not an object; one whose class has no
//! `ClientUnlockAchievement` when an entry is selected. Ordinary VM argument errors apply.

use std::rc::Rc;

use ow_core::ParityMode;
use ow_object::ids::PropAddr;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use crate::equip::{bad, field};

/// The record this module follows.
pub const RECORD: &str = "NATIVE_UNLOCK_ACHIEVEMENT_IF_CONDITIONS_MET.md";
pub const UNLOCK_IF_CONDITIONS_MET: &str = "WillowGame.WillowPlayerController.UnlockAchievementIfConditionsMet";
/// The script event the native calls on its receiver, by name.
pub const CLIENT_UNLOCK_ACHIEVEMENT: &str = "ClientUnlockAchievement";
/// The declared struct of `AchievementData`'s entries.
pub const ACHIEVEMENT_UNLOCK_DATA: &str = "WillowGame.AchievementUtility.AchievementUnlockData";

const WHAT: &str = "WillowPlayerController.UnlockAchievementIfConditionsMet";

/// One `AchievementUnlockData` entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnlockData {
    pub unlock_type: u8,
    pub achievement: u8,
    pub string: String,
    pub int: i32,
}

/// The call's conditions.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Conditions {
    pub unlock_type: u8,
    pub string: String,
    pub int: i32,
    pub can_be_greater: bool,
    pub check_string_and_int: bool,
    pub can_be_less: bool,
}

/// The achievement ids `entries` select for `call`, in entry order (module documentation, step 2): the selection as
/// one function of the native's six inputs.
pub fn achievements_to_unlock(entries: &[UnlockData], call: &Conditions) -> Vec<u8> {
    entries.iter().filter(|e| selected(e, call)).map(|e| e.achievement).collect()
}

/// Whether `entry` is selected for `call` (module documentation, step 2).
pub fn selected(entry: &UnlockData, call: &Conditions) -> bool {
    if entry.achievement == 0 || entry.unlock_type != call.unlock_type {
        return false;
    }
    let string = !entry.string.is_empty() && entry.string.eq_ignore_ascii_case(&call.string);
    let int = entry.int >= 1
        && (call.int == entry.int
            || (call.can_be_greater && call.int > entry.int)
            || (call.can_be_less && call.int < entry.int));
    if call.check_string_and_int { string && int } else { string || int }
}

/// The counters of #518 at one moment: any change means a write committed.
#[derive(PartialEq, Eq)]
struct Marks {
    writes: u64,
    draws: u64,
    loads: u64,
}

fn marks(vm: &Vm) -> Marks {
    Marks { writes: vm.space.heap.committed_writes(), draws: vm.rng.draws(), loads: vm.defs.load_transitions() }
}

/// `AchievementData` of `definition`, checked against its declaration before any value is read.
pub fn achievement_data(vm: &Vm, definition: ow_object::ids::ObjRef) -> Result<Vec<UnlockData>, VmError> {
    let class = vm.space.heap.get(definition)?.class;
    let p = vm
        .space
        .prop(class, "AchievementData")
        .ok_or_else(|| bad(WHAT, "the globals definition declares no AchievementData"))?;
    let ty = match vm.space.classes.decl(p).map(|d| (&d.kind, d.array_dim)) {
        Some((PropKind::Array(inner), 1)) => match **inner {
            PropKind::Struct(ty) => ty,
            ref k => return Err(bad(WHAT, format!("AchievementData holds {k:?}, not a struct"))),
        },
        other => return Err(bad(WHAT, format!("AchievementData is declared as {other:?}, not a dynamic array"))),
    };
    if vm.space.classes.struct_by_path(ACHIEVEMENT_UNLOCK_DATA) != Some(ty) {
        return Err(bad(WHAT, format!("AchievementData holds a struct that is not {ACHIEVEMENT_UNLOCK_DATA}")));
    }
    let layout = vm.space.classes.struct_layout(ty).ok_or_else(|| bad(WHAT, "AchievementUnlockData has no layout"))?;
    for name in ["UnlockType", "Achievement", "StringData", "IntData"] {
        let decl = vm.space.names.find(name).and_then(|id| layout.field(id)).map(|(_, d)| (d.kind.clone(), d.array_dim));
        let fits = matches!(
            (name, &decl),
            ("UnlockType" | "Achievement", Some((PropKind::Byte { .. }, 1)))
                | ("StringData", Some((PropKind::Str, 1)))
                | ("IntData", Some((PropKind::Int, 1)))
        );
        if !fits {
            return Err(bad(WHAT, format!("AchievementUnlockData.{name} is declared as {decl:?}")));
        }
    }
    let entries = match vm.space.read(definition, PropAddr::scalar(p))? {
        Value::Array(a) => a.as_ref().clone(),
        other => return Err(bad(WHAT, format!("AchievementData holds {}, not an array", other.kind_name()))),
    };
    entries
        .iter()
        .map(|e| {
            let byte = |name: &str| match field(vm, e, name)? {
                Value::Byte(b) => Ok(b),
                other => Err(bad(WHAT, format!("AchievementUnlockData.{name} holds {}", other.kind_name()))),
            };
            let string = match field(vm, e, "StringData")? {
                Value::Str(s) => s.to_string(),
                other => return Err(bad(WHAT, format!("AchievementUnlockData.StringData holds {}", other.kind_name()))),
            };
            let int = match field(vm, e, "IntData")? {
                Value::Int(i) => i,
                other => return Err(bad(WHAT, format!("AchievementUnlockData.IntData holds {}", other.kind_name()))),
            };
            Ok(UnlockData { unlock_type: byte("UnlockType")?, achievement: byte("Achievement")?, string, int })
        })
        .collect()
}

/// The call's six inputs. An omitted argument, or one that arrives as `Value::None` (how the VM passes an omitted
/// optional parameter, as the turn-in's call omits `StringData` and the last two flags), is its type's zero: 0, the
/// empty string, false (the convention of `ow-world`'s `arg_bool`).
fn conditions(c: &NativeCall<'_>) -> Result<Conditions, VmError> {
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
    Ok(Conditions {
        unlock_type: match given(0)? {
            None => 0,
            Some(Value::Byte(b)) => *b,
            Some(other) => {
                return Err(bad(WHAT, format!("AchievementUnlockType is {}, not a byte", other.kind_name())));
            }
        },
        string: match given(1)? {
            None => String::new(),
            Some(Value::Str(s)) => s.to_string(),
            Some(other) => return Err(bad(WHAT, format!("StringData is {}, not a string", other.kind_name()))),
        },
        int: match given(2)? {
            None => 0,
            Some(Value::Int(i)) => *i,
            Some(other) => return Err(bad(WHAT, format!("IntData is {}, not an int", other.kind_name()))),
        },
        can_be_greater: flag(3, "bCanBeGreater")?,
        check_string_and_int: flag(4, "bCheckStringAndIntData")?,
        can_be_less: flag(5, "bCanBeLess")?,
    })
}

/// The native (module documentation).
pub fn unlock_achievement_if_conditions_met(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let entry = marks(c.vm);
    let result = run(c);
    match result {
        Err(e) if marks(c.vm) != entry => Err(ow_world::pools::after_partial_writes(e)),
        other => other,
    }
}

fn run(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let this = c.this.ok_or_else(|| bad(WHAT, "called with no receiver"))?;
    let call = conditions(c)?;
    // Step 1: no globals object is the lookup's own LayerAbsent refusal, before any write (review r1 of #546).
    let globals = super::world_globals(c.vm)?;
    // The existing helper's refusals (None and an unloaded handle, the hub's ruling), through its read-only form so a
    // cold definition class is not loaded on this path (lane COLD-LOAD-FIXES).
    let definition = super::globals_definition_resident(c.vm, globals, WHAT)?;
    // Step 2: the declared types, then the selection, reads only.
    let ids = achievements_to_unlock(&achievement_data(c.vm, definition)?, &call);
    // Step 3.
    if ids.is_empty() {
        return Ok(Value::Bool(false));
    }
    // Step 4: the script event by name, once per selected entry, in order.
    for id in ids {
        let o = c.vm.space.heap.get(this)?;
        let (class, state) = (o.class, o.state);
        let f = c.vm.find_method(class, CLIENT_UNLOCK_ACHIEVEMENT, state)?.ok_or_else(|| {
            bad(WHAT, format!("{} declares no {CLIENT_UNLOCK_ACHIEVEMENT}", ow_attr::eval::class_path(&c.vm.space, this)))
        })?;
        c.call_script(f, Some(this), vec![Value::Int(i32::from(id))])?;
    }
    Ok(Value::Bool(true))
}

/// Binds the native in `ParityMode::Off`, first-wins: a key already bound is kept.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let Ok(id) = vm.find_function(UNLOCK_IF_CONDITIONS_MET) else { return Ok(()) };
    let Some(key) = vm.function(id).and_then(|d| d.native_key.clone()) else { return Ok(()) };
    if !vm.has_native(&key) {
        let f: NativeFn = Rc::new(unlock_achievement_if_conditions_met);
        vm.register_native(&key, f)?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "achievement_tests.rs"]
mod tests;
