//! Owner: lane H4-WEAPON. The held weapon of the play mode: left mouse fires, a ray against the H4 collision gives
//! the impact point, and the shot reports the weapon's damage number. No weapon maths of its own: the numbers are
//! `ow-items`' (W4-STATS: `instantiate_stats`, `instance_stat`), the refire interval goes through `ow-combat`'s
//! `firing::fire_interval` (W1-FIRING), and what has no ported rule answers `NotYet` (rule R3).
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-61-*`.
//!
//! **Which weapon.** The player's equipped weapon would come from the save (its format waits on decision D9) or a
//! starting loadout (not ported), so it is `NotYet` ([`equipped_weapon`]). A weapon file (`--weapon FILE`,
//! [`parse_spec`]) names one instead: its definition, balance, manufacturer, parts, prefix, title and level, the
//! fields of `ow-package --item-stats --data`. It is evaluated with that command's Off-mode rule ([`evaluate`]):
//! `instantiate_stats`, then the instance's
//! `WeaponDamage`, `WeaponFireInterval` and `WeaponProjectilesPerShot` through `ow-attr`, and its `WeaponRange`
//! property (`Weapon.GetTraceRange`, NATIVE_WEAPON_TRIGGER_AMMO.md: the hit-scan range). A weapon file names game
//! objects: keep it under ignored `local/`.
//!
//! **In which world.** Not `ow-balance`'s `OfflineWorld`, which `ow-package --item-stats --data` uses: it answers
//! every attribute change notification `NotYet` (it has no world), and instantiating a weapon writes an attribute
//! whose owner is notified, so a weapon never evaluates there (seen on the installed game, decision 61). The weapon
//! is evaluated in an `ow_world::World` instead: the VM over the install's store with the core natives, the world's
//! own packs and no other, at the weapon's level. The VM dispatches a change to the owner class's registered
//! override and does nothing without one (NATIVE_ATTRIBUTES.md section 4, UNVERIFIED); no weapon override is
//! registered here (the weapon `ClipSize` override is still open in that record), so the notifications do nothing.
//!
//! **One shot** ([`Gun::step`], once per simulation step, so a script's shots are a function of its steps):
//! - the trigger is held and the refire time is up: the weapon's fire interval (`firing::fire_interval` with no
//!   spin-up: the firing mode's spin-up fields are not read here); with no weapon, one shot per press;
//! - one ray from the eye along the view (the pawn's rotation after the step's look), as long as the weapon's
//!   `WeaponRange`, or [`PROBE_RANGE`] (a host stand-in, labelled in every shot) when there is no weapon or no range;
//! - against the H4 collision (`CollisionWorld::trace` with a shape of no size): the impact point, its normal and the
//!   brush, or none;
//! - the damage: the weapon's `WeaponDamage` (the number its card shows), per projectile. Against what it hits it is
//!   `NotYet`: the play mode has no pawns, and the hit chain and damage pipeline (`ow-combat` `hit`, `damage`) run
//!   on a world's pawns.
//! - Not applied, each open: spread, the firing pattern, accuracy, kick and recoil, one ray per pellet, ammunition
//!   and reload, bullets in flight (most guns are Bullet, NATIVE_WEAPON_FIRING.md section 1.2), the burst. Their
//!   rules are in `ow-combat::firing`; wiring them needs the weapon's firing mode and a `GameRng` stream.
//!
//! Everything is UNVERIFIED in the running game.

use std::path::Path;

use std::rc::Rc;

use ow_attr::AttributeSystem;
use ow_attr::api::{AttrWorld, EvalCx};
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount, Playthrough,
};
use ow_core::units::Rotator;
use ow_core::{DetMap, NotYet, ObjectPath, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_items::ItemError;
use ow_items::data::{DefinitionData, ItemKind};
use ow_items::registry::WEAPON_SLOTS;
use ow_items::stats::{instance_stat, instantiate_stats};
use ow_object::heap::ObjectSpace;
use ow_object::ids::PropAddr;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_world::world::{BalanceState, World};

use crate::frame::rotator_axes;
use crate::movement::collide::{CollisionWorld, Shape};
use crate::movement::geom::{V3, add, scale};

/// The record a `NotYet` of this module names.
pub const RECORD: &str = "docs/decisions/2026-11/2026-11-16-61-host-held-weapon-fire-in-the-play-mode-h4-weapon.md";

/// The ray's length when no weapon range is known, uu: a host stand-in for the impact marker, not a game value.
pub const PROBE_RANGE: f64 = 100_000.0;

/// The player's equipped weapon: not available yet (module documentation).
pub fn equipped_weapon() -> NotYet {
    NotYet::layer_absent(
        "the player's equipped weapon (no save format, decision D9; no starting loadout ported)",
        "ow-save, and the play mode's player inventory",
        RECORD,
    )
}

/// What a shot's damage does to what it hits: not available in the play mode.
pub fn target_damage() -> NotYet {
    NotYet::layer_absent(
        "the damage a shot deals to what it hits (the hit chain and damage pipeline run on a world's pawns)",
        "ow-world pawns in the play mode",
        RECORD,
    )
}

/// A weapon named by a file (module documentation).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WeaponSpec {
    pub definition: String,
    pub balance: Option<String>,
    pub manufacturer: Option<String>,
    /// (slot, part path), slots as `ow_items::registry::WEAPON_SLOTS` names them.
    pub parts: Vec<(String, String)>,
    pub prefix: Option<String>,
    pub title: Option<String>,
    pub level: i32,
}

/// Reads a weapon file: one `key value` per line, the keys `definition`, `balance`, `manufacturer`, `part <slot>`,
/// `prefix`, `title` and `level`; empty lines and lines starting with `#` are skipped. `definition` and `level` are
/// required; a key twice, an unknown key or slot, or a level outside 1 to 80 is refused.
pub fn parse_spec(text: &str) -> Result<WeaponSpec, String> {
    let mut spec = WeaponSpec::default();
    let (mut definition, mut level) = (None, None);
    for (n, line) in text.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let bad = |what: &str| format!("weapon line {}: {what}: {line:?}", n + 1);
        let mut words = line.split_whitespace();
        let key = words.next().unwrap_or_default();
        let once = |slot: &mut Option<String>, value: Option<&str>| -> Result<(), String> {
            let value = value.ok_or_else(|| bad("no value"))?;
            if slot.replace(value.to_string()).is_some() {
                return Err(bad("given twice"));
            }
            Ok(())
        };
        match key {
            "definition" => once(&mut definition, words.next())?,
            "balance" => once(&mut spec.balance, words.next())?,
            "manufacturer" => once(&mut spec.manufacturer, words.next())?,
            "prefix" => once(&mut spec.prefix, words.next())?,
            "title" => once(&mut spec.title, words.next())?,
            "level" => {
                let value = words.next().and_then(|w| w.parse::<i32>().ok()).filter(|l| (1..=80).contains(l));
                let value = value.ok_or_else(|| bad("level must be a whole number from 1 to 80"))?;
                if level.replace(value).is_some() {
                    return Err(bad("given twice"));
                }
            }
            "part" => {
                let (Some(slot), Some(path)) = (words.next(), words.next()) else {
                    return Err(bad("part needs a slot and a path"));
                };
                if !WEAPON_SLOTS.contains(&slot) {
                    return Err(bad("not a weapon slot"));
                }
                if spec.parts.iter().any(|(s, _)| s == slot) {
                    return Err(bad("slot given twice"));
                }
                spec.parts.push((slot.to_string(), path.to_string()));
            }
            _ => return Err(bad("unknown key")),
        }
        if words.next().is_some() {
            return Err(bad("more than one value"));
        }
    }
    spec.definition = definition.ok_or("the weapon file names no definition")?;
    spec.level = level.ok_or("the weapon file names no level")?;
    Ok(spec)
}

