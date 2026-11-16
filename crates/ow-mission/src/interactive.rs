//! Owner: lane W7-WORLDLOGIC. Interactive objects: which object the use key reaches, the cost of a use, and what of
//! the use is not read yet. AI-assisted (Claude Code). Decision 2026-11-16-60.
//!
//! **Every rule here is UNVERIFIED in the running game.** The records are NATIVE_USE_INTERACTION.md (the per-frame
//! choice of the usable object, read from native code), NATIVE_CONTROLLER_HELPERS.md (the cost natives, read from
//! native code) and the installed script (`WillowPlayerController.PerformedUseAction` and friends,
//! `WillowInteractiveObject.UseObject`, `UsedBy`, `ConditionalReactToUse`, read with `research/script_disasm.py`).
//!
//! **What is script, and so not here (architecture rule R2).** The press of the use key, the afford check before the
//! use, the use itself and the payment after it are installed script, which the VM runs. Read from the script (own
//! words): the key reaches `PerformedUseAction` on the authority; it needs a current usable object and no revive
//! target; it asks `CanAffordToUseUsableObject`; a golden-key cost (currency 3) asks for confirmation first; the use
//! (`UseObject`) comes **before** the payment (`PayForUsedObject`); a refused afford check calls the object's
//! `NotifyUserCouldNotAffordAttemptedUse` instead. An interactive object's primary use remembers the used component,
//! then fires its definition's `OnUsedBy` behavior event when the object's `bCanBeUsed` entry for that use type is set
//! or the component is usable (`IsComponentUsable`), passing the component only when it is usable; the secondary use
//! fires `OnSecondaryUsedBy` the same way and always answers false.
//!
//! **What this module owns.**
//! - [`choose_usable`]: the per-frame choice of the usable object along the view ray (a native rule, offline: the
//!   caller gives the ray's hits).
//! - [`next_evaluation`]: the per-frame throttle (`UsableObjectUpdateRate`, class default 30 per second).
//! - [`cost_of`], [`can_afford`] and [`payment`]: the cost natives' rules on plain inputs.
//! - [`census`]: the interactive objects a map places or spawns at its opportunity points, with the definition each
//!   one's use would reach (package data), for the headless count.
//! - [`Pack`]: `WillowPlayerController.CanAffordToUseUsableObject` and `PayForUsedObject` bound into the VM in
//!   [`ParityMode::Off`] (Reference binds nothing: the C++ reference binds neither, so the VM's stub policy answers
//!   there as before), and `WillowInteractiveObject.IsComponentUsable` bound to `NotYet` in Off (not read).
//!
//! **Not read, so `NotYet`:** `WillowInteractiveObject.IsComponentUsable` (a virtual of the object, NATIVE_CONTROLLER_HELPERS.md
//! "Not read yet"); the definition's event natives (`InteractiveObjectDefinition.OnUsedBy` and its three twins: the AI
//! class's `OnUsed` was read, the interactive definition's was not, so the event name and filter are not assumed);
//! the golden-key count a currency-3 afford check compares with (the key record layout is not read); the cost query
//! of any `IUsable` class other than `WillowInteractiveObject`, `WillowAIPawn` and the `WillowPawn` base; and the
//! payment when the controller has no `WillowPlayerReplicationInfo`.

use std::rc::Rc;

use ow_combat::hit::access::{get, integer, is_a, object_of};
use ow_combat::hit::{arg_object, call_method, this};
use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, NativePack, Vm};
use ow_vm::error::VmError;

use crate::vendors::{MapView, Skip};
use ow_items::registry::Pv;

#[cfg(test)]
mod tests;

/// The record of the use-target choice and the use key's path.
pub const RECORD_USE: &str = "docs/verification/NATIVE_USE_INTERACTION.md";
/// The record of the cost natives.
pub const RECORD_COST: &str =
    "docs/verification/NATIVE_CONTROLLER_HELPERS.md (CanAffordToUseUsableObject / PayForUsedObject)";

