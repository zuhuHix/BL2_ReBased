//! Owner: lane FIX-FIRE-TO-HIT. The held weapon's steps of `--combat-run`: `weapon:<file>` and `fire:<pawn>`.
//! AI-assisted (Claude Code). Decision 2026-11-19-06.
//!
//! The C++ run has neither step (it has the scripted `hit`), so both behave the same in the two parity modes; what
//! differs by mode is the hit chain they feed (`crate::hit`, `crate::damage`).
//!
//! - **`weapon:<file>`**: a weapon file (`ow_items::data::parse_weapon_spec`, the format of `ow-host --weapon`; it
//!   names game objects, so keep it under ignored `local/`; the path is everything after the first `:`) is built with
//!   W4-STATS (`ow_items::stats::instantiate_stats`) in the run's own VM at the file's level, then held
//!   ([`Run::hold_weapon`]). Result: the weapon's path, `damage` (its `InstantHitDamage`), `projectiles`, the fire
//!   mode 0 `source`, `damage_type` and `impact`, the firing mode and its `fire_type`.
//! - **[`Run::hold_weapon`]**: the script's fire mode 0 setup (`crate::hit::shot::initialize_fire_mode`) and the
//!   firing mode (`crate::hit::shot::firing_mode`). Tests hold an invented weapon this way.
//! - **`fire:<pawn>`**: one shot of the held weapon at pawn `<pawn>`: `crate::hit::shot::shot` (its `NotYet`s stop
//!   the step), then the player's hit with the shot's damage, damage class, damage type, impact definition and the
//!   weapon as causer: the `hit` step's chain and bookkeeping, with rows `fire.<class>.L<level>.<n>.arriving`,
//!   `.applied` and `.health_after` counted apart from the `hit` rows. It needs a player (`player:` or `maya:`) and a
//!   held weapon. The shot always reaches the pawn: aim, spread, range and the bullet's flight are not modelled
//!   (`crate::hit::shot`, "Not modelled").

use std::fmt::Write as _;

use ow_attr::api::EvalCx;
use ow_core::DetMap;
use ow_core::fmt::{fmt_g, json_quote};
use ow_items::ItemError;
use ow_items::data::{WeaponSlot, parse_weapon_spec, weapon_definition_data};
use ow_items::stats::instantiate_stats;
use ow_object::ids::ObjRef;
use ow_vm::error::VmError;

use super::Run;
use crate::hit::access::{array, error, get, integer, label_of, number, object_of, path_of};
use crate::hit::shot::{self, FIRE_TYPES, WeaponParts};
use crate::hit::{self};

/// An `ow-items` error as the step's error: a `NotYet` stays one.
fn item_error(e: ItemError) -> VmError {
    match e {
        ItemError::NotYet(n) => VmError::NotYet(n),
        other => error(format!("combat-run: the weapon: {other}")),
    }
}

impl Run {
    /// Holds `weapon` (a `WillowWeapon` instance) built from `parts` as the player's weapon (module documentation).
    pub fn hold_weapon(&mut self, weapon: ObjRef, parts: &WeaponParts) -> Result<(), VmError> {
        let vm = &mut self.world.vm;
        shot::initialize_fire_mode(vm, weapon, parts)?;
        let mode = shot::firing_mode(vm, parts)?;
        self.held = Some((weapon, mode));
        Ok(())
    }

