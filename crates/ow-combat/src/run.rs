//! Owner: lane W6-COMBAT. The scripted combat run of `ow-package --combat-run` (plan steps S-a to S-f): spawns pawns
//! on a [`World`], applies the steps and returns the JSON report whose `rows` object `tools/verify_combat_pawn.py
//! --vm` reads. Port of `src/combat_run.cpp`. AI-assisted (Claude Code).
//!
//! The steps, the report shape and every text are the C++ ones (`src/combat.hpp`, `combatRun`), with these
//! exceptions:
//! - **The mission bridge** (W6-MISSION's `ow_mission::bridge`, lane W6-COMBAT-RUN2): a run with a `maya` step is
//!   built over it, as the C++ (`player::PlayerBridge`, which `ow-cli` implements over the bridge). The `maya`, `tick`,
//!   `cash`, `bleed` and `mayahealth` steps, the S-d credit into the player's pool and the S-f bleed-out are in
//!   `run/player.rs`; without the bridge those steps answer the C++ texts. The player's health pool is the C++ bridge's
//!   hand-built stand-in, `ParityMode::Reference` only (decision 2026-11-16-54).
//! - **Loot** (plan step S-g, lane W6-LOOT): the rules are `crate::loot`'s; this module holds the steps (`seed`,
//!   `rollpool`, `lootbatch`), the loot setup of a spawn with a pawn balance and a kill's `loot` report, as the C++.
//!   In `Off` mode the death roll is the installed script's, so a kill records no roll and `lootbatch` is `NotYet`
//!   (decision 2026-11-16-41).
//! - **`SetHealth` events.** The C++ pool half logs `SetHealth <value>` into the event list; the Rust `SetHealth` is
//!   W5-POOLS' native, which logs nothing, so those lines are missing from a hit's `events` (the handoff's contract
//!   item: an observer hook in `ow_world::pools::set_health`). The `order` list still shows each call.
//! - **A behavior event raised on a provider that is running** (a script behavior broadcasting to its own provider) is
//!   raised after the running event returns, not inside it (`Provider` takes `&mut self`; the C++ re-enters).
//! - **Timing fields** (`milliseconds`, `elapsed_ms`) are the Rust run's own.
//! - **The AI holds** (decision 2026-11-16-83). In `Off` mode a pawn's `Behavior_AIHold` runs on the context itself
//!   and stops `NotYet` on the pawn question, and the `hold` / `release` steps pass the pawn as the hold's tag,
//!   because NATIVE_AI_HOLDS.md's rule refuses a None tag (`crate::holds`, which `hit::Pack` binds in `Off`). In
//!   `Reference` mode both are the C++: the pawn-to-mind context, a None tag and the C++ demigod list.
//!
//! **The damage-dealing behavior handlers.** The C++ registers its handlers per provider, each knowing its pawn (the
//! script's `SelfObject`): `Behavior_AIHold` and `Behavior_Conditional` on a pawn's class provider,
//! `Behavior_CustomEvent` and `Behavior_CompareBool` on its death definition's provider, all running their installed
//! script, and the classes that act on the world at the boundary. This module does the same through
//! `Provider::handle`. None is registered into the VM's global registry: each needs the consumer object, which
//! `registry::HandlerFn` does not receive (the additive `RunInfo` route of decision 2026-11-15-30 is W5-BEHAVIOR's
//! file). The C++ has no handler that deals damage itself (`Behavior_CauseDamage` is only read as data by
//! `hit::delivery_of`), so none is written: that class stays unregistered (`NotYet`).

use std::cell::RefCell;
use std::fmt::Write as _;
use std::rc::Rc;
use std::time::Instant;

use ow_behavior::kernel::{BehaviorId, Provider};
use ow_behavior::provider::BehaviorError;
use ow_behavior::registry::Handler;
use ow_core::fmt::{fmt_g, json_quote};
use ow_core::num::{StrtodPrefix, strtod_prefix};
use ow_core::{DetMap, NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;
use ow_world::world::World;

use crate::death::BehaviorEvent;
use crate::hit::access::{
    class_name, error, error_text, field, get, integer, loaded, number, object, object_of, path_of,
    set, truth,
};
use crate::hit::{self, Enemy, HitReport, Player, TakeDamageEvent, call_method, note, with_state};

mod player;
pub use player::{BridgeGain, PlayerBridge};
mod shots;
use player::{Episode, Fee, HealthPool};

/// The damage type of a player's hit ([`Run::player_hit`]): the `hit` step's stock path, resolved inside the chain
/// (so a bad path is the bridge guard's error, as the C++ `hitFromPlayer`; `""` is None), or a shot's object.
#[derive(Clone, Copy)]
enum DamageTypeOf<'a> {
    Path(&'a str),
    Object(Option<ObjRef>),
}

impl DamageTypeOf<'_> {
    fn resolve(self, vm: &mut Vm) -> Result<Option<ObjRef>, VmError> {
        match self {
            DamageTypeOf::Path("") => Ok(None),
            DamageTypeOf::Path(p) => Ok(Some(hit::stock_object(vm, p)?)),
            DamageTypeOf::Object(o) => Ok(o),
        }
    }
}

/// The record of the run.
pub const RECORD: &str = "src/combat.hpp (combatRun); COMBAT_GOLDEN_TRACE.md; the COMBAT_* records";

/// The stock archetypes the steps name: kind -> package, archetype path, pawn balance (empty for none).
pub const KINDS: [(&str, &str, &str, &str); 5] = [
    (
        "dummy",
        "Sanctuary_Dynamic",
        "GD_TargetDummy.Character.Pawn_TargetDummy",
        "",
    ),
    (
        "regular",
        "Ice_Dynamic",
        "GD_PrimalBeast.Character.Pawn_PrimalBeast",
        "GD_Population_PrimalBeast.Balance.PawnBalance_PrimalBeast",
    ),
    (
        "adult",
        "Ice_Dynamic",
        "GD_PrimalBeastAdult.Character.Pawn_PrimalBeastAdult",
        "GD_Population_PrimalBeast.Balance.PawnBalance_PrimalBeastAdult",
    ),
    (
        "ranged",
        "Ice_Dynamic",
        "GD_PrimalBeast_Ranged.Character.Pawn_PrimalBeast_Ranged",
        "GD_Population_PrimalBeast.Balance.PawnBalance_PrimalBeast_Slinger",
    ),
    (
        "baby",
        "Ice_Dynamic",
        "GD_PrimalBeastBaby.Character.Pawn_PrimalBeastBaby",
        "GD_Population_PrimalBeast.Balance.PawnBalance_PrimalBeast_Monglet",
    ),
];

/// What a swing step can name instead of a definition path: kind -> definition, cause damage (empty for none). The
/// throw is the Slinger's first throw animation's (UNVERIFIED that it is the one used live).
pub const SWINGS: [(&str, &str, &str); 6] = [
    (
        "melee",
        "GD_PrimalBeastShared.Melee.MeleeDef_PrimalBeast_Melee",
        "",
    ),
    (
        "leap",
        "GD_PrimalBeastShared.Melee.MeleeDef_PrimalBeast_RadiusLeap",
        "",
    ),
    (
        "badass",
        "GD_PrimalBeastShared.Melee.MeleeDef_PrimalBeast_RadiusBadass",
        "",
    ),
    (
        "grab",
        "GD_PrimalBeastShared.Melee.MeleeDef_PrimalBeast_MeleeGrabBaby",
        "",
    ),
    (
        "charge",
        "GD_PrimalBeastShared.Melee.MeleeDef_Melee_Charge",
        "",
    ),
    (
        "slinger",
        "GD_PrimalBeastShared.Anims.AnimMotion_SlingerThrow1.BehaviorProviderDefinition_43.Behavior_AIThrowProjectileAtTarget_142",
        "GD_PrimalBeastShared.Projectiles.Projectile_IceRock.BehaviorProviderDefinition_0.Behavior_CauseDamage_13",
    ),
];

/// Where the per-level base of the enemy health formula sits.
pub const BASE_HEALTH_PATH: &str =
    "GD_Balance_HealthAndDamage.HealthAndDamage.Init_BaseEnemyHealth";

/// The steps after which the C++ adds no `state` of the pawn the second field names (`rollpool` is not one: the C++
/// reports the pawn's state after it).
const NO_STATE: [&str; 14] = [
    "weapon",
    "spawn",
    "base",
    "player",
    "attr",
    "maya",
    "tick",
    "killtag",
    "players",
    "bleed",
    "cash",
    "seed",
    "mayahealth",
    "lootbatch",
];

/// What the loot batch kills with (C++ `killDamageType`).
const KILL_DAMAGE_TYPE: &str = "GD_Impact.DamageType.DmgType_Normal";

/// The `std::sto*` family as the MSVC library reads a step's numbers (the C++ texts of its exceptions).
fn integer_prefix(text: &str) -> Option<(bool, &str)> {
    let t = text.trim_start_matches([' ', '\t', '\n', '\r', '\x0b', '\x0c']);
    let (negative, rest) = match t.as_bytes().first() {
        Some(b'-') => (true, &t[1..]),
        Some(b'+') => (false, &t[1..]),
        _ => (false, t),
    };
    let digits = rest.bytes().take_while(u8::is_ascii_digit).count();
    (digits > 0).then(|| (negative, &rest[..digits]))
}

/// `std::stoi`.
pub fn stoi(text: &str) -> Result<i32, VmError> {
    let (negative, digits) = integer_prefix(text).ok_or_else(|| error("invalid stoi argument"))?;
    let v: i64 = digits
        .parse()
        .map_err(|_| error("stoi argument out of range"))?;
    let v = if negative { -v } else { v };
    i32::try_from(v).map_err(|_| error("stoi argument out of range"))
}

/// `std::stoll`.
pub fn stoll(text: &str) -> Result<i64, VmError> {
    let (negative, digits) = integer_prefix(text).ok_or_else(|| error("invalid stoll argument"))?;
    let v: i128 = digits
        .parse()
        .map_err(|_| error("stoll argument out of range"))?;
    let v = if negative { -v } else { v };
    i64::try_from(v).map_err(|_| error("stoll argument out of range"))
}

/// `std::stoul` (an unsigned long is 32 bits here; a leading `-` negates modulo 2^32, as `strtoul`).
pub fn stoul(text: &str) -> Result<usize, VmError> {
    let (negative, digits) = integer_prefix(text).ok_or_else(|| error("invalid stoul argument"))?;
    let v: u64 = digits
        .parse()
        .map_err(|_| error("stoul argument out of range"))?;
    let v = u32::try_from(v).map_err(|_| error("stoul argument out of range"))?;
    let v = if negative { v.wrapping_neg() } else { v };
    Ok(usize::try_from(v).unwrap_or(usize::MAX))
}

/// `std::stod`: decimal text, `inf` and `nan`; an overflow is `out_of_range`; a hexadecimal float is refused.
pub fn stod(text: &str) -> Result<f64, VmError> {
    let Some(StrtodPrefix {
        start,
        end,
        hexadecimal,
        ..
    }) = strtod_prefix(text)
    else {
        return Err(error("invalid stod argument"));
    };
    if hexadecimal {
        return Err(error(format!(
            "hexadecimal float text is not read by the Rust binary: {text}"
        )));
    }
    let lower = text[start..end].to_ascii_lowercase();
    let unsigned = lower.trim_start_matches(['+', '-']);
    let value = if unsigned.starts_with("inf") {
        f64::INFINITY
    } else if unsigned.starts_with("nan") {
        f64::NAN
    } else {
        let v = unsigned
            .parse::<f64>()
            .map_err(|_| error("invalid stod argument"))?;
        if v.is_infinite() {
            return Err(error("stod argument out of range"));
        }
        v
    };
    Ok(if lower.starts_with('-') {
        -value
    } else {
        value
    })
}

/// The C++ `number()` of `combat_run.cpp` (`setprecision(9)`).
fn num(x: f64) -> String {
    fmt_g(x, 9)
}

fn flag(b: bool) -> &'static str {
    if b { "true" } else { "false" }
}

/// A behavior provider error as a VM error.
fn provider_error(e: BehaviorError) -> VmError {
    match e {
        BehaviorError::Vm(v) => v,
        other => error(other.to_string()),
    }
}

// ---------------------------------------------------------------------------------------------------------------
// The providers (a VM service, so the natives' raises reach them)
// ---------------------------------------------------------------------------------------------------------------

/// Which provider of a pawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Which {
    /// Its AI class's `BehaviorProviderDefinition`.
    Class,
    /// Its death definition's.
    Death,
}

/// One provider and how many of its trace lines the events already list. `provider` is `None` while it runs (and
/// while it is being made), so a raise that reaches it then is deferred, not run on a second copy.
struct Held {
    provider: Option<Provider>,
    reported: usize,
}

