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
//! 2. The placement is checked against the loaded Sanctuary collision before anything is made or spawned (reviews r1
//!    and r2 of #470), so a refused placement writes nothing:
//!    - the dummy's collision cylinder is read from the package, not from a made object: the archetype export's own
//!      `CylinderComponent` reference and that component export's own `CollisionRadius` and `CollisionHeight` tags
//!      ([`dummy_shape`]); where they cannot be read (no such export, a reference out of the file, an untagged
//!      value), the placement checks are not available and the step is a `NotYet`;
//!    - the floor: the dummy's cylinder is swept down from [`FightArea::floor_above`] uu above the centre to
//!      [`FightArea::floor_below`] uu below it, and must land on a floor Maya could walk on (its normal's Z at least
//!      her `WalkableFloorZ`), from a start that is not in solid, and must not stand in solid where it lands;
//!    - Maya's reach: her feet and the dummy's must be within her `MaxStepHeight` of each other (the same floor, not
//!      one below or above), and she must walk there on the ground: the port's own mover ([`Mover`]) walks her from her
//!      walked end straight at the dummy's spot, steering each tick, and she must arrive within [`ARRIVE`] uu without
//!      ever dropping more than her step height below where she started (a gap or a ledge on the way) and within a
//!      tick budget (a wall or a corner she cannot pass). A straight walk is a conservative check of the port's own,
//!      not a path search (review r2 of #470: a raised straight sweep passed two platforms across a gap);
//!    - each miss is an `error` at `fight_area`.
//! 3. The dummy is then placed through `ow_world`'s spawn path ([`ow_world::actors::spawn`]): the stock target dummy
//!    archetype, made from its export in `Sanctuary_Dynamic`, as the template, at the floor spot found in step 2,
//!    spawned by the level's `WorldInfo` in the slice gate's Off world, before play begins (so no begin-play script
//!    runs), with `Spawn`'s `bNoCollisionFail` set, since a fixture stands where it is put (the VM world has no
//!    collision hash for the free-spot search; step 2 is the port's check of the spot instead);
//! 4. the link passes when the spawn made an actor that is in the world's actor list and stands where it was put,
//!    inside the area.
//!
//! A refusal keeps its kind by its typed error (a `VmError::NotYet` stays a `notyet`; any other error is an `error`,
//! whatever its text says), the world's setup included. The first write is making the template (step 3; a lazy
//! definition load counts): a `NotYet` from there on is terminal and its text starts with [`PARTIAL_WRITES`], its
//! kind and record kept ([`after_writes`]).

use std::path::Path;

use ow_assets::movement::{CollisionWorld, MoveInput, MovementParams, Mover, Physics, Shape};
use ow_package::props::{self, NoArrayTypes, PropValue};
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

impl From<crate::cmd::inventory::equip_run::SetupError> for Failed {
    fn from(e: crate::cmd::inventory::equip_run::SetupError) -> Failed {
        match e {
            crate::cmd::inventory::equip_run::SetupError::Vm(v) => Failed::Vm(v),
            crate::cmd::inventory::equip_run::SetupError::Other(t) => Failed::Check(t),
        }
    }
}

/// The mark of a refusal after committed writes (the hub's ruling, the text the other Off paths use).
pub const PARTIAL_WRITES: &str = "stopped after partial writes";

