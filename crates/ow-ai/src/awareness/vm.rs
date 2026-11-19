//! Owner: lane W7-AI (part 1). The awareness rules over the VM's objects: the pawn, its `WillowMind`, the mind's
//! `AIComponent` and the definitions, and the script-facing natives. AI-assisted (Claude Code).
//!
//! **Reads.** Settings come from the component's `AIDef` (`TargetSearchRadius`; `MinEngagementTime` and
//! `MinProvokedTime` when it is a den definition) and `TargetingDef` (the search radius override, the evaluators, the
//! iterators by class with their `Weight` and fields), the mind (`SightViewCone`,
//! `MinAggroTimeToConsiderAwareOfTarget`), the pawn's `CylinderComponent.CollisionRadius`, and
//! `GlobalsDefinition.AICloseThreatRadius`. A component whose `AIDef` or `TargetingDef` is None is `NotYet`: the game
//! binds them natively when the AI starts, and that code is not read (installed script never sets `AIDef`; only
//! `Behavior_AITargeting` swaps `TargetingDef`). A caller that sets them as the live game shows them (the AI class's
//! `AIDef` and its `TargetingDef`) gets the rules.
//!
//! **Writes after a tick.** `CurrentTarget`, `LastStateUpdateTime` and `LastTargetUpdateTime` on the component;
//! `bProvoked` and `ProvokedLocation` on the mind; a target's `NumAICurrentlyTargeting`. Script can clear `bProvoked`
//! (`Action_Patrol.SetMoveNode`), so the mind's flag and location, the component's `Holds` and `ScriptedTarget` are
//! read back before every tick. What the adapter does not apply (the `Target_*` events to the tree's actions, the
//! pawn's `SetLookAtTarget`) is in the report: the actions are AI part 2, the head look is presentation.
//!
//! **What the host answers** ([`Host`]): the exposure of a target to a viewer (the engine's visibility service, line
//! of sight through the level's collision), the world's registry of targetables, and an actor's own "can be targeted"
//! answer. A vehicle is `NotYet` (vehicles are not in the Rust world).
//!
//! **No parent.** A component's parent (`GetParentComponent`, a den's component for a den member) is not modelled:
//! the Rust world spawns no population den members, so every component here is lone. When dens arrive, the adapter
//! sets [`Settings::has_parent`] and the rules answer `NotYet` until the den's rules are ported.
//!
//! **The behavior kernel.** No AI behavior class runs through a handler here. The awareness behaviors
//! (`Behavior_AIProvoke`, `Behavior_AITargeting`, `Behavior_AIPriority`) are script behaviors whose scripts call the
//! natives this pack binds; whoever runs their provider runs them with `ow_behavior::kernel::natives::collect` (the
//! shared collector) and its consumer routing, as `ow-combat` does for a pawn's providers.

use std::cell::RefCell;
use std::rc::Rc;

use ow_combat::hit::access::{field, get, is_a, number, object, set, set_field, truth};
use ow_core::units::Rotator;
use ow_core::{DetMap, NotYet};
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, NativePack, Vm};
use ow_vm::error::VmError;

use super::record::{ActorFacts, ActorKind, Component, Knowledge, Opinion, Prioritizer, Settings, ThreatLevel};
use super::targets::Effect;
use super::tick::{TickReport, tick};
use super::{DECISION, RECORD_AWARENESS, RECORD_MELEE, Senses, Vec3, threat};

#[cfg(test)]
pub(crate) mod tests;

/// The stock globals that hold `AICloseThreatRadius`.
pub const GLOBALS_PATH: &str = "GD_Globals.General.Globals";

