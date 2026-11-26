//! Owner: lane IMPL-LINKS-13-14-CLASS-INPUTS. The reads links 3, 4 and 13 meet on `TookDamageFromEnemy` and
//! `PlayHit`, in `ParityMode::Off`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_LINKS_13_14.md section 6 (#796 at its r1 fix 2bedfd46)
//! alone; this lane did not read the executable. **Every rule is UNVERIFIED in the running game.** Reference binds
//! nothing. Each read is write-free and load-free: each field is read with its declared kind, else `VmError::Script`;
//! each object read must be resident, else `NotRead`.
//!
//! - **`Pawn.GetController()`** (6.1): the pawn's `Controller`; else, with a `DrivenVehicle`, that vehicle's
//!   `Controller`; else None.
//! - **`WillowGameInfo.UpdateTookDamageFromEnemyStats(...)`** (6.2): a `MyCharacterClass` that is an
//!   `AIClassDefinition` with `bCountsTowardDamageAndKillStats` clear: nothing. The credited player is `InstigatedBy`
//!   when it is a `WillowPlayerController`; else, when `InstigatedBy`'s `Pawn` is a `WillowAIPawn`, its `PlayerMaster`,
//!   else its `ThoughtlockMaster` (by fit in the note). No player, or a player whose `PlayerStats` is None: nothing.
//!   Otherwise [`PLAYER_STATS`] (`NotRead`).
//! - **`WillowDamageSource.GetAmmoTheftAmounts(out Instigated, out Received, Source, Instigator, Victim)`** (6.3,
//!   static): the source's kind (Bullet with its subclasses; StatusEffect, Grenade, Rocket and Melee exactly, Melee by
//!   fit); `Instigated` := the instigator controller's `Instigated<Kind>DamageAmmoTheftAmount`, `Received` := the
//!   victim controller's `Received<Kind>DamageAmmoTheftAmount`, each the stored int attribute value; a missing
//!   controller, no source or another class, 0. The classes are found in the class table only (no load).
//! - **`WillowDamagePipeline.DamageTakenSkillEventsTriggered(out byte)`** and **`DamageDealtSkillEventsTriggered`**
//!   (6.4, iterators): the summary's list, in order, up to the first 255, at most seven entries.
//! - **`Pawn.PawnCheckPlayHitPreconditions(Pipeline)`** (6.5): false when the pipeline is set and its
//!   `DamageSummary.FinalDamage` is above 0; else false when the pawn is in god mode (`crate::link34::pawn_god`);
//!   else true.

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{IteratorResult, NativeCall, Vm};
use ow_vm::error::VmError;

use crate::hit::bind_fn;

/// The record of the rules.
pub const RECORD: &str = "docs/verification/NATIVE_LINKS_13_14.md";
pub const NOT_LOADED: &str = "an object the store has not loaded (a load is a write)";
pub const PLAYER_STATS: &str = "UpdateTookDamageFromEnemyStats: the player's damage statistics";
pub const GET_CONTROLLER: &str = "Engine.Pawn.GetController";
pub const UPDATE_STATS: &str = "WillowGame.WillowGameInfo.UpdateTookDamageFromEnemyStats";
pub const AMMO_THEFT: &str = "WillowGame.WillowDamageSource.GetAmmoTheftAmounts";
pub const PLAY_HIT_PRECONDITIONS: &str = "Engine.Pawn.PawnCheckPlayHitPreconditions";
/// The iterator keys and the summary list each walks.
pub const SKILL_EVENT_ITERATORS: [(&str, &str); 2] = [
    ("WillowDamagePipeline.DamageTakenSkillEventsTriggered(byte)", "DamageTakenSkillEventsTriggered"),
    ("WillowDamagePipeline.DamageDealtSkillEventsTriggered(byte)", "DamageDealtSkillEventsTriggered"),
];
/// The most entries an iterator visits (the note's reading, UNVERIFIED).
pub const MAX_TAGS: usize = 7;
/// The damage sources of `GetAmmoTheftAmounts`, in test order: (class path, field kind, matched with subclasses).
pub const THEFT_SOURCES: [(&str, &str, bool); 5] = [
    ("WillowGame.WillowDmgSource_Bullet", "Bullet", true),
    ("WillowGame.WillowDmgSource_StatusEffect", "StatusEffect", false),
    ("WillowGame.WillowDmgSource_Grenade", "Grenade", false),
    ("WillowGame.WillowDmgSource_Rocket", "Rocket", false),
    ("WillowGame.WillowDmgSource_Melee", "Melee", false),
];

