//! Owner: lane FIX-FIRE-TO-HIT. One shot of a held weapon, from its definition data to the hit chain's inputs.
//! AI-assisted (Claude Code). Decision 2026-11-19-06.
//!
//! **What it joins.** A weapon instance (W4-STATS' `ow_items::stats::instantiate_stats`, which sets the attribute
//! bases `InstantHitDamage`, `ProjectilesPerShot` and the rest from the weapon type) gets fire mode 0's damage source,
//! damage type and impact definition ([`initialize_fire_mode`]), and a shot ([`shot`]) reads them back with the
//! weapon's `InstantHitDamage` as [`crate::hit::hit_with`]'s inputs. Every rule is **UNVERIFIED** in the running game
//! unless a line says how it was seen there.
//!
//! **Where each rule comes from.**
//! - [`initialize_fire_mode`]: the installed script `WillowWeapon.ClientInitializeFromWeaponTypeDefinition` (read with
//!   `ow-package --disasm`, 2026-11-19), in its own words: element 0 of `InstantHitDamageTypes` takes the weapon type's
//!   `InstantHitDamageType`; element 0 of `InstantHitDamageTypeDefinitions` takes `StaticGetWeaponDamageType` of the
//!   definition data; element 0 of `InstantHitImpactDefinitions` takes the type's `WeaponImpact`, then the elemental
//!   part's `WeaponImpact` when there is an elemental part and its `WeaponImpact` is not None. The script does more
//!   (behavior providers, meshes, attribute slots); only these three are written here.
//! - [`weapon_damage_type`], the native `StaticGetWeaponDamageType`: the elemental part's `CustomDamageTypeDefinition`,
//!   else the type's `DefaultDamageTypeDefinition`; no other part is consulted (read from native code 2026-10-23,
//!   `tools/weapon_recipe.py` `native_damage_type`, NATIVE_WEAPON_RULES.md section 5; the name rule built on it was
//!   confirmed live once, a shock SMG's title).
//! - [`firing_mode`], the native `GetFiringModeDefinition`: the barrel part's `CustomFiringModeDefinition` when set,
//!   else the type's `DefaultFiringModeDefinition` (NATIVE_WEAPON_FIRING.md section 1.1).
//! - [`shot`]: NATIVE_WEAPON_TRIGGER_AMMO.md (`FiringModeDefinitionFire`, `ProcessInstantHitBullet`) and
//!   NATIVE_WEAPON_FIRING.md sections 1.1, 1.7 and 6. A Bullet or HitScan pellet's damage is the weapon's
//!   `InstantHitDamage`, the instigator the shooter's controller, the damage class `InstantHitDamageTypes[fire mode]`,
//!   the weapon the damage causer, and with a firing mode the impact definition `InstantHitImpactDefinitions[fire
//!   mode]` unless the mode sets `bSuppressImpactEffects`. **Seen live** (PROJECTILE_FIRING_PATTERNS_LIVE.md R6, 59
//!   `TakeDamage` rows on bullymongs): the damage equals the stored bullet damage, one `TakeDamage` per pellet, and the
//!   damage class is the weapon's source class (`WillowDmgSource_Pistol`, `_Shotgun`).
//!
//! **The damage type definition the pipeline carries** is read as `InstantHitDamageTypeDefinitions[fire mode]`: the
//! array the script fills for the shot, and what NATIVE_WEAPON_FIRING.md section 6 calls "the damage type" the damage
//! is applied with. That the native passes this element, and not another object, is not separated in the notes:
//! UNVERIFIED. It agrees with the one live shotgun row of COMBAT_HIT_CHAIN.md (L3 section A: `DmgType_Normal`), which
//! is the non-elemental type default.
//!
//! **What answers `NotYet`** (both parity modes: the C++ has no shot, so there is no Reference stand-in, rule R3):
//! - a firing mode whose `FireType` is Beam or Rocket (`NotPorted`: their own paths, not a bullet's impact);
//! - more than one projectile per shot (`NotRead`: whether a pellet carries `InstantHitDamage` or
//!   `GetMultiProjectileDamage`'s share depends on a shooter check the notes did not identify);
//! - a fire mode other than 0 (`NotRead`: the script sets up mode 0 only);
//! - a firing mode with `ImpactResponses` (`NotRead`: the response is picked by the surface type of the impact,
//!   `GetDamageSurfaceTypeFromImpact`, which is not read, and a run's hit has no impact record), with `OnAnyImpact`
//!   behaviors (`NotRead`: `RunBehaviorsForImpact` was decompiled, not worked through), or with its own
//!   `DamageTypeDefinition` (`NotRead`: how it combines with the weapon's is not read).
//!
//! **Not modelled, said so:** a Bullet's flight (its damage lands after `distance / speed`, R6; a run's shot lands at
//! once), the momentum (the ray direction times `InstantHitMomentum`: a run has no aim, so the zero vector), the hit
//! location and the hit information (bone, physical material), spread and pattern (which decide whether the pellet
//! hits at all), ammunition. Actors that forward a hit to an owner are not separated: the enemy pawn takes the direct
//! path, as the live rows show for bullymongs.

