//! Owner: lane H-WORLD. The loaded world behind the play mode: a map's placed actors in an `ow_world` [`World`]
//! (W5-MAPLOAD's `load_map`), ticked once per play step, with the placed pawns' transforms handed back for drawing.
//!
//! AI-assisted (Claude Code). Decision: `docs/decisions/2026-11/2026-11-19-01-*`.
//!
//! **Where the world runs.** `World` and its `Vm` are `!Send` (D18), and the VM wants a deep stack
//! (`ow_vm::interp::STACK_SIZE`, as `ow-cli` runs every VM command). So [`WorldLink::spawn`] starts one thread with
//! that stack, builds the world *on* it from `Send` inputs (a builder closure), and serves tick requests over two
//! channels. A tick is a request and a wait for its answer: the play loop stays a function of its inputs, and the
//! world sees exactly one tick per play step, of the play step's length, whatever the frame times. Nothing but
//! [`WorldFrame`] and [`LoadSummary`] (plain data) comes back across the channel.
//!
//! **What a tick does.** `World::tick(dt)`, which advances the world clock and writes `WorldInfo.TimeSeconds`,
//! `RealTimeSeconds` and `DeltaSeconds`, then ticks the actors (`ow_world::world`, module notes). Afterwards the link
//! reads back `TimeSeconds`, `DeltaSeconds` and each placed pawn's `Location` and `Rotation`.
//!
//! **When a tick fails** (an unread native's `NotYet`, a script error), the world is *halted*: the error is kept with
//! the tick it happened on, and no later tick runs, because a world stopped partway through its actor pass is not a
//! state the next tick should build on. The play mode goes on (the pawn's movement is the host's, not the world's);
//! [`WorldFrame::halted`] says why the world stopped, and the play report prints it. No `NotYet` is turned into a
//! value here (rule R3).
//!
//! **The physics step.** The world asks its host for the physics of every actor whose `Physics` is not `PHYS_None`.
//! [`install_physics`] gives it one that answers `NotYet` for every mode, naming the actor's class and mode: the H4
//! mover is the player pawn's, and nothing else in the host moves world actors yet. On Sanctuary that halts the first
//! tick (decision 2026-11-19-01 has the census of what blocks it).
//!
//! **What stays `NotYet`** before any tick: W5-MAPLOAD's three pending steps (the begin-play pass, the game info,
//! the local player), listed in [`LoadSummary::pending`]. Their implementer is a separate lane (NATIVE_MAP_LOAD.md);
//! this module does not run or stand in for them.
//!
//! **Pawns.** A placed actor whose class derives from `Engine.Pawn` is reported with its object path, class, archetype
//! path (from its export's archetype, when it has one) and its `Location` / `Rotation` after each tick. The drawer
//! reads each pawn's look from the archetype through its own store (`crate::anim::pawns`), so drawing never reads
//! from, or loads into, the world's object space.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::mpsc::{self, Receiver, Sender};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use ow_attr::AttributeSystem;
use ow_balance::types::{
    AwesomeLevel, BalanceSnapshot, ExpLevel, GameStage, LevelCapUpgrades, OpLevel, PlayerCount, Playthrough,
};
use ow_core::units::Rotator;
use ow_core::{PackageName, ParityMode};
use ow_defs::DefStore;
use ow_defs::identity::IdentityPolicy;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::{NativePack, Vm};
use ow_vm::error::VmError;
use ow_world::world::mapload::{MapReport, load_map};
use ow_world::world::{BalanceState, World, WorldError};

/// The class whose placed subclasses are reported as pawns.
pub const PAWN_CLASS: &str = "Engine.Pawn";

/// One placed pawn as the world last left it.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct PawnActor {
    /// The actor's full object path: its map file's package name, then its path in that file
    /// (`Sanctuary_P.TheWorld.PersistentLevel.WillowAIPawn_3`), so two levels' actors of the same name stay apart.
    pub path: String,
    /// Its class path.
    pub class: String,
    /// The object path of its export's archetype, when the export names one.
    pub archetype: Option<String>,
    /// `Location`, uu.
    pub location: [f32; 3],
    /// `Rotation`.
    pub rotation: Rotator,
}

