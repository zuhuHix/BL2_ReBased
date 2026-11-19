//! Owner: lane LINK-2-FIGHT-AREA. Link 2's fight area: a labelled **PORT FIXTURE**, not parity. AI-assisted (Claude
//! Code). Decision 2026-11-19-76.
//!
//! The gate needs enemies where Maya can reach them, and the port has no counterpart of Sanctuary's own population
//! yet (the dens lane is a later seam, decision 2026-11-16-59). The hub's ruling (SLICE_RUN_PLAN.md section 3.2 calls
//! the choice a scheduling one): like the UE5 host's range, the fight area is a fixed spot that H4's scripted walk
//! reaches, with the target dummy placed there by the slice driver. Nothing here is the game's: the spot is chosen by
//! the port, and the dummy is put there by the driver, not by any spawner of the map. Every check is **UNVERIFIED**
//! as a game rule; it checks only that the port's walk and the port's spawn path agree on one place.
//!
//! **The area** ([`FIGHT_AREA`]): a vertical cylinder around a spot, given as an offset from the map's player start
//! (the one the walk starts at), so no map coordinate is written here. The offset is where H4's walk ended on the
//! installed Sanctuary when the fixture was set (about 943 uu along X and -43 along Y, on the start's floor height),
//! rounded; the radius leaves room for the walk's small drift. Distances are horizontal: the area is a place on the
//! floor, and the walk ends standing.
//!
//! **The step** (`fight_area`, after a passing walk):
//! 1. the dummy is placed through `ow_world`'s spawn path ([`ow_world::actors::spawn`]): the stock target dummy
//!    archetype, made from its export in `Sanctuary_Dynamic`, as the template, at the area's centre, spawned by the level's `WorldInfo` in the slice gate's Off
//!    world, before play begins (so no begin-play script runs), with `Spawn`'s `bNoCollisionFail` set, since a
//!    fixture stands where it is put (no free-spot search; that search needs the collision hash the VM world lacks);
//! 2. the link passes when Maya's walked end is within the area (checked first: the dummy is placed only then), and
//!    the spawn made an actor that is in the world's actor list and stands within the area.
//!
//! A refusal of the spawn keeps its kind (a `NotYet` stays a `notyet`); a dummy or a walk end outside the area is an
//! `error` at `fight_area`.

use std::path::Path;

use ow_core::fmt::fmt_f;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_world::actors::SpawnRequest;

use super::{Outcome, Ran, Stop, classify};

/// The fight area: a cylinder around `offset` from the player start, `radius` uu across horizontally. A PORT
/// FIXTURE (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FightArea {
    /// From the player start, uu (X, Y, Z).
    pub offset: [f64; 3],
    /// Horizontal radius, uu.
    pub radius: f64,
}

/// Link 2's fight area (module documentation): the spot H4's walk reaches on the installed Sanctuary, rounded, and
/// room for the walk's drift. PORT FIXTURE, decision 2026-11-19-76; not the game's, UNVERIFIED.
pub const FIGHT_AREA: FightArea = FightArea {
    offset: [950.0, -50.0, 0.0],
    radius: 200.0,
};

/// The package and archetype of the dummy the driver places.
pub const DUMMY_PACKAGE: &str = "Sanctuary_Dynamic";
pub use ow_mission::slice::DUMMY_PAWN;

/// The area's centre for the player start `start`.
pub fn centre(area: &FightArea, start: [f64; 3]) -> [f64; 3] {
    [start[0] + area.offset[0], start[1] + area.offset[1], start[2] + area.offset[2]]
}

/// Horizontal distance between `a` and `b`, uu.
pub fn horizontal(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (dx, dy) = (a[0] - b[0], a[1] - b[1]);
    ow_core::num::sqrt_f64(dx * dx + dy * dy)
}