use ow_core::NotYet;
use ow_object::ids::{ClassId, ObjRef};
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use super::ShotParts;
use super::access::{array, error, get, integer, loaded, number, object, path_of, set, truth};

/// The records behind the rules (module documentation).
pub const RECORD: &str = "NATIVE_WEAPON_TRIGGER_AMMO.md (FiringModeDefinitionFire, ProcessInstantHitBullet); \
     NATIVE_WEAPON_FIRING.md sections 1.1, 1.7 and 6; decision 2026-11-19-06";

/// `FiringModeDefinition.EWillowWeaponFireType` (NATIVE_WEAPON_FIRING.md section 1.1, decoded from the package).
pub const FIRE_TYPES: [&str; 4] = ["Bullet", "Beam", "Rocket", "HitScan"];

/// The definition-data objects a shot's setup reads: the weapon type, the elemental part and the barrel part.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WeaponParts {
    pub weapon_type: ObjRef,
    pub elemental: Option<ObjRef>,
    pub barrel: Option<ObjRef>,
}

impl WeaponParts {
    pub fn new(weapon_type: ObjRef, elemental: Option<ObjRef>, barrel: Option<ObjRef>) -> WeaponParts {
        WeaponParts {
            weapon_type,
            elemental,
            barrel,
        }
    }
}

/// What one pellet of a shot hands the hit chain.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shot {
    /// The weapon's `InstantHitDamage`.
    pub damage: f64,
    /// `InstantHitDamageTypes[fire mode]`.
    pub source: ClassId,
    /// `InstantHitDamageTypeDefinitions[fire mode]` (None when the array has no such element).
    pub damage_type: Option<ObjRef>,
    /// The impact definition and the causer (the weapon).
    pub parts: ShotParts,
    /// The firing mode's `FireType` (an index into [`FIRE_TYPES`]), or `None` with no firing mode (the HitScan path).
    pub fire_type: Option<usize>,
}

/// `StaticGetWeaponDamageType` (module documentation).
pub fn weapon_damage_type(vm: &mut Vm, parts: &WeaponParts) -> Result<Option<ObjRef>, VmError> {
    if let Some(part) = parts.elemental {
        let part = loaded(vm, part)?;
        if let Some(custom) = object(vm, part, "CustomDamageTypeDefinition")? {
            return Ok(Some(custom));
        }
    }
    let weapon_type = loaded(vm, parts.weapon_type)?;
    object(vm, weapon_type, "DefaultDamageTypeDefinition")
}

/// `GetFiringModeDefinition` (module documentation).
pub fn firing_mode(vm: &mut Vm, parts: &WeaponParts) -> Result<Option<ObjRef>, VmError> {
    if let Some(barrel) = parts.barrel {
        let barrel = loaded(vm, barrel)?;
        if let Some(custom) = object(vm, barrel, "CustomFiringModeDefinition")? {
            return Ok(Some(custom));
        }
    }
    let weapon_type = loaded(vm, parts.weapon_type)?;
    object(vm, weapon_type, "DefaultFiringModeDefinition")
}

/// Writes element 0 of the weapon's array `name`: the element replaced, or the array grown to one element when it
/// is empty (UnrealScript's assignment to an element past the end grows the array).
fn set_first(vm: &mut Vm, weapon: ObjRef, name: &str, v: Value) -> Result<(), VmError> {
    let mut items = array(vm, weapon, name)?;
    match items.first_mut() {
        Some(first) => *first = v,
        None => items.push(v),
    }
    if !set(vm, weapon, name, Value::array(items))? {
        return Err(error(format!("combat shot: the weapon has no array {name}")));
    }
    Ok(())
}