/// What a tick left: the world clock as the `WorldInfo` holds it, the pawns, and why the world stopped if it did.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct WorldFrame {
    /// World ticks that ran to their end.
    pub ticks: u64,
    /// `WorldInfo.TimeSeconds` and `DeltaSeconds` after the last tick (`None` where the class declares none).
    pub time_seconds: Option<f32>,
    pub delta_seconds: Option<f32>,
    pub pawns: Vec<PawnActor>,
    /// Placed pawns left out because their transform cannot be read: path and reason.
    pub unplaced: Vec<String>,
    pub halted: Option<Halt>,
}

/// Why the world stopped ticking (module notes).
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq)]
pub struct Halt {
    /// The tick that failed, counting from 1.
    pub tick: u64,
    pub message: String,
    /// The failure is a `NotYet` (an unread rule or an absent layer), not a script or data error.
    pub not_yet: bool,
}

/// What loading the map did, as plain data.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Default)]
pub struct LoadSummary {
    pub map: String,
    pub mode: Option<ParityMode>,
    /// The world's actor list after the load (the persistent `WorldInfo` first).
    pub actors: usize,
    /// Per level: name, listed entries, placed, refused.
    pub levels: Vec<(String, usize, usize, usize)>,
    /// Refused actors by class with the first reason, over all levels.
    pub refused: Vec<(String, usize, String)>,
    pub missing_levels: Vec<String>,
    /// The `NotYet`s the map load leaves (W5-MAPLOAD's three).
    pub pending: Vec<String>,
    /// The pawns as placed, before any tick.
    pub pawns: Vec<PawnActor>,
    /// Placed pawns left out because their transform cannot be read: path and reason.
    pub unplaced: Vec<String>,
    /// How long opening the store and loading the map took.
    pub load_time: Duration,
}

impl LoadSummary {
    /// The summary of `report` for a world whose actor list is `actors` long.
    pub fn from_report(map: &str, mode: ParityMode, report: &MapReport, actors: usize) -> LoadSummary {
        let mut refused = Vec::new();
        let mut levels = Vec::new();
        for level in &report.levels {
            let mut n = 0;
            for (class, r) in &level.refused {
                n += r.count;
                refused.push((class.clone(), r.count, r.first.clone()));
            }
            levels.push((level.name.clone(), level.listed, level.placed_total, n));
        }
        LoadSummary {
            map: map.to_string(),
            mode: Some(mode),
            actors,
            levels,
            refused,
            missing_levels: report.missing_levels.clone(),
            pending: report.pending.iter().map(ToString::to_string).collect(),
            ..LoadSummary::default()
        }
    }

    /// The report's lines: one for the load, one per level, one per refused class and pending step.
    pub fn text(&self) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let placed: usize = self.levels.iter().map(|l| l.2).sum();
        let refused: usize = self.levels.iter().map(|l| l.3).sum();
        let _ = writeln!(
            out,
            "world {} ({:?}): {} actors in the world, {placed} placed and {refused} refused over {} levels, {} pawns, \
             loaded in {} ms",
            self.map,
            self.mode.unwrap_or(ParityMode::Off),
            self.actors,
            self.levels.len(),
            self.pawns.len(),
            self.load_time.as_millis()
        );
        for (name, listed, placed, refused) in &self.levels {
            let _ = writeln!(out, "world level {name}: listed {listed}, placed {placed}, refused {refused}");
        }
        for (class, n, first) in &self.refused {
            let _ = writeln!(out, "world refused {class} x{n}: {first}");
        }
        for name in &self.missing_levels {
            let _ = writeln!(out, "world missing level {name}");
        }
        for p in &self.pending {
            let _ = writeln!(out, "world pending: {p}");
        }
        for p in &self.pawns {
            let _ = writeln!(out, "world pawn {}", pawn_text(p));
        }
        for p in &self.unplaced {
            let _ = writeln!(out, "world pawn left out: {p}");
        }
        out
    }
}