fn bad(what: &str, message: impl Into<String>) -> VmError {
    VmError::Script { function: what.to_string(), message: message.into() }
}

fn read(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<(Value, PropKind), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| bad(what, format!("{} declares no {name}", path(vm, o))))?;
    let kind = vm.space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| bad(what, format!("{name} undeclared")))?;
    Ok((vm.space.read(o, ow_object::ids::PropAddr::scalar(p))?.clone(), kind))
}

fn path(vm: &Vm, o: ObjRef) -> String {
    vm.space.heap.get(o).ok().and_then(|x| vm.space.classes.get_layout(x.class)).map_or_else(String::new, |l| l.path().to_string())
}

fn object(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<Option<ObjRef>, VmError> {
    match read(vm, o, name, what)? {
        (Value::Object(x), PropKind::Object { .. }) => Ok(x),
        _ => Err(bad(what, format!("{name} is not declared as an object"))),
    }
}

fn resident(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short {
        return Err(NotYet::not_read(format!("{}: {NOT_LOADED}", path(vm, o)), RECORD).into());
    }
    Ok(())
}

fn is_a(vm: &Vm, o: ObjRef, wanted: &str) -> Result<bool, VmError> {
    let mut cursor = Some(vm.space.heap.get(o)?.class);
    let mut steps = 0;
    while let Some(c) = cursor.filter(|_| steps < 256) {
        steps += 1;
        let Some(l) = vm.space.classes.get_layout(c) else { return Ok(false) };
        if l.path().as_str().eq_ignore_ascii_case(wanted) {
            return Ok(true);
        }
        cursor = l.super_class();
    }
    Ok(false)
}

fn arg_object(c: &NativeCall<'_>, i: usize, what: &str) -> Result<Option<ObjRef>, VmError> {
    if i >= c.arg_count() {
        return Ok(None);
    }
    match c.arg(i)? {
        Value::Object(o) | Value::Interface(o) => Ok(*o),
        Value::None => Ok(None),
        other => Err(bad(what, format!("argument {i} is {}, not an object", other.kind_name()))),
    }
}

fn this(c: &NativeCall<'_>, what: &str) -> Result<ObjRef, VmError> {
    c.this.ok_or_else(|| bad(what, "called without a receiver"))
}

// --- GetController --------------------------------------------------------------------------------------------------

/// `Pawn.GetController()` (module documentation). Reads only.
pub fn get_controller(vm: &Vm, pawn: ObjRef) -> Result<Option<ObjRef>, VmError> {
    const WHAT: &str = "Pawn.GetController";
    resident(vm, pawn)?;
    let controller = object(vm, pawn, "Controller", WHAT)?;
    let vehicle = object(vm, pawn, "DrivenVehicle", WHAT)?;
    if controller.is_some() {
        return Ok(controller);
    }
    let Some(vehicle) = vehicle else { return Ok(None) };
    resident(vm, vehicle)?;
    object(vm, vehicle, "Controller", WHAT)
}

// --- UpdateTookDamageFromEnemyStats -----------------------------------------------------------------------------------

/// The player `UpdateTookDamageFromEnemyStats` credits (6.2's steps 1 to 3): `None` when nothing happens. Reads only.
pub fn credited_player(vm: &Vm, instigated_by: Option<ObjRef>, class: Option<ObjRef>) -> Result<Option<ObjRef>, VmError> {
    const WHAT: &str = "WillowGameInfo.UpdateTookDamageFromEnemyStats";
    if let Some(class) = class
        && is_a(vm, class, "WillowGame.AIClassDefinition")?
    {
        resident(vm, class)?;
        match read(vm, class, "bCountsTowardDamageAndKillStats", WHAT)? {
            (Value::Bool(false), PropKind::Bool) => return Ok(None),
            (Value::Bool(true), PropKind::Bool) => {}
            _ => return Err(bad(WHAT, "bCountsTowardDamageAndKillStats is not declared as a bool")),
        }
    }
    let Some(by) = instigated_by else { return Ok(None) };
    resident(vm, by)?;
    let player = if is_a(vm, by, "WillowGame.WillowPlayerController")? {
        Some(by)
    } else {
        match object(vm, by, "Pawn", WHAT)? {
            Some(pawn) if is_a(vm, pawn, "WillowGame.WillowAIPawn")? => {
                resident(vm, pawn)?;
                match object(vm, pawn, "PlayerMaster", WHAT)? {
                    Some(m) => Some(m),
                    None => object(vm, pawn, "ThoughtlockMaster", WHAT)?,
                }
            }
            _ => None,
        }
    };
    let Some(player) = player else { return Ok(None) };
    resident(vm, player)?;
    Ok(object(vm, player, "PlayerStats", WHAT)?.map(|_| player))
}

fn update_stats(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "WillowGameInfo.UpdateTookDamageFromEnemyStats";
    let by = arg_object(c, 1, WHAT)?;
    let class = arg_object(c, 2, WHAT)?;
    if credited_player(c.vm, by, class)?.is_some() {
        return Err(NotYet::not_read(PLAYER_STATS, RECORD).into());
    }
    Ok(Value::None)
}

// --- GetAmmoTheftAmounts --------------------------------------------------------------------------------------------

/// The `THEFT_SOURCES` kind of the source class, by the class table only (a class the table does not hold matches
/// nothing; nothing loads).
pub fn theft_kind(vm: &Vm, source: Option<ow_object::ids::ClassId>) -> Option<&'static str> {
    let source = source?;
    THEFT_SOURCES.iter().find_map(|&(path, kind, subclasses)| {
        let class = vm.space.classes.by_path(path)?;
        let hit = if subclasses { vm.space.classes.is_child_of(source, class) } else { source == class };
        hit.then_some(kind)
    })
}

