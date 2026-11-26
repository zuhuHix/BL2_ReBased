//! Owner: lane IMPL-LINKS-3-4-TAIL (decision 2026-11-26-NNN). The natives links 3 and 4 meet past their spawns, in
//! `ParityMode::Off`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_LINKS_3_4_TAIL.md (#812 as landed) and, for the plain
//! reads, NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md section 4.2, alone; this lane did not read the executable.
//! **Every rule is UNVERIFIED in the running game.** Reference binds nothing. Each native here reads only: nothing is
//! written, loaded or drawn; each field read with its declared kind, else `VmError::Script`; each object read resident,
//! else `NotRead`.
//!
//! - **The plain reads** (section 4.2, the note's rows 1 and 2): `DamagePipeline.GetBarrelSourceTime` and
//!   `GetPlantSourceTime` (the pipeline's floats), `GetCanInflictFriendlyFire` (its `bCanHitFriendly`),
//!   `Pawn.IsUsingWalkingPhysics` (`Physics` is `PHYS_Walking` or `PHYS_NavMeshWalking`, by the enum's names) and
//!   `SpecialMoveComponent.IsPlayingAny` (its `CurrentSMData.SMD` set).
//! - **`BaseBalanceDefinition.GetDisplayNameAtGrade(GradeIndex)`** (section 5): the base class an empty string (the
//!   engine's pure-virtual log); an `AIPawnBalanceDefinition` ignores the grade: the playthrough from the getter's own
//!   source ([`display_playthrough`]); above 1 [`NAME_FIXUP`] (`NotRead`); else capped at the list's last entry, and
//!   out of range an empty string, else that entry's `DisplayName`. Another balance class: [`OTHER_BALANCE`].
//! - **`WillowPlayerController.NotifyKilledEnemy(EnemyName)`** (section 6): no `WorldInfo`, or no globals object,
//!   nothing; the globals definition not resident (None is its first load, a handle its lazy load) `NotRead`;
//!   `PlayerStats` None nothing; set [`KILL_BOOKKEEPING`] (`NotRead`).
//! - **`WillowPawn.IsActionSkillRunning()`** (section 7): `MyActionSkill` is not None.
//! - **`WillowDialogGlobalsDefinition.Get()`** (section 8): the world's globals object, its `TheGlobalsDefinition`, that
//!   definition's `Dialog`, as a `WillowDialogGlobalsDefinition`; any link None, or another class, None. Each object
//!   resident, else `NotRead`. Bound where no world bound it first (the mission bridge binds its own).

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::combat_music::{bad, is_a, member, object, path, read, this};
use crate::hit::bind_fn;

/// The record of the rules.
pub const RECORD: &str = "docs/verification/NATIVE_LINKS_3_4_TAIL.md";
/// The record of the plain reads.
pub const RECORD_READS: &str = "docs/verification/NATIVE_SET_DEFAULT_PHYSICS_AND_SPAWN_BALANCE.md (section 4.2)";
pub const NAME_FIXUP: &str = "GetDisplayNameAtGrade: the playthrough name fixup";
pub const OTHER_BALANCE: &str = "GetDisplayNameAtGrade: the virtual of a balance class other than AIPawnBalanceDefinition";
pub const KILL_BOOKKEEPING: &str = "NotifyKilledEnemy: the player stats' kill bookkeeping";
pub const DEFINITION_COLD: &str =
    "the globals definition: not loaded (None is its first load, a handle its lazy load; a load is a write)";

pub const BARREL_SOURCE_TIME: [&str; 2] =
    ["Engine.DamagePipeline.GetBarrelSourceTime", "WillowGame.WillowDamagePipeline.GetBarrelSourceTime"];
pub const PLANT_SOURCE_TIME: [&str; 2] =
    ["Engine.DamagePipeline.GetPlantSourceTime", "WillowGame.WillowDamagePipeline.GetPlantSourceTime"];
pub const CAN_INFLICT_FRIENDLY_FIRE: [&str; 2] =
    ["Engine.DamagePipeline.GetCanInflictFriendlyFire", "WillowGame.WillowDamagePipeline.GetCanInflictFriendlyFire"];
pub const IS_USING_WALKING_PHYSICS: &str = "Engine.Pawn.IsUsingWalkingPhysics";
pub const IS_PLAYING_ANY: [&str; 2] =
    ["GearboxFramework.SpecialMoveComponent.IsPlayingAny", "WillowGame.SpecialMoveComponent.IsPlayingAny"];