/// `GlobalsDefinition.PlayerInteractionDistance` as the package holds it (`GD_Globals.General.Globals`): the length of
/// the view ray, in uu (package data, NATIVE_USE_INTERACTION.md; UNVERIFIED that the game reads no other value).
pub const PLAYER_INTERACTION_DISTANCE: f32 = 350.0;
/// `WillowPlayerController.UsableObjectUpdateRate`'s class default: evaluations per second.
pub const USABLE_OBJECT_UPDATE_RATE: f32 = 30.0;
/// `IUsable.EUsabilityType`.
pub const UT_PRIMARY: u8 = 0;
pub const UT_SECONDARY: u8 = 1;
/// The currency type of golden keys (`ECurrencyType` 3), which the controller counts, not the replication info.
pub const GOLDEN_KEYS: u8 = 3;
/// The largest golden-key cost [`payment`] spends key by key; above it the cost is refused as bad data. A guard of
/// ours, UNVERIFIED as anything the game does: NATIVE_CONTROLLER_HELPERS.md reads that the native `PayForUsedObject`
/// never refuses, and fires `SpendGoldenKey` once per unit of any cost. The guard keeps bad data from looping; the
/// golden chest costs one.
pub const MAX_GOLDEN_KEY_COST: i32 = 1_000;

/// The classes whose cost query was read, and the native keys this module binds.
pub const INTERACTIVE_OBJECT: &str = "WillowGame.WillowInteractiveObject";
pub const AI_PAWN: &str = "WillowGame.WillowAIPawn";
pub const WILLOW_PAWN: &str = "WillowGame.WillowPawn";
pub const REPLICATION_INFO: &str = "WillowGame.WillowPlayerReplicationInfo";
pub const CAN_AFFORD: &str = "WillowGame.WillowPlayerController.CanAffordToUseUsableObject";
pub const PAY: &str = "WillowGame.WillowPlayerController.PayForUsedObject";
pub const IS_COMPONENT_USABLE: &str = "WillowGame.WillowInteractiveObject.IsComponentUsable";

// ---------------------------------------------------------------------------------------------------------------
// The use target (per-frame evaluation)
// ---------------------------------------------------------------------------------------------------------------

/// What the per-frame evaluation learns of one hit of the view ray. The caller asks the world; this module only
/// applies the order.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UseHit {
    /// The hit actor carries the flag the walk skips without blocking (read as "pending destroy").
    pub skipped: bool,
    /// The actor implements `IUsable` and says it is usable by this user. `None`: not usable at all.
    pub usable: Option<Usability>,
    /// The hit is another player's trade proxy (its primary icon is of the trade type).
    pub trade_proxy: bool,
    /// Had it not been accepted, the hit would block the ray (the actor's or the component's blocking flag, and not
    /// a pawn with the exemption flag).
    pub blocks: bool,
}

/// The primary-use answers of a usable hit.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Usability {
    /// The hit component's instance-data entry: its `bIsUsable`, `None` when the actor has no entry for it.
    pub component_primary: Option<bool>,
    /// The object's own `CanBeUsed(User, Component, UT_Primary)` answer.
    pub object_primary: bool,
}

impl Usability {
    pub fn new(component_primary: Option<bool>, object_primary: bool) -> Self {
        Usability {
            component_primary,
            object_primary,
        }
    }
}

/// The evaluation's result: the hit that is the current usable object, and the trade proxy remembered on the way.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UseChoice {
    pub usable: Option<usize>,
    pub trade_proxy: Option<usize>,
}

/// The per-frame choice of the usable object (NATIVE_USE_INTERACTION.md, "Per-frame usable evaluation"), over the
/// hits of one ray from the camera, nearest first. `gate_open` is false under a cinematic input lock, without a pawn
/// or with a pending-destroy pawn: then nothing is chosen.
///
/// | Hit | Result |
/// |---|---|
/// | carries the skip flag | passed over; it neither is chosen nor blocks |
/// | another player's trade proxy | remembered as the trade proxy (the first one); never the usable object |
/// | usable, none chosen yet, primary use available (the component entry's flag, else the object's answer) | chosen; later hits are looked at only for a trade proxy (UNVERIFIED reading: the record says the walk then continues only to look for a trade proxy, which this reads as a blocking hit after the choice changing nothing) |
/// | anything else that blocks | ends the walk |
/// | anything else | passed over |
///
/// So the nearest usable hit in front of the first blocking hit wins; there is no cone and no priority between NPCs
/// and objects. Status: UNVERIFIED (read from native code).
pub fn choose_usable(hits: &[UseHit], gate_open: bool) -> UseChoice {
    let mut choice = UseChoice::default();
    if !gate_open {
        return choice;
    }
    for (i, hit) in hits.iter().enumerate() {
        if hit.skipped {
            continue;
        }
        if hit.trade_proxy {
            if choice.trade_proxy.is_none() {
                choice.trade_proxy = Some(i);
            }
            continue;
        }
        if choice.usable.is_some() {
            continue;
        }
        if let Some(u) = hit.usable
            && (u.component_primary == Some(true) || u.object_primary)
        {
            choice.usable = Some(i);
            continue;
        }
        if hit.blocks {
            break;
        }
    }
    choice
}