/// The weapon's numbers the play mode uses.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WeaponStats {
    /// `WeaponDamage`, per projectile.
    pub damage: f64,
    /// `WeaponFireInterval`, seconds.
    pub fire_interval: f64,
    /// `WeaponProjectilesPerShot`, when it resolves.
    pub projectiles: Option<f64>,
    /// The `WeaponRange` property, uu, when the instance has one.
    pub range: Option<f64>,
}

impl WeaponStats {
    pub fn new(damage: f64, fire_interval: f64, projectiles: Option<f64>, range: Option<f64>) -> WeaponStats {
        WeaponStats { damage, fire_interval, projectiles, range }
    }
}

/// The balance state an offline evaluation runs in: the weapon's level as game stage and experience level, the
/// rest the defaults `ow-package --item-stats` uses.
fn snapshot(level: i32) -> Result<BalanceSnapshot, ItemError> {
    Ok(BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::new(0)?,
        PlayerCount::SINGLE,
        ExpLevel::new(level.clamp(1, 80)),
        GameStage(level),
        AwesomeLevel::new(0),
        LevelCapUpgrades::ALL,
    )?)
}

fn object_path(p: &str) -> Result<ObjectPath, ItemError> {
    ObjectPath::parse(p).map_err(|e| ItemError::BadData(format!("{p}: {e}")))
}