/// Grades the `fight_area` step (module documentation) from the area's `centre` and Maya's walked `end`, in order:
/// Maya's arrival first, then the dummy, which `place` puts at the centre only once she has arrived (its location, or
/// the spawn's refusal text). Pure but for `place`, so the tests share it.
pub fn graded(
    r: &mut Ran,
    area: &FightArea,
    centre: [f64; 3],
    end: [f64; 3],
    place: impl FnOnce() -> Result<[f64; 3], String>,
) {
    r.steps.push("fight_area".into());
    r.evident = false;
    let outside = |what: &str, d: f64| {
        (
            Outcome::Error,
            Stop {
                step: "fight_area".into(),
                text: format!(
                    "{what} {} uu from the fight area's centre, outside its {} uu radius",
                    fmt_f(d, 1),
                    fmt_f(area.radius, 1)
                ),
                ..Stop::default()
            },
        )
    };
    let maya = horizontal(end, centre);
    r.harness.push(format!(
        "fight area: PORT FIXTURE (decision 2026-11-19-76), radius {} uu; Maya's walk ended {} uu from its \
         centre",
        fmt_f(area.radius, 1),
        fmt_f(maya, 1)
    ));
    if maya.is_nan() || maya > area.radius {
        r.stop = Some(outside("Maya's walk ended", maya));
        return;
    }
    let dummy = match place() {
        Ok(d) => d,
        Err(text) => {
            r.stop = Some(classify("fight_area", &text));
            return;
        }
    };
    let placed = horizontal(dummy, centre);
    r.harness.push(format!("fight area: the placed dummy stands {} uu from its centre", fmt_f(placed, 1)));
    if placed.is_nan() || placed > area.radius {
        r.stop = Some(outside("the placed dummy stands", placed));
        return;
    }
    r.evident = true;
}

/// Places the target dummy at `centre` through `ow_world`'s spawn path in the slice gate's Off world (module
/// documentation); where the made actor stands, or why it was not made.
pub fn place_dummy(cooked: &Path, centre: [f64; 3]) -> Result<[f64; 3], String> {
    let mut world = crate::cmd::inventory::equip_run::gate_world(cooked)?;
    let actors = world.actors();
    let info = actors
        .try_borrow()
        .map_err(|_| "the actor list is borrowed".to_string())?
        .world_info()
        .ok_or("the gate's world has no WorldInfo")?;
    let vm = &mut world.vm;
    // The archetype export of the dummy's own package, made as an object (as the mission slice and `--combat-run`
    // take it): the path alone names another file's object under the store's identity policy.
    let archetype = ow_combat::hit::instantiate_copy(vm, DUMMY_PACKAGE, DUMMY_PAWN)
        .map_err(|e| text(&e))?
        .ok_or_else(|| format!("{DUMMY_PACKAGE} has no {DUMMY_PAWN}"))?;
    let class = vm.space.heap.get(archetype).map_err(|e| e.to_string())?.class;
    let location = vector_at(vm, info, centre)?;
    // `Spawn`'s bNoCollisionFail: the fixture is placed exactly at its chosen spot, so no free-spot search is asked
    // (that search needs a collision hash the VM world does not model).
    let req = SpawnRequest::new(Some(class))
        .with_template(Some(archetype))
        .with_location(location)
        .with_no_collision_fail(true);
    let made = ow_world::actors::spawn(vm, info, &req)
        .map_err(|e| text(&e))?
        .ok_or("the spawn path refused the dummy (no actor made)")?;
    let listed = actors
        .try_borrow()
        .map_err(|_| "the actor list is borrowed".to_string())?
        .list()
        .contains(&made);
    if !listed {
        return Err("the spawned dummy is not in the world's actor list".into());
    }
    location_of(vm, made)
}

fn text(e: &ow_vm::error::VmError) -> String {
    ow_combat::hit::access::error_text(e)
}

/// `info`'s `Location` value (a vector of the world's own type) set to `at`.
fn vector_at(vm: &mut Vm, info: ObjRef, at: [f64; 3]) -> Result<Value, String> {
    let mut v = ow_combat::hit::access::get(vm, info, "Location")
        .map_err(|e| text(&e))?
        .ok_or("WorldInfo has no Location")?;
    for (name, x) in ["X", "Y", "Z"].into_iter().zip(at) {
        // N5: a coordinate as the script's float.
        if !ow_combat::hit::access::set_field(vm, &mut v, name, Value::Float(ow_combat::hit::access::to_f32(x))) {
            return Err(format!("a Location has no {name}"));
        }
    }
    Ok(v)
}

/// Where actor `o` stands (its `Location`).
fn location_of(vm: &mut Vm, o: ObjRef) -> Result<[f64; 3], String> {
    let v = ow_combat::hit::access::get(vm, o, "Location")
        .map_err(|e| text(&e))?
        .ok_or("the dummy has no Location")?;
    let mut out = [0.0; 3];
    for (slot, name) in out.iter_mut().zip(["X", "Y", "Z"]) {
        match ow_combat::hit::access::field(vm, &v, name) {
            Some(Value::Float(x)) => *slot = f64::from(x),
            other => return Err(format!("the dummy's Location.{name} is {other:?}")),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