/// `f` met after the step's first write: a `NotYet` gets [`PARTIAL_WRITES`] at the very start of its text (once), its
/// kind and record kept; anything else is unchanged (review r2 of #470, item 3).
pub fn after_writes(f: Failed) -> Failed {
    match f {
        Failed::Vm(VmError::NotYet(n)) if !n.what.starts_with(PARTIAL_WRITES) => {
            Failed::Vm(VmError::NotYet(NotYet::new(format!("{PARTIAL_WRITES}: {}", n.what), n.missing, n.record)))
        }
        other => other,
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
    let mut world = crate::cmd::inventory::equip_run::gate_world_typed(cooked)?;
    let actors = world.actors();
    let info = actors
        .try_borrow()
        .map_err(|_| Failed::Check("the actor list is borrowed".into()))?
        .world_info()
        .ok_or_else(|| Failed::Check("the gate's world has no WorldInfo".into()))?;
    let vm = &mut world.vm;
    // Every placement check comes before the first write (review r2 of #470, item 1): the cylinder is read from the
    // package, then the floor, Maya's reach and the final location vector, and only then is the template made.
    let shape = dummy_shape(vm)?;
    let spot = floor_spot(collision, area, &MovementParams::maya(), shape, centre)?;
    reachable(collision, &MovementParams::maya(), maya, shape, spot)?;
    let location = vector_at(vm, info, spot)?;
    spawn_at(vm, &actors, info, location, spot).map_err(after_writes)
}

/// Makes the template and spawns the dummy at `location` (`spot`): the step's writes, from the first on (module
/// documentation).
fn spawn_at(
    vm: &mut Vm,
    actors: &std::rc::Rc<std::cell::RefCell<ow_world::actors::Actors>>,
    info: ObjRef,
    location: Value,
    spot: [f64; 3],
) -> Result<[f64; 3], Failed> {
    // The archetype export of the dummy's own package, made as an object (as the mission slice and `--combat-run`
    // take it): the path alone names another file's object under the store's identity policy.
    let archetype = ow_combat::hit::instantiate_copy(vm, DUMMY_PACKAGE, DUMMY_PAWN)?
        .ok_or_else(|| Failed::Check(format!("{DUMMY_PACKAGE} has no {DUMMY_PAWN}")))?;
    let class = vm.space.heap.get(archetype).map_err(VmError::from)?.class;
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

/// The dummy's collision cylinder (`CollisionRadius`, `CollisionHeight`, a half height), read from the package
/// without making any object (review r2 of #470, item 1): the archetype export's own `CylinderComponent` tag, a
/// reference to an export of the same file, and that export's own two tags. Where any of it cannot be read, the
/// placement checks are not available: a `NotYet`.
pub fn dummy_shape(vm: &mut Vm) -> Result<Shape, Failed> {
    let name = ow_core::PackageName::parse(DUMMY_PACKAGE).map_err(|e| Failed::Check(format!("{DUMMY_PACKAGE}: {e}")))?;
    let (_, store) = vm.defs.code_mut();
    let found = store.resolve_path(&name, DUMMY_PAWN).map_err(|e| Failed::Check(format!("{DUMMY_PACKAGE}: {e}")))?;
    let Some((id, archetype)) = found else {
        return Err(Failed::Check(format!("{DUMMY_PACKAGE} has no {DUMMY_PAWN}")));
    };
    cylinder_of(&store.shared(id), archetype)
}

/// The cylinder of archetype export `archetype` of `pkg` ([`dummy_shape`]): pure, so the tests share it.
pub fn cylinder_of(pkg: &ow_package::package::Package, archetype: i32) -> Result<Shape, Failed> {
    let unreadable = |what: &str| -> Failed {
        Failed::Vm(
            NotYet::not_read(
                format!("fight area: the dummy's collision cylinder ({what}), which the placement checks need"),
                RECORD,
            )
            .into(),
        )
    };
    let tags = |export: i32, prefix: ow_object::prefix::PropertyPrefix, what: &str| {
        props::read_tagged(pkg, export, prefix.bytes(), &NoArrayTypes)
            .map(|t| t.entries)
            .map_err(|e| unreadable(&format!("{what}'s tags: {e}")))
    };
    let own = tags(archetype, ow_object::prefix::PropertyPrefix::Ordinary, "the archetype")?;
    let component = match own.iter().find(|e| e.name == "CylinderComponent").map(|e| &e.value) {
        Some(PropValue::Object(r)) if *r > 0 => *r,
        Some(PropValue::Object(r)) if *r < 0 => return Err(unreadable("a CylinderComponent in another file")),
        _ => return Err(unreadable("no CylinderComponent of its own")),
    };
    let cylinder = tags(component, ow_object::prefix::PropertyPrefix::Component, "the CylinderComponent")?;
    let read = |name: &str| -> Result<f64, Failed> {
        match cylinder.iter().find(|e| e.name == name).map(|e| &e.value) {
            Some(PropValue::Float(x)) if x.is_finite() && *x > 0.0 => Ok(f64::from(*x)),
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

/// How close, horizontally, Maya's grounded walk must come to the dummy's spot, uu (a port choice, not the game's).
pub const ARRIVE: f64 = 16.0;

/// The reach walk's time step, s: the scripted walk's own.
fn reach_dt() -> f64 {
    ow_assets::movement::sanctuary::walk_dt()
}

/// Whether `walker`, ending its walk at `from` (its shape's centre), can reach a pawn of `shape` standing at `to`
/// (module documentation, step 2): the feet within the walker's step height of each other, and a grounded walk by
/// the port's mover from `from` straight at `to`, steered each tick, arriving within [`ARRIVE`] uu, never more than
/// the step height below its start, inside a tick budget (twice the straight time at full speed, plus two seconds).
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
    let dt = reach_dt();
    let straight = horizontal(from, to) / (walker.ground_speed * dt);
    let budget = ow_assets::movement::geom::floor_index(2.0 * straight + 2.0 / dt, 1_000_000);
    let mut mover = Mover::spawn(*walker, from, ow_core::units::Rotator::new(0, 0, 0));
    mover.physics = Physics::Walking;
    let lowest = from[2] - step;
    for _ in 0..budget {
        let (dx, dy) = (to[0] - mover.location[0], to[1] - mover.location[1]);
        if horizontal(mover.location, to) <= ARRIVE {
            return Ok(());
        }
        // Yaw 0 faces +X with +Y to the right, so the direction is the input itself (the mover normalizes it).
        mover.tick(world, &MoveInput::new(dx, dy, false, 0, 0), dt);
        if mover.location[2].is_nan() || mover.location[2] < lowest {
            return Err(Failed::Check(format!(
                "no grounded way from Maya's walked end to the dummy: she drops below her start {} uu from it (a gap \
                 or a ledge)",
                fmt_f(horizontal(mover.location, to), 1)
            )));
        }
    }
    Err(Failed::Check(format!(
        "no grounded way from Maya's walked end to the dummy: her walk stops {} uu short of it",
        fmt_f(horizontal(mover.location, to), 1)
    )))
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