/// One deferred raise: the pawn, which provider, the event, its output values, whether a pawn whose class has no
/// provider is skipped (a broadcast's class target, as the C++ catches that error), and where the target provider
/// stood when the event was deferred ([`Mark`]).
type Deferred = (ObjRef, Which, String, DetMap<String, String>, bool, Mark);

/// Where a provider that was running stood when an event was deferred onto it (the order tripwire, [`with_provider`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mark {
    /// The target provider was not running (a broadcast's class share queued behind its death share while the class
    /// provider is idle): not checked.
    NotHeld,
    /// The target provider was running. `trace`: its trace length when the raising behavior started (a script
    /// behavior of this run marks it; `None` for a handler that marks nothing, checked conservatively: more than one
    /// behavior ran). `notes`: the length of the events list at the deferral.
    Held { trace: Option<usize>, notes: usize },
}

/// The note a `Reference` run logs when an event deferred onto a running provider runs after behaviors that provider
/// ran after the raise ([`with_provider`]).
pub const ORDER_NOTE: &str = "note: combat-run (Rust): an event raised on a running behavior provider was deferred \
     past what the run did after the raise; the C++ runs it inside the raise, so its lines come earlier";

/// The most deferred raises one outermost raise drains (a Rust-only bound: events that keep raising each other on
/// running providers are a runaway, reported as an error rather than run forever).
pub const DEFERRED_BUDGET: usize = 10_000;

/// The run's providers, one class provider and one death provider per pawn, made on first use.
#[derive(Default)]
struct Providers {
    held: DetMap<(ObjRef, Which), Held>,
    /// Events raised on a provider that was running (module documentation), drained by the outermost raise.
    deferred: Vec<Deferred>,
    /// Raises in progress: only the outermost (depth 1) drains `deferred`.
    depth: u32,
    /// The script behaviors running, innermost last: the pawn, which provider, its trace length then ([`Mark`]). Its
    /// own cell, borrowed only for a push, a truncation or a lookup, so [`marked`] always clears its entry.
    marks: Rc<RefCell<Vec<(ObjRef, Which, usize)>>>,
    /// Where the running provider's own final report of lines began in the events list ([`report_lines`]), for the
    /// innermost [`with_provider`].
    report_at: Option<usize>,
}

/// Runs `f` as a behavior of the provider of `which` of `pawn` whose trace length is `len`, so a raise that `f` defers
/// onto that same provider records where it stood ([`Mark::Held`]).
fn marked<T>(
    vm: &mut Vm,
    pawn: ObjRef,
    which: Which,
    len: usize,
    f: impl FnOnce(&mut Vm) -> Result<T, VmError>,
) -> Result<T, VmError> {
    let marks = Rc::clone(&borrow(&providers(vm)?)?.marks);
    let depth = {
        let mut m = marks
            .try_borrow_mut()
            .map_err(|_| VmError::ServiceBorrowed("Providers.marks"))?;
        m.push((pawn, which, len));
        m.len() - 1
    };
    let result = f(vm);
    // Back to the depth before the push whatever `f` did (review r2 of PR #356): a stale entry never survives.
    let cleared = marks
        .try_borrow_mut()
        .map(|mut m| m.truncate(depth))
        .map_err(|_| VmError::ServiceBorrowed("Providers.marks"));
    let value = result?;
    cleared?;
    Ok(value)
}

/// The mark of an event about to be deferred onto the provider of `which` of `pawn` ([`Mark`]).
fn held_mark(vm: &Vm, pawn: ObjRef, which: Which) -> Result<Mark, VmError> {
    let service = providers(vm)?;
    let s = borrow(&service)?;
    let running = matches!(s.held.get(&(pawn, which)), Some(h) if h.provider.is_none());
    if !running {
        return Ok(Mark::NotHeld);
    }
    let trace = s
        .marks
        .try_borrow()
        .map_err(|_| VmError::ServiceBorrowed("Providers.marks"))?
        .iter()
        .rev()
        .find(|m| m.0 == pawn && m.1 == which)
        .map(|m| m.2);
    drop(s);
    let notes = with_state(vm, |st| st.events.len())?;
    Ok(Mark::Held { trace, notes })
}

/// The providers service, borrowed for a short scope (no script call inside).
fn borrow(service: &Rc<RefCell<Providers>>) -> Result<std::cell::RefMut<'_, Providers>, VmError> {
    service
        .try_borrow_mut()
        .map_err(|_| VmError::ServiceBorrowed("Providers"))
}

fn providers(vm: &Vm) -> Result<Rc<RefCell<Providers>>, VmError> {
    vm.service::<Providers>()
        .ok_or_else(|| NotYet::layer_absent("the run's providers", "ow-combat run", RECORD).into())
}

/// A behavior that is installed script runs its `ApplyBehaviorToContext` on the VM, once per context object the
/// kernel resolves for it; the output links its script selects (`ActivateBehaviorOutputLink`) are the ids returned.
fn run_script_behavior(
    vm: &mut Vm,
    p: &mut Provider,
    id: BehaviorId,
    pawn: ObjRef,
    which: Which,
) -> Result<Vec<i32>, VmError> {
    let behavior = p.behavior(id).clone();
    let class = vm.space.heap.get(behavior.object)?.class;
    let Some(apply) = vm.find_method(class, "ApplyBehaviorToContext", None)? else {
        return Err(error(format!(
            "combat-run: no ApplyBehaviorToContext on {}",
            behavior.class
        )));
    };
    let params = vm
        .function(apply)
        .map(|d| d.params.clone())
        .unwrap_or_default();
    let contexts = p.contexts_for(vm, id, pawn)?;
    let len = p.trace.len();
    // The output links go to the behavior kernel pack's shared collector (ow_behavior::kernel::natives::collect).
    // A raise the script defers onto this provider records where it stood (the order tripwire).
    let (result, ids) = marked(vm, pawn, which, len, |vm| ow_behavior::kernel::natives::collect(vm, |vm| {
        for context in contexts {
            let args = params
                .iter()
                .map(|d| match vm.space.names.text(d.name).as_ref() {
                    "ContextObject" => Value::Object(Some(context)),
                    "SelfObject" => Value::Object(Some(pawn)),
                    _ => vm.space.classes.zero_value(&d.kind),
                })
                .collect();
            vm.call(apply, Some(behavior.object), args)?;
        }
        Ok::<(), VmError>(())
    }))?;
    result.map(|()| ids)
}

/// A provider over `definition`, with the handlers of `which` (module documentation) and registered on its consumer.
fn make_provider(
    vm: &mut Vm,
    definition: ObjRef,
    pawn: ObjRef,
    which: Which,
) -> Result<Provider, VmError> {
    let mut p = Provider::new(vm, definition).map_err(provider_error)?;
    let script = move |vm: &mut Vm, p: &mut Provider, id: BehaviorId, _: &str| {
        run_script_behavior(vm, p, id, pawn, which).map_err(BehaviorError::Vm)
    };
    match which {
        Which::Class => {
            p.handle(
                "GearboxFramework.Behavior_AIHold",
                Handler::run(move |vm, p, id, _| {
                    let object = p.behavior(id).object;
                    // Off: the context itself, and the pawn question NotYet (decision 2026-11-16-83); Reference: the
                    // C++ pawn-to-mind stand-in.
                    if vm.parity == ParityMode::Off {
                        let contexts = p.contexts_for(vm, id, pawn)?;
                        crate::holds::apply_hold_behavior(vm, object, &contexts)?;
                        return Ok(Vec::new());
                    }
                    for context in p.contexts_for(vm, id, pawn)? {
                        let target = hit::ai_interface_for(vm, context)?;
                        call_method(
                            vm,
                            object,
                            "ApplyBehaviorToContext",
                            vec![Value::Object(Some(target))],
                        )?;
                    }
                    Ok(Vec::new())
                }),
            );
            // Behavior_Conditional is installed script: its conditions are expression trees over the pawn, loaded
            // before the script calls Evaluate on them.
            p.handle(
                "WillowGame.Behavior_Conditional",
                Handler::run(move |vm, p, id, event| {
                    let object = p.behavior(id).object;
                    if let Some(conditions) = get(vm, object, "Conditions")? {
                        for entry in conditions.elements().unwrap_or(&[]) {
                            if let Some(tree) =
                                field(vm, entry, "Condition").as_ref().and_then(object_of)
                            {
                                loaded(vm, tree)?;
                            }
                        }
                    }
                    script(vm, p, id, event)
                }),
            );
            for class in [
                "WillowGame.Behavior_Transform",
                "WillowGame.Behavior_AttemptStatusEffect",
                "WillowGame.Behavior_UpdateMissionObjective",
                "WillowGame.Behavior_RegisterTargetable",
                "GearboxFramework.Behavior_ChangeRemoteBehaviorSequenceState",
                "Engine.Behavior_RemoteEvent",
            ] {
                p.report_at_boundary(class, None, Vec::new());
            }
            p.report_at_boundary("GearboxFramework.Behavior_SpecialMove", None, vec![0]);
        }
        Which::Death => {
            for class in [
                "GearboxFramework.Behavior_CustomEvent",
                "GearboxFramework.Behavior_CompareBool",
            ] {
                p.handle(class, Handler::run(script));
            }
            for class in [
                "WillowGame.Behavior_CoordinatedEffect",
                "GearboxFramework.Behavior_TriggerDialogEvent",
                "WillowGame.Behavior_ChangeInstanceDataSwitch",
                "WillowGame.PlayerBehavior_UnlockAchievementForAllPlayers",
                "WillowGame.Behavior_SpawnProjectile",
                "GearboxFramework.Behavior_PostAkEvent",
                "WillowGame.Behavior_SpawnParticleSystem",
                "WillowGame.Behavior_CreateImpactEffect",
                "Engine.Behavior_Destroy",
                "GearboxFramework.Behavior_ChangeRemoteBehaviorSequenceState",
                "WillowGame.Behavior_MissionCustomEvent",
                "WillowGame.Behavior_SpawnItems",
                "WillowGame.Behavior_DamageSourceSwitch",
                "WillowGame.Behavior_AddObjectInstanceData",
            ] {
                p.report_at_boundary(class, None, Vec::new());
            }
        }
    }
    p.register_consumer(vm).map_err(provider_error)?;
    Ok(p)
}

/// The definition the provider of `which` of `pawn` is made over.
fn definition_of(
    vm: &mut Vm,
    pawn: ObjRef,
    which: Which,
    death: Option<ObjRef>,
) -> Result<ObjRef, VmError> {
    let found = match which {
        Which::Class => match object(vm, pawn, "AIClass")? {
            Some(class) => object(vm, class, "BehaviorProviderDefinition")?,
            None => None,
        },
        Which::Death => match death {
            Some(d) => object(vm, d, "BehaviorProviderDefinition")?,
            None => None,
        },
    };
    found.ok_or_else(|| {
        error(match which {
            Which::Class => "combat-run: the pawn's AI class has no behavior provider",
            Which::Death => "combat-run: the death definition has no behavior provider",
        })
    })
}

/// Whether the pawn's AI class names a behavior provider definition (`AIClass` -> `BehaviorProviderDefinition`), the
/// class test of what [`definition_of`] reads for [`Which::Class`]. A broadcast's class share skips a pawn without
/// one, as the C++ catches that error; any other failure of the class provider is an error here.
fn has_class_provider(vm: &mut Vm, pawn: ObjRef) -> Result<bool, VmError> {
    Ok(match object(vm, pawn, "AIClass")? {
        Some(class) => object(vm, class, "BehaviorProviderDefinition")?.is_some(),
        None => false,
    })
}