/// The script's fire mode 0 setup on `weapon` (module documentation).
pub fn initialize_fire_mode(vm: &mut Vm, weapon: ObjRef, parts: &WeaponParts) -> Result<(), VmError> {
    let weapon_type = loaded(vm, parts.weapon_type)?;
    let source = match get(vm, weapon_type, "InstantHitDamageType")? {
        Some(Value::Class(c)) => Value::Class(c),
        Some(Value::None) | None => Value::Class(None),
        Some(other) => {
            return Err(error(format!(
                "combat shot: the weapon type's InstantHitDamageType holds {}, not a class",
                other.kind_name()
            )));
        }
    };
    set_first(vm, weapon, "InstantHitDamageTypes", source)?;
    let damage_type = weapon_damage_type(vm, parts)?;
    set_first(
        vm,
        weapon,
        "InstantHitDamageTypeDefinitions",
        Value::Object(damage_type),
    )?;
    let mut impact = object(vm, weapon_type, "WeaponImpact")?;
    if let Some(part) = parts.elemental {
        let part = loaded(vm, part)?;
        if let Some(own) = object(vm, part, "WeaponImpact")? {
            impact = Some(own);
        }
    }
    set_first(vm, weapon, "InstantHitImpactDefinitions", Value::Object(impact))
}

fn not_read(what: impl Into<String>) -> VmError {
    NotYet::not_read(what.into(), RECORD).into()
}

/// One pellet of `weapon`'s shot with the firing mode `mode` (module documentation): the checks, then the values.
pub fn shot(vm: &mut Vm, weapon: ObjRef, mode: Option<ObjRef>) -> Result<Shot, VmError> {
    let fire_mode = get(vm, weapon, "CurrentFireMode")?.map_or(0, |v| integer(&v));
    if fire_mode != 0 {
        return Err(not_read(format!(
            "a shot in fire mode {fire_mode} (the weapon's setup script fills fire mode 0 only)"
        )));
    }
    let projectiles = get(vm, weapon, "ProjectilesPerShot")?.map_or(1, |v| integer(&v));
    if projectiles != 1 {
        return Err(not_read(format!(
            "the damage each of {projectiles} projectiles carries (InstantHitDamage or GetMultiProjectileDamage's \
             share, behind a shooter check the notes did not identify)"
        )));
    }
    let mut fire_type = None;
    let mut impact = None;
    if let Some(mode) = mode {
        let mode = loaded(vm, mode)?;
        let kind = get(vm, mode, "FireType")?.map_or(0, |v| integer(&v));
        let index = usize::try_from(kind)
            .ok()
            .filter(|i| *i < FIRE_TYPES.len())
            .ok_or_else(|| error(format!("combat shot: the firing mode's FireType {kind} is not one of the four")))?;
        if matches!(index, 1 | 2) {
            return Err(NotYet::not_ported(
                format!(
                    "a {} shot (its own path, not a bullet's impact: {})",
                    FIRE_TYPES[index],
                    path_of(vm, Some(mode))
                ),
                RECORD,
            )
            .into());
        }
        fire_type = Some(index);
        if object(vm, mode, "DamageTypeDefinition")?.is_some() {
            return Err(not_read(
                "a firing mode with its own DamageTypeDefinition (how it combines with the weapon's is not read)",
            ));
        }
        if !array(vm, mode, "ImpactResponses")?.is_empty() {
            return Err(not_read(
                "a firing mode's ImpactResponses (the response is picked by the impact's surface type, \
                 GetDamageSurfaceTypeFromImpact, which is not read)",
            ));
        }
        if !array(vm, mode, "OnAnyImpact")?.is_empty() {
            return Err(not_read(
                "a firing mode's OnAnyImpact behaviors (RunBehaviorsForImpact was not worked through)",
            ));
        }
        let suppress = get(vm, mode, "bSuppressImpactEffects")?.is_some_and(|v| truth(vm, &v));
        if !suppress {
            impact = array(vm, weapon, "InstantHitImpactDefinitions")?
                .first()
                .and_then(super::access::object_of);
        }
    }
    let source = match array(vm, weapon, "InstantHitDamageTypes")?.first() {
        Some(Value::Class(Some(c))) => *c,
        _ => {
            return Err(error(
                "combat shot: the weapon's InstantHitDamageTypes has no damage class for fire mode 0",
            ));
        }
    };
    let damage_type = array(vm, weapon, "InstantHitDamageTypeDefinitions")?
        .first()
        .and_then(super::access::object_of);
    let damage = get(vm, weapon, "InstantHitDamage")?
        .map(|v| number(&v))
        .ok_or_else(|| error("combat shot: the weapon has no InstantHitDamage"))?;
    Ok(Shot {
        damage,
        source,
        damage_type,
        parts: ShotParts::new(impact, Some(weapon)),
        fire_type,
    })
}