/// One pawn's line: path, class, archetype, location and yaw.
pub fn pawn_text(p: &PawnActor) -> String {
    let f = |x: f32| ow_core::fmt::fmt_f(f64::from(x), 2);
    format!(
        "{} ({}, archetype {}) at {} {} {} yaw {}",
        p.path,
        p.class,
        p.archetype.as_deref().unwrap_or("none"),
        f(p.location[0]),
        f(p.location[1]),
        f(p.location[2]),
        p.rotation.yaw
    )
}

/// The world as the play loop sees it: one tick per play step. [`WorldLink`] is the real one; tests give their own.
pub trait WorldTick {
    /// Ticks the world by `dt` seconds (once halted: does nothing) and returns what it left.
    fn tick(&mut self, dt: f32) -> WorldFrame;
}

/// What builds the world on its thread: the world with its map loaded, and the load's summary.
pub type Builder = Box<dyn FnOnce() -> Result<(World, LoadSummary), String> + Send>;

enum Request {
    Tick(f32),
}

/// The world on its own thread (module notes).
pub struct WorldLink {
    requests: Option<Sender<Request>>,
    replies: Receiver<WorldFrame>,
    thread: Option<JoinHandle<()>>,
    last: WorldFrame,
}

impl std::fmt::Debug for WorldLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorldLink").field("last", &self.last).finish_non_exhaustive()
    }
}

impl WorldLink {
    /// Starts the world thread, builds the world on it with `build` and waits for the load. `Err` with the builder's
    /// error, or when the thread could not start or died.
    pub fn spawn(build: Builder) -> Result<(WorldLink, LoadSummary), String> {
        let (request_tx, request_rx) = mpsc::channel::<Request>();
        let (reply_tx, reply_rx) = mpsc::channel::<WorldFrame>();
        let (load_tx, load_rx) = mpsc::channel::<Result<LoadSummary, String>>();
        let thread = std::thread::Builder::new()
            .name("ow-world".into())
            .stack_size(ow_vm::interp::STACK_SIZE)
            .spawn(move || serve(build, &load_tx, &request_rx, &reply_tx))
            .map_err(|e| format!("cannot start the world thread: {e}"))?;
        let summary = match load_rx.recv() {
            Ok(Ok(summary)) => summary,
            Ok(Err(e)) => {
                let _ = thread.join();
                return Err(e);
            }
            Err(_) => {
                return Err(match thread.join() {
                    Err(payload) => format!("the world thread panicked before loading the map: {}", panic_text(payload.as_ref())),
                    Ok(()) => "the world thread stopped before loading the map".into(),
                });
            }
        };
        let last = WorldFrame {
            pawns: summary.pawns.clone(),
            unplaced: summary.unplaced.clone(),
            ..WorldFrame::default()
        };
        Ok((WorldLink { requests: Some(request_tx), replies: reply_rx, thread: Some(thread), last }, summary))
    }
}

impl WorldTick for WorldLink {
    fn tick(&mut self, dt: f32) -> WorldFrame {
        if self.last.halted.is_some() {
            return self.last.clone();
        }
        let sent = self.requests.as_ref().is_some_and(|tx| tx.send(Request::Tick(dt)).is_ok());
        match sent.then(|| self.replies.recv().ok()).flatten() {
            Some(frame) => self.last = frame,
            None => {
                // The thread is gone: join it for its panic message, if it panicked.
                let why = match self.thread.take().map(JoinHandle::join) {
                    Some(Err(payload)) => format!("the world thread panicked: {}", panic_text(payload.as_ref())),
                    _ => "the world thread stopped".to_string(),
                };
                self.last.halted = Some(Halt { tick: self.last.ticks + 1, message: why, not_yet: false });
            }
        }
        self.last.clone()
    }
}

