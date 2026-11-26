//! Owner: lane IMPL-LINKS-3-4-TAIL (decision 2026-11-26-NNN). `AdjustDamage`'s hit-region cap (stage 13): the
//! hit-region consumer's `AddDamageToHitRegion`, in `ParityMode::Off`. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_LINKS_3_4_TAIL.md section 4 (#812 as landed) alone; this
//! lane did not read the executable. **UNVERIFIED in the running game**; the owner's health pair is by fit (the
//! pipeline's own `GetHealth` / `GetMaxHealth` reads).
//!
//! [`cap`] is a plan and a commit. **The plan** reads only, and makes every refusal before anything is written:
//! - the region resident, `bTrackDamage` clear: nothing;
//! - the victim a `WillowPawn` (else [`CONSUMER`] `NotRead`); `MyHitRegionHelper` None: nothing; the helper resident;
//! - every field read with its declared kind, else `VmError::Script`; a region `BehaviorProviderDefinition` resident;
//! - a `SeqEvent_TakeHitRegionDamage` among the victim's `GeneratedEvents`: [`KISMET`];
//! - the region's share `M` (the maximum health, times `HitRegionHealthScaleAttribute` with
//!   `bScaleHealthWithAttribute`, times `OwnerHealthPercent` when independent, else times a positive
//!   `PercentOfMaxHealthLimit`), the record's remainder, the spent-region, limit and allowed-type rules on `D`;
//! - with `M` above 0: a damage reaction that would fire, or the region's death with `RegionDeathBehaviors` or a
//!   provider holding `BehaviorSequences`: [`BEHAVIORS`] (they would run behaviours this port does not wire here). A
//!   reaction whose other conditions hold is refused whatever its `DamageTriggers` say (not read here);
//! - an independent region with an instigator and `bShowDamageNumbers`: [`DAMAGE_NUMBERS`].
//!
//! **The commit** writes the helper's `HitRegionDamageList` once: the record (made with its two fraction arrays sized
//! to the region's reaction lists, or found), its `Damage` and `RecentDamage` (with `M` above 0), and on the region's
//! death `Damage` := `M` and `bHasGored`. It answers the capped `D` (0 for an independent region); the pipeline sets
//! `HitRegionCapReduction` from it and goes on with `D`.

use ow_core::NotYet;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::combat_music::{bad, is_a, member, object, path, read};
use crate::links34_tail::{boolean, float, resident};

/// The record of the rule.
pub const RECORD: &str = "docs/verification/NATIVE_LINKS_3_4_TAIL.md (section 4)";
pub const CONSUMER: &str = "AddDamageToHitRegion: the hit-region consumer of a class other than WillowPawn";
pub const KISMET: &str = "AddDamageToHitRegion: the hit-region Kismet event (SeqEvent_TakeHitRegionDamage)";
pub const BEHAVIORS: &str = "AddDamageToHitRegion: the hit region's behaviours";
pub const DAMAGE_NUMBERS: &str = "AddDamageToHitRegion: the hit region's damage-number notification";
const WHAT: &str = "HitRegionHelper.AddDamageToHitRegion";
const KISMET_CLASS: &str = "WillowGame.SeqEvent_TakeHitRegionDamage";

fn objects(vm: &Vm, o: ObjRef, name: &str) -> Result<Vec<Option<ObjRef>>, VmError> {
    match read(vm, o, name, WHAT)? {
        (Value::Array(items), PropKind::Array(inner)) if matches!(*inner, PropKind::Object { .. }) => items
            .iter()
            .map(|v| match v {
                Value::Object(x) => Ok(*x),
                other => Err(bad(WHAT, format!("{name} holds {}", other.kind_name()))),
            })
            .collect(),
        (_, kind) => Err(bad(WHAT, format!("{name} is declared as {kind:?}, not an array of objects"))),
    }
}

