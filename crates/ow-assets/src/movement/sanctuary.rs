//! Owner: lane H4-MOVEMENT. `ow-host --game <install> --walk`: Sanctuary's scene, its collision, and the scripted
//! walk from its player start, run headless at the fixed simulation step, printed as text (counts, sampled
//! positions, the segments with their path digests, and the result). Nothing is written to a file and nothing is
//! drawn. [`load`] and [`spawn`] are shared with the play mode (`ow-host`'s `play`, lane H4-PLAY), so both start from the
//! same collision and the same pawn.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-16-52-*`.

use std::fmt::Write as _;
use std::time::Instant;

use ow_core::fmt::fmt_f;
use ow_core::sim::DEFAULT_STEP_SECONDS;
use ow_core::{PackageName, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::ObjectSpace;

use super::collide::CollisionWorld;
use super::level::{self, CollisionCensus, LevelCollision};
use super::mover::{MovementParams, Mover, Physics};
use super::walk::{self, SANCTUARY_SCRIPT, WalkReport};
use std::path::Path;

use crate::scene::{Scene, SceneAssets};

/// The map the walk loads.
pub const MAP: &str = "Sanctuary_P";

/// Samples printed: one every this many ticks (a quarter second at 1/60 s).
pub const SAMPLE_EVERY: u64 = 15;

/// The text report and whether the walk passed.
pub struct WalkRun {
    pub text: String,
    pub passed: bool,
}

fn p3(v: [f64; 3]) -> String {
    format!("{} {} {}", fmt_f(v[0], 2), fmt_f(v[1], 2), fmt_f(v[2], 2))
}

fn physics(p: Physics) -> &'static str {
    match p {
        Physics::Walking => "walking",
        Physics::Falling => "falling",
    }
}

/// A map loaded for movement: its scene, the store and object space it was read with, its collision, and the load
/// report's text.
pub struct Loaded {
    pub scene: Scene,
    pub defs: DefStore,
    pub space: ObjectSpace,
    pub collision: LevelCollision,
    pub text: String,
}

/// The package a map name means in the install's cooked folder `cooked`: the name itself when it has `<name>.upk`,
/// else `<name>_P` when it has that (so `Sanctuary` is `Sanctuary_P`), else the name as given (the load then reports
/// it missing).
pub fn resolve_map(cooked: &Path, name: &str) -> String {
    let has = |n: &str| cooked.join(format!("{n}.upk")).is_file();
    if !has(name) && has(&format!("{name}_P")) { format!("{name}_P") } else { name.to_string() }
}

/// Loads map `map` from the install's cooked folder `cooked` (ParityMode::Off, as every host run) and builds its
/// collision.
pub fn load(cooked: &Path, map: &str) -> Result<Loaded, String> {
    let started = Instant::now();
    let mut defs = DefStore::open(cooked.to_path_buf(), IdentityPolicy::FirstInScanOrder, ParityMode::Off)
        .map_err(|e| format!("definition store: {e}"))?;
    let mut space = ObjectSpace::new();
    let assets = SceneAssets::new(cooked.to_path_buf());
    let name = PackageName::parse(map).map_err(|e| format!("{map}: {e:?}"))?;
    let scene = Scene::load(&mut defs, &mut space, &assets, &name).map_err(|e| format!("scene: {e}"))?;
    let scene_time = started.elapsed();
    let built = Instant::now();
    let collision = level::build(&scene, &mut defs, &mut space);
    let build_time = built.elapsed();
    let mut text = String::new();
    let _ = writeln!(
        text,
        "scene: {map}, {} levels, {} placements, loaded in {} s; collision built in {} s",
        scene.levels.len(),
        scene.placements.len(),
        fmt_f(scene_time.as_secs_f64(), 1),
        fmt_f(build_time.as_secs_f64(), 1)
    );
    census_text(&mut text, &collision.census);
    Ok(Loaded { scene, defs, space, collision, text })
}

/// Maya at the scene's first player start, facing its yaw, lifted out of solid when she starts in it (a host
/// stand-in, [`Mover::lift_out_of_solid`]); the spawn lines go to `out`.
pub fn spawn(scene: &Scene, world: &CollisionWorld, out: &mut String) -> Result<Mover, String> {
    let Some(start) = scene.player_starts.values().next() else {
        return Err("the scene has no player start".into());
    };
    let l = start.pose.location;
    let location = [f64::from(l[0]), f64::from(l[1]), f64::from(l[2])];
    let mut view = start.pose.rotation;
    view.pitch = 0;
    view.roll = 0;
    let _ = writeln!(
        out,
        "spawn: {} ({}) at {} yaw {}; {} player start(s)",
        start.path,
        start.class,
        p3(location),
        view.yaw,
        scene.player_starts.len()
    );
    let mut mover = Mover::spawn(MovementParams::maya(), location, view);
    match mover.lift_out_of_solid(world) {
        Some(lift) => {
            let _ = writeln!(out, "spawn: lifted {} uu out of solid (host stand-in)", fmt_f(lift, 0));
        }
        None => {
            let _ = writeln!(out, "spawn: inside solid and no free height within {} uu", fmt_f(super::mover::SPAWN_LIFT, 0));
        }
    }
    Ok(mover)
}

