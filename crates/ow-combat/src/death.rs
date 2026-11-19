//! Owner: lane W6-COMBAT. Death on the VM (plan step S-c): the world a kill needs, what a pawn's death needs, and the
//! natives the installed death chain reaches. Port of `src/combat_death.cpp`. AI-assisted (Claude Code).
//!
//! Records: COMBAT_DEATH.md and NATIVE_COMBAT_GROUNDWORK.md section 4.2. Every rule is **UNVERIFIED** in the running
//! game unless a line says otherwise.
//!
//! **What is not here.** The loot natives the C++ death half binds (`DropLootOnDeath` and the pool natives,
//! `src/combat_loot.cpp`) are lane W6-LOOT's (`crate::loot`). Two natives the C++ binds here are the lower packs'
//! (`hit::LEFT_TO_LOWER_PACKS`): `ResourcePool.GetCurrentValue` (W5-POOLS, the same reading) and `Actor.Destroy`
//! (W5-WORLD: the world's destroy rule, which removes the actor where the C++ only sets `bDeleteMe` and logs
//! `Destroy <name>`).

use std::rc::Rc;

use ow_behavior::kernel::natives;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;
use ow_vm::interp::instantiate_from_defaults;

use crate::hit::access::{
    field, get, integer, loaded, name_of, object, object_of, required, set, set_field,
    set_required, truth,
};
use crate::hit::{
    arg_object, arg_text, bind_fn, call_method, copy_of, is_player_controller, not_implemented,
    note, this, with_state,
};

/// The records of death.
pub const RECORD: &str = "COMBAT_DEATH.md; NATIVE_COMBAT_GROUNDWORK.md section 4.2";

/// This crate's name as the owner of the consumer handles it gives out (`ow_behavior::kernel::natives`).
pub const CONSUMER_OWNER: &str = "ow-combat";

/// A behavior event the death chain raises on a pawn's providers (`BodyClassDeathDefinition.OnKilledBy`,
/// `OnPlayDeathPizazz`, `OnDeathNonGib`, `OnDeathGib`, and a custom event a script behavior broadcasts). The owner of
/// the providers raises it ([`crate::hit::CombatState::on_behavior_event`]).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BehaviorEvent {
    pub pawn: ObjRef,
    pub definition: Option<ObjRef>,
    pub killer: Option<ObjRef>,
    pub name: String,
    /// `BehaviorKernel.BroadcastBehaviorEventFromScript`: every provider of the consumer, not the death definition's.
    pub broadcast: bool,
}

/// The death definition events the natives raise.
pub const DEATH_EVENTS: [&str; 4] = [
    "OnKilledBy",
    "OnPlayDeathPizazz",
    "OnDeathNonGib",
    "OnDeathGib",
];

/// Death bookkeeping with nothing to keep in the VM (no status effects, no sounds, no ragdoll, no allegiance
/// hierarchy): bound no-ops, as the C++ binds them.
pub const PRESENTATION_NO_OPS: [&str; 8] = [
    "WillowGame.WillowPawn.StopSounds",
    "WillowGame.WillowAIPawn.DropCombatPerch",
    "WillowGame.WillowAIPawn.SetVehicleRBActorEnabled",
    "Engine.Pawn.PlayDyingSound",
    "Engine.PrimitiveComponent.SetRBChannel",
    "Engine.PrimitiveComponent.SetRBCollidesWithChannel",
    "GearboxFramework.SpecialMoveComponent.StopAny",
    "WillowGame.WillowPawn.PlayPainSound",
];

/// The same, with an event naming what had nothing to update.
pub const LOGGED_NO_OPS: [&str; 3] = [
    "Engine.PawnAllegiance.RemoveParent",
    "Engine.PawnAllegiance.RemoveChildren",
    "WillowGame.StatusEffectsComponent.OwnerDied",
];

