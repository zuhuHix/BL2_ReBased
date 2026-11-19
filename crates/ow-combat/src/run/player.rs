//! Owner: lane W6-COMBAT-RUN2. The mission bridge's player in `--combat-run` (plan steps S-d and S-f): the `maya`,
//! `tick`, `cash`, `bleed` and `mayahealth` steps, the bridge's hits and swings, the kill credit into the player's
//! experience pool and the bleed-out record. Port of the bridge half of `src/combat_run.cpp`. AI-assisted (Claude
//! Code).
//!
//! **The bridge** is W6-MISSION's `ow_mission::bridge::MissionScript`, which this crate cannot name (`ow-mission`
//! depends on `ow-combat`). The run takes it as a [`PlayerBridge`], which `ow-cli` implements over the bridge, so its
//! `player_controller` and `player_pawn` are the player here: no private copy. As the C++, the bridge is built when a
//! `maya` step is among the steps, over the stock Fire mission, and the run uses the bridge's level `WorldInfo`.
//!
//! **The player's health pool** ([`HealthPool`]). The C++ bridge builds one pool by hand for the player
//! (`ensureHealthPool` / `seedHealthPool`: a manager on the controller, the class's `HealthPoolDefinition`, the maximum
//! evaluated from `BaseMaxValue` with the controller as the source, the pawn's `HealthPool` reference set to slot 0).
//! That is a stand-in for the game's class-defaults routine; the Rust bridge does not build it
//! (`MissionScript::set_player_health` is `NotYet`). Under architecture R3 it is reproduced here in
//! `ParityMode::Reference` only. In `ParityMode::Off` the game's own route runs instead (decision 2026-11-16-82): the
//! controller's script `ApplyCharacterClassDefaults` on the VM, which creates the pool through `CreatePool`
//! ([`HealthPool::ensure_off`]); unread hooks and other unsupported natives on that route still refuse.
//!
//! **Script errors.** The C++ runs a bridged hit, swing and each bleed frame inside the bridge's guard, which records
//! the error (`mission script: <text>`) and goes on; the run lists those under the step. The same here: a bleed frame's
//! error is recorded and the next frame runs; a hit or swing error is recorded the same way, and the step's result is
//! the error (the C++ prints the report the chain had built so far, which this port does not keep).
//!
//! The timing stand-ins of the bleed step are the C++ ones (COMBAT_GOLDEN_TRACE.md section D steps 11 to 16; live
//! delays, UNVERIFIED, not derived from data): [`SPECIAL_MOVE_END`], [`RAGDOLL_DELAY`], [`RESPAWN_DIALOG_DELAY`].

use std::fmt::Write as _;

use ow_attr::api::EvalCx;
use ow_core::fmt::json_quote;
use ow_core::ParityMode;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;
use ow_vm::interp::instantiate_from_defaults;

use crate::hit::access::{error, error_text, get, integer, number, object, set, set_field, to_f32};

/// The injured special move's end (`SwitchToSidearm`), seconds after the injury.
pub const SPECIAL_MOVE_END: f64 = 2.28;
/// The injured-death animation's end (`InjuredDeathAnimDone`), seconds after the bleed-out.
pub const RAGDOLL_DELAY: f64 = 0.84;
/// The respawn dialog (`ShowRespawnDialog`), seconds after the respawn.
pub const RESPAWN_DIALOG_DELAY: f64 = 11.8;
/// The C++ tolerance on those delays.
pub(crate) const EPSILON: f64 = 1e-9;

/// One gain of the level-up path, as the run reports it (C++ `MissionScript::Gain`).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct BridgeGain {
    /// `experience`, `level`, `skill_points`, `max_health`, `health`, ...
    pub kind: &'static str,
    pub amount: i32,
    pub health: f32,
    pub maximum: f32,
}

impl BridgeGain {
    pub fn new(kind: &'static str, amount: i32, health: f32, maximum: f32) -> Self {
        BridgeGain {
            kind,
            amount,
            health,
            maximum,
        }
    }
}