/// `controller`'s `<prefix><kind>DamageAmmoTheftAmount`, its stored int attribute value; 0 with no controller.
fn theft_amount(vm: &Vm, controller: Option<ObjRef>, prefix: &str, kind: &str) -> Result<i32, VmError> {
    const WHAT: &str = "WillowDamageSource.GetAmmoTheftAmounts";
    let Some(controller) = controller else { return Ok(0) };
    resident(vm, controller)?;
    let name = format!("{prefix}{kind}DamageAmmoTheftAmount");
    match read(vm, controller, &name, WHAT)? {
        (Value::Int(v), PropKind::IntAttribute) => Ok(v),
        _ => Err(bad(WHAT, format!("{name} is not declared as an int attribute"))),
    }
}

/// `GetAmmoTheftAmounts`' two answers: (instigated, received). Reads only.
pub fn ammo_theft(
    vm: &Vm,
    source: Option<ow_object::ids::ClassId>,
    instigator: Option<ObjRef>,
    victim: Option<ObjRef>,
) -> Result<(i32, i32), VmError> {
    let Some(kind) = theft_kind(vm, source) else { return Ok((0, 0)) };
    Ok((theft_amount(vm, instigator, "Instigated", kind)?, theft_amount(vm, victim, "Received", kind)?))
}

fn get_ammo_theft(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "WillowDamageSource.GetAmmoTheftAmounts";
    let source = match c.arg(2)? {
        Value::Class(k) => *k,
        Value::None => None,
        other => return Err(bad(WHAT, format!("DamageSource is {}, not a class", other.kind_name()))),
    };
    let instigator = arg_object(c, 3, WHAT)?;
    let victim = arg_object(c, 4, WHAT)?;
    let (instigated, received) = ammo_theft(c.vm, source, instigator, victim)?;
    c.set_out(0, Value::Int(instigated))?;
    c.set_out(1, Value::Int(received))?;
    Ok(Value::None)
}