/// When the next fresh evaluation may run, after one at `now` (seconds of game time): `now + 1 / rate`. A rate that
/// is not a positive finite number gives `None` (no throttle can be formed; the class default is 30). After a
/// successful use the script writes 0 into the stored time, so the next frame evaluates again.
pub fn next_evaluation(now: f32, rate: f32) -> Option<f32> {
    if !(rate.is_finite() && rate > 0.0) {
        return None;
    }
    Some(now + 1.0 / rate)
}

// ---------------------------------------------------------------------------------------------------------------
// The cost of a use
// ---------------------------------------------------------------------------------------------------------------

/// What a use costs: a currency type and an amount.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UseCost {
    pub currency: u8,
    pub amount: i32,
}

impl UseCost {
    pub fn new(currency: u8, amount: i32) -> Self {
        UseCost { currency, amount }
    }
}

/// The cost query of `WillowInteractiveObject` and `WillowAIPawn` for one usability type: the object costs when its
/// `bCostsToUse` entry is nonzero, and then reports its `CostsToUseType` and `CostsToUseAmount` entries.
pub fn cost_of(costs_to_use: u8, currency: u8, amount: i32) -> Option<UseCost> {
    (costs_to_use != 0).then_some(UseCost::new(currency, amount))
}

/// `CanAffordToUseUsableObject` for a cost that is not in golden keys: true for no cost, else `amount <= on_hand`
/// (the replication info's `GetCurrencyOnHand(currency)`). A golden-key cost compares with the controller's key count,
/// which is not read: `NotYet`.
pub fn can_afford(cost: Option<UseCost>, on_hand: i64) -> Result<bool, NotYet> {
    match cost {
        None => Ok(true),
        Some(c) if c.currency == GOLDEN_KEYS => Err(golden_key_count()),
        Some(c) => Ok(i64::from(c.amount) <= on_hand),
    }
}

/// What `PayForUsedObject` does for a cost. It never checks affordability (the script asks first).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Payment {
    /// No cost: nothing happens.
    Nothing,
    /// Golden keys: the controller's script event `SpendGoldenKey` fires once per unit.
    SpendGoldenKeys(u32),
    /// Any other currency: the replication info's `AddCurrencyOnHand(currency, delta)`, `delta` the negated amount
    /// (the caps and the floor of 0 are that native's, `ow_combat::down`).
    AddCurrency { currency: u8, delta: i32 },
}

/// The payment of a cost (module documentation). A golden-key amount above [`MAX_GOLDEN_KEY_COST`] is refused (our
/// guard). Any other amount is negated with wrapping, so `i32::MIN` stays `i32::MIN`, a negative delta that
/// `AddCurrencyOnHand` clamps to 0 (UNVERIFIED how the native negates; no data gives such a cost).
pub fn payment(cost: Option<UseCost>) -> Result<Payment, String> {
    let Some(c) = cost else {
        return Ok(Payment::Nothing);
    };
    if c.currency == GOLDEN_KEYS {
        if c.amount > MAX_GOLDEN_KEY_COST {
            return Err(format!("a golden-key cost of {} is out of range", c.amount));
        }
        // A cost of 0 or less spends no key; in range it converts exactly.
        return Ok(Payment::SpendGoldenKeys(u32::try_from(c.amount.max(0)).unwrap_or(0)));
    }
    let delta = c.amount.wrapping_neg();
    Ok(Payment::AddCurrency {
        currency: c.currency,
        delta,
    })
}