/// What the run needs of the mission bridge (module documentation). Errors are the bridge's, as VM errors.
pub trait PlayerBridge {
    /// The player's controller (the bridge's, never a copy).
    fn player_controller(&self) -> ObjRef;
    /// The player's VM pawn (made on first use by the bridge).
    fn player_pawn(&self, vm: &mut Vm) -> Result<ObjRef, VmError>;
    /// The experience level and pool value.
    fn set_player_experience(
        &self,
        vm: &mut Vm,
        level: i32,
        experience: i64,
    ) -> Result<(), VmError>;
    /// The pool's per-frame update (`ApplyExpPointsToExpLevel(false)`).
    fn update_experience_pool(&self, vm: &mut Vm) -> Result<(), VmError>;
    /// The level-up path's gains since the last call.
    fn take_gains(&self) -> Vec<BridgeGain>;
    fn player_level(&self, vm: &Vm) -> Result<i32, VmError>;
    fn experience_pool(&self) -> f32;
    /// Every script error the bridge recorded so far, in order.
    fn errors(&self) -> Vec<String>;
    /// The player's class definition (`CharClass_Siren`): it names the health pool and the injured definition.
    fn player_class(&self, vm: &mut Vm) -> Result<Option<ObjRef>, VmError>;
    /// Gives the player's controller its class through the game's own script route, once (a labelled harness input:
    /// a class-select call naming the class, not a save; decision 2026-11-19-41). The default does nothing.
    fn select_player_class(&self, _vm: &mut Vm) -> Result<(), VmError> {
        Ok(())
    }
}

/// The Reference stand-in of the player's health pool (module documentation): the manager and the pool, once built.
#[derive(Debug, Default)]
pub(crate) struct HealthPool {
    manager: Option<ObjRef>,
    pool: Option<ObjRef>,
    failed: bool,
}

impl HealthPool {
    /// C++ `ensureHealthPool`: builds the manager and the pool once; false (and no pool, ever) when the data lacks a
    /// part, as the C++ gives up quietly. As the C++ catch, any error of the build other than a `NotYet` is false,
    /// with the manager and the pool cleared (review r1 of PR #356); a `NotYet` is passed on, also cleared. Then the
    /// pawn is linked. `Off` mode uses the installed script route below.
    fn ensure(&mut self, vm: &mut Vm, bridge: &dyn PlayerBridge) -> Result<bool, VmError> {
        if vm.parity != ParityMode::Reference {
            return self.ensure_off(vm, bridge);
        }
        if self.pool.is_some() {
            return Ok(true);
        }
        if self.failed {
            return Ok(false);
        }
        self.failed = true;
        match self.build(vm, bridge) {
            Ok(true) => {
                self.failed = false;
                let pawn = bridge.player_pawn(vm)?;
                self.relink(vm, pawn)?;
                Ok(true)
            }
            Ok(false) => Ok(false),
            Err(e) => {
                self.manager = None;
                self.pool = None;
                match e {
                    VmError::NotYet(n) => Err(VmError::NotYet(n)),
                    _ => Ok(false),
                }
            }
        }
    }