/// Takes the provider of `which` of `pawn` out of the service (made on first use; `death` is the death definition),
/// runs `f` on it and puts it back. A placeholder marks it running from before it is made until it is back, so a raise
/// that reaches it meanwhile is `Ok(None)` (deferred by the caller) and never makes a second provider over the same
/// definition. A provider that fails to be made leaves no entry.
fn with_provider<T>(
    vm: &mut Vm,
    pawn: ObjRef,
    which: Which,
    death: Option<ObjRef>,
    f: impl FnOnce(&mut Vm, &mut Provider, &mut usize) -> Result<T, VmError>,
) -> Result<Option<T>, VmError> {
    let service = providers(vm)?;
    let taken = {
        let mut s = borrow(&service)?;
        match s.held.get_mut(&(pawn, which)) {
            Some(held) => match held.provider.take() {
                Some(p) => Some((p, held.reported)),
                None => return Ok(None),
            },
            None => {
                let placeholder = Held {
                    provider: None,
                    reported: 0,
                };
                s.held.insert((pawn, which), placeholder);
                None
            }
        }
    };
    let (mut p, mut reported) = match taken {
        Some(pair) => pair,
        None => {
            let made = definition_of(vm, pawn, which, death)
                .and_then(|d| make_provider(vm, d, pawn, which));
            match made {
                Ok(p) => {
                    let reported = p.trace.len();
                    (p, reported)
                }
                Err(e) => {
                    borrow(&service)?.held.remove(&(pawn, which));
                    return Err(e);
                }
            }
        }
    };
    // The order tripwire looks at what is deferred from here on: a raise made while the provider was being made
    // (`make_provider` above, its consumer registration) falls before `before` and is not checked.
    let (start, before, outer_report) = {
        let mut s = borrow(&service)?;
        (p.trace.len(), s.deferred.len(), s.report_at.take())
    };
    let result = f(vm, &mut p, &mut reported);
    let end = p.trace.len();
    let back = Held {
        provider: Some(p),
        reported,
    };
    let own_report = {
        let mut s = borrow(&service)?;
        s.held.insert((pawn, which), back);
        std::mem::replace(&mut s.report_at, outer_report)
    };
    let notes_end = match own_report {
        Some(at) => at,
        None => with_state(vm, |st| st.events.len())?,
    };
    let late: Vec<String> = {
        let s = borrow(&service)?;
        let held: Vec<&Deferred> = s
            .deferred
            .get(before..)
            .unwrap_or(&[])
            .iter()
            .filter(|d| d.0 == pawn && d.1 == which && d.5 != Mark::NotHeld)
            .collect();
        // Two events deferred onto the one held provider: the C++ runs the first inside, before the raiser goes on to
        // the second (review r2 of PR #356).
        let second = held.len() > 1;
        held.iter()
            .filter(|d| match d.5 {
                Mark::NotHeld => false,
                Mark::Held { trace, notes } => {
                    second
                        // Its own run went on past the raising behavior.
                        || trace.map_or(end.saturating_sub(start) > 1, |m| end > m)
                        // Something was listed after the deferral and before the provider's own final report: the C++
                        // lists the provider's lines at the raise, before it (review r2 of PR #356).
                        || notes_end > notes
                }
            })
            .map(|d| d.2.clone())
            .collect()
    };
    let value = result?;
    order_tripwire(vm, &late)?;
    Ok(Some(value))
}

/// The order tripwire (reviews r1 and r2 of PR #356, decision 2026-11-16-54): an event deferred onto a running
/// provider runs after whatever that provider's run did after the raise, where the C++ re-enters and runs it (and lists
/// the provider's lines) at the raise. It trips when, after the deferral, the provider ran more of its own behaviors,
/// anything was listed before the provider's own final report (a note, another provider's lines, a second broadcast),
/// or a second event was deferred onto the same held provider (the hit's events list and a later `CompareBool` can
/// differ). `Off`: `NotYet { NotPorted }`; `Reference`: the labelled
/// [`ORDER_NOTE`], and the run goes on as before.
fn order_tripwire(vm: &mut Vm, late: &[String]) -> Result<(), VmError> {
    if late.is_empty() {
        return Ok(());
    }
    let events = late.join(", ");
    if vm.parity == ow_core::ParityMode::Reference {
        vm.log.push(format!("{ORDER_NOTE} ({events})"));
        return Ok(());
    }
    Err(NotYet::not_ported(
        format!(
            "{events} raised on a running behavior provider whose run went on after the raise (more behaviors, more \
             listed lines, or a second deferred event): the C++ runs it inside the raise; this port defers it past them"
        ),
        RECORD,
    )
    .into())
}

/// Runs `f` as one raise. The outermost raise then drains the raises deferred meanwhile, in order (each of which may
/// defer more), at most [`DEFERRED_BUDGET`] of them; an inner raise leaves them to it. A failed outermost raise drops
/// what it deferred: nothing runs after its error.
fn as_raise(vm: &mut Vm, f: impl FnOnce(&mut Vm) -> Result<(), VmError>) -> Result<(), VmError> {
    let service = providers(vm)?;
    let step = |service: &Rc<RefCell<Providers>>, enter: bool| -> Result<u32, VmError> {
        let mut s = service
            .try_borrow_mut()
            .map_err(|_| VmError::ServiceBorrowed("Providers"))?;
        s.depth = if enter {
            s.depth.saturating_add(1)
        } else {
            s.depth.saturating_sub(1)
        };
        Ok(s.depth)
    };
    let depth = step(&service, true)?;
    let mut result = f(vm);
    if result.is_ok() && depth == 1 {
        result = drain(vm, &service);
    }
    if result.is_err()
        && depth == 1
        && let Ok(mut s) = service.try_borrow_mut()
    {
        s.deferred.clear();
    }
    step(&service, false)?;
    result
}

/// The drain of [`as_raise`], depth first as the C++ re-entry runs them: what a drained event defers runs before the
/// events deferred ahead of it. No provider runs at the outermost drain, so a drained event that would be deferred
/// again names a provider held outside every raise: an error at once.
fn drain(vm: &mut Vm, service: &Rc<RefCell<Providers>>) -> Result<(), VmError> {
    for _ in 0..DEFERRED_BUDGET {
        let next = {
            let mut s = borrow(service)?;
            if s.deferred.is_empty() {
                None
            } else {
                let item = s.deferred.remove(0);
                Some((item, s.deferred.len()))
            }
        };
        let Some(((pawn, which, name, outputs, skip_missing, _), rest)) = next else {
            return Ok(());
        };
        let death = if which == Which::Death {
            object(vm, pawn, "MyDeathDef")?
        } else {
            None
        };
        let deferred_again = if skip_missing && !has_class_provider(vm, pawn)? {
            false
        } else {
            fire_or_defer(vm, pawn, which, death, &name, &outputs)?
        };
        let mut s = borrow(service)?;
        if deferred_again {
            let which = match which {
                Which::Class => "class",
                Which::Death => "death",
            };
            return Err(error(format!(
                "combat-run: {name} was raised on the {which} provider of a pawn while that provider is held outside the raise"
            )));
        }
        // What this event deferred (pushed after the `rest` already waiting) runs next.
        let own = s.deferred.split_off(rest);
        s.deferred.splice(0..0, own);
    }
    Err(error(format!(
        "combat-run: more than {DEFERRED_BUDGET} behavior events raised on running providers (a runaway)"
    )))
}

/// Fires `name` with `outputs` on the provider of `which` of `pawn` and lists what ran, or defers it when that
/// provider is running. True when it was deferred.
fn fire_or_defer(
    vm: &mut Vm,
    pawn: ObjRef,
    which: Which,
    death: Option<ObjRef>,
    name: &str,
    outputs: &DetMap<String, String>,
) -> Result<bool, VmError> {
    let done = with_provider(vm, pawn, which, death, |vm, p, reported| {
        p.fire_event(vm, name, outputs, -1, &[])
            .map_err(provider_error)?;
        report_lines(vm, p, reported)
    })?;
    if done.is_none() {
        let mark = held_mark(vm, pawn, which)?;
        defer(vm, (pawn, which, name.to_string(), outputs.clone(), false, mark))?;
        return Ok(true);
    }
    Ok(false)
}

/// Appends a deferred raise.
fn defer(vm: &Vm, item: Deferred) -> Result<(), VmError> {
    borrow(&providers(vm)?)?.deferred.push(item);
    Ok(())
}

/// Lists the provider's trace lines since `reported` and its errors as events (`behavior ...`, `provider error: ...`).
fn report_lines(vm: &Vm, p: &mut Provider, reported: &mut usize) -> Result<(), VmError> {
    // Where this provider's own report begins (the order tripwire's end of what counts as "listed after").
    let at = with_state(vm, |st| st.events.len())?;
    borrow(&providers(vm)?)?.report_at = Some(at);
    let lines: Vec<String> = p.trace.get(*reported..).unwrap_or(&[]).to_vec();
    *reported = p.trace.len();
    for line in lines {
        note(vm, format!("behavior {line}"))?;
    }
    for e in std::mem::take(&mut p.errors) {
        note(vm, format!("provider error: {e}"))?;
    }
    Ok(())
}

/// A behavior event of the death chain: raised on the death definition's provider, a broadcast on the pawn's class
/// provider too (C++ `raiseBehaviorEvent`). An event that reaches a running provider runs after the outermost raise's
/// own event ([`as_raise`]).
fn raise_behavior_event(vm: &mut Vm, event: &BehaviorEvent) -> Result<(), VmError> {
    as_raise(vm, |vm| {
        let definition = match event.definition {
            Some(d) => Some(d),
            None => object(vm, event.pawn, "MyDeathDef")?,
        };
        let none = DetMap::new();
        let mut death_deferred = false;
        if definition.is_some() {
            death_deferred =
                fire_or_defer(vm, event.pawn, Which::Death, definition, &event.name, &none)?;
        }
        if event.broadcast {
            // The C++ order is [death provider, class provider]: once the death share waits, the class share waits
            // right behind it. A pawn whose class has no provider has nothing more to raise it to (the C++ catches
            // the error).
            if death_deferred {
                // Checked like any deferral when the class provider itself is running (review r2 of PR #356).
                let mark = held_mark(vm, event.pawn, Which::Class)?;
                defer(
                    vm,
                    (event.pawn, Which::Class, event.name.clone(), none, true, mark),
                )?;
                return Ok(());
            }
            if has_class_provider(vm, event.pawn)? {
                fire_or_defer(vm, event.pawn, Which::Class, None, &event.name, &none)?;
            }
        }
        Ok(())
    })
}

/// Enables or disables the sequences `name` of the pawn's class provider (the `sequence` step): one raise, since the
/// enabled and disabled events run behaviors that may raise on the same provider ([`as_raise`]). False when there is
/// no such sequence, or when the provider is running (the C++ would re-enter; no step reaches that).
///
/// **The tripwire** (decision 2026-11-16-54). An event the step's own events raise on this same class provider is
/// deferred until the step has put the provider back, so it runs on the provider with the sequence already changed,
/// where the C++ re-enters and runs it before the change is complete (decision 2026-11-15-35, the cost of deferring).
/// That run is not the C++ one, so when the step leaves such an event deferred: `Off` answers `NotYet { NotPorted }`
/// (the sequence change itself has happened; the deferred event is dropped, not run), and `Reference` logs the
/// labelled note [`SEQUENCE_DEFERRAL_NOTE`] and runs it as before.
fn set_sequence(vm: &mut Vm, pawn: ObjRef, name: &str, on: bool) -> Result<bool, VmError> {
    let mut changed = false;
    as_raise(vm, |vm| {
        // As the C++ step, the lines its events add are not listed here (a later raise of the provider lists them).
        changed = with_provider(vm, pawn, Which::Class, None, |vm, p, _| {
            p.set_sequence_enabled(vm, name, on).map_err(provider_error)
        })?
        .unwrap_or(false);
        let own: Vec<String> = borrow(&providers(vm)?)?
            .deferred
            .iter()
            .filter(|d| d.0 == pawn && d.1 == Which::Class)
            .map(|d| d.2.clone())
            .collect();
        if !own.is_empty() {
            let events = own.join(", ");
            if vm.parity == ow_core::ParityMode::Reference {
                vm.log.push(format!("{SEQUENCE_DEFERRAL_NOTE} ({events})"));
            } else {
                return Err(NotYet::not_ported(
                    format!(
                        "the sequence step's events raised {events} on the same class provider: the C++ runs them \
                         inside the step, before the sequence change is complete; this port defers them past it"
                    ),
                    RECORD,
                )
                .into());
            }
        }
        Ok(())
    })?;
    Ok(changed)
}

/// The note a `Reference` run logs when the sequence step's own events raise on its class provider ([`set_sequence`]).
pub const SEQUENCE_DEFERRAL_NOTE: &str = "note: combat-run (Rust): the sequence step deferred events on its own class \
     provider; they run after the sequence change, where the C++ runs them inside it";

/// `NotifyTakeHit`'s raise (NATIVE_DAMAGE_DEATH.md section 5): `OnTakeDamage` on the pawn's class provider with the
/// damage type definition and source as the event's `DamageType` and `DamageSource` outputs (stock paths); the
/// behaviors that ran are added to the hit's event list.
fn raise_take_damage(vm: &mut Vm, event: &TakeDamageEvent) -> Result<(), VmError> {
    note(
        vm,
        format!("OnTakeDamage raised (damage={})", num(event.damage)),
    )?;
    let definition = match object(vm, event.pawn, "AIClass")? {
        Some(class) => object(vm, class, "BehaviorProviderDefinition")?,
        None => None,
    };
    if definition.is_none() {
        return note(vm, "the pawn's AI class has no behavior provider");
    }
    let mut outputs = DetMap::new();
    outputs.insert("DamageType".to_string(), path_of(vm, event.damage_type));
    let source = match event.source {
        Some(k) => vm
            .space
            .classes
            .get_layout(k)
            .map(|l| l.path().as_str().to_string())
            .unwrap_or_default(),
        None => String::new(),
    };
    outputs.insert("DamageSource".to_string(), source);
    let pawn = event.pawn;
    as_raise(vm, |vm| {
        fire_or_defer(vm, pawn, Which::Class, None, "OnTakeDamage", &outputs).map(|_| ())
    })
}