/// The text of a panic payload (`&str` or `String`), else a placeholder.
fn panic_text(payload: &(dyn std::any::Any + Send)) -> String {
    payload
        .downcast_ref::<&str>()
        .map(|s| (*s).to_string())
        .or_else(|| payload.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "a payload that is not text".to_string())
}

impl Drop for WorldLink {
    fn drop(&mut self) {
        // Closing the request channel ends the thread's loop; it drops the world on its own thread.
        self.requests = None;
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

/// The world thread: build, report the load, then one tick per request until the channel closes.
fn serve(
    build: Builder,
    load: &Sender<Result<LoadSummary, String>>,
    requests: &Receiver<Request>,
    replies: &Sender<WorldFrame>,
) {
    let (mut world, mut summary) = match build() {
        Ok(built) => built,
        Err(e) => {
            let _ = load.send(Err(e));
            return;
        }
    };
    let mut frame = WorldFrame::default();
    match pawns(&mut world.vm) {
        Ok((p, unplaced)) => {
            summary.pawns = p.clone();
            summary.unplaced = unplaced.clone();
            frame.pawns = p;
            frame.unplaced = unplaced;
        }
        Err(e) => {
            let _ = load.send(Err(format!("reading the placed pawns: {e}")));
            return;
        }
    }
    if load.send(Ok(summary)).is_err() {
        return;
    }
    while let Ok(Request::Tick(dt)) = requests.recv() {
        if frame.halted.is_none() {
            step(&mut world, &mut frame, dt);
        }
        if replies.send(frame.clone()).is_err() {
            return;
        }
    }
}

/// One tick and the read-back (module notes). The read-back runs after a failed tick too: the world advances its
/// clock and writes the `WorldInfo` time fields before its actor pass, so they show how far the failed tick got.
fn step(world: &mut World, frame: &mut WorldFrame, dt: f32) {
    let tick = frame.ticks + 1;
    match world.tick(dt) {
        Ok(_) => frame.ticks = tick,
        Err(e) => {
            let not_yet = matches!(e, WorldError::Vm(VmError::NotYet(_)));
            frame.halted = Some(Halt { tick, message: e.to_string(), not_yet });
        }
    }
    type ReadBack = (Option<f32>, Option<f32>, (Vec<PawnActor>, Vec<String>));
    let mut read = || -> Result<ReadBack, VmError> {
        let wi = world.actors().try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.world_info();
        let (t, d) = match wi {
            Some(wi) => (float(&world.vm.space, wi, "TimeSeconds"), float(&world.vm.space, wi, "DeltaSeconds")),
            None => (None, None),
        };
        Ok((t, d, pawns(&mut world.vm)?))
    };
    match read() {
        Ok((t, d, (p, unplaced))) => {
            frame.time_seconds = t;
            frame.delta_seconds = d;
            frame.pawns = p;
            frame.unplaced = unplaced;
        }
        Err(e) if frame.halted.is_none() => {
            frame.halted = Some(Halt { tick, message: format!("reading the world back: {e}"), not_yet: false });
        }
        Err(_) => {}
    }
}

/// Property `name` of `o` when the class declares it.
fn read(space: &ObjectSpace, o: ObjRef, name: &str) -> Option<Value> {
    let class = space.heap.get(o).ok()?.class;
    let p = space.prop(class, name)?;
    space.read(o, PropAddr::scalar(p)).ok().cloned()
}

fn float(space: &ObjectSpace, o: ObjRef, name: &str) -> Option<f32> {
    match read(space, o, name) {
        Some(Value::Float(f)) => Some(f),
        _ => None,
    }
}

fn field<'v>(space: &ObjectSpace, v: &'v Value, name: &str) -> Option<&'v Value> {
    let s = v.as_struct()?;
    let layout = space.classes.struct_layout(s.ty)?;
    let (entry, _) = layout.field(space.names.find(name)?)?;
    s.fields.get(usize::try_from(entry).ok()?)
}