    /// `ParityMode::Off` (decision 2026-11-16-82): the game's own route instead of the C++ stand-in. The controller's
    /// installed script `ApplyCharacterClassDefaults(class, pawn)` runs on the VM: it makes the controller's manager
    /// and creates the health pool through `Pawn.SetHealthResourcePoolDefinition` and `CreatePool`
    /// (NATIVE_COMBAT_GROUNDWORK.md sections 1.8 and 1.10), whose own script sets the pool's starting values. The pool
    /// kept is the one the pawn's `HealthPool` reference names afterwards. A `NotYet` from inside (the pool script's
    /// `ResetIdleTimer` hook for an unread subclass, NATIVE_RESOURCE_POOL_IDLE.md) is passed on; no class, or no
    /// pool afterwards, is false, as the stand-in gives up.
    fn ensure_off(&mut self, vm: &mut Vm, bridge: &dyn PlayerBridge) -> Result<bool, VmError> {
        if self.pool.is_some() {
            return Ok(true);
        }
        if self.failed {
            return Ok(false);
        }
        self.failed = true;
        let Some(class) = bridge.player_class(vm)? else {
            return Ok(false);
        };
        let controller = bridge.player_controller();
        let pawn = bridge.player_pawn(vm)?;
        // The arguments by parameter name (the pawn is `P`, the class the other one), not by position.
        let class_id = vm.space.heap.get(controller)?.class;
        let state = vm.space.heap.get(controller)?.state;
        let Some(f) = vm.find_method(class_id, "ApplyCharacterClassDefaults", state)? else {
            return Err(error("combat-run: the controller has no ApplyCharacterClassDefaults"));
        };
        let params = vm.function(f).map(|d| d.params.clone()).unwrap_or_default();
        let mut args = Vec::with_capacity(params.len());
        for p in &params {
            let name = vm.space.names.text(p.name).into_owned();
            args.push(if name.eq_ignore_ascii_case("P") {
                Value::Object(Some(pawn))
            } else if args.len() < 2 {
                Value::Object(Some(class))
            } else {
                vm.space.classes.zero_value(&p.kind)
            });
        }
        vm.call(f, Some(controller), args)?;
        let Some(pool) = ow_world::pools::health_pool(&vm.space, pawn)? else {
            return Ok(false);
        };
        self.manager = object(vm, controller, "ResourcePoolManager")?;
        self.pool = Some(pool);
        self.failed = false;
        Ok(true)
    }

    /// Whether no pool is kept (tests).
    #[cfg(test)]
    pub(crate) fn pool_is_none(&self) -> bool {
        self.pool.is_none() && self.manager.is_none()
    }

    /// The build of [`HealthPool::ensure`]: the manager and the pool, kept only when every part was found.
    fn build(&mut self, vm: &mut Vm, bridge: &dyn PlayerBridge) -> Result<bool, VmError> {
        let controller = bridge.player_controller();
        let Some(class) = bridge.player_class(vm)? else {
            return Ok(false);
        };
        let Some(definition) = object(vm, class, "HealthPoolDefinition")? else {
            return Ok(false);
        };
        let Some(resource) = object(vm, definition, "Resource")? else {
            return Ok(false);
        };
        // The pool class: a class value, or an object naming the class export.
        let pool_class = match get(vm, resource, "DefaultResourcePoolClass")? {
            Some(Value::Class(Some(c))) => c,
            _ => return Ok(false),
        };
        let manager_class = vm.find_class("Engine.ResourcePoolManager")?;
        let manager = instantiate_from_defaults(vm, manager_class)?;
        if let Some(role) = get(vm, controller, "Role")? {
            set(vm, manager, "Role", role)?;
        }
        set(vm, manager, "Owner", Value::Object(Some(controller)))?;
        let pool = instantiate_from_defaults(vm, pool_class)?;
        match get(vm, manager, "ResourcePools")? {
            Some(Value::Array(slots)) => {
                let mut slots = slots.as_ref().clone();
                if slots.is_empty() {
                    slots.push(Value::None);
                }
                slots[0] = Value::Object(Some(pool));
                set(vm, manager, "ResourcePools", Value::array(slots))?;
            }
            Some(_) => {
                set(vm, manager, "ResourcePools", Value::Object(Some(pool)))?;
            }
            None => {
                return Err(error(
                    "mission script bridge: ResourcePoolManager has no ResourcePools",
                ));
            }
        }
        for (name, value) in [
            ("Definition", Value::Object(Some(definition))),
            ("AssociatedProvider", Value::Object(Some(controller))),
            ("bIsAuthoritative", Value::Bool(true)),
        ] {
            if !set(vm, pool, name, value)? {
                return Err(error(format!(
                    "mission script bridge: the pool has no {name}"
                )));
            }
        }
        if !set(
            vm,
            controller,
            "ResourcePoolManager",
            Value::Object(Some(manager)),
        )? {
            return Err(error(
                "mission script bridge: the controller has no ResourcePoolManager",
            ));
        }
        self.manager = Some(manager);
        self.pool = Some(pool);
        Ok(true)
    }