/// What the host answers that the VM does not hold (module documentation).
pub trait Host {
    /// The exposure of `target` to `viewer`, 0 to 1 (`TargetExposureToMe`): line of sight through the level.
    fn exposure(&mut self, vm: &mut Vm, viewer: ObjRef, target: ObjRef) -> Result<f32, VmError>;
    /// The world's registry of targetables, in its order.
    fn targetables(&mut self, vm: &mut Vm) -> Result<Vec<ObjRef>, VmError>;
    /// Whether `actor` is in that registry. The default lists the registry; a host that keeps a set answers directly.
    fn is_registered(&mut self, vm: &mut Vm, actor: ObjRef) -> Result<bool, VmError> {
        Ok(self.targetables(vm)?.contains(&actor))
    }
    /// The actor's own "can be targeted" answer.
    fn can_be_targeted(&mut self, vm: &mut Vm, actor: ObjRef) -> Result<bool, VmError>;
}

/// The rules' [`Senses`] over the VM and a host.
pub struct VmSenses<'a> {
    pub vm: &'a mut Vm,
    pub host: &'a mut dyn Host,
}

fn vector(vm: &Vm, v: Option<Value>) -> Vec3 {
    let Some(v) = v else { return [0.0; 3] };
    let axis = |name: &str| field(vm, &v, name).map_or(0.0, |x| number(&x) as f32);
    [axis("X"), axis("Y"), axis("Z")]
}

fn rotator(vm: &Vm, v: Option<Value>) -> Rotator {
    let Some(v) = v else {
        return Rotator::default();
    };
    let axis = |name: &str| match field(vm, &v, name) {
        Some(Value::Int(i)) => i,
        _ => 0,
    };
    Rotator::new(axis("Pitch"), axis("Yaw"), axis("Roll"))
}

fn flag(vm: &mut Vm, o: ObjRef, name: &str) -> Result<bool, VmError> {
    Ok(match get(vm, o, name)? {
        Some(v) => truth(vm, &v),
        None => false,
    })
}

fn float(vm: &mut Vm, o: ObjRef, name: &str) -> Result<f32, VmError> {
    match get(vm, o, name)? {
        Some(v) => Ok(number(&v) as f32),
        None => Err(missing(vm, o, name)),
    }
}

fn missing(vm: &Vm, o: ObjRef, name: &str) -> VmError {
    VmError::Script {
        function: "ai awareness".to_string(),
        message: format!("{} has no property {name}", ow_combat::hit::access::class_name(vm, o)),
    }
}

fn class_path(vm: &Vm, o: ObjRef) -> Result<String, VmError> {
    let class = vm.space.heap.get(o)?.class;
    Ok(vm
        .space
        .classes
        .get_layout(class)
        .map(|l| l.path().as_str().to_string())
        .unwrap_or_default())
}

impl Senses for VmSenses<'_> {
    fn facts(&mut self, actor: ObjRef) -> Result<ActorFacts, VmError> {
        let vm = &mut *self.vm;
        let location = get(vm, actor, "Location")?;
        let location = vector(vm, location);
        let rotation = get(vm, actor, "Rotation")?;
        let rotation = rotator(vm, rotation);
        let kind = if is_a(vm, actor, "Engine.Vehicle")? {
            return Err(NotYet::not_ported("a vehicle as an AI's target", RECORD_AWARENESS).into());
        } else if is_a(vm, actor, "Engine.Pawn")? {
            ActorKind::Pawn {
                controlled: object(vm, actor, "Controller")?.is_some(),
                driving: object(vm, actor, "DrivenVehicle")?.is_some(),
                weapon_seat: is_a(vm, actor, "WillowGame.WillowWeaponPawn")?,
            }
        } else {
            ActorKind::Other
        };
        let mut facts = ActorFacts::new(location, rotation, kind);
        facts.can_be_targeted = self.host.can_be_targeted(vm, actor)?;
        facts.has_allegiance = object(vm, actor, "Allegiance")?.is_some();
        facts.cloaked = flag(vm, actor, "bCloaked")?;
        facts.hidden = flag(vm, actor, "bHidden")?;
        facts.deleted = flag(vm, actor, "bDeleteMe")?;
        Ok(facts)
    }

    fn opinion(&mut self, pawn: ObjRef, other: ObjRef) -> Result<Opinion, VmError> {
        Ok(match ow_combat::hit::opinion(self.vm, pawn, Some(other))? {
            0 => Opinion::Enemy,
            2 => Opinion::Friendly,
            _ => Opinion::Neutral,
        })
    }

    fn exposure(&mut self, viewer: ObjRef, target: ObjRef) -> Result<f32, VmError> {
        self.host.exposure(self.vm, viewer, target)
    }

    fn targetables(&mut self) -> Result<Vec<ObjRef>, VmError> {
        self.host.targetables(self.vm)
    }

    fn targeted_by(&mut self, actor: ObjRef) -> Result<i32, VmError> {
        // 1 when the actor has no readable counter (NATIVE_AI_MELEE.md, `WillowTI_PrioritizeIdle`).
        Ok(match get(self.vm, actor, "NumAICurrentlyTargeting")? {
            Some(Value::Int(i)) => i,
            _ => 1,
        })
    }

    fn is_registered(&mut self, actor: ObjRef) -> Result<bool, VmError> {
        self.host.is_registered(self.vm, actor)
    }

    fn tested_actor(&mut self, actor: ObjRef) -> Result<ObjRef, VmError> {
        let vm = &mut *self.vm;
        // A controller without a pawn is tested as itself (step 2 replaces only when the pawn is set).
        if is_a(vm, actor, "Engine.Controller")? {
            return Ok(object(vm, actor, "Pawn")?.unwrap_or(actor));
        }
        if let Some(controller) = object(vm, actor, "Controller")?
            && let Some(pawn) = object(vm, controller, "Pawn")?
        {
            return Ok(pawn);
        }
        Ok(actor)
    }
}