// ---------------------------------------------------------------------------------------------------------------
// The run
// ---------------------------------------------------------------------------------------------------------------

/// One spawned pawn.
struct Spawned {
    package: String,
    path: String,
    row_class: String,
    level: i32,
    stage: i32,
    milliseconds: f64,
    enemy: Enemy,
    /// The references the pool step made, in order.
    pools: Vec<Value>,
}

/// A kill's row (`kill.<class>.L<level>.p<player level>[.<tag>]`): the award `CalculateExperiencePointsForKill`
/// returned and, with the mission bridge, the experience its pool gained.
struct KillRow {
    id: String,
    award: f64,
    credit: Option<i32>,
}

/// The scripted run over a world (C++ `Run`).
pub struct Run {
    world: World,
    pawns: Vec<Spawned>,
    players: Vec<Player>,
    rows: DetMap<String, f64>,
    hit_counts: DetMap<String, u32>,
    kill_rows: Vec<KillRow>,
    kill_tag: String,
    kills_seen: usize,
    /// The death rolls already reported (C++ `dropsSeen_`).
    drops_seen: usize,
    results: Vec<String>,
    errors: Vec<String>,
    /// The mission bridge's player (a run with a `maya` step), with the state of its steps.
    bridge: Option<Rc<dyn PlayerBridge>>,
    health: HealthPool,
    episode: Episode,
    injuries: i32,
    /// When the respawn happened, while its dialog (the fee) is still due; negative when none is.
    respawn_at: f64,
    fee_rows: Vec<Fee>,
    /// The bridge's errors the steps already listed, and the script errors of the step running.
    bridge_errors_seen: usize,
    script_errors: Vec<String>,
    /// The player's held weapon (the `weapon` step, [`Run::hold_weapon`]): the instance and its firing mode.
    held: Option<(ObjRef, Option<ObjRef>)>,
    /// Off: the stop of a class switch that failed part-way (a `maya` step), which every later step answers with
    /// (decision 2026-11-19-46, review of #431). The switch writes the controller's class before it can stop, so a
    /// later step would build on a half-switched player.
    class_switch_stop: Option<VmError>,
}

impl std::fmt::Debug for Run {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Run")
            .field("pawns", &self.pawns.len())
            .field("errors", &self.errors)
            .finish_non_exhaustive()
    }
}

impl Run {
    /// A run over `world`, whose VM holds the combat pack (`hit::Pack`): finds `ROLE_Authority` by its enum name,
    /// makes the level's `WorldInfo` (authority) and places it, loads `Startup` when the set has it, sets the raises of
    /// the providers and enables deaths (C++ `CombatSystem` and `Run` constructors).
    pub fn new(world: World) -> Result<Run, VmError> {
        Run::with_bridge(world, None)
    }

    /// [`Run::new`], over the mission bridge's player when `bridge` is given (a run with a `maya` step). The bridge has
    /// placed the level's `WorldInfo` already (its `MissionScript::new`), so the run uses that one, as the C++ combat
    /// system of the bridge does, and places none.
    pub fn with_bridge(mut world: World, bridge: Option<Rc<dyn PlayerBridge>>) -> Result<Run, VmError> {
        let vm = &mut world.vm;
        let roles = ow_behavior::provider::enum_names(vm, "Engine", "Actor.ENetRole")
            .map_err(provider_error)?;
        let authority = roles
            .iter()
            .position(|r| r == "ROLE_Authority")
            .and_then(|i| u8::try_from(i).ok())
            .ok_or_else(|| error("combat: ENetRole has no ROLE_Authority"))?;
        let placed = world
            .actors()
            .try_borrow()
            .map_err(|_| VmError::ServiceBorrowed("Actors"))?
            .world_info();
        let info = match (&bridge, placed) {
            (Some(_), Some(info)) => info,
            (Some(_), None) => {
                return Err(error("combat-run: the mission bridge placed no WorldInfo"));
            }
            (None, _) => {
                let vm = &mut world.vm;
                let class = vm.find_class("Engine.WorldInfo")?;
                let info = ow_vm::interp::instantiate_from_defaults(vm, class)?;
                crate::hit::access::set_required(vm, info, "Role", Value::Byte(authority), "combat")?;
                world
                    .actors()
                    .try_borrow_mut()
                    .map_err(|_| VmError::ServiceBorrowed("Actors"))?
                    .place_world_info(info)?;
                info
            }
        };
        let vm = &mut world.vm;
        if let Ok(startup) = ow_core::PackageName::parse("Startup") {
            let _ = vm.defs.code_mut().1.load(&startup);
        }
        // The drop weights' local player (Reference only: the C++ stand-in; decision 2026-11-16-73).
        crate::loot::register_local_player_pool(vm, info)?;
        with_state(vm, |s| {
            s.world = Some(info);
            s.authority = authority;
            s.on_take_damage = Some(Rc::new(raise_take_damage));
            s.on_behavior_event = Some(Rc::new(raise_behavior_event));
        })?;
        if let Some(b) = &bridge {
            // The injured definition the player's class names (C++ `MissionScript::injuredDefinition`): the store's
            // definition of the reference (the C++ makes a copy; nothing writes to it).
            let b = Rc::clone(b);
            let hook: hit::ObjectFn = Rc::new(move |vm: &mut Vm| match b.player_class(vm)? {
                Some(class) => object(vm, class, "CharacterInjuredDefinition"),
                None => Ok(None),
            });
            with_state(vm, |s| s.injured_definition = Some(hook))?;
        }
        if vm.service::<Providers>().is_none() {
            vm.add_service(Providers::default())?;
        }
        crate::death::enable_deaths(vm)?;
        crate::loot::enable(vm)?;
        Ok(Run {
            world,
            pawns: Vec::new(),
            players: Vec::new(),
            rows: DetMap::new(),
            hit_counts: DetMap::new(),
            kill_rows: Vec::new(),
            kill_tag: String::new(),
            kills_seen: 0,
            drops_seen: 0,
            results: Vec::new(),
            errors: Vec::new(),
            bridge,
            health: HealthPool::default(),
            episode: Episode::new(),
            injuries: 0,
            respawn_at: -1.0,
            fee_rows: Vec::new(),
            bridge_errors_seen: 0,
            script_errors: Vec::new(),
            held: None,
            class_switch_stop: None,
        })
    }

    /// The world the run is on.
    pub fn world(&mut self) -> &mut World {
        &mut self.world
    }

    /// Whether a step failed.
    pub fn failed(&self) -> bool {
        !self.errors.is_empty()
    }

    /// The errors of the steps so far (`<step>: <text>`).
    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    /// The ids of the report's rows so far, read only: the value rows, then one id per kill row (a repeated kill id
    /// repeats; the report letters them). Lane SLICE-RUN-OFF: the slice gate grades each link on the rows its own steps
    /// added.
    pub fn row_ids(&self) -> impl Iterator<Item = &str> {
        self.rows.keys().map(String::as_str).chain(self.kill_rows.iter().map(|k| k.id.as_str()))
    }

    /// Pawn `n` of the spawn steps, in spawn order.
    pub fn enemy(&self, n: usize) -> Option<Enemy> {
        self.pawns.get(n).map(|s| s.enemy)
    }

    /// Player `n` of the player steps.
    pub fn player(&self, n: usize) -> Option<Player> {
        self.players.get(n).copied()
    }

    fn row(&mut self, id: &str, value: f64) {
        let mut key = id.to_string();
        let mut copy = 2;
        while self.rows.contains_key(&key) {
            key = format!("{id}#{copy}");
            copy += 1;
        }
        self.rows.insert(key, value);
    }

    fn pawn(&self, index: &str) -> Result<usize, VmError> {
        let at = stoul(index)?;
        if at >= self.pawns.len() {
            return Err(error(format!("combat-run: no pawn {index}")));
        }
        Ok(at)
    }

    /// A method by name on `o`'s class (C++ `Run::call`): an error when the class has none.
    fn call(
        &mut self,
        o: ObjRef,
        name: &str,
        args: Vec<Value>,
    ) -> Result<ow_vm::api::CallResult, VmError> {
        let vm = &mut self.world.vm;
        let class = vm.space.heap.get(o)?.class;
        let Some(f) = vm.find_method(class, name, None)? else {
            return Err(error(format!(
                "combat-run: {} has no function {name}",
                class_name(vm, o)
            )));
        };
        vm.call(f, Some(o), args)
    }

    /// The export `path` of package `package` (it must hold it), as the store's definition of the path.
    fn export_of(&mut self, package: &str, path: &str) -> Result<Option<ObjRef>, VmError> {
        let vm = &mut self.world.vm;
        let Ok(name) = ow_core::PackageName::parse(package) else {
            return Ok(None);
        };
        let held = {
            let (_, store) = vm.defs.code_mut();
            let id = store
                .load(&name)
                .map_err(|e| error(format!("combat-run: {e}")))?;
            store
                .get(id)
                .find_export(path)
                .ok()
                .flatten()
                .is_some_and(|i| i > 0)
        };
        if !held {
            return Ok(None);
        }
        let Ok(parsed) = ow_core::ObjectPath::parse(path) else {
            return Ok(None);
        };
        let found = vm.defs.find(&mut vm.space, &parsed);
        vm.log.extend(vm.defs.take_log());
        found.map_err(VmError::Def)
    }

    /// Applies one step (C++ `Run::apply`): its result, or `{"error":...}` with the error recorded; then the state of
    /// the pawn the step names.
    pub fn apply(&mut self, step: &str) {
        let parts: Vec<&str> = step.split(':').collect();
        let mut result = match self.step(&parts, step) {
            Ok(r) => r,
            Err(e) => {
                let text = error_text(&e);
                self.errors.push(format!("{step}: {text}"));
                format!("{{\"error\":{}}}", json_quote(&text))
            }
        };
        // A script run the bridge's guard caught: the step failed (C++ `script_->errors`).
        if let Some(b) = &self.bridge {
            let all = b.errors();
            for e in all.iter().skip(self.bridge_errors_seen) {
                self.errors.push(format!("{step}: {e}"));
            }
            self.bridge_errors_seen = all.len();
        }
        for e in std::mem::take(&mut self.script_errors) {
            self.errors.push(format!("{step}: {e}"));
        }
        if parts.len() > 1
            && !NO_STATE.contains(&parts[0])
            && let Ok(index) = stoul(parts[1])
            && index < self.pawns.len()
            && let Ok(state) = self.state_json(index)
        {
            result.pop();
            if result.len() > 1 {
                result.push(',');
            }
            let _ = write!(result, "\"state\":{{{state}}}}}");
        }
        self.results.push(format!(
            "{{\"step\":{},\"result\":{result}}}",
            json_quote(step)
        ));
    }