pub const GET_DISPLAY_NAME: [&str; 2] = [
    "Engine.BaseBalanceDefinition.GetDisplayNameAtGrade",
    "WillowGame.AIPawnBalanceDefinition.GetDisplayNameAtGrade",
];
pub const NOTIFY_KILLED_ENEMY: &str = "WillowGame.WillowPlayerController.NotifyKilledEnemy";
pub const IS_ACTION_SKILL_RUNNING: &str = "WillowGame.WillowPawn.IsActionSkillRunning";
pub const DIALOG_GLOBALS_GET: &str = "WillowGame.WillowDialogGlobalsDefinition.Get";

/// `o` usable without a load: an unloaded or failed handle, or an object short of its slots, is `NotRead` naming it.
pub(crate) fn resident(vm: &Vm, o: ObjRef) -> Result<(), VmError> {
    let x = vm.space.heap.get(o)?;
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    if vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short {
        let name = vm.defs.handle_path(o).unwrap_or_else(|| path(vm, o));
        return Err(NotYet::not_read(format!("{name}: an object the store has not loaded (a load is a write)"), RECORD).into());
    }
    Ok(())
}

pub(crate) fn float(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<f32, VmError> {
    match read(vm, o, name, what)? {
        (Value::Float(f), PropKind::Float) => Ok(f),
        (_, kind) => Err(bad(what, format!("{name} is declared as {kind:?}, not a float"))),
    }
}

pub(crate) fn boolean(vm: &Vm, o: ObjRef, name: &str, what: &str) -> Result<bool, VmError> {
    match read(vm, o, name, what)? {
        (Value::Bool(b), PropKind::Bool) => Ok(b),
        (_, kind) => Err(bad(what, format!("{name} is declared as {kind:?}, not a bool"))),
    }
}

// --- The plain reads ------------------------------------------------------------------------------------------------

fn walking(vm: &Vm, pawn: ObjRef) -> Result<bool, VmError> {
    const WHAT: &str = "Pawn.IsUsingWalkingPhysics";
    let (value, _) = read(vm, pawn, "Physics", WHAT)?;
    let Value::Byte(now) = value else { return Err(bad(WHAT, "Physics does not hold a byte")) };
    let walk = crate::default_physics::physics_mode(vm, pawn, "PHYS_Walking")?;
    let nav = crate::default_physics::physics_mode(vm, pawn, "PHYS_NavMeshWalking")?;
    Ok(now == walk || now == nav)
}

fn playing_any(vm: &Vm, component: ObjRef) -> Result<bool, VmError> {
    const WHAT: &str = "SpecialMoveComponent.IsPlayingAny";
    let (data, kind) = read(vm, component, "CurrentSMData", WHAT)?;
    if !matches!(kind, PropKind::Struct(_)) {
        return Err(bad(WHAT, "CurrentSMData is not declared as a struct"));
    }
    match member(vm, &data, "SMD", WHAT)? {
        (Value::Object(o), PropKind::Object { .. }) => Ok(o.is_some()),
        _ => Err(bad(WHAT, "CurrentSMData.SMD is not declared as an object")),
    }
}

// --- GetDisplayNameAtGrade ------------------------------------------------------------------------------------------

/// The getter's own playthrough source (section 5, step 2): no globals object -1; else the world info's `GRI` as a
/// `WillowGameReplicationInfo`: absent (or no world info) 0, else its `CurrentPlaythrough`, a negative value -1. Not
/// yet capped. Reads only.
pub fn display_playthrough(vm: &Vm) -> Result<i32, VmError> {
    const WHAT: &str = "AIPawnBalanceDefinition.GetDisplayNameAtGrade";
    let globals = match vm.service::<ow_world::query::WorldQuery>() {
        Some(q) => q.try_borrow().map_err(|_| VmError::ServiceBorrowed("WorldQuery"))?.globals(),
        None => None,
    };
    if globals.is_none() {
        return Ok(-1);
    }
    let info = match vm.service::<ow_world::actors::Actors>() {
        Some(a) => a.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.world_info(),
        None => None,
    };
    let Some(info) = info else { return Ok(0) };
    resident(vm, info)?;
    let Some(gri) = object(vm, info, "GRI", WHAT)? else { return Ok(0) };
    resident(vm, gri)?;
    if !is_a(vm, gri, "WillowGame.WillowGameReplicationInfo")? {
        return Ok(0);
    }
    let playthrough = match read(vm, gri, "CurrentPlaythrough", WHAT)? {
        (Value::Int(i), PropKind::Int) => i,
        _ => return Err(bad(WHAT, "CurrentPlaythrough is not declared as an int")),
    };
    Ok(playthrough.max(-1))
}

/// `GetDisplayNameAtGrade` on `balance` (module documentation). Reads only.
pub fn display_name(vm: &mut Vm, balance: ObjRef) -> Result<String, VmError> {
    const WHAT: &str = "BaseBalanceDefinition.GetDisplayNameAtGrade";
    resident(vm, balance)?;
    if !is_a(vm, balance, "WillowGame.AIPawnBalanceDefinition")? {
        let class = vm.space.heap.get(balance)?.class;
        let base = vm.space.classes.by_path("Engine.BaseBalanceDefinition");
        if base == Some(class) {
            vm.log.push(format!("{WHAT}: pure virtual not implemented (an empty string)"));
            return Ok(String::new());
        }
        return Err(NotYet::not_read(format!("{OTHER_BALANCE} ({})", path(vm, balance)), RECORD).into());
    }
    // The list first, declared, so a mistyped one is a data error whatever the playthrough.
    let list = match read(vm, balance, "PlayThroughs", WHAT)? {
        (Value::Array(items), PropKind::Array(inner)) if matches!(*inner, PropKind::Struct(_)) => items,
        (_, kind) => return Err(bad(WHAT, format!("PlayThroughs is declared as {kind:?}, not an array of structs"))),
    };
    let playthrough = display_playthrough(vm)?;
    if playthrough > 1 {
        return Err(NotYet::not_read(NAME_FIXUP, RECORD).into());
    }
    let index = playthrough.min(i32::try_from(list.len()).unwrap_or(i32::MAX) - 1);
    let Some(entry) = usize::try_from(index).ok().and_then(|i| list.get(i)) else { return Ok(String::new()) };
    match member(vm, entry, "DisplayName", WHAT)? {
        (Value::Str(s), PropKind::Str) => Ok(s.to_string()),
        _ => Err(bad(WHAT, "DisplayName is not declared as a string")),
    }
}

// --- NotifyKilledEnemy ----------------------------------------------------------------------------------------------

/// The world's globals object (the world query's), None when it has none. Reads only.
fn world_globals(vm: &Vm) -> Result<Option<ObjRef>, VmError> {
    match vm.service::<ow_world::query::WorldQuery>() {
        Some(q) => Ok(q.try_borrow().map_err(|_| VmError::ServiceBorrowed("WorldQuery"))?.globals()),
        None => Ok(None),
    }
}

/// `NotifyKilledEnemy` on `controller` (module documentation): `Ok(())` is "nothing". Reads only.
pub fn notify_killed_enemy(vm: &Vm, controller: ObjRef) -> Result<(), VmError> {
    const WHAT: &str = "WillowPlayerController.NotifyKilledEnemy";
    resident(vm, controller)?;
    // The declarations first (section 6's Off answer).
    let class = vm.space.heap.get(controller)?.class;
    for (name, want) in [
        ("PlayerStats", "an object"),
        ("PlayerClass", "an object"),
        ("PlayerSkillTree", "an object"),
        ("RecentKillHistory", "an array"),
        ("CurrKillingSpree", "an int"),
    ] {
        let kind = vm.space.prop(class, name).and_then(|p| vm.space.classes.decl(p)).map(|d| d.kind.clone());
        let fits = matches!(
            (want, &kind),
            ("an object", Some(PropKind::Object { .. })) | ("an array", Some(PropKind::Array(_))) | ("an int", Some(PropKind::Int))
        );
        if !fits {
            return Err(bad(WHAT, format!("{name} is declared as {kind:?}, not {want}")));
        }
    }
    let Some(_info) = object(vm, controller, "WorldInfo", WHAT)? else { return Ok(()) };
    let Some(globals) = world_globals(vm)? else { return Ok(()) };
    resident(vm, globals)?;
    match object(vm, globals, "TheGlobalsDefinition", WHAT)? {
        None => return Err(NotYet::not_read(DEFINITION_COLD, RECORD).into()),
        Some(d) if vm.defs.is_unloaded(&vm.space, d) || vm.defs.is_failed(&vm.space, d) => {
            return Err(NotYet::not_read(DEFINITION_COLD, RECORD).into());
        }
        Some(d) => resident(vm, d)?,
    }
    if object(vm, controller, "PlayerStats", WHAT)?.is_none() {
        return Ok(());
    }
    Err(NotYet::not_read(KILL_BOOKKEEPING, RECORD).into())
}

// --- The dialog globals ---------------------------------------------------------------------------------------------

/// `WillowDialogGlobalsDefinition.Get()` (module documentation). Reads only.
pub fn dialog_globals(vm: &Vm) -> Result<Option<ObjRef>, VmError> {
    const WHAT: &str = "WillowDialogGlobalsDefinition.Get";
    let Some(globals) = world_globals(vm)? else { return Ok(None) };
    resident(vm, globals)?;
    let Some(definition) = object(vm, globals, "TheGlobalsDefinition", WHAT)? else { return Ok(None) };
    resident(vm, definition)?;
    let Some(dialog) = object(vm, definition, "Dialog", WHAT)? else { return Ok(None) };
    resident(vm, dialog)?;
    Ok(is_a(vm, dialog, "WillowGame.WillowDialogGlobalsDefinition")?.then_some(dialog))
}

/// The harness line of [`preload_dialog_globals`].
pub fn dialog_input_line(path: &str) -> String {
    format!("harness input: preload {path} (the globals definition's Dialog, which the game keeps resident; UNVERIFIED)")
}

/// The combat world's set-up for [`dialog_globals`] (Off only): the globals definition's `Dialog` loaded when it is a
/// handle the store has not loaded, logged ([`dialog_input_line`]); a failed load logged and left, so `Get`'s own
/// refusal shows it. The number loaded. The same shape as the combat music manager's inputs (#808).
pub fn preload_dialog_globals(vm: &mut Vm) -> Result<usize, VmError> {
    const WHAT: &str = "the dialog globals' set-up";
    if vm.parity != ParityMode::Off {
        return Ok(0);
    }
    let Some(globals) = world_globals(vm)? else { return Ok(0) };
    let Some(definition) = object(vm, globals, "TheGlobalsDefinition", WHAT)? else { return Ok(0) };
    if vm.defs.is_unloaded(&vm.space, definition) || vm.defs.is_failed(&vm.space, definition) {
        return Ok(0);
    }
    let Some(dialog) = object(vm, definition, "Dialog", WHAT)? else { return Ok(0) };
    if !vm.defs.is_unloaded(&vm.space, dialog) {
        return Ok(0);
    }
    let name = vm.defs.handle_path(dialog).unwrap_or_else(|| path(vm, dialog));
    vm.log.push(dialog_input_line(&name));
    let r = vm.defs.ensure_loaded(&mut vm.space, dialog);
    vm.log.extend(vm.defs.take_log());
    match r {
        Ok(()) => Ok(1),
        Err(e) => {
            vm.log.push(format!("harness input: preload {name}: failed ({e})"));
            Ok(0)
        }
    }
}

/// Binds the natives in Off (keep-existing).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    for key in BARREL_SOURCE_TIME {
        bind_fn(vm, key, |c| Ok(Value::Float(float(c.vm, this(c, "GetBarrelSourceTime")?, "BarrelSourceTime", "DamagePipeline.GetBarrelSourceTime")?)))?;
    }
    for key in PLANT_SOURCE_TIME {
        bind_fn(vm, key, |c| Ok(Value::Float(float(c.vm, this(c, "GetPlantSourceTime")?, "PlantSourceTime", "DamagePipeline.GetPlantSourceTime")?)))?;
    }
    for key in CAN_INFLICT_FRIENDLY_FIRE {
        bind_fn(vm, key, |c| {
            Ok(Value::Bool(boolean(c.vm, this(c, "GetCanInflictFriendlyFire")?, "bCanHitFriendly", "DamagePipeline.GetCanInflictFriendlyFire")?))
        })?;
    }
    bind_fn(vm, IS_USING_WALKING_PHYSICS, |c| Ok(Value::Bool(walking(c.vm, this(c, "IsUsingWalkingPhysics")?)?)))?;
    for key in IS_PLAYING_ANY {
        bind_fn(vm, key, |c| Ok(Value::Bool(playing_any(c.vm, this(c, "IsPlayingAny")?)?)))?;
    }
    for key in GET_DISPLAY_NAME {
        bind_fn(vm, key, |c| {
            let me = this(c, "GetDisplayNameAtGrade")?;
            Ok(Value::Str(display_name(c.vm, me)?.into()))
        })?;
    }
    bind_fn(vm, NOTIFY_KILLED_ENEMY, |c| {
        notify_killed_enemy(c.vm, this(c, "NotifyKilledEnemy")?)?;
        Ok(Value::None)
    })?;
    bind_fn(vm, IS_ACTION_SKILL_RUNNING, |c| {
        const WHAT: &str = "WillowPawn.IsActionSkillRunning";
        let me = this(c, WHAT)?;
        resident(c.vm, me)?;
        Ok(Value::Bool(object(c.vm, me, "MyActionSkill", WHAT)?.is_some()))
    })?;
    bind_fn(vm, DIALOG_GLOBALS_GET, |c| Ok(Value::Object(dialog_globals(c.vm)?)))?;
    Ok(())
}

#[cfg(test)]
#[path = "links34_tail_tests.rs"]
mod tests;
