//! Owner: lane IMPL-GO-DOWN-NATIVES. The natives slice links 13 (`go_down`) and 14 (`bleed_out_respawn`) reach past
//! link 4, in `ParityMode::Off` only: a rule where NATIVE_GO_DOWN_PATH.md (or the note it points to) gives one, else a
//! named `NotYet`. AI-assisted (Claude Code). Every rule is **UNVERIFIED** in the running game.
//!
//! **Why Off only.** Reference keeps its stub policy (a zero for an unbound native), so Reference is unchanged.
//!
//! **Placeholders lose to nothing registered earlier, and win over everything registered later.** Binding is
//! first-wins ([`bind_fn`] skips a key already bound), and this module registers last in `ow_combat::hit::Pack`. So a
//! real binding of one of these keys from a pack registered after the combat pack (the skill manager, pawn data or
//! skill tree packs `cmd/combat.rs` adds) would silently lose to the placeholder here. A lane that gives one of them a
//! real answer must replace its entry here, or register its pack before the combat pack.
//!
//! **No prefix here.** Each `NotYet` below is answered at the native's entry, before any write of its own, as a plain
//! `NotYet`. The layer that wrote before (the combat run, the slice gate, the attribute evaluator) marks an escaping
//! `NotYet` "stopped after partial writes" (NATIVE_GO_DOWN_PATH.md section 1). Grade by the kind, never by text.
//!
//! **What is answered** (NATIVE_GO_DOWN_PATH.md sections 3 and 4):
//! - rules: `Pawn.IsLocallyControlled` (3.1), `WillowPlayerController.GetSkillManager` (3.5),
//!   `WillowPawn.GetTechDeathIndexForDamageType` up to its unread test (3.4), `Engine.GetCurrentWorldInfo`,
//!   `Engine.IsSplitScreen`, `InteractionProxy.IsInteractionDebugEnabled`, the three `WillowGlobals` getters (4.2 and
//!   NATIVE_RESPAWN_STATIONS.md 3.2), `WillowPlayerPawn.SetShadowCasting` with nothing to visit (4.4),
//!   `WillowPlayerPawn.InitRagdoll` (death.rs's presentation answer, 3.2), `Controller.IsUsingVehicleEx` with no pawn
//!   (NATIVE_RESPAWN_STATIONS.md 3.1), the revived and killed events' telemetry ([`TELEMETRY`], hub ruling R3 of
//!   #504, NATIVE_MISSION_TEXT_LIST.md section 6: nothing, a port answer), `GearboxDialogComponent.StopTalking` with no live line and no flag (NATIVE_DIALOG.md);
//! - `NotRead`: the overridable implementations of 3.2, `ClearAllStatusEffects` (3.3), `SetCollision` and
//!   `SetCollisionSize` (4.1), `SetLocation`, `SetRotation`, `IsUsingVehicle`,
//!   `ResetAI`'s entries and `StaticTriggerTemplateEvent`'s contents;
//! - `NotPorted`: `Pawn.SetAnchor` (read as bookkeeping in NATIVE_DOWN_TIMERS.md, not ported here) and the parts of
//!   `StopTalking` this lane does not port;
//! - `LayerAbsent`: `Actor.Trace`, "Actor.Trace: the world's collision" (4.5).
//!
//! **The native iterators** of 4.5 cannot be bound by name: the VM's own refusal for an unbound iterator names its
//! key in Off, and a lane that binds one answers `NotYet { NotRead }`, "the engine iterator <key>".

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use crate::hit::access::{error, get, integer, is_a, required, truth};
use crate::hit::{arg_object, arg_truth, bind_fn, call_method, this, with_state};

/// Where these rules are written down.
pub const RECORD: &str = "NATIVE_GO_DOWN_PATH.md";
/// The record of the respawn natives this module answers from.
pub const RECORD_RESPAWN: &str = "NATIVE_RESPAWN_STATIONS.md";
/// The record of the dialog natives.
pub const RECORD_DIALOG: &str = "NATIVE_DIALOG.md";
/// The record of the down-timer natives.
pub const RECORD_DOWN: &str = "NATIVE_DOWN_TIMERS.md";