    fn step(&mut self, parts: &[&str], step: &str) -> Result<String, VmError> {
        if let Some(stop) = &self.class_switch_stop {
            return Err(stop.clone());
        }
        let verb = parts.first().copied().unwrap_or("");
        let at = |i: usize| -> Result<&str, VmError> {
            parts
                .get(i)
                .copied()
                .ok_or_else(|| error("invalid vector subscript"))
        };
        match verb {
            "spawn" => self.spawn(parts),
            "base" => {
                let index = self.pawn(at(1)?)?;
                let (package, pawn, level) = {
                    let s = &self.pawns[index];
                    (s.package.clone(), s.enemy.pawn, s.level)
                };
                let Some(definition) = self.export_of(&package, BASE_HEALTH_PATH)? else {
                    return Err(error(format!(
                        "combat-run: the package has no {BASE_HEALTH_PATH}"
                    )));
                };
                let vm = &mut self.world.vm;
                let data = ow_attr::api::InitData {
                    initialization_definition: Some(definition),
                    ..ow_attr::api::InitData::constant(0.0)
                };
                let attrs = vm.attrs.clone();
                let value = f64::from(attrs.evaluate_init(
                    &data,
                    Some(pawn),
                    None,
                    &mut ow_attr::api::EvalCx::new(vm),
                )?);
                self.row(&format!("base.L{level}"), value);
                Ok(format!("{{\"value\":{}}}", num(value)))
            }
            "sethealth" | "setmax" => {
                let index = self.pawn(at(1)?)?;
                let pawn = self.pawns[index].enemy.pawn;
                // N5: a step's number as the script's float (round to nearest).
                let v = crate::hit::access::to_f32(stod(at(2)?)?);
                let name = if verb == "sethealth" {
                    "SetHealth"
                } else {
                    "SetMaxHealth"
                };
                self.call(pawn, name, vec![Value::Float(v)])?;
                Ok("{}".to_string())
            }
            "pool" => {
                let index = self.pawn(at(1)?)?;
                let (package, pawn, mind) = {
                    let s = &self.pawns[index];
                    (s.package.clone(), s.enemy.pawn, s.enemy.mind)
                };
                let path = at(2)?;
                let Some(definition) = self.export_of(&package, path)? else {
                    return Err(error(format!("combat-run: no such pool definition {path}")));
                };
                let vm = &mut self.world.vm;
                let manager_class = vm.find_class("Engine.ResourcePoolManager")?;
                let Some(create) = vm.find_method(manager_class, "CreatePool", None)? else {
                    return Err(error("combat-run: no CreatePool"));
                };
                let manager = get(vm, mind, "ResourcePoolManager")?.unwrap_or(Value::None);
                let made = vm.call(
                    create,
                    None,
                    vec![
                        manager,
                        Value::Object(Some(mind)),
                        Value::Object(Some(definition)),
                        Value::Bool(false),
                    ],
                )?;
                let written = made.outs.first().cloned().unwrap_or(Value::None);
                set(vm, mind, "ResourcePoolManager", written)?;
                let reference = made.ret;
                self.pawns[index].pools.push(reference.clone());
                let valid = self
                    .call(pawn, "IsResourcePoolValid", vec![reference.clone()])?
                    .ret;
                let vm = &self.world.vm;
                let slot = field(vm, &reference, "PoolIndexInManager").map_or(0, |v| integer(&v));
                let guid = field(vm, &reference, "PoolGUID").map_or(0, |v| integer(&v));
                Ok(format!(
                    "{{\"valid\":{},\"slot\":{slot},\"guid\":{guid}}}",
                    flag(truth(vm, &valid))
                ))
            }
            "copyref" => {
                let index = self.pawn(at(1)?)?;
                let k = stoul(at(2)?)?;
                let copy = self.pawns[index]
                    .pools
                    .get(k)
                    .cloned()
                    .ok_or_else(|| error("invalid vector subscript"))?;
                self.pawns[index].pools.push(copy);
                Ok("{}".to_string())
            }
            "valid" => {
                let index = self.pawn(at(1)?)?;
                let k = stoul(at(2)?)?;
                let reference = self.pawns[index]
                    .pools
                    .get(k)
                    .cloned()
                    .ok_or_else(|| error("invalid vector subscript"))?;
                let pawn = self.pawns[index].enemy.pawn;
                let valid = self.call(pawn, "IsResourcePoolValid", vec![reference])?.ret;
                Ok(format!(
                    "{{\"valid\":{}}}",
                    flag(truth(&self.world.vm, &valid))
                ))
            }
            "unpool" | "destroyhealth" => {
                let index = self.pawn(at(1)?)?;
                let pawn = self.pawns[index].enemy.pawn;
                let k = if verb == "unpool" {
                    Some(stoul(at(2)?)?)
                } else {
                    None
                };
                let held = match k {
                    Some(k) => self.pawns[index]
                        .pools
                        .get(k)
                        .cloned()
                        .ok_or_else(|| error("invalid vector subscript"))?,
                    None => get(&mut self.world.vm, pawn, "HealthPool")?.unwrap_or(Value::None),
                };
                let manager = field(&self.world.vm, &held, "PoolManager")
                    .as_ref()
                    .and_then(object_of);
                let Some(manager) = manager else {
                    return Err(error("combat-run: the reference has no manager"));
                };
                let vm = &mut self.world.vm;
                let class = vm.space.heap.get(manager)?.class;
                let Some(destroy) = vm.find_method(class, "DeletePoolByReference", None)? else {
                    return Err(error("combat-run: no DeletePoolByReference"));
                };
                let out = vm.call(destroy, Some(manager), vec![held])?;
                let written = out.outs.first().cloned().unwrap_or(Value::None);
                match k {
                    Some(k) => self.pawns[index].pools[k] = written,
                    None => {
                        set(&mut self.world.vm, pawn, "HealthPool", written)?;
                    }
                }
                Ok("{}".to_string())
            }
            "player" => {
                let level = stoi(at(1)?)?;
                let player = hit::spawn_player(&mut self.world.vm, level)?;
                self.players.push(player);
                if self.players.len() == 1 {
                    self.one_local_player()?;
                }
                Ok(format!("{{\"player\":{}}}", self.players.len() - 1))
            }
            "maya" => self.maya(parts),
            "tick" => self.tick(parts),
            "players" => {
                let n = stoi(at(1)?)?;
                let vm = &mut self.world.vm;
                let world = with_state(vm, |s| s.world)?;
                let game = match world {
                    Some(w) => object(vm, w, "Game")?,
                    None => None,
                };
                let written = match game {
                    Some(g) => set(vm, g, "EffectiveNumPlayers", Value::Int(n))?,
                    None => false,
                };
                if !written {
                    return Err(error(
                        "combat-run: the world has no game with EffectiveNumPlayers",
                    ));
                }
                Ok("{}".to_string())
            }
            "killtag" => {
                self.kill_tag = parts.get(1).copied().unwrap_or("").to_string();
                Ok("{}".to_string())
            }
            "hold" | "release" => {
                let index = self.pawn(at(1)?)?;
                let mind = self.pawns[index].enemy.mind;
                let reason = at(2)?;
                let vm = &mut self.world.vm;
                let Some(component) = object(vm, mind, "AIComponent")? else {
                    return Err(error("combat-run: the pawn's mind has no AI component"));
                };
                let name = vm.space.names.intern(reason);
                let function = if verb == "hold" {
                    "HoldDemigod"
                } else {
                    "ReleaseDemigod"
                };
                // The tag: None in Reference, as the C++ passes. In Off the note's rule refuses a None tag, so the step
                // passes the pawn, the object the hold behavior passes as its context (a run set-up, not a rule:
                // decision 2026-11-16-83).
                let tag = match vm.parity {
                    ParityMode::Off => Some(self.pawns[index].enemy.pawn),
                    _ => None,
                };
                self.call(component, function, vec![Value::Name(name), Value::Object(tag)])?;
                Ok("{}".to_string())
            }
            "attr" => {
                let which = stoul(at(1)?)?;
                if self.players.len() <= which {
                    return Err(error(format!("combat-run: no such player {}", at(1)?)));
                }
                let controller = self.players[which].controller;
                let name = at(2)?;
                // N5: a step's number as the attribute's float.
                let v = crate::hit::access::to_f32(stod(at(3)?)?);
                if !set(&mut self.world.vm, controller, name, Value::Float(v))? {
                    return Err(error(format!(
                        "combat-run: the controller has no attribute {name}"
                    )));
                }
                Ok("{}".to_string())
            }
            "sequence" => {
                let index = self.pawn(at(1)?)?;
                let on = at(3)? == "on";
                if !on && at(3)? != "off" {
                    return Err(error("combat-run: sequence:<pawn>:<name>:on|off"));
                }
                let pawn = self.pawns[index].enemy.pawn;
                let changed = set_sequence(&mut self.world.vm, pawn, at(2)?, on)?;
                Ok(format!("{{\"changed\":{}}}", flag(changed)))
            }
            "cash" => self.cash(parts),
            "bleed" => self.bleed_step(parts),
            "mayahealth" => self.maya_health(parts),
            "swing" => self.swing(parts),
            "hit" => self.hit(parts),
            "weapon" => self.weapon_step(parts),
            "fire" => self.fire(parts),
            "seed" => {
                // seed:<n>: the loot's generator restarts from n (`crate::loot::seed`).
                let n = u32::try_from(stoul(at(1)?)?)
                    .map_err(|_| error("stoul argument out of range"))?;
                crate::loot::seed(&mut self.world.vm, n);
                Ok("{}".to_string())
            }
            "rollpool" => self.roll_pool_steps(parts),
            "lootbatch" => self.loot_batch(parts),
            _ => Err(error(format!("combat-run: unknown step {step}"))),
        }
    }

    /// Scenario setup for a run with one local player (review r1 of PR #348, decision 2026-11-16-49): when the run spawns
    /// its first player (a `player:` step, or since decision 2026-11-16-54 a `maya:` step: the bridge's player), the
    /// game's `EffectiveNumPlayers` becomes 1 if it still holds its class default 0. That is the
    /// count the C++ evaluator stands in for every `NumberOfPlayers` condition operand (`src/progression.cpp:298`,
    /// `AttributeContext::numberOfPlayers`), now that the port reads that attribute's chain from the game info. That
    /// the game's login sets the count to 1 for one local player is UNVERIFIED. A later `players:<n>` step, or a count
    /// already set, wins; a run whose game has no such property (a package set without `WillowGameInfo`) is unchanged.
    fn one_local_player(&mut self) -> Result<(), VmError> {
        let vm = &mut self.world.vm;
        let Some(world) = with_state(vm, |s| s.world)? else {
            return Ok(());
        };
        let Some(game) = object(vm, world, "Game")? else {
            return Ok(());
        };
        if matches!(get(vm, game, "EffectiveNumPlayers")?, Some(Value::Int(0))) {
            set(vm, game, "EffectiveNumPlayers", Value::Int(1))?;
        }
        Ok(())
    }

    fn spawn(&mut self, parts: &[&str]) -> Result<String, VmError> {
        if parts.len() < 3 || parts.len() > 5 {
            return Err(error(
                "combat-run: spawn:<kind>:<level>[:<stage>[:<balance path>]]",
            ));
        }
        let mut package = String::new();
        let mut path = String::new();
        let mut balance = String::new();
        if let Some((p, a)) = parts[1].split_once('/') {
            package = p.to_string();
            path = a.to_string();
        }
        for (kind, p, a, b) in KINDS {
            if parts[1] == kind {
                package = p.to_string();
                path = a.to_string();
                balance = b.to_string();
            }
        }
        if path.is_empty() {
            return Err(error(format!("combat-run: unknown pawn kind {}", parts[1])));
        }
        let level = stoi(parts[2])?;
        let stage = if parts.len() >= 4 {
            stoi(parts[3])?
        } else {
            level
        };
        if parts.len() == 5 {
            balance = parts[4].to_string();
        }
        let leaf = path.rsplit_once('.').map_or(path.as_str(), |(_, l)| l);
        let row_class = leaf.strip_prefix("Pawn_").unwrap_or(leaf).to_string();
        let started = Instant::now();
        let enemy = hit::spawn_enemy(&mut self.world.vm, &package, &path, level, stage)?;
        // The stock spawn's loot setup (plan step S-g): the item game stage and the pawn's pool list.
        let mut loot_setup = String::new();
        if !balance.is_empty() {
            let Some(b) = hit::instantiate_copy(&mut self.world.vm, &package, &balance)? else {
                return Err(error(format!("combat-run: {package} has no {balance}")));
            };
            let setup = crate::loot::setup_loot(&mut self.world.vm, enemy.pawn, b)?;
            let pools: Vec<String> = setup.pools.iter().map(|p| json_quote(p)).collect();
            loot_setup = format!(
                ",\"loot_setup\":{{\"item_stage\":{},\"pools\":[{}]}}",
                setup.item_stage,
                pools.join(",")
            );
        }
        let milliseconds = started.elapsed().as_secs_f64() * 1000.0;
        let id = format!("{row_class}.L{level}");
        let health = number(&self.call(enemy.pawn, "GetHealth", Vec::new())?.ret);
        let max = number(&self.call(enemy.pawn, "GetMaxHealth", Vec::new())?.ret);
        self.row(&format!("health.{id}"), health);
        self.row(&format!("max_health.{id}"), max);
        self.pawns.push(Spawned {
            package,
            path,
            row_class,
            level,
            stage,
            milliseconds,
            enemy,
            pools: Vec::new(),
        });
        Ok(format!(
            "{{\"pawn\":{},\"milliseconds\":{}{loot_setup}}}",
            self.pawns.len() - 1,
            num(milliseconds)
        ))
    }

