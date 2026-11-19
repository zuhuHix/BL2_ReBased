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
//! - [`firing_mode`], the native `GetFiringModeDefinition`. **The two records disagree** (review r1 of #393):
//!   NATIVE_WEAPON_FIRING.md section 1.1 reads the barrel part's `CustomFiringModeDefinition`, else the type's
//!   `DefaultFiringModeDefinition`; NATIVE_WEAPON_TRIGGER_AMMO.md section 3 ("Which fire type") puts the elemental
//!   part's between them. Where they agree (the barrel has one, or the elemental part has none) that is the answer;
//!   an elemental part with its own firing mode and no barrel mode is `NotYet { NotRead }`.
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
//! - a weapon with no firing mode (`NotRead`). **The records disagree** (review r1 of #393):
//!   NATIVE_WEAPON_TRIGGER_AMMO.md section 3 says `InstantFire` does nothing at all without a definition (the ammunition
//!   is still spent), while NATIVE_WEAPON_FIRING.md section 1.2 step 4 lists "HitScan (or no firing mode)" inside
//!   `FiringModeDefinitionFire`, which only runs once a definition exists. Neither reading is taken;
//! - a firing mode whose `FireType` is Beam or Rocket (`NotPorted`: their own paths, not a bullet's impact);
//! - more than one projectile per shot (`NotRead`: whether a pellet carries `InstantHitDamage` or
//!   `GetMultiProjectileDamage`'s share depends on a shooter check the notes did not identify);
//! - a fire mode other than 0 (`NotRead`: the script sets up mode 0 only);
//! - a firing mode with `ImpactResponses` (`NotRead`: the response is picked by the surface type of the impact,
//!   `GetDamageSurfaceTypeFromImpact`, which is not read, and a run's hit has no impact record), with `OnAnyImpact`
//!   behaviors (`NotRead`: `RunBehaviorsForImpact` was decompiled, not worked through), or with its own
//!   `DamageTypeDefinition` (`NotRead`: how it combines with the weapon's is not read).
//!
//! **Bad data:** every property a refusal reads is read as declared (`access::required`): a weapon or firing mode
//! whose class does not declare it is an error, not a pass (review r1 of #393).
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
use super::access::{
    array, error, get, integer, loaded, number, object, object_of, path_of, required, set, truth,
};

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
    /// The firing mode's `FireType`, an index into [`FIRE_TYPES`] (Bullet or HitScan: the others are refused).
    pub fire_type: usize,
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

/// `GetFiringModeDefinition` (module documentation): the barrel's mode, else (both records agree only when the
/// elemental part has none) the type's.
pub fn firing_mode(vm: &mut Vm, parts: &WeaponParts) -> Result<Option<ObjRef>, VmError> {
    if let Some(barrel) = parts.barrel {
        let barrel = loaded(vm, barrel)?;
        if let Some(custom) = object(vm, barrel, "CustomFiringModeDefinition")? {
            return Ok(Some(custom));
        }
    }
    if let Some(part) = parts.elemental {
        let part = loaded(vm, part)?;
        if object(vm, part, "CustomFiringModeDefinition")?.is_some() {
            return Err(not_read(
                "the firing mode of a weapon whose elemental part has its own and whose barrel has none                  (NATIVE_WEAPON_TRIGGER_AMMO.md section 3 takes the elemental part's, NATIVE_WEAPON_FIRING.md                  section 1.1 the type's)",
            ));
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

/// The unit a missing property's error names.
const UNIT: &str = "combat shot";

/// Array property `name` of `o`, which the class must declare as an array (bad data otherwise).
fn required_array(vm: &mut Vm, o: ObjRef, name: &str) -> Result<Vec<Value>, VmError> {
    match required(vm, o, name, UNIT)? {
        Value::Array(items) => Ok((*items).clone()),
        other => Err(error(format!("{UNIT}: {name} holds {}, not an array", other.kind_name()))),
    }
}

fn not_read(what: impl Into<String>) -> VmError {
    NotYet::not_read(what.into(), RECORD).into()
}

/// One pellet of `weapon`'s shot with the firing mode `mode` (module documentation): the checks, then the values.
pub fn shot(vm: &mut Vm, weapon: ObjRef, mode: Option<ObjRef>) -> Result<Shot, VmError> {
    let fire_mode = integer(&required(vm, weapon, "CurrentFireMode", UNIT)?);
    if fire_mode != 0 {
        return Err(not_read(format!(
            "a shot in fire mode {fire_mode} (the weapon's setup script fills fire mode 0 only)"
        )));
    }
    let projectiles = integer(&required(vm, weapon, "ProjectilesPerShot", UNIT)?);
    if projectiles != 1 {
        return Err(not_read(format!(
            "the damage each of {projectiles} projectiles carries (InstantHitDamage or GetMultiProjectileDamage's \
             share, behind a shooter check the notes did not identify)"
        )));
    }
    let Some(mode) = mode else {
        return Err(not_read(
            "a shot from a weapon with no firing mode (NATIVE_WEAPON_TRIGGER_AMMO.md section 3: InstantFire does              nothing, the ammunition is still spent; NATIVE_WEAPON_FIRING.md section 1.2 step 4: the HitScan path)",
        ));
    };
    let fire_type;
    let mut impact = None;
    {
        let mode = loaded(vm, mode)?;
        let kind = integer(&required(vm, mode, "FireType", UNIT)?);
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
        fire_type = index;
        if object_of(&required(vm, mode, "DamageTypeDefinition", UNIT)?).is_some() {
            return Err(not_read(
                "a firing mode with its own DamageTypeDefinition (how it combines with the weapon's is not read)",
            ));
        }
        if !required_array(vm, mode, "ImpactResponses")?.is_empty() {
            return Err(not_read(
                "a firing mode's ImpactResponses (the response is picked by the impact's surface type, \
                 GetDamageSurfaceTypeFromImpact, which is not read)",
            ));
        }
        if !required_array(vm, mode, "OnAnyImpact")?.is_empty() {
            return Err(not_read(
                "a firing mode's OnAnyImpact behaviors (RunBehaviorsForImpact was not worked through)",
            ));
        }
        let suppress = required(vm, mode, "bSuppressImpactEffects", UNIT)?;
        let suppress = truth(vm, &suppress);
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