/// The settings of `mind`'s component (module documentation, "Reads"). `globals` holds `AICloseThreatRadius`.
pub fn settings(vm: &mut Vm, mind: ObjRef, globals: ObjRef) -> Result<Settings, VmError> {
    let pawn = object(vm, mind, "Pawn")?.ok_or_else(|| missing(vm, mind, "Pawn"))?;
    let component = object(vm, mind, "AIComponent")?.ok_or_else(|| missing(vm, mind, "AIComponent"))?;
    let unbound = || -> VmError {
        NotYet::not_read(
            "AIComponent.AIDef / TargetingDef unset: the native that binds them when the AI starts is not read",
            DECISION,
        )
        .into()
    };
    let aidef = object(vm, component, "AIDef")?.ok_or_else(unbound)?;
    let targeting = object(vm, component, "TargetingDef")?.ok_or_else(unbound)?;
    let search_radius = if flag(vm, targeting, "bOverrideSearchRadius")? {
        float(vm, targeting, "SearchRadius")?
    } else {
        float(vm, aidef, "TargetSearchRadius")?
    };
    let den = get(vm, aidef, "MinEngagementTime")?.is_some();
    let (seen_time, provoked_time) = if den {
        (float(vm, aidef, "MinEngagementTime")?, float(vm, aidef, "MinProvokedTime")?)
    } else {
        // The non-den defaults read from the executable (G26); not seen live (every live pawn had a den).
        (2.5, 10.0)
    };
    let collision_radius = match object(vm, pawn, "CylinderComponent")? {
        Some(c) => Some(float(vm, c, "CollisionRadius")?),
        None => None,
    };
    let mut knowledge = Vec::new();
    for item in ow_combat::hit::access::array(vm, targeting, "Knowledge")? {
        let Value::Object(Some(o)) = item else {
            knowledge.push(Knowledge::Empty);
            continue;
        };
        let o = ow_combat::hit::access::loaded(vm, o)?;
        knowledge.push(match class_path(vm, o)?.as_str() {
            "WillowGame.WillowTI_CalcThreat" => Knowledge::Threat,
            "WillowGame.WillowTI_CalcCanFire" => Knowledge::CanFire,
            "WillowGame.WillowTI_CalcExposure" => Knowledge::Exposure,
            "WillowGame.WillowTI_CalcDistances" => Knowledge::Distances,
            "WillowGame.WillowTI_CalcCombat" => Knowledge::Combat,
            other => {
                return Err(NotYet::not_read(format!("the Knowledge iterator {other}"), RECORD_MELEE).into());
            }
        });
    }
    let mut prioritization = Vec::new();
    for item in ow_combat::hit::access::array(vm, targeting, "Prioritization")? {
        let Value::Object(Some(o)) = item else { continue };
        let o = ow_combat::hit::access::loaded(vm, o)?;
        let weight = float(vm, o, "Weight")?;
        let p = match class_path(vm, o)?.as_str() {
            "GearboxFramework.TargetIteratorPrioritizeExposedTargets" => Prioritizer::Exposed,
            "GearboxFramework.TargetIteratorPrioritizeNearbyTarget" => Prioritizer::Nearby {
                max_distance: float(vm, o, "MaxDistance")?,
            },
            "GearboxFramework.TargetIteratorPrioritizePreviousTarget" => Prioritizer::Previous,
            "GearboxFramework.TI_PrioritizeBonus" => Prioritizer::Bonus,
            "WillowGame.WillowTI_PrioritizeIdle" => Prioritizer::Idle {
                max_targeting: get(vm, o, "MaxAICurrentlyTargeting")?.map(|v| number(&v) as f32),
            },
            "WillowGame.WillowTI_PrioritizeAttacking" => Prioritizer::Attacking,
            other => {
                return Err(NotYet::not_read(format!("the Prioritization iterator {other}"), RECORD_MELEE).into());
            }
        };
        prioritization.push((p, weight));
    }
    Ok(Settings {
        update_rate: float(vm, component, "UpdateRate")?,
        search_radius,
        sight_view_cone: float(vm, mind, "SightViewCone")?,
        close_threat_radius: float(vm, globals, "AICloseThreatRadius")?,
        collision_radius,
        min_aggro_time: float(vm, mind, "MinAggroTimeToConsiderAwareOfTarget")?,
        seen_time,
        provoked_time,
        knowledge,
        prioritization,
        can_target_friendlies: flag(vm, targeting, "bCanTargetFriendlies")?,
        can_target_if: object(vm, targeting, "CanTargetIf")?.is_some(),
        stop_targeting_if: object(vm, targeting, "StopTargetingIf")?.is_some(),
        can_target_friendlies_if: object(vm, targeting, "CanTargetFriendliesIf")?.is_some(),
        has_parent: false,
    })
}