fn structs(vm: &Vm, o: ObjRef, name: &str) -> Result<(Vec<Value>, ow_object::ids::StructId), VmError> {
    match read(vm, o, name, WHAT)? {
        (Value::Array(items), PropKind::Array(inner)) => match *inner {
            PropKind::Struct(id) => Ok(((*items).clone(), id)),
            kind => Err(bad(WHAT, format!("{name} is declared as an array of {kind:?}, not of structs"))),
        },
        (_, kind) => Err(bad(WHAT, format!("{name} is declared as {kind:?}, not an array of structs"))),
    }
}

fn m_float(vm: &Vm, v: &Value, name: &str) -> Result<f32, VmError> {
    match member(vm, v, name, WHAT)? {
        (Value::Float(f), PropKind::Float) => Ok(f),
        (_, kind) => Err(bad(WHAT, format!("{name} is declared as {kind:?}, not a float"))),
    }
}

fn m_bool(vm: &Vm, v: &Value, name: &str) -> Result<bool, VmError> {
    match member(vm, v, name, WHAT)? {
        (Value::Bool(b), PropKind::Bool) => Ok(b),
        (_, kind) => Err(bad(WHAT, format!("{name} is declared as {kind:?}, not a bool"))),
    }
}

fn m_floats(vm: &Vm, v: &Value, name: &str) -> Result<Vec<f32>, VmError> {
    match member(vm, v, name, WHAT)? {
        (Value::Array(items), PropKind::Array(inner)) if matches!(*inner, PropKind::Float) => {
            Ok(items.iter().map(|x| if let Value::Float(f) = x { *f } else { 0.0 }).collect())
        }
        (_, kind) => Err(bad(WHAT, format!("{name} is declared as {kind:?}, not an array of floats"))),
    }
}

fn m_array_len(vm: &Vm, v: &Value, name: &str) -> Result<usize, VmError> {
    match member(vm, v, name, WHAT)? {
        (Value::Array(items), PropKind::Array(_)) => Ok(items.len()),
        (_, kind) => Err(bad(WHAT, format!("{name} is declared as {kind:?}, not an array"))),
    }
}

/// `v` with field `name` set to `x` (its declared slot).
fn with_field(vm: &Vm, v: &mut Value, name: &str, x: Value) -> Result<(), VmError> {
    let id = vm.space.names.find(name).ok_or_else(|| bad(WHAT, format!("no {name}")))?;
    let s = v.struct_mut().ok_or_else(|| bad(WHAT, "a record is not a struct"))?;
    let (i, _) = vm.space.classes.struct_layout(s.ty).and_then(|l| l.field(id)).ok_or_else(|| bad(WHAT, format!("no {name}")))?;
    s.fields[i as usize] = x;
    Ok(())
}

/// The region's numbers the plan reads.
struct Region {
    scale_health: bool,
    independent: bool,
    owner_percent: f32,
    limit: f32,
    allowed: Vec<Option<ObjRef>>,
    reactions: Vec<Value>,
    healing_len: usize,
    death_behaviors: usize,
    provider: Option<ObjRef>,
    show_numbers: bool,
}

fn region(vm: &Vm, r: ObjRef) -> Result<Region, VmError> {
    let (reactions, _) = structs(vm, r, "DamageTriggeredReactions")?;
    let (healing, _) = structs(vm, r, "HealingTriggeredReactions")?;
    // Only whether it is empty matters here (a non-empty list on the region's death refuses).
    let death = match read(vm, r, "RegionDeathBehaviors", WHAT)? {
        (Value::Array(items), PropKind::Array(_)) => items.len(),
        (_, kind) => return Err(bad(WHAT, format!("RegionDeathBehaviors is declared as {kind:?}, not an array"))),
    };
    Ok(Region {
        scale_health: boolean(vm, r, "bScaleHealthWithAttribute", WHAT)?,
        independent: boolean(vm, r, "bUseIndependentHealth", WHAT)?,
        owner_percent: float(vm, r, "OwnerHealthPercent", WHAT)?,
        limit: float(vm, r, "PercentOfMaxHealthLimit", WHAT)?,
        allowed: objects(vm, r, "AllowedDamageTypes")?,
        reactions,
        healing_len: healing.len(),
        death_behaviors: death,
        provider: object(vm, r, "BehaviorProviderDefinition", WHAT)?,
        show_numbers: boolean(vm, r, "bShowDamageNumbers", WHAT)?,
    })
}