fn golden_key_count() -> NotYet {
    NotYet::not_read(
        "the golden-key count a currency-3 use cost compares with (the controller's key records are not read)",
        RECORD_COST,
    )
}

// ---------------------------------------------------------------------------------------------------------------
// The natives
// ---------------------------------------------------------------------------------------------------------------

fn script_error(text: impl Into<String>) -> VmError {
    VmError::Script {
        function: "interactive".to_string(),
        message: text.into(),
    }
}

/// Element `index` of the property `name` of `o` (a static array, or a scalar at index 0); `None` when the class has
/// no such property. An index past the array's size is bad data.
fn element(vm: &mut Vm, o: ObjRef, name: &str, index: u8) -> Result<Option<Value>, VmError> {
    // Loads the object's definition data first, as every other read of this crate's helpers does.
    if get(vm, o, name)?.is_none() {
        return Ok(None);
    }
    let class = vm.space.heap.get(o)?.class;
    let Some(p) = vm.space.prop(class, name) else {
        return Ok(None);
    };
    let dim = vm.space.classes.decl(p).map_or(1, |d| d.array_dim);
    if u32::from(index) >= u32::try_from(dim).unwrap_or(1) {
        return Err(script_error(format!(
            "{name}[{index}]: the property has {dim} element(s)"
        )));
    }
    Ok(Some(
        vm.space
            .read(o, PropAddr {
                prop: p,
                index: u32::from(index),
            })?
            .clone(),
    ))
}

/// The `IUsable` cost query of `object` for usability type `ty` (module documentation): `Ok(None)` for no cost.
pub fn query_cost(vm: &mut Vm, object: ObjRef, ty: u8) -> Result<Option<UseCost>, VmError> {
    if is_a(vm, object, INTERACTIVE_OBJECT)? || is_a(vm, object, AI_PAWN)? {
        let read = |vm: &mut Vm, name: &str| -> Result<i64, VmError> {
            element(vm, object, name, ty)?
                .map(|v| integer(&v))
                .ok_or_else(|| script_error(format!("the usable object's class has no {name}")))
        };
        let costs = read(vm, "bCostsToUse")?;
        let currency = read(vm, "CostsToUseType")?;
        let amount = read(vm, "CostsToUseAmount")?;
        // A byte and an int field: in range by their kinds.
        let costs = u8::try_from(costs).map_err(|_| script_error("bCostsToUse out of range"))?;
        let currency =
            u8::try_from(currency).map_err(|_| script_error("CostsToUseType out of range"))?;
        let amount =
            i32::try_from(amount).map_err(|_| script_error("CostsToUseAmount out of range"))?;
        return Ok(cost_of(costs, currency, amount));
    }
    if is_a(vm, object, WILLOW_PAWN)? {
        return Ok(None);
    }
    let class = vm.space.heap.get(object)?.class;
    let name = vm
        .space
        .classes
        .get_layout(class)
        .map(|l| l.path().as_str().to_string())
        .unwrap_or_default();
    Err(NotYet::not_read(
        format!("the IUsable cost query of {name} (read for WillowInteractiveObject, WillowAIPawn, WillowPawn only)"),
        RECORD_COST,
    )
    .into())
}

/// The controller's `PlayerReplicationInfo` when it is a `WillowPlayerReplicationInfo`.
fn replication_info(vm: &mut Vm, controller: ObjRef) -> Result<Option<ObjRef>, VmError> {
    let info = get(vm, controller, "PlayerReplicationInfo")?
        .as_ref()
        .and_then(object_of);
    match info {
        Some(i) if is_a(vm, i, REPLICATION_INFO)? => Ok(Some(i)),
        _ => Ok(None),
    }
}

/// The usability type argument (a byte; a missing one is primary).
fn usability_type(c: &NativeCall<'_>, i: usize) -> Result<u8, VmError> {
    if i >= c.arg_count() {
        return Ok(UT_PRIMARY);
    }
    u8::try_from(integer(c.arg(i)?)).map_err(|_| script_error("a usability type out of range"))
}