/// The awareness state of every mind the adapter has ticked (a VM service), and the host the natives use.
#[derive(Default)]
pub struct Minds {
    minds: DetMap<ObjRef, Component>,
    globals: Option<ObjRef>,
    host: Option<Rc<RefCell<dyn Host>>>,
}

impl std::fmt::Debug for Minds {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Minds")
            .field("minds", &self.minds.keys().collect::<Vec<_>>())
            .field("globals", &self.globals)
            .field("host", &self.host.is_some())
            .finish()
    }
}

fn minds(vm: &mut Vm) -> Result<Rc<RefCell<Minds>>, VmError> {
    match vm.service::<Minds>() {
        Some(m) => Ok(m),
        None => vm.add_service(Minds::default()),
    }
}

fn borrow(m: &Rc<RefCell<Minds>>) -> Result<std::cell::RefMut<'_, Minds>, VmError> {
    m.try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("ow_ai::awareness::vm::Minds"))
}

/// Sets the host the natives ask and the globals object (`GlobalsDefinition`) the settings read.
pub fn install(vm: &mut Vm, host: Rc<RefCell<dyn Host>>, globals: ObjRef) -> Result<(), VmError> {
    let m = minds(vm)?;
    let mut m = borrow(&m)?;
    m.host = Some(host);
    m.globals = Some(globals);
    Ok(())
}

