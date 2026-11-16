//! Owner: lane W6-COMBAT. The damage pipeline natives: `WillowDamagePipeline.AdjustDamage` and the damage type
//! definition's two modifier natives. Port of the `AdjustDamage` half of `src/combat_hit.cpp`. AI-assisted (Claude
//! Code).
//!
//! **AdjustDamage** follows NATIVE_COMBAT_GROUNDWORK.md section 3.2's stage order; every stage is **UNVERIFIED**
//! unless a live row confirms it. Written here, as in the C++: the stages a player hit on an AI pawn can reach (level
//! difference, damage source, instigator damage type, hit region and crit, intrinsic armor, damage surface x element)
//! and, for an AI instigator's blow on the player (plan step S-e), the AI damage scale of stage 1. Left out and said
//! so in the events: the AI weapon scales, backstab, first/last shot, reflection, the shield stage, the hit-region
//! cap, averted death and the protection timer. The result goes into the summary's `FinalDamage` and the summary is
//! returned; the out arguments are not written (the live call showed them unset).
//!
//! **The 6.1 row "a property reading 0 with base 0 is the identity 1"** (architecture section 6.1, owner W6-COMBAT).
//! `GetPawnDamageTypeModifier` reads the pawn's attribute that the damage type names. The VM does not run the stock
//! spawn that initialises the pawn's attributes (`PopulationFactoryBalancedAIPawn.ApplyAttributeStartingValues`, not
//! read), so the C++ reads a value of 0 with a base value of 0 as uninitialised, i.e. 1. That reading is
//! `ParityMode::Reference` only ([`resistance`]); in `ParityMode::Off` such a value is `NotYet { NotRead }` naming the
//! start-up values, and any other value is the pawn's as read. The same holds for the critical-hit bonus of stage 7:
//! Off reads the instigator's value, and a value of 0 with base 0 is `NotYet { NeedsRealGame(8) }` (architecture
//! question 8: Maya's crit multiplier source; her stored base is 0); Reference reads it as the C++ does (the value,
//! 1 when the class does not declare it).

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ClassId, ObjRef};
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use crate::hit::access::{
    error, field, g5, get, integer, number, object, path_of, set_field, to_f32, truth,
};
use crate::hit::{
    SOURCE_CLASSES, arg_object, attribute, bind_fn, call_method, evaluate_for, globals, hit_region,
    is_player_controller, level_of, note, note_once, this,
};

/// The record of the stages.
pub const RECORD: &str =
    "NATIVE_COMBAT_GROUNDWORK.md section 3.2; NATIVE_DAMAGE_DEATH.md sections 1 and 6";
/// The record of the 6.1 row.
pub const RECORD_START_UP: &str = "PopulationFactoryBalancedAIPawn.ApplyAttributeStartingValues (not read; architecture section 6.1)";

/// A victim below this share of its maximum is "injured" for the summary and the melee stage.
pub const INJURED_SHARE: f32 = 0.3;
/// The victim's amplify modifier applies above this value.
pub const AMPLIFY_THRESHOLD: f32 = 1.0001;
/// The `EDamageType` names of the instigator element modifiers (`Instigated<Name>DamageModifier`), by enum value.
pub const ELEMENT_MODIFIERS: [Option<&str>; 8] = [
    None,
    Some("Incindiary"),
    Some("Shock"),
    Some("Explosive"),
    Some("Corrosive"),
    Some("Impact"),
    None,
    Some("Amp"),
];

pub(crate) fn register(vm: &mut Vm) -> Result<(), VmError> {
    // GetSurfaceDamageTypeModifier(surface, instigator) and GetPawnDamageTypeModifier(pawn) (DD section 6).
    bind_fn(
        vm,
        "Engine.DamageTypeDefinition.GetSurfaceDamageTypeModifier",
        |c| {
            let me = this(c, "GetSurfaceDamageTypeModifier")?;
            let surface = integer(c.arg(0)?);
            let instigator = arg_object(c, 1)?;
            Ok(Value::Float(surface_modifier(
                c.vm, me, surface, instigator,
            )?))
        },
    )?;
    bind_fn(
        vm,
        "Engine.DamageTypeDefinition.GetPawnDamageTypeModifier",
        |c| {
            let me = this(c, "GetPawnDamageTypeModifier")?;
            Ok(Value::Float(match arg_object(c, 0)? {
                Some(pawn) => resistance(c.vm, pawn, me)?,
                None => 1.0,
            }))
        },
    )
}