    /// The `weapon:<file>` step (module documentation).
    pub(super) fn weapon_step(&mut self, parts: &[&str]) -> Result<String, VmError> {
        if parts.len() < 2 || parts[1..].join(":").is_empty() {
            return Err(error("combat-run: weapon:<weapon file>"));
        }
        let file = parts[1..].join(":");
        let text = std::fs::read_to_string(&file)
            .map_err(|e| error(format!("combat-run: the weapon file {file}: {e}")))?;
        let spec = parse_weapon_spec(&text).map_err(|e| error(format!("combat-run: {e}")))?;
        let data = weapon_definition_data(&spec).map_err(item_error)?;
        let sys = self.world.vm.attrs.clone();
        let weapon = {
            let mut cx = EvalCx::new(&mut self.world.vm);
            instantiate_stats(&data, spec.level, &sys, &mut cx, &DetMap::new())
                .map_err(item_error)?
                .object
        };
        let vm = &mut self.world.vm;
        let weapon_type = hit::stock_object(vm, data.definition.as_str())?;
        let mut part = |slot: WeaponSlot| -> Result<Option<ObjRef>, VmError> {
            match data.weapon_part(slot) {
                Ok(Some(p)) => Ok(Some(hit::stock_object(vm, p.as_str())?)),
                Ok(None) => Ok(None),
                Err(e) => Err(error(format!("combat-run: the weapon: {e}"))),
            }
        };
        let parts = WeaponParts::new(weapon_type, part(WeaponSlot::Elemental)?, part(WeaponSlot::Barrel)?);
        self.hold_weapon(weapon, &parts)?;
        self.weapon_json(weapon)
    }

    /// The held weapon's report: its numbers and fire mode 0's objects.
    fn weapon_json(&mut self, weapon: ObjRef) -> Result<String, VmError> {
        let mode = self.held.and_then(|(_, m)| m);
        let vm = &mut self.world.vm;
        let damage = get(vm, weapon, "InstantHitDamage")?.map_or(0.0, |v| number(&v));
        let projectiles = get(vm, weapon, "ProjectilesPerShot")?.map_or(0, |v| integer(&v));
        let first = |vm: &mut ow_vm::api::Vm, name: &str| -> Result<String, VmError> {
            let items = array(vm, weapon, name)?;
            Ok(match items.first() {
                Some(ow_object::value::Value::Class(Some(c))) => vm
                    .space
                    .classes
                    .get_layout(*c)
                    .map(|l| l.path().to_string())
                    .unwrap_or_default(),
                Some(v) => path_of(vm, object_of(v)),
                None => String::new(),
            })
        };
        let source = first(vm, "InstantHitDamageTypes")?;
        let damage_type = first(vm, "InstantHitDamageTypeDefinitions")?;
        let impact = first(vm, "InstantHitImpactDefinitions")?;
        let fire_type = match mode {
            Some(m) => {
                let m = crate::hit::access::loaded(vm, m)?;
                let k = get(vm, m, "FireType")?.map_or(0, |v| integer(&v));
                usize::try_from(k)
                    .ok()
                    .and_then(|i| FIRE_TYPES.get(i))
                    .map_or_else(|| k.to_string(), |n| (*n).to_string())
            }
            None => String::new(),
        };
        let mut out = String::new();
        let _ = write!(
            out,
            "{{\"weapon\":{},\"damage\":{},\"projectiles\":{projectiles},\"source\":{},\"damage_type\":{},\
             \"impact\":{},\"firing_mode\":{},\"fire_type\":{}}}",
            json_quote(&label_of(vm, Some(weapon))),
            fmt_g(damage, 6),
            json_quote(&source),
            json_quote(&damage_type),
            json_quote(&impact),
            json_quote(&path_of(vm, mode)),
            json_quote(&fire_type),
        );
        Ok(out)
    }

    /// The `fire:<pawn>` step (module documentation).
    pub(super) fn fire(&mut self, parts: &[&str]) -> Result<String, VmError> {
        if parts.len() != 2 {
            return Err(error("combat-run: fire:<pawn>"));
        }
        if self.players.is_empty() && self.bridge.is_none() {
            return Err(error(
                "combat-run: a shot needs a player (the step player:<level> or maya:<level>) first",
            ));
        }
        let Some((weapon, mode)) = self.held else {
            return Err(error(
                "combat-run: a shot needs a held weapon (the step weapon:<file>) first",
            ));
        };
        let index = self.pawn(parts[1])?;
        let s = shot::shot(&mut self.world.vm, weapon, mode)?;
        self.player_hit(index, s.damage, super::DamageTypeOf::Object(s.damage_type), s.source, s.parts, "fire")
    }
}

#[cfg(test)]
mod tests;