/// `X`, `Y`, `Z` of a vector value: `None` unless all three are floats.
fn vector_of(space: &ObjectSpace, v: &Value) -> Option<[f32; 3]> {
    let mut out = [0.0f32; 3];
    for (i, axis) in ["X", "Y", "Z"].iter().enumerate() {
        match field(space, v, axis) {
            Some(Value::Float(f)) => out[i] = *f,
            _ => return None,
        }
    }
    Some(out)
}

/// `Pitch`, `Yaw`, `Roll` of a rotator value: `None` unless all three are ints.
fn rotator_of(space: &ObjectSpace, v: &Value) -> Option<Rotator> {
    let mut r = [0i32; 3];
    for (i, axis) in ["Pitch", "Yaw", "Roll"].iter().enumerate() {
        match field(space, v, axis) {
            Some(Value::Int(n)) => r[i] = *n,
            _ => return None,
        }
    }
    Some(Rotator::new(r[0], r[1], r[2]))
}

/// `Location` and `Rotation` of `o`, or why there is none: a pawn whose transform cannot be read is reported and left
/// out, never placed at a made-up origin (rule R3).
fn transform(space: &ObjectSpace, o: ObjRef) -> Result<([f32; 3], Rotator), &'static str> {
    let location = read(space, o, "Location").ok_or("no Location")?;
    let location = vector_of(space, &location).ok_or("its Location is not a vector")?;
    let rotation = read(space, o, "Rotation").ok_or("no Rotation")?;
    let rotation = rotator_of(space, &rotation).ok_or("its Rotation is not a rotator")?;
    Ok((location, rotation))
}

/// The listed actors whose class derives from [`PAWN_CLASS`] and that are not destroyed, in list order; and, apart,
/// those whose transform cannot be read (path and reason), which are left out.
fn pawns(vm: &mut Vm) -> Result<(Vec<PawnActor>, Vec<String>), VmError> {
    let Ok(pawn_class) = vm.find_class(PAWN_CLASS) else {
        return Ok((Vec::new(), Vec::new()));
    };
    let list = {
        let actors = ow_world::actors::actors_of(vm)?;
        let list = actors.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.list().to_vec();
        list
    };
    let mut out = Vec::new();
    let mut unplaced = Vec::new();
    for o in list {
        let object = vm.space.heap.get(o)?;
        if object.destroyed || !vm.space.classes.is_child_of(object.class, pawn_class) {
            continue;
        }
        let (path, archetype) = match object.origin {
            Origin::Export { package, index } | Origin::Resource { package, index } => {
                let pkg = vm.defs.packages().get(package);
                let path = format!("{}.{}", pkg.name, pkg.path_of(index).unwrap_or_default());
                let archetype = pkg
                    .object(index)
                    .ok()
                    .map(|e| e.archetype)
                    .filter(|&a| a != 0)
                    .and_then(|a| pkg.path_of(a).ok());
                (path, archetype)
            }
            _ => (String::new(), None),
        };
        let (location, rotation) = match transform(&vm.space, o) {
            Ok(t) => t,
            Err(why) => {
                unplaced.push(format!("{path}: {why}"));
                continue;
            }
        };
        out.push(PawnActor {
            path,
            class: ow_attr::eval::class_path(&vm.space, o),
            archetype,
            location,
            rotation,
        });
    }
    Ok((out, unplaced))
}

/// The record the host's physics step names.
pub const PHYSICS_RECORD: &str = "NATIVE_PLAYER_MOVEMENT.md";

/// The name of `o`'s `Physics` value in the property's enum, else its number.
fn physics_name(space: &ObjectSpace, o: ObjRef) -> String {
    let value = match read(space, o, "Physics") {
        Some(Value::Byte(b)) => b,
        _ => return "no Physics".into(),
    };
    let name = space
        .heap
        .get(o)
        .ok()
        .and_then(|object| space.prop(object.class, "Physics"))
        .and_then(|p| match space.classes.decl(p).map(|d| &d.kind) {
            Some(ow_object::layout::PropKind::Byte { enum_: Some(e) }) => space.classes.enum_layout(*e),
            _ => None,
        })
        .and_then(|layout| layout.values().get(usize::from(value)).map(|n| space.names.text(*n).into_owned()));
    name.unwrap_or_else(|| format!("Physics {value}"))
}

