//! Owner: lane IMPL-MAYA9-NATIVES (decision 2026-11-26-39). Two static natives `maya:8`'s class defaults reach, in
//! `ParityMode::Off`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_MAYA9_AND_AFTER.md sections 3.3 and 3.4 alone (lane
//! ANALYST-MAYA9-AND-AFTER, #776) and the hub's rulings; this lane did not read the executable. **Every rule is
//! UNVERIFIED in the running game.** Reference binds nothing (the stub policy answers).
//!
//! **`FlagDefinition.ApplyFlagInitializationData(FlagsToSet, ContextSource)`** (section 3.3): the first argument
//! declared as an array of a struct, else `VmError::Script`; an empty array or a None context does nothing; any entry
//! is [`FLAG_ENTRY`] (`NotRead`): the value resolver's set-up step is not read. The hub ruled the game's C-runtime
//! `rand()` draw is the VM's RNG stream, counted and UNVERIFIED; no draw is made while every entry refuses.
//!
//! **A skipped array** (lane MAYA9-SKIPPED-FLAG-LAYERABSENT, the note's section 3.3): `FlagsToSet` is declared `const
//! out`, so from script the argument aliases the caller's property (`WillowCharacterClassDefinition.
//! DesignerFlagStartingValues` at the slice's call). When that property's decode was skipped
//! ([`DefStore::is_skipped`](ow_defs::DefStore::is_skipped)), the array is not the data: [`FLAG_SKIPPED`]
//! (`LayerAbsent`), before the empty test, with nothing loaded or written. An argument that aliases no object property
//! (a local, a temporary, a call from the host) has no store mark and is answered by its value, as before.
//!
//! **`BalanceModifierDefinition.ApplyPlayThroughBasedPlayerAttributeEffects(Object)`** (section 3.4): a None argument
//! does nothing; no world is `LayerAbsent`; the world's `WorldInfo` (none: nothing), its `GRI` as a `WillowGame.WillowGameReplicationInfo` (missing, or of another
//! class: nothing), its `CurrentPlaythrough` an int (else `VmError::Script`) as stored; at 1 or below nothing; above
//! 1 [`PLAYTHROUGH_TABLE`] (`NotRead`). Reads only.

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

/// The record of the rules.
pub const RECORD: &str = "docs/verification/NATIVE_MAYA9_AND_AFTER.md (sections 3.3 and 3.4)";
pub const APPLY_FLAG_INITIALIZATION: &str = "GearboxFramework.FlagDefinition.ApplyFlagInitializationData";
pub const APPLY_PLAYTHROUGH_EFFECTS: &str =
    "WillowGame.BalanceModifierDefinition.ApplyPlayThroughBasedPlayerAttributeEffects";
pub const FLAG_SKIPPED: &str =
    "flag initialization: the flags array's property was skipped when the store decoded it (its struct layout)";
/// The layer [`FLAG_SKIPPED`] waits on.
pub const FLAG_SKIPPED_LAYER: &str = "ow-defs: the decode of an array of a struct the store skipped";
pub const FLAG_ENTRY: &str =
    "flag initialization: the value resolver's set-up and the runtime random draw are not read";
pub const PLAYTHROUGH_TABLE: &str =
    "ApplyPlayThroughBasedPlayerAttributeEffects above playthrough 1: the level table lookup and the effect applier";

fn bad(function: &str, message: impl Into<String>) -> VmError {
    VmError::Script { function: function.to_string(), message: message.into() }
}

/// Section 3.3 (module documentation).
pub fn apply_flag_initialization(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "FlagDefinition.ApplyFlagInitializationData";
    let declared = c.vm.function(c.function).and_then(|d| d.params.first()).map(|p| p.kind.clone());
    if !matches!(&declared, Some(PropKind::Array(inner)) if matches!(**inner, PropKind::Struct(_))) {
        return Err(bad(WHAT, "FlagsToSet is not declared as an array of a struct"));
    }
    // A skipped source array (module documentation): asked of the store's mark, before the value is trusted.
    if let Some((owner, at)) = c.arg_property(0)
        && c.vm.defs.is_skipped(&c.vm.space, owner, at.prop)
    {
        return Err(NotYet::layer_absent(FLAG_SKIPPED, FLAG_SKIPPED_LAYER, RECORD).into());
    }
    let empty = c.arg(0)?.elements().is_none_or(|e| e.is_empty());
    let context = if c.arg_count() > 1 { matches!(c.arg(1)?, Value::Object(Some(_))) } else { false };
    if empty || !context {
        return Ok(Value::None);
    }
    Err(NotYet::not_read(FLAG_ENTRY, RECORD).into())
}

fn field(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<(Value, PropKind), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| bad(what, format!("declares no {name}")))?;
    let kind = vm.space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| bad(what, format!("{name} undeclared")))?;
    Ok((vm.space.read(o, PropAddr::scalar(p))?.clone(), kind))
}

fn is_a(vm: &Vm, o: ObjRef, path: &str) -> Result<bool, VmError> {
    let mut cursor = Some(vm.space.heap.get(o)?.class);
    let mut steps = 0;
    while let Some(c) = cursor.filter(|_| steps < 256) {
        steps += 1;
        let Some(l) = vm.space.classes.get_layout(c) else { return Ok(false) };
        if l.path().as_str().eq_ignore_ascii_case(path) {
            return Ok(true);
        }
        cursor = l.super_class();
    }
    Ok(false)
}

/// Section 3.4 (module documentation).
pub fn apply_playthrough_effects(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "BalanceModifierDefinition.ApplyPlayThroughBasedPlayerAttributeEffects";
    if !matches!(c.arg(0)?, Value::Object(Some(_))) {
        return Ok(Value::None);
    }
    let vm: &Vm = c.vm;
    // No world is the world layer's absence (`LayerAbsent`); a world without its `WorldInfo` has no GRI: nothing.
    let actors = ow_world::actors::actors_of(vm)?;
    let world = actors.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.world_info();
    let Some(world) = world else {
        return Ok(Value::None);
    };
    let gri = match field(vm, world, "GRI", WHAT)? {
        (Value::Object(g), PropKind::Object { .. }) => g,
        _ => return Err(bad(WHAT, "GRI is not declared as an object")),
    };
    let Some(gri) = gri else { return Ok(Value::None) };
    if !is_a(vm, gri, "WillowGame.WillowGameReplicationInfo")? {
        return Ok(Value::None);
    }
    let playthrough = match field(vm, gri, "CurrentPlaythrough", WHAT)? {
        (Value::Int(i), PropKind::Int) => i,
        _ => return Err(bad(WHAT, "CurrentPlaythrough is not declared as an int")),
    };
    if playthrough <= 1 {
        return Ok(Value::None);
    }
    Err(NotYet::not_read(PLAYTHROUGH_TABLE, RECORD).into())
}

/// Binds both in Off (keep-existing).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    crate::hit::bind_fn(vm, APPLY_FLAG_INITIALIZATION, apply_flag_initialization)?;
    crate::hit::bind_fn(vm, APPLY_PLAYTHROUGH_EFFECTS, apply_playthrough_effects)?;
    Ok(())
}

#[cfg(test)]
#[path = "maya9_tests.rs"]
mod tests;