/// The damage type's modifier of surface `surface` (1 flesh, 2 armor, 3 shield) evaluated with `instigator` as the
/// context source; 1 for another surface or a definition without that field.
pub fn surface_modifier(
    vm: &mut Vm,
    damage_type: ObjRef,
    surface: i64,
    instigator: Option<ObjRef>,
) -> Result<f32, VmError> {
    let field_name = match surface {
        1 => "FleshDamageModifier",
        2 => "ArmorDamageModifier",
        3 => "ShieldDamageModifier",
        _ => return Ok(1.0),
    };
    match get(vm, damage_type, field_name)? {
        Some(data) => evaluate_for(vm, &data, instigator),
        None => Ok(1.0),
    }
}

/// `GetPawnDamageTypeModifier` (DD section 6): the value of the pawn's attribute that the damage type names
/// (`CharacterDamageTypeModifierAttribute`), the pawn's property of the attribute's own name. A damage type naming
/// none is 1; a pawn whose class does not declare it is [`attribute`]'s rule; 0 with a base of 0 is the 6.1 row
/// (module documentation).
pub fn resistance(vm: &mut Vm, pawn: ObjRef, damage_type: ObjRef) -> Result<f32, VmError> {
    let Some(named) = object(vm, damage_type, "CharacterDamageTypeModifierAttribute")? else {
        return Ok(1.0);
    };
    let path = path_of(vm, Some(named));
    let leaf = path
        .rsplit_once('.')
        .map_or(path.as_str(), |(_, l)| l)
        .to_string();
    let Some(value) = get(vm, pawn, &leaf)? else {
        return attribute(vm, pawn, &leaf, 1.0);
    };
    let base = get(vm, pawn, &format!("{leaf}BaseValue"))?;
    if number(&value) == 0.0 && base.as_ref().is_none_or(|b| number(b) == 0.0) {
        return match vm.parity {
            ParityMode::Reference => Ok(1.0),
            ParityMode::Off => Err(NotYet::not_read(
                format!("the pawn's {leaf}: 0 with a base of 0, i.e. not initialised by the start-up values"),
                RECORD_START_UP,
            )
            .into()),
        };
    }
    Ok(to_f32(number(&value)))
}

/// The critical-hit bonus of the instigator (stage 7, module documentation).
fn crit_bonus(vm: &mut Vm, instigator: ObjRef, melee: bool) -> Result<f32, VmError> {
    let name = if melee {
        "CurrentMeleeCriticalHitBonus"
    } else {
        "CurrentInstantHitCriticalHitBonus"
    };
    if vm.parity == ParityMode::Off
        && let Some(v) = get(vm, instigator, name)?
    {
        let base = get(vm, instigator, &format!("{name}BaseValue"))?;
        if number(&v) == 0.0 && base.as_ref().is_none_or(|b| number(b) == 0.0) {
            return Err(NotYet::needs_real_game(
                format!("the critical-hit bonus {name}: 0 with a base of 0 (its source is unread)"),
                8,
                RECORD,
            )
            .into());
        }
    }
    attribute(vm, instigator, name, 1.0)
}

/// The damage source kind (an index into [`SOURCE_CLASSES`], `None` for any other class): the last class of the
/// table the source derives from, as the C++ loop leaves it.
fn source_kind(vm: &mut Vm, source: Option<ClassId>) -> Option<usize> {
    let source = source?;
    let mut kind = None;
    for (i, (path, _)) in SOURCE_CLASSES.iter().enumerate() {
        if let Ok(base) = vm.find_class(path)
            && vm.space.classes.is_child_of(source, base)
        {
            kind = Some(i);
        }
    }
    kind
}