    /// C++ `linkPlayerHealth`, which the C++ bridge runs on every fetch of the player's pawn (`pawnFor`): the pawn's
    /// `HealthPool` reference to slot 0 of the manager, the pool's own GUID. Off keeps the script-written reference.
    /// As the C++ catch, an error other than a `NotYet` leaves the pawn as it is.
    pub(crate) fn relink(&self, vm: &mut Vm, pawn: ObjRef) -> Result<(), VmError> {
        // Off: the pawn's reference is the one `SetHealthResourcePoolDefinition` stored; nothing to relink.
        if vm.parity != ParityMode::Reference {
            return Ok(());
        }
        match self.link(vm, pawn) {
            Err(VmError::NotYet(n)) => Err(VmError::NotYet(n)),
            _ => Ok(()),
        }
    }

    fn link(&self, vm: &mut Vm, pawn: ObjRef) -> Result<(), VmError> {
        let (Some(manager), Some(pool)) = (self.manager, self.pool) else {
            return Ok(());
        };
        let Some(mut reference) = get(vm, pawn, "HealthPool")? else {
            return Ok(());
        };
        let guid = get(vm, pool, "PoolGUID")?.map_or(0, |v| integer(&v));
        let all = set_field(
            vm,
            &mut reference,
            "PoolManager",
            Value::Object(Some(manager)),
        ) && set_field(vm, &mut reference, "PoolIndexInManager", Value::Byte(0))
            && set_field(
                vm,
                &mut reference,
                "PoolGUID",
                Value::Byte(crate::hit::access::low_u8(guid)),
            )
            && set_field(vm, &mut reference, "Data", Value::Object(Some(pool)));
        if all {
            set(vm, pawn, "HealthPool", reference)?;
        }
        Ok(())
    }

    /// C++ `seedHealthPool`: the maximum from the definition's `BaseMaxValue` (the controller as the source), the
    /// minimum from `BaseMinValue`, current as given (negative: the maximum, else clamped), then the recorded values.
    /// A failure is a script error of the bridge (`errors`), as the C++ records it.
    fn seed(&self, vm: &mut Vm, bridge: &dyn PlayerBridge, current: f32, errors: &mut Vec<String>) {
        let Some(pool) = self.pool else {
            return;
        };
        // Off: the pool's own script set its starting values; a given share is set through `Pawn.SetHealth`
        // (NATIVE_COMBAT_GROUNDWORK.md section 2), not the stand-in's seed.
        if vm.parity != ParityMode::Reference {
            if current >= 0.0 {
                let set = bridge
                    .player_pawn(vm)
                    .and_then(|pawn| ow_world::pools::set_health(vm, pawn, current));
                if let Err(e) = set {
                    errors.push(format!("mission script: {}", error_text(&e)));
                }
            }
            return;
        }
        if let Err(e) = seed_pool(vm, pool, bridge.player_controller(), current) {
            errors.push(format!("mission script: {}", error_text(&e)));
        }
    }

    /// `ensure`, then `seed` (the C++ `if (ensureHealthPool()) seedHealthPool(current)`).
    pub(crate) fn ensure_and_seed(
        &mut self,
        vm: &mut Vm,
        bridge: &dyn PlayerBridge,
        current: f32,
        errors: &mut Vec<String>,
    ) -> Result<(), VmError> {
        if self.ensure(vm, bridge)? {
            self.seed(vm, bridge, current, errors);
        }
        Ok(())
    }
}