/// The layer `Actor.Trace` needs.
pub const COLLISION_LAYER: &str = "the world's collision";

/// The natives of section 3.2 whose class implementation is not resolved: `NotRead` at entry.
pub const UNRESOLVED: [&str; 16] = [
    "Engine.Controller.IsLocalController",
    "Engine.PlayerController.IsLocalController",
    "WillowGame.WillowPawn.GetParent",
    "WillowGame.WillowAIPawn.GetParent",
    "WillowGame.WillowPawn.SetDefaultStance",
    "WillowGame.WillowPlayerPawn.SetDefaultStance",
    "WillowGame.WillowAIPawn.SetDefaultStance",
    "Engine.Actor.SetHidden",
    "Engine.Actor.SetHardAttach",
    "Engine.Actor.SetBase",
    "Engine.Pawn.TermRagdoll",
    "WillowGame.WillowPlayerController.PlayCameraAnim",
    "WillowGame.WillowPlayerController.StopCameraAnim",
    "WillowGame.WillowPlayerController.UpdateInputContext",
    "WillowGame.WillowPlayerPawn.MarkDLEsForUpdate",
    "WillowGame.WillowPawn.RefreshPawnGearLikenessComponents",
];

/// The natives answered `NotRead` with a fixed text, and that text's record.
pub const NOT_READ: [(&str, &str, &str); 8] = [
    (
        "WillowGame.StatusEffectsComponent.ClearAllStatusEffects",
        "StatusEffectsComponent.ClearAllStatusEffects: the per-kind removal and the skill manager step",
        RECORD,
    ),
    ("Engine.Actor.SetCollision", "Actor.SetCollision: the collision hash and touch updates", RECORD),
    ("Engine.Actor.SetCollisionSize", "Actor.SetCollisionSize: the component's world update", RECORD),
    ("Engine.Actor.SetLocation", "Actor.SetLocation: not read", RECORD_RESPAWN),
    ("Engine.Actor.SetRotation", "Actor.SetRotation: not read", RECORD_RESPAWN),
    ("Engine.Controller.IsUsingVehicle", "Controller.IsUsingVehicle", RECORD),
    ("WillowGame.WillowGameInfo.ResetAI", "WillowGameInfo.ResetAI: the per-entry conditions and the AI resets", RECORD_DOWN),
    (
        "WillowGame.WillowDialogGlobalsDefinition.StaticTriggerTemplateEvent",
        "WillowDialogGlobalsDefinition.StaticTriggerTemplateEvent: the template event's contents",
        RECORD_DIALOG,
    ),
];

/// The events that end in the shared statistics submit routine (NATIVE_MISSION_TEXT_LIST.md section 5): answered
/// nothing in Off under hub ruling R3. `RecordMissionStatusChangedEventForPlayer` shares it too and is bound by the lane
/// that implements that note.
pub const TELEMETRY: [&str; 2] = [
    "WillowGame.WillowLeviathanService.RecordCharacterRevivedEventForPlayer",
    "WillowGame.WillowLeviathanService.RecordCharacterKilledEventForPlayer",
];

/// The status-effect damage type bytes 3.4's unread test runs for.
pub const TECH_DEATH_KINDS: [i64; 4] = [1, 2, 4, 7];