/// Loads Sanctuary from the install's cooked folder `cooked`, builds its collision and runs the walk.
pub fn run(cooked: &Path) -> Result<WalkRun, String> {
    let loaded = load(cooked, MAP)?;
    let mut out = loaded.text.clone();
    let mut mover = spawn(&loaded.scene, &loaded.collision.world, &mut out)?;
    let dt = f64::from(DEFAULT_STEP_SECONDS);
    let walked = Instant::now();
    let report = walk::run(&loaded.collision.world, &mut mover, &SANCTUARY_SCRIPT, dt, SAMPLE_EVERY);
    let walk_time = walked.elapsed();
    walk_text(&mut out, &report, dt);
    blockers(&mut out, &loaded.scene, &loaded.collision.world, &report);
    let _ = writeln!(out, "walk: {} ticks in {} s of real time", report.ticks, fmt_f(walk_time.as_secs_f64(), 3));
    Ok(WalkRun { text: out, passed: report.passed() })
}

/// For each stalled segment: the placement and mesh of the brush it ran into, so a reader can tell which wall it was.
fn blockers(out: &mut String, scene: &Scene, world: &CollisionWorld, r: &WalkReport) {
    for (i, s) in r.segments.iter().enumerate().filter(|(_, s)| s.stalled) {
        let Some(b) = s.blocker else {
            let _ = writeln!(out, "segment {i} stalled with nothing in the way: STUCK");
            continue;
        };
        let Some(brush) = world.brushes().get(b.brush as usize) else { continue };
        let placement = scene.placements.iter().nth(brush.source as usize);
        let level = placement.and_then(|(k, _)| scene.levels.get(k.level as usize)).map_or("?", |l| l.name.as_str());
        let placement = placement.map(|(_, p)| p);
        let _ = writeln!(
            out,
            "segment {i} stalled at a wall: normal {} of {} {} ({}), brush bounds {} .. {}",
            p3(b.normal),
            level,
            placement.map_or("?", |p| p.path.as_str()),
            placement.map_or("?", |p| p.mesh.as_str()),
            p3(brush.lo),
            p3(brush.hi)
        );
    }
}

fn census_text(out: &mut String, c: &CollisionCensus) {
    let _ = writeln!(
        out,
        "collision: placements {}, blocking {}, not blocking {} (of them by a class-default flag: {}), flags unreadable {}",
        c.placements,
        c.blocking,
        c.not_blocking,
        c.not_blocking_by_class_default,
        c.flags_unreadable.values().sum::<usize>()
    );
    for (reason, n) in c.flags_unreadable.iter() {
        let _ = writeln!(out, "  flags unreadable: {n} x {reason}");
    }
    let _ = writeln!(
        out,
        "  blocking placements: with hulls {}, mesh without body {}, body refused {}, body empty {}, mesh not decoded {}, pose refused {}",
        c.placed_with_hulls, c.mesh_without_body, c.body_refused, c.body_empty, c.mesh_not_decoded, c.pose_refused
    );
    let _ = writeln!(
        out,
        "  meshes of blocking placements: with a body {} ({} hulls), without a body {}, body refused {}",
        c.meshes_with_body,
        c.body_hulls,
        c.meshes_without_body,
        c.meshes_body_refused.values().sum::<usize>()
    );
    for (reason, n) in c.meshes_body_refused.iter() {
        let _ = writeln!(out, "  body refused: {n} x {reason}");
    }
    let _ = writeln!(
        out,
        "  brushes placed {}, refused {}",
        c.brushes,
        c.brushes_refused.values().sum::<usize>()
    );
    let by_level: Vec<String> = c.brushes_by_level.iter().map(|(l, n)| format!("{l} {n}")).collect();
    let _ = writeln!(out, "  brushes by level: {}", by_level.join(", "));
    let beyond = c
        .brushes_refused
        .get(super::collide::BrushError::OutOfBounds.to_string().as_str())
        .copied()
        .unwrap_or(0);
    let _ = writeln!(
        out,
        "  world bound +-{} uu: {beyond} hull(s) refused beyond it",
        fmt_f(super::collide::WORLD_BOUND, 0)
    );
    for (reason, n) in c.brushes_refused.iter() {
        let _ = writeln!(out, "  brush refused: {n} x {reason}");
    }
    for (what, n, not_yet) in &c.not_yet {
        let _ = writeln!(out, "  not yet: {what}: {n} ({not_yet})");
    }
}

fn walk_text(out: &mut String, r: &WalkReport, dt: f64) {
    for s in &r.samples {
        let _ = writeln!(
            out,
            "t={} seg={} at {} v {} {} yaw {}",
            fmt_f(s.tick as f64 * dt, 2),
            s.segment,
            p3(s.location),
            p3(s.velocity),
            physics(s.physics),
            s.yaw
        );
    }
    for (i, s) in r.segments.iter().enumerate() {
        let _ = writeln!(
            out,
            "segment {i} {}: from {} to {}, moved {} of {} at full speed, landings {}, stepped up {}, digest {:016x}{}",
            s.name,
            p3(s.start),
            p3(s.end),
            fmt_f(s.moved, 1),
            fmt_f(s.full_speed, 1),
            s.landings,
            fmt_f(s.stepped_up, 1),
            s.digest.0,
            if s.stuck {
                ", STUCK"
            } else if s.stalled {
                ", stalled at a wall"
            } else {
                ""
            }
        );
    }
    let landing = r.first_landing.map_or_else(|| "never".to_string(), p3);
    let _ = writeln!(
        out,
        "result: {} (fell through: {}, stuck: {}); travelled {} uu, first landing {}, lowest z {}, longest fall {} s, start-solid ticks {}, ticks {}, digest {:016x}",
        if r.passed() { "PASS" } else { "FAIL" },
        r.fell_through,
        r.stuck,
        fmt_f(r.travelled, 1),
        landing,
        fmt_f(r.lowest_z, 2),
        fmt_f(r.longest_fall, 2),
        r.start_solid_ticks,
        r.ticks,
        r.digest.0
    );
}