/// `CanAffordToUseUsableObject(UsedObject, UsabilityType)` (module documentation).
fn can_afford_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let controller = this(c, "CanAffordToUseUsableObject")?;
    let Some(object) = arg_object(c, 0)? else {
        return Ok(Value::Bool(true));
    };
    let ty = usability_type(c, 1)?;
    let cost = query_cost(c.vm, object, ty)?;
    let Some(cost) = cost else {
        return Ok(Value::Bool(true));
    };
    if cost.currency == GOLDEN_KEYS {
        return Err(golden_key_count().into());
    }
    let Some(info) = replication_info(c.vm, controller)? else {
        return Ok(Value::Bool(true));
    };
    let on_hand = call_method(c.vm, info, "GetCurrencyOnHand", vec![Value::Byte(cost.currency)])?
        .ok_or_else(|| script_error("the replication info has no GetCurrencyOnHand"))?;
    Ok(Value::Bool(can_afford(Some(cost), integer(&on_hand))?))
}

/// `PayForUsedObject(UsedObject, UsabilityType)` (module documentation).
fn pay_native(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let controller = this(c, "PayForUsedObject")?;
    let Some(object) = arg_object(c, 0)? else {
        return Ok(Value::None);
    };
    let ty = usability_type(c, 1)?;
    let cost = query_cost(c.vm, object, ty)?;
    match payment(cost).map_err(script_error)? {
        Payment::Nothing => {}
        Payment::SpendGoldenKeys(n) => {
            for _ in 0..n {
                call_method(c.vm, controller, "SpendGoldenKey", Vec::new())?
                    .ok_or_else(|| script_error("the controller has no SpendGoldenKey"))?;
            }
        }
        Payment::AddCurrency { currency, delta } => {
            let Some(info) = replication_info(c.vm, controller)? else {
                return Err(NotYet::not_read(
                    "PayForUsedObject with no WillowPlayerReplicationInfo on the controller",
                    RECORD_COST,
                )
                .into());
            };
            call_method(
                c.vm,
                info,
                "AddCurrencyOnHand",
                vec![Value::Byte(currency), Value::Int(delta)],
            )?
            .ok_or_else(|| script_error("the replication info has no AddCurrencyOnHand"))?;
        }
    }
    Ok(Value::None)
}

/// `WillowInteractiveObject.IsComponentUsable` (not read).
fn is_component_usable_native(_: &mut NativeCall<'_>) -> Result<Value, VmError> {
    Err(NotYet::not_read(
        "WillowInteractiveObject.IsComponentUsable (a virtual of the object; NATIVE_CONTROLLER_HELPERS.md, Not read yet)",
        RECORD_COST,
    )
    .into())
}

/// Binds `f` to the native at `path` when the package set declares it native and no pack bound it yet (the binding
/// rule of `ow_combat::hit::bind`).
fn bind(vm: &mut Vm, path: &str, f: NativeFn) -> Result<(), VmError> {
    ow_combat::hit::bind(vm, path, f)
}

/// The interactive-object natives (module documentation). Off only; Reference binds nothing. The currency natives
/// it calls (`GetCurrencyOnHand`, `AddCurrencyOnHand`) are `ow-combat`'s (register `ow_combat::hit::Pack` too).
#[derive(Debug, Clone, Copy, Default)]
pub struct Pack;

impl NativePack for Pack {
    fn register(&self, vm: &mut Vm) -> Result<(), VmError> {
        if vm.parity != ParityMode::Off {
            return Ok(());
        }
        bind(vm, CAN_AFFORD, Rc::new(can_afford_native))?;
        bind(vm, PAY, Rc::new(pay_native))?;
        bind(vm, IS_COMPONENT_USABLE, Rc::new(is_component_usable_native))
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The census of a map (package data)
// ---------------------------------------------------------------------------------------------------------------

/// The population definitions of lootables, chests and ammo crates are below this path (`prepare_interactives.py`).
pub const TREASURE: &str = "GD_Population_Treasure.";

/// How an interactive object gets into the level.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Origin {
    /// An actor the level places.
    Placed,
    /// Spawned at an opportunity point by its population's factory.
    Spawned,
}

/// What the use of one object reaches, as far as package data tells.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct CensusObject {
    pub level: String,
    /// The placed actor, or the opportunity point.
    pub path: String,
    pub origin: Origin,
    /// The placed actor's class, or the population factory's class.
    pub class: String,
    /// The interactive object definition its use fires events on, when it resolves.
    pub definition: Option<String>,
    /// The definition holds a behavior provider (where `OnUsedBy` would be answered).
    pub has_provider: bool,
    /// The definition holds a behavior that sets or changes the use cost.
    pub has_cost_behavior: bool,
}