/// Installs the host's physics step on `world` (`World::set_physics_step`). Every mode answers `NotYet`, naming the
/// actor's class and its physics mode: the H4 mover (`crate::movement`) is the player pawn's walking and falling, and
/// it is not applied to other actors. A placed pawn's movement parameters and its controller's acceleration are not
/// read, the interpolating and rotating modes are not ported, and the world has no player pawn before the local
/// player step W5-MAPLOAD leaves pending.
pub fn install_physics(world: &mut World) -> Result<(), String> {
    world
        .set_physics_step(Rc::new(|vm: &mut Vm, actor: ObjRef, _dt: f32| {
            let class = ow_attr::eval::class_path(&vm.space, actor);
            let mode = physics_name(&vm.space, actor);
            Err(VmError::NotYet(ow_core::NotYet::not_ported(
                format!("the host's {mode} physics for a placed {class}"),
                PHYSICS_RECORD,
            )))
        }))
        .map_err(|e| e.to_string())
}

/// The balance inputs of the play world: a host choice (normal playthrough, one player, level 1, no overpower), as
/// `ow-cli`'s combat run uses. The game reads them from the save; the play mode has no save yet.
pub fn play_balance() -> Result<BalanceState, String> {
    let snapshot = BalanceSnapshot::new(
        Playthrough::NORMAL,
        OpLevel::NONE,
        PlayerCount::SINGLE,
        ExpLevel::new(1),
        GameStage(1),
        AwesomeLevel::new(0),
        LevelCapUpgrades::NONE,
    )
    .map_err(|e| e.to_string())?;
    Ok(BalanceState::new(snapshot))
}

/// A world over the store at `cooked` in `mode` with `map` loaded: the Core natives, the behavior handlers, and in
/// Off the INT localization roots of the install (as `ow-cli --slice-run` builds its VM). Runs on the world thread.
pub fn build_installed(cooked: PathBuf, map: String, mode: ParityMode) -> Result<(World, LoadSummary), String> {
    let started = Instant::now();
    let defs = DefStore::open(cooked.clone(), IdentityPolicy::FirstInScanOrder, mode).map_err(|e| e.to_string())?;
    let attrs = AttributeSystem::standard().map_err(|e| e.to_string())?;
    let mut vm = Vm::new(ObjectSpace::new(), defs, Rc::new(attrs), mode);
    ow_vm::natives_core::register(&mut vm).map_err(|e| e.to_string())?;
    ow_behavior::handlers::Pack.register(&mut vm).map_err(|e| e.to_string())?;
    if mode != ParityMode::Reference {
        let game = cooked.parent().and_then(std::path::Path::parent).ok_or("the cooked folder has no install parent")?;
        let roots = ["Engine", "Gearbox", "WillowGame"].map(|n| game.join(n).join("Localization")).to_vec();
        let backend = ow_world::world::localize::Localization::ready(roots, "INT", false).map_err(|e| e.to_string())?;
        vm.add_service(backend).map_err(|e| e.to_string())?;
    }
    let mut world = World::new(vm, play_balance()?, &[]).map_err(|e| e.to_string())?;
    install_physics(&mut world)?;
    let name = PackageName::parse(&map).map_err(|e| format!("{map}: {e}"))?;
    let report = load_map(&mut world, &name).map_err(|e| format!("loading {map} into the world: {e}"))?;
    let actors = world.actors().borrow().list().len();
    let mut summary = LoadSummary::from_report(&map, mode, &report, actors);
    summary.load_time = started.elapsed();
    Ok((world, summary))
}

#[cfg(test)]
mod tests;