/// Binds this module's natives, in `ParityMode::Off` only (module documentation). A key already bound keeps its
/// binding ([`bind_fn`]).
pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    for path in UNRESOLVED {
        let (class, function) = class_and_function(path);
        let what = format!("{class}.{function}: the class's own implementation is not resolved");
        bind_fn(vm, path, move |_| Err(NotYet::not_read(what.clone(), RECORD).into()))?;
    }
    for (path, what, record) in NOT_READ {
        bind_fn(vm, path, move |_| Err(NotYet::not_read(what, record).into()))?;
    }
    bind_fn(vm, "Engine.Actor.Trace", |_| {
        Err(NotYet::layer_absent("Actor.Trace: the world's collision", COLLISION_LAYER, RECORD).into())
    })?;
    bind_fn(vm, "Engine.Pawn.SetAnchor", |_| {
        Err(NotYet::not_ported("Pawn.SetAnchor: the anchor bookkeeping", RECORD_DOWN).into())
    })?;
    bind_fn(vm, "WillowGame.WillowPlayerPawn.InitRagdoll", crate::death::init_ragdoll)?;
    bind_fn(vm, "Engine.Pawn.IsLocallyControlled", is_locally_controlled)?;
    bind_fn(vm, "WillowGame.WillowPlayerController.GetSkillManager", get_skill_manager)?;
    bind_fn(vm, "WillowGame.WillowPawn.GetTechDeathIndexForDamageType", tech_death_index)?;
    bind_fn(vm, "Engine.Engine.GetCurrentWorldInfo", |c| Ok(Value::Object(Some(world(c.vm)?))))?;
    bind_fn(vm, "Engine.Engine.IsSplitScreen", |c| Ok(Value::Bool(local_player_count(c.vm)? > 1)))?;
    bind_fn(vm, "WillowGame.InteractionProxy.IsInteractionDebugEnabled", |_| Ok(Value::Bool(false)))?;
    bind_fn(vm, "WillowGame.WillowGlobals.GetCustomizationManager", |c| {
        globals_field(c, "GetCustomizationManager", "TheCustomizationManager")
    })?;
    bind_fn(vm, "WillowGame.WillowGlobals.GetEffectCoordinator", |c| {
        globals_field(c, "GetEffectCoordinator", "TheEffectsCoordinator")
    })?;
    bind_fn(vm, "WillowGame.WillowGlobals.GetVehicleLifetimeManager", |c| {
        let v = globals_field(c, "GetVehicleLifetimeManager", "TheVehicleLifetimeManager")?;
        if matches!(v, Value::Object(None)) {
            return Err(NotYet::not_read("the vehicle lifetime manager's lazy creation", RECORD).into());
        }
        Ok(v)
    })?;
    bind_fn(vm, "WillowGame.WillowPlayerPawn.SetShadowCasting", set_shadow_casting)?;
    bind_fn(vm, "Engine.Controller.IsUsingVehicleEx", is_using_vehicle_ex)?;
    // The events that end in the statistics submit routine (hub ruling R3, review of #504; NATIVE_MISSION_TEXT_LIST.md
    // section 6, step 4): the port has no telemetry service, so in Off each answers nothing, with no write and no
    // `NotYet`, for every player. A port answer, UNVERIFIED.
    for path in TELEMETRY {
        bind_fn(vm, path, |_| Ok(Value::None))?;
    }
    bind_fn(vm, "GearboxFramework.GearboxDialogComponent.StopTalking", stop_talking)
}

/// `Package.Class.Function` -> (`Class`, `Function`).
fn class_and_function(path: &str) -> (&str, &str) {
    let mut parts = path.rsplitn(3, '.');
    let function = parts.next().unwrap_or(path);
    let class = parts.next().unwrap_or(path);
    (class, function)
}

/// The world's `WorldInfo` (the combat state's), or `NotYet { LayerAbsent }`, "the world's WorldInfo".
fn world(vm: &Vm) -> Result<ObjRef, VmError> {
    with_state(vm, |s| s.world)?
        .ok_or_else(|| NotYet::layer_absent("the world's WorldInfo", "ow-world (World::new)", RECORD).into())
}

/// The actor's world (notes 3.1 and 3.5): the receiver's own `WorldInfo` field when its class declares it and it is
/// set, as the behavior kernel's `CanPlayBehaviorData` reads it; else the world's (the combat state's); with neither,
/// `NotYet { LayerAbsent }`, "the world's WorldInfo".
fn actor_world(vm: &mut Vm, me: ObjRef) -> Result<ObjRef, VmError> {
    if let Some(v) = get(vm, me, "WorldInfo")? {
        if let Some(w) = crate::hit::access::object_of(&v) {
            return Ok(w);
        }
    }
    world(vm)
}