fn text(v: &Pv) -> Option<String> {
    v.as_str().filter(|s| !s.is_empty() && *s != "None").map(str::to_string)
}

fn short(class: &str) -> &str {
    class.rsplit('.').next().unwrap_or(class)
}

/// Whether a placed actor's class is one this census counts: `WillowInteractiveObject` and the classes named after
/// it, the travel stations and the vending machines (the class hierarchy is not read offline; names only).
fn interactive_class(class: &str) -> bool {
    let c = short(class);
    c.contains("InteractiveObject") || c.ends_with("TravelStation") || c.contains("VendingMachine")
}

/// The interactive objects of every level of `view`: placed actors of an interactive class, then opportunity points
/// whose population is a treasure or shop population, each with the definition its use reaches, and what had to be
/// left out (an actor, point, factory or balance that did not decode), with the reason. The factory of a population
/// is the tool's (`LevelView::factory_of`). A use runs as script on the VM; in Off mode it stops at
/// `IsComponentUsable` (`NotYet`), so no object's use resolves there yet.
pub fn census(view: &MapView) -> (Vec<CensusObject>, Vec<Skip>) {
    let mut out = Vec::new();
    let mut skipped = Vec::new();
    for level in &view.levels {
        // A referenced object: `None` when absent; a decode failure is recorded and also gives `None`.
        let follow = |path: &str, skipped: &mut Vec<Skip>| match level.props(path) {
            Ok(p) => p,
            Err(e) => {
                skipped.push(Skip::new(&level.name, path, e));
                None
            }
        };
        let below = |definition: &Option<String>| -> (bool, bool) {
            let Some(d) = definition else {
                return (false, false);
            };
            let subs = level.lookup.below(d);
            let provider = subs.iter().any(|(_, c)| short(c) == "BehaviorProviderDefinition");
            let cost = subs
                .iter()
                .any(|(_, c)| matches!(short(c), "Behavior_SetUsabilityCost" | "Behavior_ChangeUsabilityCost"));
            (provider, cost)
        };
        for (path, class) in &level.exports {
            if path.matches('.').count() != 2 || !path.starts_with("TheWorld.PersistentLevel.") {
                continue;
            }
            if !interactive_class(class) && short(class) != "WillowPopulationOpportunityPoint" {
                continue;
            }
            let props = match level.props(path) {
                Ok(Some(p)) => p,
                Ok(None) => continue,
                Err(e) => {
                    skipped.push(Skip::new(&level.name, path, e));
                    continue;
                }
            };
            if interactive_class(class) {
                let definition = text(props.at("InteractiveObjectDefinition"));
                let (has_provider, has_cost_behavior) = below(&definition);
                out.push(CensusObject {
                    level: level.name.clone(),
                    path: path.clone(),
                    origin: Origin::Placed,
                    class: short(class).to_string(),
                    definition,
                    has_provider,
                    has_cost_behavior,
                });
                continue;
            }
            if short(class) != "WillowPopulationOpportunityPoint" {
                continue;
            }
            let Some(popdef) = text(props.at("PopulationDef")) else { continue };
            if !popdef.starts_with(TREASURE) && !popdef.starts_with(crate::vendors::POPULATION) {
                continue;
            }
            let (class, definition) = match level.factory_of(&popdef) {
                Some((f, c)) => {
                    let fp = follow(&f, &mut skipped).unwrap_or(Pv::Null);
                    let balance = text(fp.at("ObjectBalanceDefinition")).and_then(|b| follow(&b, &mut skipped));
                    let definition = text(fp.at("ObjectDefinition"))
                        .or_else(|| balance.as_ref().and_then(|b| text(b.at("DefaultInteractiveObject"))));
                    (short(&c).to_string(), definition)
                }
                None => (String::new(), None),
            };
            let (has_provider, has_cost_behavior) = below(&definition);
            out.push(CensusObject {
                level: level.name.clone(),
                path: path.clone(),
                origin: Origin::Spawned,
                class,
                definition,
                has_provider,
                has_cost_behavior,
            });
        }
    }
    (out, skipped)
}