    fn swing(&mut self, parts: &[&str]) -> Result<String, VmError> {
        if parts.len() < 3 || parts.len() > 4 {
            return Err(error(
                "combat-run: swing:<attacker>:<kind or definition path>[:<cause damage path>]",
            ));
        }
        if self.bridge.is_none() && self.players.is_empty() {
            return Err(error(
                "combat-run: a swing needs a player to hit (the step maya:<level>, Maya's VM pawn, or player:<level>)",
            ));
        }
        let index = self.pawn(parts[1])?;
        let mut definition = parts[2].to_string();
        let mut cause = parts.get(3).map(|s| s.to_string()).unwrap_or_default();
        for (kind, d, c) in SWINGS {
            if definition == kind {
                definition = d.to_string();
                cause = c.to_string();
            }
        }
        let (package, mind, row_class, level) = {
            let s = &self.pawns[index];
            (
                s.package.clone(),
                s.enemy.mind,
                s.row_class.clone(),
                s.level,
            )
        };
        // The C++ instantiates both from the attacker's package with the 4-byte prefix (`hit::instantiate_copy`).
        let Some(definition_object) =
            hit::instantiate_copy(&mut self.world.vm, &package, &definition)?
        else {
            return Err(error(format!("combat-run: {package} has no {definition}")));
        };
        let cause_object = if cause.is_empty() {
            None
        } else {
            match hit::instantiate_copy(&mut self.world.vm, &package, &cause)? {
                Some(o) => Some(o),
                None => return Err(error(format!("combat-run: {package} has no {cause}"))),
            }
        };
        let delivery = hit::delivery_of(&mut self.world.vm, definition_object, cause_object)?;
        let swing = match self.bridge.clone() {
            // The bridge's player pawn is the victim, with the controller's Pawn set to it (C++ `swingAtPlayer`).
            // An error is the bridge guard's: recorded, and the step goes on with an empty swing (arrival 0), as
            // the C++ (review r1 of PR #356).
            Some(b) => {
                let victim = self.bridge_pawn(&*b)?;
                let controller = b.player_controller();
                match player::with_player_pawn(&mut self.world.vm, controller, victim, |vm, victim| {
                    hit::swing(vm, mind, victim, &delivery)
                }) {
                    Ok(s) => s,
                    // A NotYet is not a script error: it stops the step (review r2 of PR #356).
                    Err(VmError::NotYet(n)) => return Err(VmError::NotYet(n)),
                    Err(e) => {
                        self.script_error(&e);
                        hit::Swing::default()
                    }
                }
            }
            None => {
                let victim = self.players[0].pawn;
                hit::swing(&mut self.world.vm, mind, victim, &delivery)?
            }
        };
        let id = format!("{row_class}.L{level}");
        self.row(&format!("arrival.{id}"), swing.arrival);
        let ratio = if swing.arrival > 0.0 {
            swing.hit.applied / swing.arrival
        } else {
            0.0
        };
        if swing.arrival > 0.0 {
            self.row(&format!("ratio.{id}"), ratio);
        }
        let player = match self.bridge {
            Some(_) => self.player_json()?,
            None => "null".to_string(),
        };
        let mut out = format!(
            "{{\"definition\":{},\"source\":{},\"damage_type\":{},\"arrival\":{},\"applied\":{},\"ratio\":{},\"shield_damage\":{},\"health_lost\":{},\"hit\":{},\"player\":{player}",
            json_quote(&delivery.what),
            json_quote(&swing.source),
            json_quote(&swing.damage_type),
            num(swing.arrival),
            num(swing.hit.applied),
            num(ratio),
            num(swing.hit.shield_damage),
            num(swing.hit.health_before - swing.hit.health_after),
            hit_json(&swing.hit)
        );
        if self.bridge.is_some() {
            self.track_down()?;
            if self.episode.active {
                let _ = write!(out, ",\"down\":{}", self.down_json()?);
            }
        }
        out.push('}');
        Ok(out)
    }

    fn hit(&mut self, parts: &[&str]) -> Result<String, VmError> {
        if parts.len() < 4 || parts.len() > 5 {
            return Err(error(
                "combat-run: hit:<pawn>:<damage>:<damage type path>[:<source>]",
            ));
        }
        if self.players.is_empty() && self.bridge.is_none() {
            return Err(error(
                "combat-run: a hit needs a player (the step player:<level> or maya:<level>) first",
            ));
        }
        let index = self.pawn(parts[1])?;
        let mut source = parts.get(4).copied().unwrap_or("Bullet").to_string();
        if !source.contains('.') {
            let prefix = if source.starts_with("WillowDmgSource_") {
                ""
            } else {
                "WillowDmgSource_"
            };
            source = format!("WillowGame.{prefix}{source}");
        }
        let damage = stod(parts[2])?;
        let source_class = self.world.vm.find_class(&source)?;
        self.player_hit(
            index,
            damage,
            DamageTypeOf::Path(parts[3]),
            source_class,
            hit::ShotParts::default(),
            "hit",
        )
    }

    /// A hit from the run's player on pawn `index` (the `hit` and `fire` steps): the chain, then the rows
    /// `<row>.<class>.L<level>.<n>.*`, the kills, the loot and the down state, as the C++ `hit` step.
    fn player_hit(
        &mut self,
        index: usize,
        damage: f64,
        damage_type: DamageTypeOf<'_>,
        source_class: ow_object::ids::ClassId,
        shot: hit::ShotParts,
        row: &str,
    ) -> Result<String, VmError> {
        let (pawn, row_class, level) = {
            let s = &self.pawns[index];
            (s.enemy.pawn, s.row_class.clone(), s.level)
        };
        let bridge = self.bridge.clone();
        let vm = &mut self.world.vm;
        let (pool_before, level_before) = match &bridge {
            Some(b) => (b.experience_pool(), b.player_level(vm)?),
            None => (0.0, 0),
        };
        let report = match &bridge {
            // The bridge's controller is the instigator, its Pawn the player's while the chain runs (C++
            // `hitFromPlayer`). An error is the bridge guard's: recorded, and the step goes on with an empty report
            // (its rows 0, the hit counted, the kills and the down state processed), as the C++ (review r1 of PR #356).
            Some(b) => {
                let controller = b.player_controller();
                let player_pawn = self.bridge_pawn(&**b)?;
                match player::with_player_pawn(&mut self.world.vm, controller, player_pawn, |vm, _| {
                    let damage_type = damage_type.resolve(vm)?;
                    hit::hit_with(vm, pawn, Some(controller), damage, damage_type, Some(source_class), shot)
                }) {
                    Ok(r) => r,
                    // A NotYet is not a script error: it stops the step (review r2 of PR #356).
                    Err(VmError::NotYet(n)) => return Err(VmError::NotYet(n)),
                    Err(e) => {
                        self.script_error(&e);
                        HitReport::default()
                    }
                }
            }
            None => {
                let instigator = self.players[0].controller;
                let damage_type = damage_type.resolve(vm)?;
                hit::hit_with(vm, pawn, Some(instigator), damage, damage_type, Some(source_class), shot)?
            }
        };
        let vm = &mut self.world.vm;
        // The kills this hit made (the experience natives record them): one row per kill, the credit being what the
        // bridge player's pool gained.
        let kills = with_state(vm, |s| s.kills.clone())?;
        for kill in kills.iter().skip(self.kills_seen) {
            if kill.killed != Some(pawn) {
                continue;
            }
            let player_level = kill.credits.first().map_or(level_before, |c| c.player_level);
            let mut id = format!("kill.{row_class}.L{level}.p{player_level}");
            if !self.kill_tag.is_empty() {
                let _ = write!(id, ".{}", self.kill_tag);
            }
            let credit = match &bridge {
                Some(b) if !kill.credits.is_empty() => {
                    Some(player::credit(pool_before, b.experience_pool())?)
                }
                _ => None,
            };
            self.kill_rows.push(KillRow {
                id,
                award: f64::from(kill.award),
                credit,
            });
        }
        self.kills_seen = kills.len();
        // A shot counts apart from the scripted hits, so a `hit` row keeps its number whatever `fire` steps run.
        let key = if row == "hit" {
            format!("{row_class}{level}")
        } else {
            format!("{row}:{row_class}{level}")
        };
        let count = self.hit_counts.entry(key).or_insert(0);
        *count += 1;
        let id = format!("{row}.{row_class}.L{level}.{count}");
        self.row(&format!("{id}.arriving"), report.arriving);
        self.row(&format!("{id}.applied"), report.applied);
        self.row(&format!("{id}.health_after"), report.health_after);
        let mut result = hit_json(&report);
        let key = id.strip_prefix("hit.").unwrap_or(&id).to_string();
        if let Some(loot) = self.loot_after(pawn, &key)? {
            result.pop();
            let _ = write!(result, ",\"loot\":{loot}}}");
        }
        if self.bridge.is_some() && (self.episode.active || self.injuries > 0) {
            self.track_down()?;
            result.pop();
            let _ = write!(result, ",\"down\":{}}}", self.down_json()?);
        }
        Ok(result)
    }

    /// The death roll(s) a hit on `pawn` made since the last report (C++ `lootAfter`): rows `loot.<key>.*` and the
    /// last roll as JSON.
    fn loot_after(&mut self, pawn: ObjRef, key: &str) -> Result<Option<String>, VmError> {
        let drops = crate::loot::drops_from(&self.world.vm, self.drops_seen)?;
        self.drops_seen += drops.len();
        let mut text = None;
        for drop in drops.iter().filter(|d| d.pawn == pawn) {
            for (k, v) in crate::loot::loot_rows(drop, key) {
                self.row(&k, v);
            }
            text = Some(crate::loot::loot_json(drop));
        }
        Ok(text)
    }

    /// `rollpool:<pawn>:<pool path>[:<stage>[:<count>]]` (C++ `rollPoolSteps`): `ItemPool.SpawnBalancedInventoryFromPool`
    /// called through the VM (the optional arguments omitted) with the pawn as the context source, at the given stage
    /// (default the pawn's item game stage), `count` times; rows `loot.roll.<pool>.stage<s>.n` / `.true` / `.items`
    /// and the last call's inventories. The pool is looked up in Startup, then in the pawn's package.
    fn roll_pool_steps(&mut self, parts: &[&str]) -> Result<String, VmError> {
        if parts.len() < 3 || parts.len() > 5 {
            return Err(error(
                "combat-run: rollpool:<pawn>:<pool path>[:<stage>[:<count>]]",
            ));
        }
        let index = self.pawn(parts[1])?;
        let (package, pawn) = {
            let s = &self.pawns[index];
            (s.package.clone(), s.enemy.pawn)
        };
        let vm = &mut self.world.vm;
        let mut pool = None;
        for name in ["Startup", package.as_str()] {
            if let Some(o) = hit::instantiate_copy(vm, name, parts[2])? {
                pool = Some(o);
                break;
            }
        }
        let Some(pool) = pool else {
            return Err(error(format!("combat-run: no such item pool {}", parts[2])));
        };
        let stage = match parts.get(3) {
            Some(s) => stoi(s)?,
            None => crate::hit::access::low_i32(integer(&crate::hit::access::required(
                vm,
                pawn,
                "GameStageForSpawnedInventory",
                "combat-run",
            )?)),
        };
        let count = match parts.get(4) {
            Some(c) => stoi(c)?,
            None => 1,
        };
        let function = vm.find_function(crate::loot::FROM_POOL)?;
        let mut returned = 0i32;
        let mut made = 0usize;
        let mut last = Vec::new();
        for _ in 0..count.max(0) {
            let args = vec![
                Value::Object(Some(pool)),
                Value::Int(stage),
                Value::Int(0),
                Value::Object(Some(pawn)),
                Value::array(Vec::new()),
            ];
            let r = vm.call(function, None, args)?;
            returned += i32::from(truth(vm, &r.ret));
            last = r
                .outs
                .get(4)
                .ok_or_else(|| error("invalid vector subscript"))?
                .elements()
                .map(<[Value]>::to_vec)
                .unwrap_or_default();
            made += last.len();
        }
        let inventories = crate::loot::inventories_json(vm, &last)?;
        let name = parts[2].rsplit_once('.').map_or(parts[2], |(_, l)| l);
        let key = format!("loot.roll.{name}.stage{stage}");
        self.row(&format!("{key}.n"), f64::from(count));
        self.row(&format!("{key}.true"), f64::from(returned));
        // A count of inventories converts to double exactly.
        self.row(&format!("{key}.items"), made as f64);
        Ok(format!(
            "{{\"stage\":{stage},\"calls\":{count},\"returned_true\":{returned},\"inventories\":{inventories}}}"
        ))
    }

    /// `lootbatch:<kind>:<level>:<stage>:<count>[:<tag>]` (C++ `lootBatch`): `count` kills of a fresh pawn each
    /// through the spawn and lethal hit steps (a shotgun hit of 9999 by player 0), the death rolls summed; nothing of
    /// the kills is kept but the sums (`crate::loot::Batch::rows`). `Off` mode is `NotYet` before any kill: the death
    /// roll is the installed script's there, which records nothing to sum.
    fn loot_batch(&mut self, parts: &[&str]) -> Result<String, VmError> {
        if parts.len() < 5 || parts.len() > 6 {
            return Err(error(
                "combat-run: lootbatch:<kind>:<level>:<stage>:<count>[:<tag>]",
            ));
        }
        if self.players.is_empty() && self.bridge.is_none() {
            return Err(error(
                "combat-run: a loot batch needs a player (the step maya:<level> or player:<level>) first",
            ));
        }
        let count = stoi(parts[4])?;
        if self.world.vm.parity == ow_core::ParityMode::Off {
            return Err(NotYet::not_ported(
                "lootbatch in Off mode: the death roll is the installed DropLootOnDeath script, whose per-pool \
                 chances and draws are not recorded (crate::loot, decision 2026-11-16-41)",
                RECORD,
            )
            .into());
        }
        let tag = parts.get(5).copied().unwrap_or(parts[1]);
        let saved_rows = self.rows.clone();
        let saved_hits = self.hit_counts.clone();
        let saved_kill_rows = self.kill_rows.len();
        let saved_results = self.results.len();
        let mut batch = crate::loot::Batch::new();
        for _ in 0..count.max(0) {
            let errors_before = self.errors.len();
            self.apply(&format!("spawn:{}:{}:{}", parts[1], parts[2], parts[3]));
            if self.errors.len() != errors_before {
                let last = self.errors.last().cloned().unwrap_or_default();
                return Err(error(format!("combat-run: lootbatch: {last}")));
            }
            let index = self.pawns.len() - 1;
            let before = crate::loot::drop_count(&self.world.vm)?;
            self.apply(&format!("hit:{index}:9999:{KILL_DAMAGE_TYPE}:Shotgun"));
            let pawn = self.pawns[index].enemy.pawn;
            for drop in crate::loot::drops_from(&self.world.vm, before)?
                .iter()
                .filter(|d| d.pawn == pawn)
            {
                batch.add(drop)?;
            }
            self.pawns.pop();
            self.results.truncate(saved_results);
            self.kill_rows.truncate(saved_kill_rows);
        }
        self.rows = saved_rows;
        self.hit_counts = saved_hits;
        crate::loot::clear_drops(&self.world.vm)?;
        self.drops_seen = 0;
        for (k, v) in batch.rows(tag) {
            self.row(&k, v);
        }
        Ok(format!("{{\"kills\":{}}}", batch.kills()))
    }