/// `WillowDamagePipeline.AdjustDamage(out IncomingDamage, out DamageSeverityPercent, DamagedActor, DamageInstigator,
/// HitLocation, DamageSource, out HitMomentum, out HitInfo)` returning the summary (module documentation).
pub fn adjust_damage(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    let pipeline = this(c, "AdjustDamage")?;
    let victim = arg_object(c, 2)?;
    let instigator = arg_object(c, 3)?;
    let source = match c.arg(5)? {
        Value::Class(k) => *k,
        _ => None,
    };
    let hit_info = if c.arg_count() > 7 {
        c.arg(7)?.clone()
    } else {
        Value::None
    };
    let mut damage = to_f32(number(c.arg(0)?));
    let vm = &mut *c.vm;
    let Some(mut summary) = get(vm, pipeline, "DamageSummary")? else {
        return Err(error(format!(
            "combat hit: {} has no property DamageSummary",
            crate::hit::access::class_name(vm, pipeline)
        )));
    };
    // A summary field the struct does not declare is left out, as the C++ `store` leaves it.
    let store = |vm: &Vm, summary: &mut Value, name: &str, v: Value| {
        set_field(vm, summary, name, v);
    };

    // Setup: the victim's health before, the hit's source kind.
    let (health, max_health) = match victim {
        Some(v) => (
            call_method(vm, v, "GetHealth", Vec::new())?.map_or(0.0, |x| to_f32(number(&x))),
            call_method(vm, v, "GetMaxHealth", Vec::new())?.map_or(0.0, |x| to_f32(number(&x))),
        ),
        None => (0.0, 0.0),
    };
    store(vm, &mut summary, "InitialDamage", Value::Float(damage));
    store(vm, &mut summary, "PreviousHealth", Value::Float(health));
    store(
        vm,
        &mut summary,
        "bWasMaxHealth",
        Value::Bool(max_health > 0.0 && health >= max_health),
    );
    store(
        vm,
        &mut summary,
        "bWasInjured",
        Value::Bool(max_health > 0.0 && health < INJURED_SHARE * max_health),
    );
    let kind = source_kind(vm, source);
    let type_definition = object(vm, pipeline, "DamageTypeDef")?;
    let player_instigator = is_player_controller(vm, instigator)?;
    let victim_mind = match victim {
        Some(v) => object(vm, v, "Controller")?,
        None => None,
    };

    // 1. AI damage scale (plan step S-e): only for an AI instigator. The globals' GlobalAIDamageScale, evaluated with
    // the instigator as the source, multiplies the damage; the AI weapon scales are not implemented and said so.
    {
        let before = damage;
        if let Some(inst) = instigator
            && !player_instigator
        {
            let g = globals(vm)?;
            match get(vm, g, "GlobalAIDamageScale")? {
                Some(scale) => damage *= evaluate_for(vm, &scale, Some(inst))?,
                None => note(
                    vm,
                    "note: the globals definition has no GlobalAIDamageScale (stage 1 read as 1)",
                )?,
            }
            if matches!(kind, Some(0) | Some(3)) {
                note_once(
                    vm,
                    "ai weapon scale",
                    "AdjustDamage stage 1: the AI weapon scales of bullet and rocket damage are not implemented",
                )?;
            }
        }
        store(
            vm,
            &mut summary,
            "AIDamageScaleReduction",
            Value::Float(before - damage),
        );
        if instigator.is_some() && !player_instigator {
            let ratio = if before > 0.0 { damage / before } else { 0.0 };
            note(
                vm,
                format!("AdjustDamage AI damage scale={}", g5(f64::from(ratio))),
            )?;
        }
    }

    // 2. Level difference: with an instigating controller and a pawn victim. Equal levels 1; else |difference|
    // (clamped to the table length) selects the entry, the higher-level attacker takes HigherLevelAttackerDmgScale and
    // the lower-level one LowerLevelAttackerDmgScale; the player table for a player attacker, the AI table otherwise.
    if let (Some(inst), Some(v)) = (instigator, victim) {
        let attacker = level_of(vm, Some(inst))?;
        let target = level_of(vm, Some(v))?;
        let mut scale = 1.0f32;
        if attacker != target {
            let g = globals(vm)?;
            let table_name = if player_instigator {
                "PlayerDamageScaleByLevelDifference"
            } else {
                "AIDamageScaleByLevelDifference"
            };
            let rows = crate::hit::access::array(vm, g, table_name)?;
            if !rows.is_empty() {
                let difference = usize::try_from(attacker.abs_diff(target)).unwrap_or(usize::MAX);
                let index = difference.min(rows.len()) - 1;
                let entry_name = if attacker > target {
                    "HigherLevelAttackerDmgScale"
                } else {
                    "LowerLevelAttackerDmgScale"
                };
                if let Some(entry) = field(vm, &rows[index], entry_name) {
                    scale = to_f32(number(&entry));
                }
            }
        }
        let before = damage;
        damage *= scale;
        store(
            vm,
            &mut summary,
            "ExpLevelDifferenceReduction",
            Value::Float(before - damage),
        );
        note(
            vm,
            format!(
                "AdjustDamage level difference attacker={attacker} victim={target} scale={}",
                g5(f64::from(scale))
            ),
        )?;
    }

    // 3. Damage source: the instigator's Instigated<Source> and the victim controller's Received<Source> modifier, with
    // the factors the note folds into the stage (bullets over the minimum health fraction, melee on an injured victim,
    // the victim's amplify).
    {
        let before = damage;
        if let Some(k) = kind {
            let name = SOURCE_CLASSES[k].1;
            if let Some(inst) = instigator {
                damage *= attribute(vm, inst, &format!("Instigated{name}DamageModifier"), 1.0)?;
            }
            if let Some(mind) = victim_mind {
                damage *= attribute(vm, mind, &format!("Received{name}DamageModifier"), 1.0)?;
            }
        }
        if let Some(inst) = instigator
            && kind == Some(0)
            && max_health > 0.0
            && health / max_health > attribute(vm, inst, "TargetMinHealthDamageBoostPercent", 2.0)?
        {
            damage *= attribute(vm, inst, "TargetOverMinHealthGunDamageMultiplier", 1.0)?;
        }
        if let Some(inst) = instigator
            && kind == Some(1)
            && max_health > 0.0
            && health < INJURED_SHARE * max_health
        {
            damage *= attribute(vm, inst, "AttackInjuredMeleeDamageModifier", 1.0)?;
        }
        if let Some(mind) = victim_mind {
            let amplify = attribute(vm, mind, "ReceivedAmplifyDamageModifier", 1.0)?;
            if amplify > AMPLIFY_THRESHOLD {
                damage *= amplify;
            }
        }
        store(
            vm,
            &mut summary,
            "DamageSourceReduction",
            Value::Float(before - damage),
        );
    }

    // 4. Instigator damage type: the per-element modifier of the damage type's EDamageType, when the definition asks
    // for the status-effect modifiers (or there is no definition).
    {
        let before = damage;
        let (which, uses) = match type_definition {
            Some(t) => (
                get(vm, t, "DamageType")?.map_or(0, |v| integer(&v)),
                get(vm, t, "bUseStatusEffectInstigatorModifiersForDamage")?
                    .is_some_and(|v| truth(vm, &v)),
            ),
            None => (0, true),
        };
        let element = usize::try_from(which)
            .ok()
            .and_then(|i| ELEMENT_MODIFIERS.get(i).copied().flatten());
        if let (Some(inst), Some(element), true) = (instigator, element, uses) {
            damage *= attribute(vm, inst, &format!("Instigated{element}DamageModifier"), 1.0)?;
        }
        store(
            vm,
            &mut summary,
            "InstigatorDamageTypeReduction",
            Value::Float(before - damage),
        );
    }

    // 6 and 7. Hit region lookup, then the critical hit and the region's own damage modifier: the region's
    // DefaultImpactResponse is read as its modifier when the pipeline carries no impact definition (the rule's else
    // branch); the per-material table is not read.
    let region = match victim {
        Some(v) => hit_region(vm, v, &hit_info)?,
        None => None,
    };
    {
        let before = damage;
        if let (Some(inst), Some(r)) = (instigator, region)
            && get(vm, r, "bCriticalHit")?.is_some_and(|v| truth(vm, &v))
            && kind != Some(4)
        {
            damage *= crit_bonus(vm, inst, kind == Some(1))?;
            store(vm, &mut summary, "bWasCrit", Value::Bool(true));
        }
        if let Some(r) = region
            && let Some(response) = get(vm, r, "DefaultImpactResponse")?
            && let Some(percent) = field(vm, &response, "DamageModifierPercent")
        {
            damage *= to_f32(number(&percent));
        }
        store(
            vm,
            &mut summary,
            "HitRegionReduction",
            Value::Float(before - damage),
        );
        if region.is_some() {
            store(vm, &mut summary, "HitRegion", Value::Object(region));
        }
    }

    // 10. Shield: a victim with a valid shield pool is not modelled (said when it would apply).
    if let Some(v) = victim
        && let Some(shield) = get(vm, v, "ShieldArmor")?
        && let Ok(r) = ow_world::pools::read_ref(&vm.space, &shield)
        && ow_world::pools::resolve(&vm.space, r)?.1.is_some()
    {
        note(
            vm,
            "note: AdjustDamage shield stage for a victim with a shield pool is not implemented",
        )?;
    }
    store(vm, &mut summary, "ShieldReduction", Value::Float(0.0));
    store(vm, &mut summary, "DamageDealtToShields", Value::Float(0.0));

    // Intrinsic armor: damage / (1 + 0.01 x the victim's IntrinsicArmor attribute) (section 3.3).
    {
        let before = damage;
        let armor = match victim {
            Some(v) => attribute(vm, v, "IntrinsicArmor", 0.0)?,
            None => 0.0,
        };
        damage /= 1.0 + 0.01 * armor;
        store(
            vm,
            &mut summary,
            "IntrinsicArmorReduction",
            Value::Float(before - damage),
        );
    }

    // 11. Damage surface: the region's DefaultDamageSurfaceType (1 flesh, 2 armor, 3 shield; flesh when the region has
    // none) selects the damage type's modifier, evaluated for the instigator; with a region the victim's resistance of
    // the damage type multiplies too.
    {
        let before = damage;
        let mut surface = 1;
        if let Some(r) = region
            && let Some(t) = get(vm, r, "DefaultDamageSurfaceType")?
            && integer(&t) != 0
        {
            surface = integer(&t);
        }
        let mut modifier = 1.0f32;
        if let Some(t) = type_definition {
            modifier = surface_modifier(vm, t, surface, instigator)?;
            if let (Some(_), Some(v)) = (region, victim) {
                modifier *= resistance(vm, v, t)?;
            }
        }
        damage *= modifier;
        store(
            vm,
            &mut summary,
            "DamageSurfaceReduction",
            Value::Float(before - damage),
        );
        note(
            vm,
            format!(
                "AdjustDamage surface={surface} modifier={}",
                g5(f64::from(modifier))
            ),
        )?;
    }

    // 12 to 14: the hit-region cap, averted death and the protection timer are not implemented; each is said once.
    if let Some(r) = region
        && get(vm, r, "bTrackDamage")?.is_some_and(|v| truth(vm, &v))
    {
        let p = path_of(vm, Some(r));
        note_once(
            vm,
            &format!("bTrackDamage {p}"),
            format!("AdjustDamage hit-region cap (bTrackDamage) is not implemented for {p}"),
        )?;
    }
    if let Some(v) = victim
        && attribute(vm, v, "PlayerAvertDeathChance", 0.0)? > 0.0
    {
        note_once(
            vm,
            "averted",
            "AdjustDamage averted death is not implemented (the victim's PlayerAvertDeathChance is above 0)",
        )?;
    }
    if is_player_controller(vm, victim_mind)? && instigator.is_some() && !player_instigator {
        note_once(
            vm,
            "protection",
            "AdjustDamage protection timer (stage 15: a player victim hit by an AI) is not implemented; it only acts on a hit that would take health below half of the maximum",
        )?;
    }

    // Tail: FinalDamage is what reaches health.
    store(vm, &mut summary, "FinalDamage", Value::Float(damage));
    crate::hit::access::set(vm, pipeline, "DamageSummary", summary.clone())?;
    let initial = field(vm, &summary, "InitialDamage").map_or(0.0, |v| number(&v));
    note(
        vm,
        format!(
            "AdjustDamage initial={} final={}",
            g5(initial),
            g5(f64::from(damage))
        ),
    )?;
    Ok(summary)
}

#[cfg(test)]
mod tests;