/// The world a kill needs (C++ `enableDeaths`). The installed `Pawn.Died` begins with a test of `WorldInfo.Game` and
/// returns without one, and the kill credit is the game's script (`WillowGameInfo.Killed`), so the world gets a
/// `WillowGameInfo`; `WillowPawn.HandleHealthDepleted` reaches the mission tracker through `WorldInfo.GRI`, so a
/// world that has none gets a `WillowGameReplicationInfo` with a `MissionTracker`. Both are plain instances of the
/// installed classes at their defaults. A package set without these classes has no deaths to run (nothing is
/// enabled, as the C++ returns). A pawn adopted from now on is prepared for death ([`prepare_for_death`]).
///
/// **The replication info is a harness stand-in** (review r1 of #393; the same kind of label as decision 2026-11-16-49's
/// game info). The running game spawns its own; this one is a class-default `WillowGameReplicationInfo`. Since
/// decision 2026-11-19-06 the world's `GameReplicationInfo` query answers it (`ow_world::query`), in both parity modes,
/// so Off's playthrough native reads its default `CurrentPlaythrough` 0: **the run is assumed to be Normal mode**
/// (playthrough count 1). UNVERIFIED as the game's state for any run.
pub fn enable_deaths(vm: &mut Vm) -> Result<(), VmError> {
    let (deaths, world, authority) = with_state(vm, |s| (s.deaths, s.world, s.authority))?;
    if deaths {
        return Ok(());
    }
    let Some(world) = world else {
        return Err(crate::hit::access::error(
            "combat death: the run has no world",
        ));
    };
    if object(vm, world, "Game")?.is_none() {
        let Ok(class) = vm.find_class("WillowGame.WillowGameInfo") else {
            return Ok(());
        };
        let game = instantiate_from_defaults(vm, class)?;
        set_required(vm, game, "Role", Value::Byte(authority), "combat death")?;
        set_required(
            vm,
            game,
            "WorldInfo",
            Value::Object(Some(world)),
            "combat death",
        )?;
        set_required(vm, world, "Game", Value::Object(Some(game)), "combat death")?;
    }
    if object(vm, world, "GRI")?.is_none() {
        let (Ok(tracker_class), Ok(info_class)) = (
            vm.find_class("WillowGame.MissionTracker"),
            vm.find_class("WillowGame.WillowGameReplicationInfo"),
        ) else {
            return Ok(());
        };
        let tracker = instantiate_from_defaults(vm, tracker_class)?;
        let info = instantiate_from_defaults(vm, info_class)?;
        set_required(
            vm,
            info,
            "MissionTracker",
            Value::Object(Some(tracker)),
            "combat death",
        )?;
        set_required(vm, world, "GRI", Value::Object(Some(info)), "combat death")?;
    }
    with_state(vm, |s| s.deaths = true)
}

/// What the stock spawn gives a pawn that its death needs (C++ `prepareForDeath`; the rest of that path is not run):
/// an inventory manager of its `InventoryManagerClass` (`WillowPawn.Died` drops loot only for a pawn that has one;
/// the inventory stays empty), its own copies of its `StatusEffectComp` and `SMComponent` (the archetype holds them
/// as references to its subobjects), the death definition of its body class (`MyDeathDef`, which the stock spawn sets
/// from `ApplyBodyClassStartingValues`), a consumer handle (from 100 up), and walking physics for a pawn whose
/// `Physics` is `PHYS_None` (a pawn placed on the ground; the stock landing sets it, UNVERIFIED stand-in, the
/// bullymong's class provider tests it).
pub fn prepare_for_death(vm: &mut Vm, pawn: ObjRef) -> Result<(), VmError> {
    let unit = "combat death";
    let authority = with_state(vm, |s| s.authority)?;
    let world = with_state(vm, |s| s.world)?;
    if let Some(manager) = get(vm, pawn, "InvManager")?
        && object_of(&manager).is_none()
        && let Some(Value::Class(Some(class))) = get(vm, pawn, "InventoryManagerClass")?
    {
        let made = instantiate_from_defaults(vm, class)?;
        set_required(vm, made, "Role", Value::Byte(authority), unit)?;
        set_required(vm, made, "WorldInfo", Value::Object(world), unit)?;
        set(vm, pawn, "InvManager", Value::Object(Some(made)))?;
    }
    for name in ["StatusEffectComp", "SMComponent"] {
        if let Some(component) = object(vm, pawn, name)?
            && matches!(
                vm.space.heap.get(component)?.origin,
                ow_object::heap::Origin::Export { .. }
            )
        {
            let own = copy_of(vm, component)?;
            set(vm, pawn, name, Value::Object(Some(own)))?;
        }
    }
    if let Some(body) = object(vm, pawn, "BodyClass")? {
        let body = loaded(vm, body)?;
        if let Some(definition) = object(vm, body, "DeathDef")? {
            let definition = loaded(vm, definition)?;
            set_required(
                vm,
                pawn,
                "MyDeathDef",
                Value::Object(Some(definition)),
                unit,
            )?;
        }
    }
    if let Some(mut handle) = get(vm, pawn, "ConsumerHandle")?
        && field(vm, &handle, "PID").is_some_and(|pid| integer(&pid) == -1)
    {
        // The handle is claimed in the kernel's consumer table, so the kernel natives route it here.
        let from = with_state(vm, |s| s.next_consumer)?;
        let pid = natives::allocate(vm, CONSUMER_OWNER, from)?;
        with_state(vm, |s| {
            s.consumers.insert(pid, pawn);
            s.next_consumer = pid.saturating_add(1);
        })?;
        set_field(vm, &mut handle, "PID", Value::Int(pid));
        set(vm, pawn, "ConsumerHandle", handle)?;
    }
    if let Ok(physics) = ow_behavior::provider::enum_names(vm, "Engine", "Actor.EPhysics")
        && let Some(walking) = physics.iter().position(|n| n == "PHYS_Walking")
        && let Some(current) = get(vm, pawn, "Physics")?
        && integer(&current) == 0
        && let Ok(walking) = u8::try_from(walking)
    {
        set(vm, pawn, "Physics", Value::Byte(walking))?;
    }
    Ok(())
}