/// The definition data of `spec`, as `ow-package --item-stats --data` builds it.
pub fn definition_data(spec: &WeaponSpec) -> Result<DefinitionData, ItemError> {
    let opt = |p: &Option<String>| p.as_deref().map(object_path).transpose();
    let mut parts: [Option<ObjectPath>; 9] = Default::default();
    for (slot, part) in &spec.parts {
        let i = WEAPON_SLOTS
            .iter()
            .position(|s| s == slot)
            .ok_or_else(|| ItemError::BadData(format!("{slot} is not a weapon slot")))?;
        if let Some(cell) = parts.get_mut(i) {
            *cell = Some(object_path(part)?);
        }
    }
    Ok(DefinitionData {
        kind: ItemKind::Weapon,
        balance: opt(&spec.balance)?,
        definition: object_path(&spec.definition)?,
        manufacturer: opt(&spec.manufacturer)?,
        manufacturer_grade: spec.level,
        parts,
        prefix: opt(&spec.prefix)?,
        title: opt(&spec.title)?,
        game_stage: GameStage(spec.level),
        unique_id: 0,
    })
}

/// Evaluates `spec` against the install at `cooked` (module documentation).
pub fn evaluate(cooked: &Path, spec: &WeaponSpec) -> Result<WeaponStats, ItemError> {
    let data = definition_data(spec)?;
    let defs = DefStore::open(cooked.to_path_buf(), IdentityPolicy::FirstInScanOrder, ParityMode::Off)?;
    let sys = Rc::new(AttributeSystem::standard()?);
    let world_error = |e: String| ItemError::BadData(format!("the world for the weapon: {e}"));
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::clone(&sys), ParityMode::Off);
    ow_vm::natives_core::register(&mut vm).map_err(|e| world_error(e.to_string()))?;
    let mut world =
        World::new(vm, BalanceState::new(snapshot(spec.level)?), &[]).map_err(|e| world_error(e.to_string()))?;
    let (object, damage, interval, projectiles) = {
        let mut cx = EvalCx::new(&mut world.vm);
        let instance = instantiate_stats(&data, spec.level, &sys, &mut cx, &DetMap::new())?;
        let o = instance.object;
        (
            o,
            instance_stat(o, "WeaponDamage", &sys, &mut cx)?,
            instance_stat(o, "WeaponFireInterval", &sys, &mut cx)?,
            instance_stat(o, "WeaponProjectilesPerShot", &sys, &mut cx)?,
        )
    };
    let unresolved = |what: &'static str| {
        ItemError::from(NotYet::not_read(format!("the weapon's {what} (its attribute did not resolve)"), RECORD))
    };
    let damage = damage.ok_or_else(|| unresolved("WeaponDamage"))?;
    let interval = interval.filter(|i| i.is_finite() && *i > 0.0).ok_or_else(|| unresolved("WeaponFireInterval"))?;
    let space = world.vm.parts().space;
    let range = space.heap.get(object).ok().and_then(|o| space.prop(o.class, "WeaponRange")).and_then(|p| {
        match space.read(object, PropAddr::scalar(p)) {
            Ok(Value::Float(r)) if r.is_finite() && *r > 0.0 => Some(f64::from(*r)),
            _ => None,
        }
    });
    Ok(WeaponStats { damage, fire_interval: interval, projectiles, range })
}