/// Whether `component` has a parent component, as the awareness state of the mind it belongs to says (a component
/// whose mind the adapter has no state for is lone: module documentation, "No parent").
pub(crate) fn component_has_parent(vm: &mut Vm, component: ObjRef) -> Result<bool, VmError> {
    let held: Vec<(ObjRef, bool)> = match vm.service::<Minds>() {
        Some(m) => m
            .try_borrow()
            .map_err(|_| VmError::ServiceBorrowed("ow_ai::awareness::vm::Minds"))?
            .minds
            .iter()
            .map(|(mind, c)| (*mind, c.settings.has_parent))
            .collect(),
        None => Vec::new(),
    };
    for (mind, parent) in held {
        if object(vm, mind, "AIComponent")? == Some(component) {
            return Ok(parent);
        }
    }
    Ok(false)
}

/// The awareness state of `mind`, if the adapter has it (a copy).
pub fn state(vm: &Vm, mind: ObjRef) -> Option<Component> {
    vm.service::<Minds>()?.try_borrow().ok()?.minds.get(&mind).cloned()
}

/// Takes `mind`'s state out of the service (made from its settings on first use), with the fields script can change
/// read back from the VM (module documentation, "Writes after a tick"). Everything that can fail is read first: a
/// failed read (a `NotYet` settings read, say) leaves the held state where it is.
fn take(vm: &mut Vm, mind: ObjRef, globals: ObjRef) -> Result<Component, VmError> {
    let settings = settings(vm, mind, globals)?;
    let pawn = object(vm, mind, "Pawn")?.ok_or_else(|| missing(vm, mind, "Pawn"))?;
    let component = object(vm, mind, "AIComponent")?.ok_or_else(|| missing(vm, mind, "AIComponent"))?;
    let last_state_update = float(vm, component, "LastStateUpdateTime")?;
    let last_target_update = float(vm, component, "LastTargetUpdateTime")?;
    let on_hold = !ow_combat::hit::access::array(vm, component, "Holds")?.is_empty();
    let scripted_target = object(vm, component, "ScriptedTarget")?;
    let provoked = flag(vm, mind, "bProvoked")?;
    let provoked_location = get(vm, mind, "ProvokedLocation")?;
    let provoked_location = vector(vm, provoked_location);
    let m = minds(vm)?;
    let held = borrow(&m)?.minds.remove(&mind);
    let mut c = match held {
        Some(mut c) => {
            c.settings = settings;
            c.pawn = pawn;
            c
        }
        None => {
            let mut c = Component::new(pawn, settings);
            c.last_state_update = last_state_update;
            c.last_target_update = last_target_update;
            c
        }
    };
    c.on_hold = on_hold;
    c.scripted_target = scripted_target;
    c.provoked = provoked;
    c.provoked_location = provoked_location;
    Ok(c)
}

/// Puts `mind`'s state back (first, so a failed write keeps it) and writes the script-visible fields; applies the
/// counter effects.
fn put(vm: &mut Vm, mind: ObjRef, c: Component, effects: &[Effect]) -> Result<(), VmError> {
    let m = minds(vm)?;
    borrow(&m)?.minds.insert(mind, c.clone());
    let component = object(vm, mind, "AIComponent")?.ok_or_else(|| missing(vm, mind, "AIComponent"))?;
    set(vm, component, "CurrentTarget", Value::Object(c.current))?;
    set(vm, component, "LastStateUpdateTime", Value::Float(c.last_state_update))?;
    set(vm, component, "LastTargetUpdateTime", Value::Float(c.last_target_update))?;
    set(vm, mind, "bProvoked", Value::Bool(c.provoked))?;
    if let Some(mut v) = get(vm, mind, "ProvokedLocation")? {
        for (axis, x) in ["X", "Y", "Z"].iter().zip(c.provoked_location) {
            set_field(vm, &mut v, axis, Value::Float(x));
        }
        set(vm, mind, "ProvokedLocation", v)?;
    }
    for e in effects {
        if let Effect::TargetingCount { actor, delta } = e
            && let Some(Value::Int(n)) = get(vm, *actor, "NumAICurrentlyTargeting")?
        {
            set(vm, *actor, "NumAICurrentlyTargeting", Value::Int(n.wrapping_add(*delta)))?;
        }
    }
    Ok(())
}