fn seed_pool(vm: &mut Vm, pool: ObjRef, controller: ObjRef, current: f32) -> Result<(), VmError> {
    let definition = object(vm, pool, "Definition")?
        .ok_or_else(|| error("mission script bridge: the pool has no Definition"))?;
    let base = get(vm, definition, "BaseMaxValue")?.ok_or_else(|| {
        error("mission script bridge: the health pool definition has no BaseMaxValue")
    })?;
    let data = ow_attr::eval::init_data(&vm.space, &base).map_err(VmError::Attr)?;
    let attrs = vm.attrs.clone();
    let maximum = attrs.evaluate_init(&data, Some(controller), None, &mut EvalCx::new(vm))?;
    let minimum = get(vm, definition, "BaseMinValue")?.map_or(0.0, |v| to_f32(number(&v)));
    let value = if current < 0.0 {
        maximum
    } else {
        minimum.max(current.min(maximum))
    };
    for (name, v) in [
        ("MaxValueBaseValue", maximum),
        ("MaxValue", maximum),
        ("MinValueBaseValue", minimum),
        ("MinValue", minimum),
        ("CurrentValue", value),
    ] {
        if !set(vm, pool, name, Value::Float(v))? {
            return Err(error(format!(
                "mission script bridge: the pool has no {name}"
            )));
        }
    }
    ow_world::pools::update_last_values(&mut vm.space, pool)
}

/// The player's down state (plan step S-f): the injury being played and the times its callbacks were played at (C++
/// `Episode`).
#[derive(Debug, Clone, Default)]
pub(crate) struct Episode {
    pub active: bool,
    pub targeted: bool,
    pub ragdolled: bool,
    pub camera_ended: bool,
    pub number: i32,
    pub down_at: f64,
    pub targeted_at: f64,
    pub dead_at: f64,
    pub total: f64,
    pub successive: f64,
}

impl Episode {
    pub fn new() -> Self {
        Episode {
            targeted_at: -1.0,
            dead_at: -1.0,
            ..Episode::default()
        }
    }
}

/// One respawn dialog: the injury, the cash before it and what `ShowRespawnDialog` charged.
#[derive(Debug, Clone)]
pub(crate) struct Fee {
    pub injury: String,
    pub cash: i64,
    pub charged: i64,
}

/// A property of `o` as a number (C++ `numberOf`): 0 when absent.
pub(crate) fn number_of(vm: &mut Vm, o: ObjRef, name: &str) -> Result<f64, VmError> {
    Ok(get(vm, o, name)?.map_or(0.0, |v| number(&v)))
}

/// The object's state name, as the C++ prints it ("" for the default state).
pub(crate) fn state_name(vm: &Vm, o: ObjRef) -> Result<String, VmError> {
    Ok(match vm.space.heap.get(o)?.state {
        Some(s) => vm.space.names.text(s).to_string(),
        None => String::new(),
    })
}

/// Whether the pawn is in its `Injured` state (the name, any case).
pub(crate) fn is_injured_state(vm: &Vm, pawn: ObjRef) -> Result<bool, VmError> {
    Ok(state_name(vm, pawn)?.eq_ignore_ascii_case("injured"))
}

/// A script method by name in the object's current state (C++ `callState`): the state's function wins. Arguments past
/// `args` are the parameters' zero values (the C++ passes empty values for the camera's out parameters).
pub(crate) fn call_state(
    vm: &mut Vm,
    o: ObjRef,
    name: &str,
    args: Vec<Value>,
) -> Result<Value, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let state = vm.space.heap.get(o)?.state;
    let Some(f) = vm.find_method(class, name, state)? else {
        let state = state_name(vm, o)?;
        return Err(error(format!(
            "combat-run: {} has no function {name} in state {state}",
            crate::hit::access::class_name(vm, o)
        )));
    };
    let params = vm.function(f).map(|d| d.params.clone()).unwrap_or_default();
    let mut all = args;
    for p in params.iter().skip(all.len()) {
        all.push(vm.space.classes.zero_value(&p.kind));
    }
    Ok(vm.call(f, Some(o), all)?.ret)
}