    // ---- the mission bridge's player (`run/player.rs`) ----------------------------------------------------------

    /// A bridged chain's error, as the bridge's guard records it (`mission script: <text>`); the step goes on.
    fn script_error(&mut self, e: &VmError) {
        self.script_errors.push(format!("mission script: {}", error_text(e)));
    }

    /// The bridge's player pawn, its health reference linked to the stand-in pool when there is one, as the C++
    /// bridge relinks on every fetch (`pawnFor`; review r1 of PR #356).
    fn bridge_pawn(&mut self, b: &dyn PlayerBridge) -> Result<ObjRef, VmError> {
        let pawn = b.player_pawn(&mut self.world.vm)?;
        self.health.relink(&mut self.world.vm, pawn)?;
        Ok(pawn)
    }

    fn bridge_or(&self, refusal: &str) -> Result<Rc<dyn PlayerBridge>, VmError> {
        self.bridge.clone().ok_or_else(|| error(refusal))
    }

    /// `maya:<level>[:<experience> | :short<n>]`: the bridge's player at that level with that experience (default 0;
    /// `short<n>` is n points short of the next level), its health pool full for the level (the Reference stand-in,
    /// `player::HealthPool`). May be given again between kills (the level is an input).
    fn maya(&mut self, parts: &[&str]) -> Result<String, VmError> {
        let b = self.bridge_or(
            "combat-run: maya needs the mission bridge (it is built when a maya step is among the steps)",
        )?;
        let level = stoi(parts.get(1).copied().ok_or_else(|| error("invalid vector subscript"))?)?;
        // The bridge's player is the run's one local player: the same scenario setup as the first `player:` step
        // (decision 2026-11-16-49, extended to `maya` by decision 2026-11-16-54), made before the pool's maximum is
        // evaluated, as the C++ context always counts 1 (review r1 of PR #356).
        self.one_local_player()?;
        let vm = &mut self.world.vm;
        // Off: the controller's class through the game's script route first (decision 2026-11-19-41).
        if vm.parity == ParityMode::Off
            && let Err(e) = b.select_player_class(vm)
        {
            // Sticky: the switch may have written part of the player before it stopped.
            self.class_switch_stop = Some(e.clone());
            return Err(e);
        }
        b.set_player_experience(vm, level, 0)?;
        self.health.ensure_and_seed(vm, &*b, -1.0, &mut self.script_errors)?;
        if let Some(amount) = parts.get(2) {
            let experience = match amount.strip_prefix("short") {
                Some(n) => player::next_level_at(vm, b.player_controller())?
                    .checked_sub(stoll(n)?)
                    .ok_or_else(|| error("stoll argument out of range"))?,
                None => stoll(amount)?,
            };
            b.set_player_experience(vm, level, experience)?;
            self.health.ensure_and_seed(vm, &*b, -1.0, &mut self.script_errors)?;
        }
        let pawn = self.bridge_pawn(&*b)?;
        crate::down::prepare_player_pawn(&mut self.world.vm, pawn)?;
        // The C++ also loads two references of the globals definition here (the death camera's end time): the
        // definition store loads a definition's references when they are read, so nothing is needed.
        Ok(format!("{{\"player\":{}}}", self.player_json()?))
    }

    /// `tick:<seconds>`: game time passes and the experience pool's per-frame update runs; the level-up's gains.
    fn tick(&mut self, parts: &[&str]) -> Result<String, VmError> {
        let b = self.bridge_or("combat-run: tick needs the mission bridge")?;
        let seconds = stod(parts.get(1).copied().ok_or_else(|| error("invalid vector subscript"))?)?;
        self.advance_time(seconds)?;
        b.update_experience_pool(&mut self.world.vm)?;
        let gains: Vec<String> = b
            .take_gains()
            .iter()
            .map(|g| {
                format!(
                    "{{\"kind\":\"{}\",\"amount\":{},\"health\":{},\"maximum\":{}}}",
                    g.kind,
                    g.amount,
                    num(f64::from(g.health)),
                    num(f64::from(g.maximum))
                )
            })
            .collect();
        Ok(format!(
            "{{\"gains\":[{}],\"player\":{}}}",
            gains.join(","),
            self.player_json()?
        ))
    }

    /// `cash:<amount>`: the player's credits on hand (a state input: the respawn fee is a share of it).
    fn cash(&mut self, parts: &[&str]) -> Result<String, VmError> {
        let b = self.bridge_or("combat-run: cash needs the mission bridge (the step maya:<level>)")?;
        let amount = stoll(parts.get(1).copied().ok_or_else(|| error("invalid vector subscript"))?)?;
        let vm = &mut self.world.vm;
        let info = player::replication_info(vm, b.player_controller())?;
        crate::down::set_currency_on_hand(vm, info, 0, amount)?;
        Ok("{}".to_string())
    }

    /// `mayahealth:<share>`: the player's health set to that share of the maximum (the Reference stand-in pool).
    fn maya_health(&mut self, parts: &[&str]) -> Result<String, VmError> {
        let b = self.bridge_or("combat-run: mayahealth needs the mission bridge (the step maya:<level>)")?;
        let share = stod(parts.get(1).copied().ok_or_else(|| error("invalid vector subscript"))?)?;
        let pawn = self.bridge_pawn(&*b)?;
        let maximum = number(&self.call(pawn, "GetMaxHealth", Vec::new())?.ret);
        let current = crate::hit::access::to_f32(share * maximum);
        self.health
            .ensure_and_seed(&mut self.world.vm, &*b, current, &mut self.script_errors)?;
        Ok("{}".to_string())
    }

    /// `bleed:<seconds>[:<dt>]` (plan step S-f): game time passes in frames of dt seconds (default 1/60) and, while
    /// the player's pawn is injured, its installed `Tick` runs each frame; the callbacks the game's animations, death
    /// camera and station time are played at the live delays. Rows `down.*` per injury.
    fn bleed_step(&mut self, parts: &[&str]) -> Result<String, VmError> {
        let b = self.bridge_or("combat-run: bleed needs the mission bridge (the step maya:<level>)")?;
        if parts.len() < 2 || parts.len() > 3 {
            return Err(error("combat-run: bleed:<seconds>[:<dt>]"));
        }
        let log_before = self.world.vm.log.len();
        let seconds = stod(parts[1])?;
        let dt = match parts.get(2) {
            Some(d) => stod(d)?,
            None => 1.0 / 60.0,
        };
        self.bleed(&*b, seconds, dt)?;
        let now = self.world_now()?;
        let down = self.down_json()?;
        let player = self.player_json()?;
        let log: Vec<String> = self.world.vm.log[log_before..]
            .iter()
            .filter(|l| !l.starts_with(hit::STUB_PREFIX))
            .map(|l| json_quote(l))
            .collect();
        Ok(format!(
            "{{\"time\":{},\"down\":{down},\"player\":{player},\"log\":[{}]}}",
            num(now),
            log.join(",")
        ))
    }

    /// The world's `TimeSeconds` (0 without one).
    fn world_now(&mut self) -> Result<f64, VmError> {
        let vm = &mut self.world.vm;
        let Some(world) = with_state(vm, |s| s.world)? else {
            return Ok(0.0);
        };
        player::number_of(vm, world, "TimeSeconds")
    }

    /// Game time passes (C++ `MissionScript::advanceTime`: the sum in double, stored as the float).
    fn advance_time(&mut self, seconds: f64) -> Result<(), VmError> {
        let vm = &mut self.world.vm;
        let Some(world) = with_state(vm, |s| s.world)? else {
            return Ok(());
        };
        if let Some(t) = get(vm, world, "TimeSeconds")? {
            let sum = crate::hit::access::to_f32(number(&t) + seconds);
            set(vm, world, "TimeSeconds", Value::Float(sum))?;
        }
        Ok(())
    }

    /// The bridge's player as JSON (C++ `playerJson`).
    fn player_json(&mut self) -> Result<String, VmError> {
        let b = self.bridge_or("combat-run: no mission bridge")?;
        let pawn = self.bridge_pawn(&*b)?;
        let vm = &mut self.world.vm;
        let level = b.player_level(vm)?;
        let next = player::next_level_at(vm, b.player_controller())?;
        let health = number(&self.call(pawn, "GetHealth", Vec::new())?.ret);
        let max = number(&self.call(pawn, "GetMaxHealth", Vec::new())?.ret);
        Ok(format!(
            "{{\"level\":{level},\"experience\":{},\"next_level_at\":{next},\"health\":{},\"max_health\":{}}}",
            num(f64::from(b.experience_pool())),
            num(health),
            num(max)
        ))
    }

    /// The down state as JSON (C++ `downJson`).
    fn down_json(&mut self) -> Result<String, VmError> {
        let b = self.bridge_or("combat-run: no mission bridge")?;
        let now = self.world_now()?;
        let pawn = self.bridge_pawn(&*b)?;
        player::down_json(&mut self.world.vm, pawn, &self.episode, now)
    }

    /// Starts the record of an injury when the player's pawn has just entered its injured state (C++ `trackDown`).
    fn track_down(&mut self) -> Result<(), VmError> {
        let b = self.bridge_or("combat-run: no mission bridge")?;
        let pawn = self.bridge_pawn(&*b)?;
        if self.episode.active || !player::is_injured_state(&self.world.vm, pawn)? {
            return Ok(());
        }
        let now = self.world_now()?;
        self.injuries = self
            .injuries
            .checked_add(1)
            .ok_or_else(|| error("combat-run: too many injuries"))?;
        let vm = &mut self.world.vm;
        let mut e = Episode::new();
        e.active = true;
        e.number = self.injuries;
        e.down_at = now;
        e.total = player::number_of(vm, pawn, "TotalBleedoutTime")?;
        e.successive = player::number_of(vm, pawn, "NumSuccessiveInjuries")?;
        let (n, total, successive) = (e.number, e.total, e.successive);
        self.episode = e;
        self.row(&format!("down.total.{n}"), total);
        self.row(&format!("down.successive.{n}"), successive);
        Ok(())
    }

    /// Game time in frames (C++ `bleed`): time moves on, then, in the injured state, the callbacks that are due and
    /// the script `Tick`. A frame's script error is recorded (the bridge's guard) and the next frame runs.
    fn bleed(&mut self, b: &dyn PlayerBridge, seconds: f64, dt: f64) -> Result<(), VmError> {
        if dt <= 0.0 || dt.is_nan() {
            return Err(error("combat-run: bleed: the frame time must be positive"));
        }
        let frames = player::frames(seconds, dt)?;
        for _ in 0..frames.max(0) {
            self.advance_time(dt)?;
            let now = self.world_now()?;
            self.track_down()?;
            let n = self.episode.number.to_string();
            let controller = b.player_controller();
            let pawn = self.bridge_pawn(b)?;
            let saved = get(&mut self.world.vm, controller, "Pawn")?;
            if saved.is_some() {
                set(&mut self.world.vm, controller, "Pawn", Value::Object(Some(pawn)))?;
            }
            let frame = self.bleed_frame(b, pawn, now, dt, &n);
            if let Some(v) = saved {
                set(&mut self.world.vm, controller, "Pawn", v)?;
            }
            match frame {
                Ok(()) => {}
                // As in the hit and the swing, a NotYet stops the step; only a script error is the guard's.
                Err(VmError::NotYet(n)) => return Err(VmError::NotYet(n)),
                Err(e) => self.script_errors.push(format!("mission script: {}", error_text(&e))),
            }
        }
        Ok(())
    }