/// One AI tick of `mind`'s component at the VM's world time (`tick`, module documentation of [`super::tick`]).
pub fn tick_mind(vm: &mut Vm, host: &mut dyn Host, mind: ObjRef, globals: ObjRef) -> Result<TickReport, VmError> {
    let mut c = take(vm, mind, globals)?;
    let now = vm.clock.time_seconds();
    let result = tick(&mut c, &mut VmSenses { vm: &mut *vm, host }, now);
    let effects = result.as_ref().map(|r| r.effects.clone()).unwrap_or_default();
    put(vm, mind, c, &effects)?;
    result
}

/// `WillowMind.NotifyAttackedBy(Attacker, bDamaged)` on `mind` at the VM's world time ([`threat::mind_attacked_by`]);
/// the three `Last*` fields are written on the component when it declares them.
pub fn mind_attacked_by(
    vm: &mut Vm,
    host: &mut dyn Host,
    mind: ObjRef,
    globals: ObjRef,
    attacker: Option<ObjRef>,
    damaged: bool,
) -> Result<(), VmError> {
    let mut c = take(vm, mind, globals)?;
    let now = vm.clock.time_seconds();
    let result = threat::mind_attacked_by(&mut c, &mut VmSenses { vm: &mut *vm, host }, now, attacker, damaged);
    let (attacked, hurt, last) = (c.last_attacked_time, c.last_hurt_time, c.last_attacker);
    put(vm, mind, c, &[])?;
    let component = object(vm, mind, "AIComponent")?.ok_or_else(|| missing(vm, mind, "AIComponent"))?;
    set(vm, component, "LastAttackedTime", Value::Float(attacked))?;
    set(vm, component, "LastHurtTime", Value::Float(hurt))?;
    set(vm, component, "LastAttacker", Value::Object(last))?;
    result
}

// ---------------------------------------------------------------------------------------------------------------
// The natives
// ---------------------------------------------------------------------------------------------------------------

/// Binds the script-facing awareness natives the VM declares: `WillowMind.NotifyAttackedBy` (called by
/// `WillowAIPawn.TakeDamage`), `WillowMind.IsAwareOfTarget` and `WillowAIComponent.GetThreatLevel`. They need
/// [`install`] (the host and the globals); without it they are `NotYet { LayerAbsent }`.
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

pub const NOTIFY_ATTACKED_BY: &str = "WillowGame.WillowMind.NotifyAttackedBy";
pub const IS_AWARE: &str = "WillowGame.WillowMind.IsAwareOfTarget";
pub const GET_THREAT: &str = "WillowGame.WillowAIComponent.GetThreatLevel";

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        minds(vm)?;
        let bodies: [(&str, NativeFn); 3] = [
            (NOTIFY_ATTACKED_BY, Rc::new(n_notify_attacked_by)),
            (IS_AWARE, Rc::new(n_is_aware)),
            (GET_THREAT, Rc::new(n_get_threat)),
        ];
        for (path, body) in bodies {
            let Ok(id) = vm.find_function(path) else { continue };
            let Some(key) = vm.function(id).and_then(|d| d.native_key.clone()) else { continue };
            if !vm.has_native(&key) {
                vm.register_native(&key, body)?;
            }
        }
        Ok(())
    }
}

fn host_and_globals(vm: &mut Vm) -> Result<(Rc<RefCell<dyn Host>>, ObjRef), VmError> {
    let m = minds(vm)?;
    let m = borrow(&m)?;
    match (&m.host, m.globals) {
        (Some(h), Some(g)) => Ok((h.clone(), g)),
        _ => Err(NotYet::layer_absent("the AI's senses", "the host (ow_ai::awareness::vm::install)", DECISION).into()),
    }
}

fn arg_object(c: &NativeCall<'_>, i: usize) -> Result<Option<ObjRef>, VmError> {
    if i >= c.arg_count() {
        return Ok(None);
    }
    Ok(match c.arg(i)? {
        Value::Object(o) | Value::Interface(o) => *o,
        _ => None,
    })
}