/// The pawn a consumer handle struct names (its `PID`), if any.
fn consumer_of(vm: &Vm, handle: &Value) -> Result<Option<ObjRef>, VmError> {
    let Some(pid) = field(vm, handle, "PID") else {
        return Ok(None);
    };
    // A PID is an int; the low 32 bits as the C++ map key.
    let pid = crate::hit::access::low_i32(integer(&pid));
    with_state(vm, |s| s.consumers.get(&pid).copied())
}

/// Hands `event` to the owner's raise, if one is set and the event has a pawn.
fn raise(vm: &mut Vm, event: BehaviorEvent) -> Result<(), VmError> {
    let hook = with_state(vm, |s| s.on_behavior_event.clone())?;
    match hook {
        Some(hook) => hook(vm, &event),
        None => Ok(()),
    }
}

/// An expression object's `Evaluate(context)` (false for a class without it).
fn evaluate_child(vm: &mut Vm, child: ObjRef, context: Value) -> Result<bool, VmError> {
    let child = loaded(vm, child)?;
    Ok(call_method(vm, child, "Evaluate", vec![context])?.is_some_and(|v| truth(vm, &v)))
}

pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    // Pawn.IsHumanControlled: the pawn's controller is a player controller.
    bind_fn(vm, "Engine.Pawn.IsHumanControlled", |c| {
        let me = this(c, "IsHumanControlled")?;
        let controller = object(c.vm, me, "Controller")?;
        Ok(Value::Bool(is_player_controller(c.vm, controller)?))
    })?;

    // MissionTracker.NotifyPawnDied(KilledPawn, InstigatedBy, DamageTypeClass, DamageTypeDefinition, DamageCauser,
    // bKilledByCritical, HitInfo) (NATIVE_COMBAT_GROUNDWORK.md section 4.2, UNVERIFIED): the defend entries of the
    // tracker, the pawn's own objective and the objectives registered under the class's KilledStatID and the
    // allegiance's stat name. The VM's tracker registers none of the latter, so with no defend entry and no objective
    // on the pawn nothing observable happens; a pawn that has one of them is not implemented, never silently skipped.
    bind_fn(vm, "WillowGame.MissionTracker.NotifyPawnDied", |c| {
        let me = this(c, "NotifyPawnDied")?;
        let dead = arg_object(c, 0)?;
        let mut stat = String::new();
        if let Some(dead) = dead {
            if object(c.vm, dead, "MissionObjectiveToUpdateOnDeath")?.is_some() {
                not_implemented(
                    c.vm,
                    "MissionTracker.NotifyPawnDied: the pawn's MissionObjectiveToUpdateOnDeath",
                    RECORD,
                )?;
            }
            if let Some(controller) = object(c.vm, dead, "Controller")?
                && let Some(class) = object(c.vm, controller, "CharacterClass")?
            {
                let class = loaded(c.vm, class)?;
                if let Some(Value::Name(n)) = get(c.vm, class, "KilledStatID")? {
                    stat = c.vm.space.names.text(n).into_owned();
                }
            }
        }
        if !crate::hit::access::array(c.vm, me, "DefendMissions")?.is_empty() {
            not_implemented(
                c.vm,
                "MissionTracker.NotifyPawnDied: defend entries",
                RECORD,
            )?;
        }
        note(
            c.vm,
            format!(
                "NotifyPawnDied killed_stat={stat} (no objective is registered with the VM tracker for it)"
            ),
        )?;
        Ok(Value::None)
    })?;

    // The death definition's behavior events (WillowPawn.Died: OnKilledBy; PlayDying: OnPlayDeathPizazz,
    // OnDeathNonGib / OnDeathGib): the native raises the event on the definition's provider for the consumer the
    // handle names. The raise belongs to the owner of the providers.
    for name in DEATH_EVENTS {
        bind_fn(
            vm,
            &format!("WillowGame.BodyClassDeathDefinition.{name}"),
            move |c| {
                let me = this(c, name)?;
                let pawn = consumer_of(c.vm, c.arg(0)?)?;
                let killer = if c.arg_count() > 1 {
                    arg_object(c, 1)?
                } else {
                    None
                };
                let shown = match pawn {
                    Some(p) => name_of(c.vm, p),
                    None => "?".to_string(),
                };
                note(c.vm, format!("{name} pawn={shown}"))?;
                if let Some(pawn) = pawn {
                    raise(
                        c.vm,
                        BehaviorEvent {
                            pawn,
                            definition: Some(me),
                            killer,
                            name: name.to_string(),
                            broadcast: false,
                        },
                    )?;
                }
                Ok(Value::None)
            },
        )?;
    }
    // BehaviorKernel.ActivateBehaviorOutputLink, IBehaviorConsumer.GetBehaviorConsumerHandle and the routed kernel
    // natives are the behavior kernel pack's (ow_behavior::kernel::natives). It runs the bodies below for the handles
    // this crate gave out, and in Reference also for a handle nobody owns, as the C++ combat pack binds them.
    // IBehaviorConsumer.GetBehaviorConsumerHandle on the pawn: its ConsumerHandle (Off reads it in the kernel pack).
    let handle: NativeFn = Rc::new(|c: &mut NativeCall<'_>| {
        let me = this(c, "GetBehaviorConsumerHandle")?;
        required(c.vm, me, "ConsumerHandle", "combat death")
    });
    natives::route(vm, CONSUMER_OWNER, natives::CONSUMER_HANDLE, handle.clone())?;
    natives::fallback(vm, CONSUMER_OWNER, natives::CONSUMER_HANDLE, handle)?;
    // BehaviorKernel.BroadcastBehaviorEventFromScript(ConsumerHandle, EventName, ...): a script behavior raises a
    // named event on every provider the consumer has registered. The note is written for every PID, as in the C++.
    let broadcast: NativeFn = Rc::new(|c: &mut NativeCall<'_>| {
        let event = arg_text(c, 1)?;
        let pawn = consumer_of(c.vm, c.arg(0)?)?;
        note(c.vm, format!("BroadcastBehaviorEventFromScript {event}"))?;
        if let Some(pawn) = pawn {
            raise(
                c.vm,
                BehaviorEvent {
                    pawn,
                    definition: None,
                    killer: None,
                    name: event,
                    broadcast: true,
                },
            )?;
        }
        Ok(Value::None)
    });
    natives::route(vm, CONSUMER_OWNER, natives::BROADCAST_FROM_SCRIPT, broadcast.clone())?;
    natives::fallback(vm, CONSUMER_OWNER, natives::BROADCAST_FROM_SCRIPT, broadcast)?;

    // Expression evaluators (Behavior_Conditional's conditions; the natives' bodies are unread, so these three are the
    // data's plain reading, UNVERIFIED): a tree evaluates its root; a compound combines its two expressions by its
    // operator enum (AND / OR by name); a physics-state expression compares the context pawn's Physics with the
    // expression's state, inverted when bIsInState is false.
    bind_fn(vm, "Engine.ExpressionTree.Evaluate", |c| {
        let me = this(c, "ExpressionTree.Evaluate")?;
        let context = c.arg(0)?.clone();
        Ok(Value::Bool(match object(c.vm, me, "RootChild")? {
            Some(root) => evaluate_child(c.vm, root, context)?,
            None => true,
        }))
    })?;
    bind_fn(
        vm,
        "GearboxFramework.CompoundExpressionEvaluator.Evaluate",
        |c| {
            let me = this(c, "CompoundExpressionEvaluator.Evaluate")?;
            let context = c.arg(0)?.clone();
            let operators = ow_behavior::provider::enum_names(
                c.vm,
                "GearboxFramework",
                "CompoundExpressionEvaluator.ECExpressionOperatorType",
            )
            .map_err(|e| crate::hit::access::error(e.to_string()))?;
            let which = integer(&required(c.vm, me, "Operator", "combat death")?);
            let name = usize::try_from(which)
                .ok()
                .and_then(|i| operators.get(i))
                .map(|n| n.to_ascii_lowercase())
                .unwrap_or_default();
            let side = |vm: &mut Vm, property: &str| -> Result<bool, VmError> {
                match object(vm, me, property)? {
                    Some(child) => evaluate_child(vm, child, context.clone()),
                    None => Ok(false),
                }
            };
            let first = side(c.vm, "Expression1")?;
            let second = side(c.vm, "Expression2")?;
            if name.ends_with("_and") {
                return Ok(Value::Bool(first && second));
            }
            if name.ends_with("_or") {
                return Ok(Value::Bool(first || second));
            }
            not_implemented(
                c.vm,
                &format!("CompoundExpressionEvaluator.Evaluate: operator {name}"),
                RECORD,
            )?;
            Ok(Value::Bool(false))
        },
    )?;
    bind_fn(
        vm,
        "GearboxFramework.PhysicsStateExpressionEvaluator.Evaluate",
        |c| {
            let me = this(c, "PhysicsStateExpressionEvaluator.Evaluate")?;
            let Some(context) = arg_object(c, 0)? else {
                return Ok(Value::Bool(false));
            };
            let physics = get(c.vm, context, "Physics")?;
            let state = integer(&required(c.vm, me, "PhysicsState", "combat death")?);
            let in_state = physics.is_some_and(|p| integer(&p) == state);
            let wanted = required(c.vm, me, "bIsInState", "combat death")?;
            Ok(Value::Bool(in_state == truth(c.vm, &wanted)))
        },
    )?;

    for path in PRESENTATION_NO_OPS {
        bind_fn(vm, path, |_| Ok(Value::None))?;
    }
    for path in LOGGED_NO_OPS {
        bind_fn(vm, path, move |c| {
            note(c.vm, format!("{path} (nothing to update in the VM)"))?;
            Ok(Value::None)
        })?;
    }
    // Pawn.InitRagdoll: the host builds the ragdoll; the death clip is the host's, so it is taken as having worked
    // (the C++ reading: a presentation answer, not a game rule).
    bind_fn(vm, "Engine.Pawn.InitRagdoll", |_| Ok(Value::Bool(true)))?;
    // The spawned-inventory levels are plain fields of the pawn (NATIVE_LOOT.md).
    bind_fn(vm, "WillowGame.WillowAIPawn.GetAwesomeLevel", |c| {
        let me = this(c, "GetAwesomeLevel")?;
        // An int field: the low 32 bits, as the C++ makeInt.
        Ok(Value::Int(
            get(c.vm, me, "AwesomeLevel")?.map_or(0, |v| crate::hit::access::low_i32(integer(&v))),
        ))
    })?;
    bind_fn(
        vm,
        "WillowGame.WillowAIPawn.GetGameStageForSpawnedInventory",
        |c| {
            let me = this(c, "GetGameStageForSpawnedInventory")?;
            Ok(Value::Int(
                get(c.vm, me, "GameStageForSpawnedInventory")?
                    .map_or(0, |v| crate::hit::access::low_i32(integer(&v))),
            ))
        },
    )?;
    Ok(())
}

#[cfg(test)]
mod tests;
