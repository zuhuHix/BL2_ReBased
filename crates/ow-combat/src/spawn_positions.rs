//! Owner: lane IMPL-COMBAT-WORLD-COLLISION. The combat run's spawn positions, an Off-only harness setting (the hub's
//! ruling on the collision lane's open question). AI-assisted (Claude Code).
//!
//! In the game a spawned enemy stands at a population spawn point. The port's combat run has no spawn points, so
//! every pawn it spawns would stand at the origin, where Maya stands, and `SetPhysics`' floor trace would start inside
//! her collision cylinder. With [`SpawnPositions`] on, `crate::hit::spawn_enemy` writes the n-th spawned pawn's
//! `Location` as (n x [`SPACING`], 0, 0), n counted from 1, before anything else of the spawn runs, and logs it. The
//! spacing is far larger than any collision cylinder the combat run's pawns carry (the largest seen in the slice's
//! combat world has radius 150 and half-height 90; Maya's has radius 42 and half-height 80), so the 8-unit floor
//! trace of each spawn starts clear of every other actor's cylinder. UNVERIFIED: a stand-in for the spawn points, not
//! a rule of the game.

use ow_core::ParityMode;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::hit::access::{class_name, error, get, set, set_field};

/// The distance between two spawn positions, along X (module documentation).
pub const SPACING: f32 = 1000.0;

/// The set-up line of [`enable_spawn_positions`] (the hub's text, with the spacing built from [`SPACING`]; review of
/// #813, LOW 5).
pub fn setup_line() -> String {
    format!(
        "harness input: the combat run's spawn positions, apart from every other actor (stand-in for the population \
         spawn points; UNVERIFIED): the n-th spawned pawn at ({SPACING} n, 0, 0), Maya left where the set-up puts her"
    )
}

/// The registration key of [`SpawnPositions`].
pub const KEY: &str = "SpawnPositions";

/// The Off-only harness setting (a VM service): how many pawns it has placed.
#[derive(Debug, Default)]
pub struct SpawnPositions {
    placed: u32,
}

/// Turns the spawn positions on for `vm`'s world (world set-up), logging [`setup_line`]. Off only, else a registration
/// refusal; turning it on twice is a registration refusal too.
pub fn enable_spawn_positions(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Err(VmError::Registration {
            key: KEY.to_string(),
            reason: "the combat run's spawn positions are an Off harness setting",
        });
    }
    vm.add_service(SpawnPositions::default())?;
    vm.log.push(setup_line());
    Ok(())
}

/// The line logged for one placed pawn.
pub fn placed_line(class: &str, x: f32) -> String {
    format!("harness input: the combat run's spawn position of a {class}: ({x}, 0, 0)")
}

/// Places `pawn`, just spawned, at the next position when the setting is on in Off; nothing otherwise. Answers the
/// position it wrote.
pub fn place(vm: &mut Vm, pawn: ObjRef) -> Result<Option<[f32; 3]>, VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(None);
    }
    let Some(service) = vm.service::<SpawnPositions>() else { return Ok(None) };
    // The mutable borrow is taken before anything is written and held across the write, so a borrowed service refuses
    // with nothing written; the counter moves only once the write succeeded (reviews of #807, LOW 3, and #813, item 1).
    let mut s = service.try_borrow_mut().map_err(|_| VmError::ServiceBorrowed("SpawnPositions"))?;
    let n = s.placed.checked_add(1).ok_or_else(|| error("combat: too many spawn positions"))?;
    let x = SPACING * n as f32;
    let Some(mut location) = get(vm, pawn, "Location")? else {
        return Err(error(format!("combat: the spawned {} declares no Location", class_name(vm, pawn))));
    };
    for (name, v) in [("X", x), ("Y", 0.0), ("Z", 0.0)] {
        if !set_field(vm, &mut location, name, Value::Float(v)) {
            return Err(error(format!("combat: the spawned pawn's Location has no float {name}")));
        }
    }
    set(vm, pawn, "Location", location)?;
    s.placed = n;
    drop(s);
    let line = placed_line(&class_name(vm, pawn), x);
    vm.log.push(line);
    Ok(Some([x, 0.0, 0.0]))
}

#[cfg(test)]
#[path = "spawn_positions_tests.rs"]
mod tests;
