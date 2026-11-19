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
//! **The step** (`fight_area`, after a passing walk), in order:
//! 1. Maya's walked end must be within the area (horizontally). A miss is an `error`, and nothing is placed.
//! 2. The placement is checked against the loaded Sanctuary collision before anything is spawned (review r1 of #470):
//!    - the dummy's collision cylinder is read from its archetype (`CylinderComponent`: `CollisionRadius` and
//!      `CollisionHeight`); where it cannot be read, the placement checks are not available and the step is a
//!      `NotYet`;
//!    - the floor: the dummy's cylinder is swept down from [`FightArea::floor_above`] uu above the centre to
//!      [`FightArea::floor_below`] uu below it, and must land on a floor Maya could walk on (its normal's Z at least
//!      her `WalkableFloorZ`), from a start that is not in solid, and must not stand in solid where it lands;
//!    - Maya's reach: her feet and the dummy's must be within her `MaxStepHeight` of each other (the same floor, not
//!      one below or above), and her own cylinder, raised by that step height, must sweep from her walked end to the
//!      dummy's spot without a hit. That straight sweep is a conservative check of the port's own, not a path search;
//!    - each miss is an `error` at `fight_area`.
//! 3. The dummy is then placed through `ow_world`'s spawn path ([`ow_world::actors::spawn`]): the stock target dummy
//!    archetype, made from its export in `Sanctuary_Dynamic`, as the template, at the floor spot found in step 2,
//!    spawned by the level's `WorldInfo` in the slice gate's Off world, before play begins (so no begin-play script
//!    runs), with `Spawn`'s `bNoCollisionFail` set, since a fixture stands where it is put (the VM world has no
//!    collision hash for the free-spot search; step 2 is the port's check of the spot instead);
//! 4. the link passes when the spawn made an actor that is in the world's actor list and stands where it was put,
//!    inside the area.
//!
//! A refusal of the spawn keeps its kind by its typed error (a `VmError::NotYet` stays a `notyet`; any other error is
//! an `error`, whatever its text says).

use std::path::Path;

use ow_assets::movement::{CollisionWorld, MovementParams, Shape};
use ow_core::NotYet;
use ow_core::fmt::fmt_f;
use ow_object::ids::ObjRef;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;
use ow_world::actors::SpawnRequest;

use super::{Outcome, Ran, Stop, typed_stop};

/// The record of the fight area.
pub const RECORD: &str = "docs/decisions/2026-11/2026-11-19-76-rust-slice-gate-link-2-fight-area-as-a-port-fixture.md";

/// The fight area: a cylinder around `offset` from the player start, `radius` uu across horizontally. A PORT
/// FIXTURE (module documentation).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FightArea {
    /// From the player start, uu (X, Y, Z).
    pub offset: [f64; 3],
    /// Horizontal radius, uu.
    pub radius: f64,
    /// How far above the centre the floor search starts, and how far below it ends, uu.
    pub floor_above: f64,
    pub floor_below: f64,
}

/// Link 2's fight area (module documentation): the spot H4's walk reaches on the installed Sanctuary, rounded, and
/// room for the walk's drift; the floor is searched from the pawn's full height above the centre
/// ([`ow_assets::movement::mover::SPAWN_LIFT`]) down to the walk's own fall limit
/// ([`ow_assets::movement::walk::MAX_DROP`]). PORT FIXTURE, decision 2026-11-19-76; not the game's, UNVERIFIED.
pub const FIGHT_AREA: FightArea = FightArea {
    offset: [950.0, -50.0, 0.0],
    radius: 200.0,
    floor_above: ow_assets::movement::mover::SPAWN_LIFT,
    floor_below: ow_assets::movement::walk::MAX_DROP,
};

/// Why the placement failed: a typed VM error (graded by its variant, [`typed_stop`]) or a check of the step's own
/// (an `error`).
#[derive(Debug)]
pub enum Failed {
    Vm(VmError),
    Check(String),
}

impl From<VmError> for Failed {
    fn from(e: VmError) -> Failed {
        Failed::Vm(e)
    }
}