/// The JSON of the down state (C++ `downJson`).
pub(crate) fn down_json(
    vm: &mut Vm,
    pawn: ObjRef,
    episode: &Episode,
    now: f64,
) -> Result<String, VmError> {
    let num = |x: f64| ow_core::fmt::fmt_g(x, 9);
    let state = state_name(vm, pawn)?;
    let mut out = format!(
        "{{\"state\":{},\"injury\":{}",
        json_quote(&state),
        episode.number
    );
    for (key, name) in [
        ("injured_state", "InjuredState"),
        ("dead_state", "InjuredDeadState"),
        ("second_wind_reason", "SecondWindReason"),
        ("total_bleedout", "TotalBleedoutTime"),
        ("successive", "NumSuccessiveInjuries"),
        ("targeted_time", "flInjuredTargetedTime"),
        ("last_injury_time", "LastInjuryTime"),
    ] {
        let _ = write!(out, ",\"{key}\":{}", num(number_of(vm, pawn, name)?));
    }
    let _ = write!(out, ",\"since_down\":{}}}", num(now - episode.down_at));
    Ok(out)
}

/// The frame count of `seconds` in frames of `dt` (C++ `std::lround(seconds / dt)`, half away from zero). N5: a
/// count outside 0..=i64 range (or not a number) is refused rather than cast.
pub(crate) fn frames(seconds: f64, dt: f64) -> Result<i64, VmError> {
    let n = (seconds / dt).round();
    if !n.is_finite() || n.abs() > 9.0e15 {
        return Err(error("combat-run: bleed: the frame count is out of range"));
    }
    // A finite whole number within +-9e15: exact in i64.
    Ok(n as i64)
}

/// The credit a kill gave the pool (C++ `int(std::lround(after - before))`, a float difference). N5: a difference past
/// the int range is refused.
pub(crate) fn credit(before: f32, after: f32) -> Result<i32, VmError> {
    let d = (after - before).round();
    if !d.is_finite() || d < i32::MIN as f32 || d > i32::MAX as f32 {
        return Err(error("combat-run: the kill credit is out of range"));
    }
    // Checked above: a whole number in the int range.
    Ok(d as i32)
}

/// The player's controller's replication info (the `cash` and fee steps).
pub(crate) fn replication_info(vm: &mut Vm, controller: ObjRef) -> Result<ObjRef, VmError> {
    object(vm, controller, "PlayerReplicationInfo")?
        .ok_or_else(|| error("combat-run: the player's controller has no PlayerReplicationInfo"))
}

/// `ExpPointsNextLevelAt` of the player's replication info.
pub(crate) fn next_level_at(vm: &mut Vm, controller: ObjRef) -> Result<i64, VmError> {
    let info = replication_info(vm, controller)?;
    Ok(get(vm, info, "ExpPointsNextLevelAt")?.map_or(0, |v| integer(&v)))
}

/// Runs `body` with the controller's `Pawn` set to the player's pawn, then puts the old value back (C++
/// `withPlayerPawn`, `hitFromPlayer`, `swingAtPlayer`); the controller is the player's while the chain runs.
pub(crate) fn with_player_pawn<T>(
    vm: &mut Vm,
    controller: ObjRef,
    pawn: ObjRef,
    body: impl FnOnce(&mut Vm, ObjRef) -> Result<T, VmError>,
) -> Result<T, VmError> {
    let saved = get(vm, controller, "Pawn")?;
    if saved.is_some() {
        set(vm, controller, "Pawn", Value::Object(Some(pawn)))?;
    }
    let result = body(vm, pawn);
    if let Some(v) = saved {
        set(vm, controller, "Pawn", v)?;
    }
    result
}