/// What the commit writes.
struct Plan {
    helper: ObjRef,
    list: Vec<Value>,
    d: f32,
}

/// The plan (module documentation). Reads only; `Ok(None)` is "nothing" (`D` unchanged, nothing written).
#[allow(clippy::too_many_arguments)]
fn plan(
    vm: &Vm,
    victim: ObjRef,
    r: ObjRef,
    instigator: Option<ObjRef>,
    type_definition: Option<ObjRef>,
    d: f32,
    health: f32,
    max_health: f32,
) -> Result<Option<Plan>, VmError> {
    resident(vm, r)?;
    if !boolean(vm, r, "bTrackDamage", WHAT)? {
        return Ok(None);
    }
    resident(vm, victim)?;
    if !is_a(vm, victim, "WillowGame.WillowPawn")? {
        return Err(NotYet::not_read(format!("{CONSUMER} ({})", path(vm, victim)), RECORD).into());
    }
    let Some(helper) = object(vm, victim, "MyHitRegionHelper", WHAT)? else { return Ok(None) };
    resident(vm, helper)?;
    let reg = region(vm, r)?;
    if let Some(p) = reg.provider {
        resident(vm, p)?;
    }
    // The Kismet event, declared on the victim (`GeneratedEvents`).
    for e in objects(vm, victim, "GeneratedEvents")?.into_iter().flatten() {
        if is_a(vm, e, KISMET_CLASS)? {
            return Err(NotYet::not_read(KISMET, RECORD).into());
        }
    }
    // The record.
    let (mut list, record_ty) = structs(vm, helper, "HitRegionDamageList")?;
    let mut found = None;
    for (i, entry) in list.iter().enumerate() {
        match member(vm, entry, "BodyHitRegion", WHAT)? {
            (Value::Object(Some(o)), PropKind::Object { .. }) if o == r => {
                found = Some(i);
                break;
            }
            (Value::Object(_), PropKind::Object { .. }) => {}
            _ => return Err(bad(WHAT, "BodyHitRegion is not declared as an object")),
        }
    }
    let at = match found {
        Some(i) => i,
        None => {
            let mut fresh = vm.space.classes.default_value(&PropKind::Struct(record_ty)).map_err(|e| bad(WHAT, format!("a new record: {e}")))?;
            with_field(vm, &mut fresh, "BodyHitRegion", Value::Object(Some(r)))?;
            let zeros = |n: usize| Value::array(vec![Value::Float(0.0); n]);
            with_field(vm, &mut fresh, "LastPctHealthForRunDamageTriggeredBehaviors", zeros(reg.reactions.len()))?;
            with_field(vm, &mut fresh, "LastPctHealthForRunHealingTriggeredBehaviors", zeros(reg.healing_len))?;
            list.push(fresh);
            list.len() - 1
        }
    };
    let record = list[at].clone();
    let damage0 = m_float(vm, &record, "Damage")?;
    let recent0 = m_float(vm, &record, "RecentDamage")?;
    let gored = m_bool(vm, &record, "bHasGored")?;
    let last = m_floats(vm, &record, "LastPctHealthForRunDamageTriggeredBehaviors")?;
    m_floats(vm, &record, "LastPctHealthForRunHealingTriggeredBehaviors")?;
    // 3. The region's share.
    let mut m = max_health;
    if reg.scale_health {
        let (v, kind) = read(vm, victim, "HitRegionHealthScaleAttribute", WHAT)?;
        match (v, kind) {
            (Value::Float(s), PropKind::Float | PropKind::FloatAttribute) => m *= s,
            (_, kind) => return Err(bad(WHAT, format!("HitRegionHealthScaleAttribute is declared as {kind:?}, not a float"))),
        }
    }
    if reg.independent {
        m *= reg.owner_percent;
    } else if reg.limit > 0.0 {
        m *= reg.limit;
    }
    // 4 to 7.
    let mut left = m - damage0;
    let unlimited = !reg.independent && reg.limit == 0.0;
    let mut d = d;
    if gored && recent0 == 0.0 && !unlimited {
        d = 0.0;
    }
    if !reg.independent && m > 0.0 && reg.limit > 0.0 {
        if left > 0.01 {
            if left - d <= 0.01 {
                d = left;
            }
        } else {
            d = 0.0;
        }
    }
    if !reg.allowed.is_empty() && !reg.allowed.contains(&type_definition) {
        d = 0.0;
    }
    // 9. The bookkeeping, the reactions and the region's death.
    if m > 0.0 {
        let before = 1.0 - left / m;
        let damage1 = damage0 + d;
        let recent1 = recent0 + d;
        left -= d;
        let after = 1.0 - left / m;
        let dies = health - d < 0.01;
        for (i, reaction) in reg.reactions.iter().enumerate() {
            let only_death = m_bool(vm, reaction, "bOnlyTriggeredOnOwnerDeath")?;
            let trigger = m_float(vm, reaction, "PercentOfHealthTrigger")?;
            let threshold = m_bool(vm, reaction, "bDamageThreshold")?;
            let repeatable = m_bool(vm, reaction, "bRepeatableEvent")?;
            m_array_len(vm, reaction, "DamageTriggers")?;
            if only_death && !dies {
                continue;
            }
            let previous = last.get(i).copied().unwrap_or(0.0);
            if trigger <= after && (before < trigger || !threshold) && (previous == 0.0 || repeatable) {
                return Err(NotYet::not_read(format!("{BEHAVIORS} (a damage reaction that would fire)"), RECORD).into());
            }
        }
        let mut entry = record;
        with_field(vm, &mut entry, "Damage", Value::Float(damage1))?;
        with_field(vm, &mut entry, "RecentDamage", Value::Float(recent1))?;
        if left <= 0.001 && !gored {
            if reg.death_behaviors > 0 {
                return Err(NotYet::not_read(format!("{BEHAVIORS} (RegionDeathBehaviors)"), RECORD).into());
            }
            if let Some(p) = reg.provider {
                let (sequences, kind) = read(vm, p, "BehaviorSequences", WHAT)?;
                match (sequences, kind) {
                    (Value::Array(items), PropKind::Array(_)) if !items.is_empty() => {
                        return Err(NotYet::not_read(format!("{BEHAVIORS} (OnRegionDeath's sequences)"), RECORD).into());
                    }
                    (Value::Array(_), PropKind::Array(_)) => {}
                    (_, kind) => return Err(bad(WHAT, format!("BehaviorSequences is declared as {kind:?}, not an array"))),
                }
            }
            with_field(vm, &mut entry, "Damage", Value::Float(m))?;
            with_field(vm, &mut entry, "bHasGored", Value::Bool(true))?;
        }
        list[at] = entry;
    }
    // 10. An independent region.
    if reg.independent {
        if instigator.is_some() && reg.show_numbers {
            return Err(NotYet::not_read(DAMAGE_NUMBERS, RECORD).into());
        }
        d = 0.0;
    }
    Ok(Some(Plan { helper, list, d }))
}

/// Stage 13 for a hit on `victim` in region `r` (module documentation): the capped damage. Off only (the caller's).
#[allow(clippy::too_many_arguments)]
pub fn cap(
    vm: &mut Vm,
    victim: ObjRef,
    r: ObjRef,
    instigator: Option<ObjRef>,
    type_definition: Option<ObjRef>,
    d: f32,
    health: f32,
    max_health: f32,
) -> Result<f32, VmError> {
    let Some(p) = plan(vm, victim, r, instigator, type_definition, d, health, max_health)? else { return Ok(d) };
    let class = vm.space.heap.get(p.helper)?.class;
    let prop = vm.space.prop(class, "HitRegionDamageList").ok_or_else(|| bad(WHAT, "no HitRegionDamageList"))?;
    vm.space.write(p.helper, PropAddr::scalar(prop), Value::array(p.list))?;
    Ok(p.d)
}

#[cfg(test)]
#[path = "hit_region_cap_tests.rs"]
mod tests;