/// The local players: the level's list when the VM has a world's actor service, else the controllers the combat
/// run made.
fn local_player_count(vm: &Vm) -> Result<usize, VmError> {
    if vm.service::<ow_world::actors::Actors>().is_some() {
        let actors = ow_world::actors::actors_of(vm)?;
        let n = actors.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.local_players().len();
        return Ok(n);
    }
    with_state(vm, |s| s.players.len())
}

/// Whether `world` is not standalone, by `NM_Standalone`'s position in the enum `NetMode` is declared with, never a
/// constant (as the behavior kernel reads `NM_Client`).
fn networked(vm: &mut Vm, world: ObjRef, what: &str) -> Result<bool, VmError> {
    let class = vm.space.heap.get(world)?.class;
    let p = vm
        .space
        .prop(class, "NetMode")
        .ok_or_else(|| error(format!("{what}: {} declares no NetMode", crate::hit::access::class_name(vm, world))))?;
    let Some(PropKind::Byte { enum_: Some(e) }) = vm.space.classes.decl(p).map(|d| d.kind.clone()) else {
        return Err(error(format!("{what}: NetMode is not declared with its enum")));
    };
    let standalone = vm
        .space
        .classes
        .enum_layout(e)
        .and_then(|l| l.values().iter().position(|v| vm.space.names.text(*v).eq_ignore_ascii_case("NM_Standalone")))
        .and_then(|i| u8::try_from(i).ok())
        .ok_or_else(|| error(format!("{what}: the NetMode enum has no NM_Standalone")))?;
    match vm.space.read(world, PropAddr::scalar(p))? {
        Value::Byte(m) => Ok(*m != standalone),
        other => Err(error(format!("{what}: NetMode holds {}", other.kind_name()))),
    }
}

/// `Pawn.IsLocallyControlled` (3.1): the pawn's `Controller`, else its `DrivenVehicle`'s; none is false; a standalone
/// world ([`actor_world`]) is true (an Off assumption); a networked one is `NotRead`.
fn is_locally_controlled(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "Pawn.IsLocallyControlled";
    let me = this(c, WHAT)?;
    let mut controller = object_required(c.vm, me, "Controller", WHAT)?;
    if controller.is_none() {
        if let Some(vehicle) = object_required(c.vm, me, "DrivenVehicle", WHAT)? {
            controller = object_required(c.vm, vehicle, "Controller", WHAT)?;
        }
    }
    if controller.is_none() {
        return Ok(Value::Bool(false));
    }
    let world = actor_world(c.vm, me)?;
    if networked(c.vm, world, WHAT)? {
        return Err(NotYet::not_read("Pawn.IsLocallyControlled on a networked world", RECORD).into());
    }
    Ok(Value::Bool(true))
}

/// The object field `name` of `o`, which its class must declare (`VmError::Script` naming it otherwise).
fn object_required(vm: &mut Vm, o: ObjRef, name: &str, what: &str) -> Result<Option<ObjRef>, VmError> {
    Ok(crate::hit::access::object_of(&required(vm, o, name, what)?))
}

/// `WillowPlayerController.GetSkillManager` (3.5): the controller's world's game info (`WorldInfo.Game`, an Off
/// assumption) when it is a `WillowGameInfo`, through its own `GetSkillManager`; else None. No world is not a None
/// answer but a missing layer ([`actor_world`]: `NotYet { LayerAbsent }`). It writes nothing.
fn get_skill_manager(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "WillowPlayerController.GetSkillManager";
    let me = this(c, WHAT)?;
    let world = actor_world(c.vm, me)?;
    let Some(game) = object_required(c.vm, world, "Game", WHAT)? else {
        return Ok(Value::Object(None));
    };
    if !is_a(c.vm, game, "WillowGame.WillowGameInfo")? {
        return Ok(Value::Object(None));
    }
    match call_method(c.vm, game, "GetSkillManager", Vec::new())? {
        Some(v) => Ok(v),
        None => Err(error(format!("{WHAT}: the game info has no GetSkillManager"))),
    }
}