/// Where a ray's length came from.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RangeSource {
    /// The weapon's `WeaponRange`.
    Weapon,
    /// [`PROBE_RANGE`], the host stand-in.
    Probe,
}

/// What the ray hit.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ShotHit {
    pub point: V3,
    pub normal: V3,
    /// The brush's index in the collision world.
    pub brush: u32,
    pub distance: f64,
}

/// One shot.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Shot {
    /// The simulation step it fired on (from 1).
    pub step: u64,
    pub origin: V3,
    pub direction: V3,
    pub range: f64,
    pub range_source: RangeSource,
    pub hit: Option<ShotHit>,
    /// The weapon's damage per projectile, or why there is none.
    pub damage: Result<f64, NotYet>,
    pub projectiles: Option<f64>,
}

/// The held weapon's state between steps.
#[derive(Debug, Clone, PartialEq)]
pub struct Gun {
    stats: Result<WeaponStats, NotYet>,
    /// Time left before the next shot may fire, seconds (0 or less: ready).
    cooldown: f64,
    /// The trigger was held on the last step (one shot per press with no weapon).
    held_before: bool,
}

/// The refire time counts as up within this much of zero, seconds: an interval that is a whole number of steps
/// (0.25 s at 1/60 s) would otherwise fire one step late on the rounding of the repeated subtraction. A numeric
/// margin, not a game value.
const READY: f64 = 1e-9;

/// A shape of no size: the trace is a ray.
const RAY: Shape = Shape::Cylinder { radius: 0.0, half_height: 0.0 };

impl Gun {
    pub fn new(stats: Result<WeaponStats, NotYet>) -> Gun {
        Gun { stats, cooldown: 0.0, held_before: false }
    }

    pub fn stats(&self) -> &Result<WeaponStats, NotYet> {
        &self.stats
    }

    /// One simulation step of `dt` with the trigger `held`, the eye at `eye` looking along `view` (module
    /// documentation). `Some` when it fired.
    pub fn step(
        &mut self,
        world: &CollisionWorld,
        held: bool,
        eye: V3,
        view: Rotator,
        dt: f64,
        step: u64,
    ) -> Option<Shot> {
        let pressed = held && !self.held_before;
        self.held_before = held;
        if self.cooldown > 0.0 {
            self.cooldown -= dt;
        }
        let fires = match &self.stats {
            Ok(_) => held && self.cooldown <= READY,
            Err(_) => pressed,
        };
        if !fires {
            // A trigger let go does not bank refire time.
            if !held && self.cooldown < 0.0 {
                self.cooldown = 0.0;
            }
            return None;
        }
        if let Ok(stats) = &self.stats {
            // No spin-up: the firing mode's spin-up fields are not read (module documentation).
            self.cooldown += ow_combat::firing::fire_interval(stats.fire_interval, false, 1.0, 0.0);
        }
        let direction = rotator_axes(view)[0];
        let (range, range_source) = match self.stats.as_ref().ok().and_then(|s| s.range) {
            Some(r) => (r, RangeSource::Weapon),
            None => (PROBE_RANGE, RangeSource::Probe),
        };
        let trace = world.trace(&RAY, eye, add(eye, scale(direction, range)));
        let hit = trace.brush.map(|brush| ShotHit {
            point: trace.end,
            normal: trace.normal,
            brush,
            distance: trace.fraction * range,
        });
        Some(Shot {
            step,
            origin: eye,
            direction,
            range,
            range_source,
            hit,
            damage: self.stats.as_ref().map(|s| s.damage).map_err(Clone::clone),
            projectiles: self.stats.as_ref().ok().and_then(|s| s.projectiles),
        })
    }
}

#[cfg(test)]
mod tests;