    /// One frame's body of [`Run::bleed`], with the controller's `Pawn` set to the player's.
    fn bleed_frame(&mut self, b: &dyn PlayerBridge, pawn: ObjRef, now: f64, dt: f64, n: &str) -> Result<(), VmError> {
        let injured = |run: &Run| player::is_injured_state(&run.world.vm, pawn);
        if self.episode.active && injured(self)? {
            if !self.episode.targeted && now - self.episode.down_at >= player::SPECIAL_MOVE_END - player::EPSILON {
                player::call_state(&mut self.world.vm, pawn, "SwitchToSidearm", Vec::new())?;
                self.episode.targeted = true;
                self.episode.targeted_at = now;
            }
            // N5: the frame time as the script's float.
            let tick = crate::hit::access::to_f32(dt);
            player::call_state(&mut self.world.vm, pawn, "Tick", vec![Value::Float(tick)])?;
            let vm = &mut self.world.vm;
            if self.episode.dead_at < 0.0 && player::number_of(vm, pawn, "InjuredDeadState")? >= 1.0 {
                self.episode.dead_at = now;
                // The script's own timer at that frame: the bleed-out total plus the part of the last frame past it.
                let over = player::number_of(vm, pawn, "flInjuredTargetedTime")? - self.episode.total;
                self.row(&format!("down.bleed_over.{n}"), over);
                if self.episode.targeted_at >= 0.0 {
                    let wall = now - self.episode.targeted_at;
                    self.row(&format!("down.bleed_wall.{n}"), wall);
                }
            }
            if self.episode.dead_at >= 0.0
                && !self.episode.ragdolled
                && injured(self)?
                && now - self.episode.dead_at >= player::RAGDOLL_DELAY - player::EPSILON
            {
                player::call_state(&mut self.world.vm, pawn, "InjuredDeathAnimDone", Vec::new())?;
                self.episode.ragdolled = true;
            }
            if self.episode.dead_at >= 0.0
                && !self.episode.camera_ended
                && injured(self)?
                && player::number_of(&mut self.world.vm, pawn, "bInjuredDeadCameraActive")? != 0.0
            {
                // The camera's per-frame view request (out: location, rotation, field of view); the script ends the
                // camera (EndInjuredDeadCamera -> InjuredRespawn) when its time is up.
                let tick = crate::hit::access::to_f32(dt);
                player::call_state(&mut self.world.vm, pawn, "CalcInjuredDeadCamera", vec![Value::Float(tick)])?;
                if player::number_of(&mut self.world.vm, pawn, "bInjuredDeadCameraActive")? == 0.0 {
                    self.episode.camera_ended = true;
                    let since = now - self.episode.dead_at;
                    self.row(&format!("down.camera.{n}"), since);
                }
            }
        }
        if self.episode.active && !injured(self)? {
            // The state ended inside this frame: a second wind, or the respawn.
            self.episode.active = false;
            let health = number(&self.call(pawn, "GetHealth", Vec::new())?.ret);
            let max = number(&self.call(pawn, "GetMaxHealth", Vec::new())?.ret);
            let ratio = health / max.max(1e-9);
            let key = if self.episode.camera_ended {
                "down.respawn_health."
            } else {
                "down.second_wind_health."
            };
            self.row(&format!("{key}{n}"), ratio);
            if self.episode.camera_ended {
                self.respawn_at = now;
            }
        }
        if self.respawn_at >= 0.0 && now - self.respawn_at >= player::RESPAWN_DIALOG_DELAY - player::EPSILON {
            self.respawn_at = -1.0;
            let vm = &mut self.world.vm;
            let info = player::replication_info(vm, b.player_controller())?;
            let cash = crate::down::currency_on_hand(vm, info, 0)?;
            player::call_state(vm, pawn, "ShowRespawnDialog", Vec::new())?;
            let after = crate::down::currency_on_hand(&mut self.world.vm, info, 0)?;
            self.fee_rows.push(Fee {
                injury: n.to_string(),
                cash,
                charged: cash.checked_sub(after).ok_or_else(|| error("combat-run: the fee is out of range"))?,
            });
        }
        Ok(())
    }

    /// A pawn's state (C++ `stateJson`): the natives' answers, the plain variables, the health reference and every
    /// occupied slot of the mind's manager.
    fn state_json(&mut self, index: usize) -> Result<String, VmError> {
        let (pawn, mind) = {
            let s = &self.pawns[index];
            (s.enemy.pawn, s.enemy.mind)
        };
        let demigod = hit::demigod(&mut self.world.vm, pawn)?;
        let health = number(&self.call(pawn, "GetHealth", Vec::new())?.ret);
        let max = number(&self.call(pawn, "GetMaxHealth", Vec::new())?.ret);
        let vm = &mut self.world.vm;
        let health_var = get(vm, pawn, "HealthVar")?.map_or(0.0, |v| number(&v));
        let health_max_var = get(vm, pawn, "HealthMaxVar")?.map_or(0.0, |v| number(&v));
        let reference = get(vm, pawn, "HealthPool")?.unwrap_or(Value::None);
        let valid = self
            .call(pawn, "IsResourcePoolValid", vec![reference.clone()])?
            .ret;
        let vm = &mut self.world.vm;
        let mut out = format!(
            "\"demigod\":{},\"health\":{},\"max_health\":{},\"health_var\":{},\"health_max_var\":{},\"pool_valid\":{},\"pool_slot\":{},\"pool_guid\":{},\"slots\":[",
            flag(demigod),
            num(health),
            num(max),
            num(health_var),
            num(health_max_var),
            flag(truth(vm, &valid)),
            field(vm, &reference, "PoolIndexInManager").map_or(0, |v| integer(&v)),
            field(vm, &reference, "PoolGUID").map_or(0, |v| integer(&v)),
        );
        if let Some(manager) = object(vm, mind, "ResourcePoolManager")? {
            let mut first = true;
            for (slot, pool) in ow_world::pools::pools_of(&vm.space, manager)? {
                let definition = object(vm, pool, "Definition")?;
                let resource = match definition {
                    Some(d) => object(vm, d, "Resource")?,
                    None => None,
                };
                let provider = object(vm, pool, "AssociatedProvider")?;
                let guid = get(vm, pool, "PoolGUID")?.map_or(0, |v| integer(&v));
                let current = get(vm, pool, "CurrentValue")?.map_or(0.0, |v| number(&v));
                let max = get(vm, pool, "MaxValue")?.map_or(0.0, |v| number(&v));
                let _ = write!(
                    out,
                    "{}{{\"slot\":{slot},\"guid\":{guid},\"resource\":{},\"current\":{},\"max\":{},\"provider_is_mind\":{}}}",
                    if first { "" } else { "," },
                    json_quote(&path_of(vm, resource)),
                    num(current),
                    num(max),
                    flag(provider == Some(mind)),
                );
                first = false;
            }
        }
        out.push(']');
        Ok(out)
    }

    /// The report (C++ `Run::report`): rows (then the kill rows), pawns, steps, the VM log's stubs (counted) and
    /// notes, the errors, and the elapsed time.
    pub fn report(&mut self, elapsed_ms: f64) -> String {
        let mut out = String::from("{\"rows\":{");
        let mut first = true;
        for (id, value) in &self.rows {
            let _ = write!(
                out,
                "{}{}:{}",
                if first { "" } else { "," },
                json_quote(id),
                num(*value)
            );
            first = false;
        }
        let mut total: DetMap<&str, u32> = DetMap::new();
        for k in &self.kill_rows {
            *total.entry(k.id.as_str()).or_insert(0) += 1;
        }
        let mut seen: DetMap<&str, u8> = DetMap::new();
        for k in &self.kill_rows {
            let id = if total[k.id.as_str()] > 1 {
                let n = seen.entry(k.id.as_str()).or_insert(0);
                let letter = char::from(b'a'.saturating_add(*n));
                *n = n.saturating_add(1);
                format!("{}.{letter}", k.id)
            } else {
                k.id.clone()
            };
            let _ = write!(
                out,
                "{}{}:{{\"award\":{}",
                if first { "" } else { "," },
                json_quote(&id),
                num(k.award)
            );
            if let Some(c) = k.credit {
                let _ = write!(out, ",\"credit\":{c}");
            }
            out.push('}');
            first = false;
        }
        for fee in &self.fee_rows {
            let _ = write!(
                out,
                "{}{}:{{\"cash\":{},\"fee\":{}}}",
                if first { "" } else { "," },
                json_quote(&format!("down.fee.{}", fee.injury)),
                fee.cash,
                fee.charged
            );
            first = false;
        }
        out.push_str("},\"pawns\":[");
        for i in 0..self.pawns.len() {
            if i > 0 {
                out.push(',');
            }
            match self.pawn_json(i) {
                Ok(text) => out.push_str(&text),
                Err(e) => {
                    self.errors
                        .push(format!("report pawn {i}: {}", error_text(&e)));
                    out.push_str("null");
                }
            }
        }
        out.push_str("],\"steps\":[");
        out.push_str(&self.results.join(","));
        let mut stubs: DetMap<String, usize> = DetMap::new();
        let mut notes = Vec::new();
        for line in &self.world.vm.log {
            match line.strip_prefix(hit::STUB_PREFIX) {
                Some(name) => *stubs.entry(name.to_string()).or_insert(0) += 1,
                None => notes.push(line.clone()),
            }
        }
        out.push_str("],\"stubs\":{");
        let stub_text: Vec<String> = stubs
            .iter()
            .map(|(k, n)| format!("{}:{n}", json_quote(k)))
            .collect();
        out.push_str(&stub_text.join(","));
        out.push_str("},\"notes\":[");
        out.push_str(
            &notes
                .iter()
                .map(|n| json_quote(n))
                .collect::<Vec<_>>()
                .join(","),
        );
        out.push_str("],\"errors\":[");
        out.push_str(
            &self
                .errors
                .iter()
                .map(|e| json_quote(e))
                .collect::<Vec<_>>()
                .join(","),
        );
        let _ = write!(out, "],\"elapsed_ms\":{}}}", num(elapsed_ms));
        out
    }

    fn pawn_json(&mut self, index: usize) -> Result<String, VmError> {
        let state = self.state_json(index)?;
        let s = &self.pawns[index];
        Ok(format!(
            "{{\"index\":{index},\"archetype\":{},\"level\":{},\"stage\":{},\"milliseconds\":{},{state}}}",
            json_quote(&format!("{}/{}", s.package, s.path)),
            s.level,
            s.stage,
            num(s.milliseconds)
        ))
    }
}

/// A hit report as JSON (C++ `hitJson`); the operators (`Core.Object.*`) are left out of `order`.
pub fn hit_json(report: &HitReport) -> String {
    let events: Vec<String> = report.events.iter().map(|e| json_quote(e)).collect();
    let order: Vec<String> = report
        .order
        .iter()
        .filter(|c| !c.starts_with("Core.Object."))
        .map(|c| json_quote(c))
        .collect();
    format!(
        "{{\"health_before\":{},\"health_after\":{},\"arriving\":{},\"applied\":{},\"shield_damage\":{},\"demigod_before\":{},\"demigod_after\":{},\"events\":[{}],\"order\":[{}]}}",
        num(report.health_before),
        num(report.health_after),
        num(report.arriving),
        num(report.applied),
        num(report.shield_damage),
        flag(report.demigod_before),
        flag(report.demigod_after),
        events.join(","),
        order.join(",")
    )
}

/// Whether the run is built over the mission bridge (C++ `combatRun`: a `maya` step is among the steps). The caller
/// builds the bridge (`ow-cli`, over the stock Fire mission, as the C++) and passes it to [`combat_run_with`].
pub fn wants_bridge(steps: &[String]) -> bool {
    steps.iter().any(|s| s.starts_with("maya:"))
}

/// The C++ mission the bridge of a `maya` run is built over: it only gives the bridge its curve and globals; nothing of
/// it is played (package, path).
pub const BRIDGE_MISSION: (&str, &str) = ("Startup", "GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire");

/// Runs `steps` on `world` and returns the report and whether a step failed (C++ `combatRun`), without the bridge. A
/// run with a `maya` step needs it: [`combat_run_with`].
pub fn combat_run(world: World, steps: &[String]) -> Result<(String, bool), VmError> {
    combat_run_with(world, steps, None)
}

/// [`combat_run`] over the mission bridge's player when `bridge` is given.
pub fn combat_run_with(
    world: World,
    steps: &[String],
    bridge: Option<Rc<dyn PlayerBridge>>,
) -> Result<(String, bool), VmError> {
    let started = Instant::now();
    let mut run = Run::with_bridge(world, bridge)?;
    for step in steps {
        run.apply(step);
    }
    let text = run.report(started.elapsed().as_secs_f64() * 1000.0);
    Ok((text, run.failed()))
}

#[cfg(test)]
pub(crate) mod fixture;
#[cfg(test)]
pub(crate) mod tests;