/// The stop for `f` at `fight_area`: a `VmError::NotYet` is `notyet`, every other VM error and every failed check an
/// `error` (review r1 of #470: by the typed variant, never by the text).
pub fn stop_of(f: &Failed) -> (Outcome, Stop) {
    match f {
        Failed::Vm(e) => typed_stop("fight_area", e),
        Failed::Check(text) => (Outcome::Error, Stop { step: "fight_area".into(), text: text.clone(), ..Stop::default() }),
    }
}

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
/// Maya's arrival first, then the dummy, which `place` checks and puts at the centre only once she has arrived (where
/// it stands, or why it was not placed, typed). Pure but for `place`, so the tests share it.
pub fn graded(
    r: &mut Ran,
    area: &FightArea,
    centre: [f64; 3],
    end: [f64; 3],
    place: impl FnOnce() -> Result<[f64; 3], Failed>,
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
        Err(f) => {
            r.stop = Some(stop_of(&f));
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

/// Checks the placement against `collision` (the loaded Sanctuary) and places the target dummy at the floor under
/// `centre` through `ow_world`'s spawn path in the slice gate's Off world (module documentation); where the made actor
/// stands, or why it was not placed. `maya` is Maya's walked end.
pub fn place_dummy(
    cooked: &Path,
    collision: &CollisionWorld,
    area: &FightArea,
    centre: [f64; 3],
    maya: [f64; 3],
) -> Result<[f64; 3], Failed> {
    let mut world = crate::cmd::inventory::equip_run::gate_world(cooked).map_err(Failed::Check)?;
    let actors = world.actors();
    let info = actors
        .try_borrow()
        .map_err(|_| Failed::Check("the actor list is borrowed".into()))?
        .world_info()
        .ok_or_else(|| Failed::Check("the gate's world has no WorldInfo".into()))?;
    let vm = &mut world.vm;
    // The location vector is built, and its fields checked, before the template is made (review r1 of #470).
    vector_at(vm, info, centre)?;
    // The archetype export of the dummy's own package, made as an object (as the mission slice and `--combat-run`
    // take it): the path alone names another file's object under the store's identity policy.
    let archetype = ow_combat::hit::instantiate_copy(vm, DUMMY_PACKAGE, DUMMY_PAWN)?
        .ok_or_else(|| Failed::Check(format!("{DUMMY_PACKAGE} has no {DUMMY_PAWN}")))?;
    let class = vm.space.heap.get(archetype).map_err(VmError::from)?.class;
    let shape = dummy_shape(vm, archetype)?;
    let spot = floor_spot(collision, area, &MovementParams::maya(), shape, centre)?;
    reachable(collision, &MovementParams::maya(), maya, shape, spot)?;
    let location = vector_at(vm, info, spot)?;
    // `Spawn`'s bNoCollisionFail: the fixture is placed exactly at the spot checked above, so no free-spot search is
    // asked (that search needs a collision hash the VM world does not model).
    let req = SpawnRequest::new(Some(class))
        .with_template(Some(archetype))
        .with_location(location)
        .with_no_collision_fail(true);
    let made = ow_world::actors::spawn(vm, info, &req)?
        .ok_or_else(|| Failed::Check("the spawn path refused the dummy (no actor made)".into()))?;
    let listed = actors
        .try_borrow()
        .map_err(|_| Failed::Check("the actor list is borrowed".into()))?
        .list()
        .contains(&made);
    if !listed {
        return Err(Failed::Check("the spawned dummy is not in the world's actor list".into()));
    }
    let at = location_of(vm, made)?;
    let off = distance(at, spot);
    // The spawn writes the location as the script's float, so the spot comes back to within a float's rounding.
    if off.is_nan() || off > 1.0 {
        return Err(Failed::Check(format!(
            "the spawned dummy stands {} uu from the spot it was placed at",
            fmt_f(off, 1)
        )));
    }
    Ok(at)
}

/// The dummy's collision cylinder, from its archetype's `CylinderComponent` (`CollisionRadius`, `CollisionHeight`, a
/// half height). Where it cannot be read, the placement checks are not available: a `NotYet`.
pub fn dummy_shape(vm: &mut Vm, archetype: ObjRef) -> Result<Shape, Failed> {
    let unreadable = |what: &str| -> Failed {
        Failed::Vm(
            NotYet::not_read(
                format!("fight area: the dummy's collision cylinder ({what}), which the placement checks need"),
                RECORD,
            )
            .into(),
        )
    };
    let component = match ow_combat::hit::access::get(vm, archetype, "CylinderComponent")? {
        Some(Value::Object(Some(c))) => c,
        _ => return Err(unreadable("no CylinderComponent")),
    };
    let mut read = |name: &str| -> Result<f64, Failed> {
        match ow_combat::hit::access::get(vm, component, name)? {
            Some(Value::Float(x)) if x.is_finite() && x > 0.0 => Ok(f64::from(x)),
            _ => Err(unreadable(name)),
        }
    };
    let radius = read("CollisionRadius")?;
    let half_height = read("CollisionHeight")?;
    Ok(Shape::Cylinder { radius, half_height })
}

/// Where a pawn of `shape` stands on the floor under `centre` in `world` (module documentation, step 2): swept down
/// from `area.floor_above` above the centre to `area.floor_below` below it; the floor must be one `walker` could walk
/// on, and the spot must not be in solid.
pub fn floor_spot(
    world: &CollisionWorld,
    area: &FightArea,
    walker: &MovementParams,
    shape: Shape,
    centre: [f64; 3],
) -> Result<[f64; 3], Failed> {
    if !centre.iter().all(|v| v.is_finite()) {
        return Err(Failed::Check("the fight area's centre is not a finite position".into()));
    }
    let start = [centre[0], centre[1], centre[2] + area.floor_above];
    let end = [centre[0], centre[1], centre[2] - area.floor_below];
    let hit = world.trace(&shape, start, end);
    if hit.start_solid {
        return Err(Failed::Check(format!(
            "the floor search for the dummy starts in solid, {} uu above the fight area's centre",
            fmt_f(area.floor_above, 1)
        )));
    }
    if !hit.blocked() {
        return Err(Failed::Check(format!(
            "no floor under the fight area's centre within {} uu below it",
            fmt_f(area.floor_below, 1)
        )));
    }
    if hit.normal[2].is_nan() || hit.normal[2] < walker.walkable_floor_z {
        return Err(Failed::Check(format!(
            "the floor under the fight area's centre is not walkable (its normal's Z is {}, below {})",
            fmt_f(hit.normal[2], 3),
            fmt_f(walker.walkable_floor_z, 3)
        )));
    }
    if world.trace(&shape, hit.end, hit.end).start_solid {
        return Err(Failed::Check("the dummy would stand in solid on the floor under the fight area's centre".into()));
    }
    Ok(hit.end)
}

/// Whether `walker`, ending its walk at `from` (its shape's centre), can reach a pawn of `shape` standing at `to`
/// (module documentation, step 2): the feet within the walker's step height of each other, and the walker's shape,
/// raised by that step height, sweeping from `from` to above `to` without a hit.
pub fn reachable(
    world: &CollisionWorld,
    walker: &MovementParams,
    from: [f64; 3],
    shape: Shape,
    to: [f64; 3],
) -> Result<(), Failed> {
    if !from.iter().chain(&to).all(|v| v.is_finite()) {
        return Err(Failed::Check("Maya's walked end or the dummy's spot is not a finite position".into()));
    }
    let step = walker.max_step_height;
    let feet = (from[2] - walker.shape.half_height()) - (to[2] - shape.half_height());
    if feet.is_nan() || feet.abs() > step {
        return Err(Failed::Check(format!(
            "Maya's feet end {} uu from the dummy's floor height, more than her {} uu step: not the same floor",
            fmt_f(feet.abs(), 1),
            fmt_f(step, 1)
        )));
    }
    let a = [from[0], from[1], from[2] + step];
    let b = [to[0], to[1], from[2] + step];
    let hit = world.trace(&walker.shape, a, b);
    if hit.start_solid || hit.blocked() {
        return Err(Failed::Check(format!(
            "no straight way from Maya's walked end to the dummy: her sweep stops at {} of the way",
            fmt_f(hit.fraction, 3)
        )));
    }
    Ok(())
}

/// Distance between `a` and `b`, uu.
fn distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let (dx, dy, dz) = (a[0] - b[0], a[1] - b[1], a[2] - b[2]);
    ow_core::num::sqrt_f64(dx * dx + dy * dy + dz * dz)
}

/// `info`'s `Location` value (a vector of the world's own type) set to `at`.
fn vector_at(vm: &mut Vm, info: ObjRef, at: [f64; 3]) -> Result<Value, Failed> {
    let mut v = ow_combat::hit::access::get(vm, info, "Location")?
        .ok_or_else(|| Failed::Check("WorldInfo has no Location".into()))?;
    for (name, x) in ["X", "Y", "Z"].into_iter().zip(at) {
        // N5: a coordinate as the script's float.
        if !ow_combat::hit::access::set_field(vm, &mut v, name, Value::Float(ow_combat::hit::access::to_f32(x))) {
            return Err(Failed::Check(format!("a Location has no {name}")));
        }
    }
    Ok(v)
}

/// Where actor `o` stands (its `Location`).
fn location_of(vm: &mut Vm, o: ObjRef) -> Result<[f64; 3], Failed> {
    let v = ow_combat::hit::access::get(vm, o, "Location")?
        .ok_or_else(|| Failed::Check("the dummy has no Location".into()))?;
    let mut out = [0.0; 3];
    for (slot, name) in out.iter_mut().zip(["X", "Y", "Z"]) {
        match ow_combat::hit::access::field(vm, &v, name) {
            Some(Value::Float(x)) => *slot = f64::from(x),
            other => return Err(Failed::Check(format!("the dummy's Location.{name} is {other:?}"))),
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests;