/// `WillowPawn.GetTechDeathIndexForDamageType(DamageType)` (3.4): -1 wherever steps 1 to 3 decide without the
/// status-effect test, else `NotRead` for that test. Writes nothing.
fn tech_death_index(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "WillowPawn.GetTechDeathIndexForDamageType";
    let me = this(c, WHAT)?;
    let none = Value::Int(-1);
    let gibbed = required(c.vm, me, "bGibbed", WHAT)?;
    if truth(c.vm, &gibbed) {
        return Ok(none);
    }
    if object_required(c.vm, me, "MyDeathDef", WHAT)?.is_none()
        || object_required(c.vm, me, "StatusEffectComp", WHAT)?.is_none()
    {
        return Ok(none);
    }
    let Some(dt) = arg_object(c, 0)? else {
        return Ok(none);
    };
    if !is_a(c.vm, dt, "WillowGame.WillowDamageTypeDefinition")? {
        return Ok(none);
    }
    let kind = integer(&required(c.vm, dt, "DamageType", WHAT)?);
    if !TECH_DEATH_KINDS.contains(&kind) {
        return Ok(none);
    }
    Err(NotYet::not_read("GetTechDeathIndexForDamageType: the status-effect test of the damage type's kind", RECORD).into())
}

/// A `WillowGlobals` getter that answers its field as stored (4.2, NATIVE_RESPAWN_STATIONS.md 3.2).
fn globals_field(c: &mut NativeCall<'_>, function: &str, field: &str) -> Result<Value, VmError> {
    let what = format!("WillowGlobals.{function}");
    let me = this(c, &what)?;
    required(c.vm, me, field, &what)
}

/// `WillowPlayerPawn.SetShadowCasting(bool)` (4.4): nothing to do with no `InstanceDataState.Data` entry; any entry
/// is `NotRead` (which entries it visits needs the component resolution, and the flags it sets are not proven).
fn set_shadow_casting(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "WillowPlayerPawn.SetShadowCasting";
    let me = this(c, WHAT)?;
    let state = required(c.vm, me, "InstanceDataState", WHAT)?;
    let Some(data) = crate::hit::access::field(c.vm, &state, "Data") else {
        return Err(error(format!("{WHAT}: InstanceDataState has no field Data")));
    };
    if data.elements().is_some_and(|d| d.is_empty()) {
        return Ok(Value::None);
    }
    if data.elements().is_none() {
        return Err(error(format!("{WHAT}: InstanceDataState.Data is not an array")));
    }
    Err(NotYet::not_read("SetShadowCasting: which component flags it sets", RECORD).into())
}

/// `Controller.IsUsingVehicleEx(bCheckDrivenVehicle, out Vehicle)` (NATIVE_RESPAWN_STATIONS.md 3.1): with no `Pawn`,
/// false and a None vehicle; with one, the pawn's own vehicle answer is not read (`NotRead`).
fn is_using_vehicle_ex(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "Controller.IsUsingVehicleEx";
    let me = this(c, WHAT)?;
    if object_required(c.vm, me, "Pawn", WHAT)?.is_some() {
        return Err(NotYet::not_read("Controller.IsUsingVehicleEx: the pawn's vehicle answer", RECORD_RESPAWN).into());
    }
    if c.arg_count() > 1 {
        c.set_out(1, Value::Object(None))?;
    }
    Ok(Value::Bool(false))
}

/// `GearboxDialogComponent.StopTalking(optional Tag, optional bool)` (NATIVE_DIALOG.md): with no live line (the
/// component's `EventData` None, or its `LiveTalkAction` None) and the flag clear, it does nothing. Every other case
/// writes through steps this lane does not port: `NotPorted`.
fn stop_talking(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "GearboxDialogComponent.StopTalking";
    let me = this(c, WHAT)?;
    let live = match object_required(c.vm, me, "EventData", WHAT)? {
        None => false,
        Some(data) => object_required(c.vm, data, "LiveTalkAction", WHAT)?.is_some(),
    };
    if !live && !arg_truth(c, 1)? {
        return Ok(Value::None);
    }
    Err(NotYet::not_ported("GearboxDialogComponent.StopTalking: the stop of a live line and its notifications", RECORD_DIALOG).into())
}

#[cfg(test)]
mod tests;