fn this(c: &NativeCall<'_>) -> Result<ObjRef, VmError> {
    c.this.ok_or_else(|| VmError::Script {
        function: "ai awareness".to_string(),
        message: "an awareness native called without an object".to_string(),
    })
}

// WillowMind.NotifyAttackedBy(Attacker, bDamaged).
fn n_notify_attacked_by(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let mind = this(c)?;
    let attacker = arg_object(c, 0)?;
    let damaged = c.arg_count() > 1 && truth(c.vm, c.arg(1)?);
    let (host, globals) = host_and_globals(c.vm)?;
    let mut host = host
        .try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("the AI host"))?;
    mind_attacked_by(c.vm, &mut *host, mind, globals, attacker, damaged)?;
    Ok(Value::None)
}

// WillowMind.IsAwareOfTarget(PotentialTarget).
fn n_is_aware(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let mind = this(c)?;
    let target = arg_object(c, 0)?;
    let now = c.vm.clock.time_seconds();
    let aware = match (state(c.vm, mind), target) {
        (Some(s), Some(t)) => threat::is_aware(&s, now, t)?,
        _ => false,
    };
    Ok(Value::Bool(aware))
}

// WillowAIComponent.GetThreatLevel(Target): the level as the installed enum's byte (by name). A component the adapter
// has not ticked yet is built from its settings (no records), so the Friendly rule still applies.
fn n_get_threat(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let component = this(c)?;
    let Some(target) = arg_object(c, 0)? else {
        return enum_byte(c.vm, ThreatLevel::NoThreat);
    };
    let tracked = {
        let m = minds(c.vm)?;
        let m = borrow(&m)?;
        m.minds
            .keys()
            .copied()
            .find(|&mind| mind_component(c.vm, mind) == Some(component))
    };
    let mind = match tracked {
        Some(m) => m,
        // The AI start binds the component's `MyWillowMind` (G26, "Holds, OnHold, ClearState, InitAI").
        None => object(c.vm, component, "MyWillowMind")?.ok_or_else(|| -> VmError {
            NotYet::not_read(
                "GetThreatLevel on a component the adapter has not ticked and whose MyWillowMind is unset",
                RECORD_AWARENESS,
            )
            .into()
        })?,
    };
    let (host, globals) = host_and_globals(c.vm)?;
    let state = match state(c.vm, mind) {
        Some(s) => s,
        None => {
            let pawn = object(c.vm, mind, "Pawn")?.ok_or_else(|| missing(c.vm, mind, "Pawn"))?;
            Component::new(pawn, settings(c.vm, mind, globals)?)
        }
    };
    let mut host = host
        .try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("the AI host"))?;
    let level = threat::get_threat(&state, &mut VmSenses { vm: &mut *c.vm, host: &mut *host }, target)?;
    enum_byte(c.vm, level)
}

fn mind_component(vm: &Vm, mind: ObjRef) -> Option<ObjRef> {
    let class = vm.space.heap.get(mind).ok()?.class;
    let p = vm.space.prop(class, "AIComponent")?;
    match vm.space.read(mind, ow_object::ids::PropAddr::scalar(p)).ok()? {
        Value::Object(o) => *o,
        _ => None,
    }
}

/// The byte of `level` in the installed `WillowAIComponent.EThreatLevel`, by name.
fn enum_byte(vm: &mut Vm, level: ThreatLevel) -> Result<Value, VmError> {
    let names = ow_behavior::provider::enum_names(vm, "WillowGame", "WillowAIComponent.EThreatLevel")
        .map_err(|e| match e {
            ow_behavior::provider::BehaviorError::Vm(v) => v,
            other => VmError::Script {
                function: "ai awareness".to_string(),
                message: other.to_string(),
            },
        })?;
    let index = names
        .iter()
        .position(|n| n.eq_ignore_ascii_case(level.name()))
        .and_then(|i| u8::try_from(i).ok())
        .ok_or_else(|| -> VmError {
            NotYet::not_read(format!("{} in the installed EThreatLevel", level.name()), RECORD_AWARENESS).into()
        })?;
    Ok(Value::Byte(index))
}