// --- The skill-event iterators --------------------------------------------------------------------------------------

/// The tags a pipeline's summary list `list` yields (6.4): in order, up to the first 255, at most [`MAX_TAGS`].
pub fn skill_event_tags(vm: &Vm, pipeline: ObjRef, list: &str) -> Result<Vec<u8>, VmError> {
    let what = format!("WillowDamagePipeline.{list}");
    resident(vm, pipeline)?;
    let (_, first) = crate::damage::summary_schema(vm, pipeline, &what)?;
    let i = if list == crate::damage::TAG_LISTS[0] { first[0] } else { first[1] };
    let (summary, _) = read(vm, pipeline, "DamageSummary", &what)?;
    let s = summary.as_struct().ok_or_else(|| bad(&what, "DamageSummary holds no struct"))?;
    let mut tags = Vec::new();
    for k in 0..MAX_TAGS {
        match s.fields.get(i + k) {
            Some(Value::Byte(crate::damage::TAG_END)) => break,
            Some(Value::Byte(b)) => tags.push(*b),
            other => return Err(bad(&what, format!("{list}[{k}] holds {other:?}, not a byte"))),
        }
    }
    Ok(tags)
}

// --- PawnCheckPlayHitPreconditions ----------------------------------------------------------------------------------

/// `Pawn.PawnCheckPlayHitPreconditions(Pipeline)` (module documentation). Reads only.
pub fn play_hit_preconditions(vm: &Vm, pawn: ObjRef, pipeline: Option<ObjRef>) -> Result<bool, VmError> {
    const WHAT: &str = "Pawn.PawnCheckPlayHitPreconditions";
    if let Some(pipeline) = pipeline {
        resident(vm, pipeline)?;
        crate::damage::summary_schema(vm, pipeline, WHAT)?;
        let (summary, _) = read(vm, pipeline, "DamageSummary", WHAT)?;
        let s = summary.as_struct().ok_or_else(|| bad(WHAT, "DamageSummary holds no struct"))?;
        let layout = vm.space.classes.struct_layout(s.ty).ok_or_else(|| bad(WHAT, "no summary layout"))?;
        let n = vm.space.names.find("FinalDamage").ok_or_else(|| bad(WHAT, "the summary declares no FinalDamage"))?;
        let (i, d) = layout.field(n).ok_or_else(|| bad(WHAT, "the summary declares no FinalDamage"))?;
        if !matches!(d.kind, PropKind::Float) {
            return Err(bad(WHAT, "FinalDamage is not declared as a float"));
        }
        if matches!(s.fields.get(i as usize), Some(Value::Float(f)) if *f > 0.0) {
            return Ok(false);
        }
    }
    Ok(!crate::link34::pawn_god(vm, pawn)?)
}

/// Binds the keys in Off (Reference binds nothing). No pack bound them before (checked by grep), and `bind_fn` keeps
/// an existing binding.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    bind_fn(vm, GET_CONTROLLER, |c| {
        let me = this(c, "Pawn.GetController")?;
        Ok(Value::Object(get_controller(c.vm, me)?))
    })?;
    bind_fn(vm, UPDATE_STATS, update_stats)?;
    bind_fn(vm, AMMO_THEFT, get_ammo_theft)?;
    bind_fn(vm, PLAY_HIT_PRECONDITIONS, |c| {
        let me = this(c, "Pawn.PawnCheckPlayHitPreconditions")?;
        let pipeline = arg_object(c, 0, "Pawn.PawnCheckPlayHitPreconditions")?;
        Ok(Value::Bool(play_hit_preconditions(c.vm, me, pipeline)?))
    })?;
    for (key, list) in SKILL_EVENT_ITERATORS {
        vm.register_iterator(
            key,
            Rc::new(move |c: &mut NativeCall<'_>| {
                let me = this(c, key)?;
                let tags = skill_event_tags(c.vm, me, list)?;
                Ok(IteratorResult::new(0, tags.into_iter().map(Value::Byte).collect()))
            }),
        )?;
    }
    Ok(())
}

#[cfg(test)]
#[path = "links1314_tests.rs"]
mod tests;
