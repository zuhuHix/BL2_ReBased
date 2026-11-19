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
//! [`install_physics`] answers with [`physics_rule`] per mode (lane H-PHYSICS-MODES, decision 2026-11-19-22): an
//! interpolating actor no loaded Matinee op names moves nothing; every other mode answers `NotYet` naming its
//! question. The Matinee scan ([`matinee_bound`]) runs once, after the map load.
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

/// The Matinee op class (NATIVE_KISMET_MATINEE.md: the op binds its group actors through object-variable links).
pub const MATINEE_OP_CLASS: &str = "Engine.SeqAct_Interp";

/// The actors a Matinee op in the placed actors' map files names through an object-variable link: every
/// `SeqAct_Interp` (or subclass) export of those files is loaded, and each `SeqVar_Object.ObjValue` its
/// `VariableLinks` name is collected. Any link counts, not only the one whose description is a group name, so the set
/// errs towards "may be driven". A linked variable that holds no `ObjValue` (a named or list variable, say) is
/// counted by class in [`MatineeBindings::unresolved`]: while any is, the set may miss an actor. Loads the ops and their
/// variables into the world's object space.
pub fn matinee_bound(vm: &mut Vm) -> Result<MatineeBindings, String> {
    let mut out = MatineeBindings::default();
    let Ok(op_class) = vm.find_class(MATINEE_OP_CLASS) else {
        return Ok(out);
    };
    let list = {
        let actors = ow_world::actors::actors_of(vm).map_err(|e| e.to_string())?;
        let list = actors.try_borrow().map_err(|_| "the actor list is borrowed")?.list().to_vec();
        list
    };
    let mut packages = ow_core::DetSet::new();
    for o in list {
        if let Ok(object) = vm.space.heap.get(o)
            && let Origin::Export { package, .. } = object.origin
        {
            packages.insert(package);
        }
    }
    let mut is_op: ow_core::DetMap<String, bool> = ow_core::DetMap::new();
    for package in packages {
        let mut ops = Vec::new();
        {
            let pkg = vm.defs.packages().get(package);
            for (i, entry) in pkg.exports.iter().enumerate() {
                if entry.class == 0 {
                    continue;
                }
                let Ok(class) = pkg.path_of(entry.class) else { continue };
                ops.push((i32::try_from(i + 1).map_err(|e| e.to_string())?, class));
            }
        }
        for (index, class) in ops {
            let op = match is_op.get(&class) {
                Some(&known) => known,
                None => {
                    let known = vm.find_class(&class).is_ok_and(|c| vm.space.classes.is_child_of(c, op_class));
                    is_op.insert(class, known);
                    known
                }
            };
            if !op {
                continue;
            }
            let Some(o) = vm.defs.resolve_ref(&mut vm.space, package, index).map_err(|e| e.to_string())? else {
                continue;
            };
            let Some(Value::Array(links)) = read(&vm.space, o, "VariableLinks") else { continue };
            let mut variables = Vec::new();
            for link in links.iter() {
                if let Some(Value::Array(vars)) = field(&vm.space, link, "LinkedVariables") {
                    variables.extend(vars.iter().filter_map(|v| match v {
                        Value::Object(Some(r)) => Some(*r),
                        _ => None,
                    }));
                }
            }
            out.ops += 1;
            for var in variables {
                vm.defs.ensure_loaded(&mut vm.space, var).map_err(|e| e.to_string())?;
                match read(&vm.space, var, "ObjValue") {
                    Some(Value::Object(Some(actor))) => {
                        out.bound.insert(actor);
                    }
                    Some(Value::Object(None)) => out.empty += 1,
                    _ => {
                        let class = vm.space.heap.get(var).map_err(|e| e.to_string())?.class;
                        let object_var = vm
                            .find_class(OBJECT_VARIABLE_CLASS)
                            .is_ok_and(|c| vm.space.classes.is_child_of(class, c));
                        if object_var || vm.space.prop(class, "ObjList").is_some() {
                            out.unreadable += 1;
                        }
                        *out.unresolved.entry(ow_attr::eval::class_path(&vm.space, var)).or_insert(0) += 1;
                    }
                }
            }
        }
    }
    Ok(out)
}

/// What [`matinee_bound`] found.
#[non_exhaustive]
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MatineeBindings {
    /// Matinee ops read.
    pub ops: usize,
    /// The actors their object variables hold.
    pub bound: ow_core::DetSet<ObjRef>,
    /// Object variables holding nothing.
    pub empty: usize,
    /// Linked variables with no `ObjValue`, by class.
    pub unresolved: ow_core::DetMap<String, usize>,
    /// Of those, the ones that could still hold an actor: an object variable (or subclass) without the value, or a
    /// variable with an `ObjList`.
    pub unreadable: usize,
}

/// The object variable class whose `ObjValue` the scan reads.
pub const OBJECT_VARIABLE_CLASS: &str = "Engine.SeqVar_Object";

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

/// What the Matinee scan says about one interpolating actor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Driver {
    /// A loaded Matinee op names it: the op's playback would move it.
    Bound,
    /// No loaded Matinee op names it, and every linked variable was read.
    Unbound,
    /// Some op links a variable the scan could not read, so whether one names it is not known.
    Unknown,
}

/// The record of the Matinee playback.
pub const KISMET_RECORD: &str = "NATIVE_KISMET_MATINEE.md";
/// The record of the actor tick and the modes it hands to physics.
pub const ENGINE_RECORD: &str = "NATIVE_ENGINE_CORE.md";

/// The host's physics for one placed actor of class `class` in physics mode `mode` (lane H-PHYSICS-MODES, decision
/// 2026-11-19-22). `Ok` is a step that moves nothing; every other answer is a `NotYet` naming its question.
/// - `PHYS_None`: nothing (the world does not call physics for it; answered for completeness).
/// - `PHYS_Interpolating`: an actor no loaded Matinee op names moves nothing. Stock UE3's interpolating mode is the
///   one Matinee drives, and the world runs no other driver: it runs no Kismet, so the object variables empty at load
///   stay empty. UNVERIFIED in the running game, and so is that the game's interpolating step does nothing else for
///   such an actor. A bound actor answers `NotYet` for the playback (layer absent); an actor the scan cannot judge,
///   `NotYet` (not read).
/// - `PHYS_Walking` and `PHYS_Falling`: `NotYet`. The H4 mover is the player pawn's and is not applied to other pawns:
///   the walking step itself is not read (NATIVE_PLAYER_MOVEMENT.md, "Not read yet"), a placed pawn's movement values
///   are not read, its acceleration comes from a controller that does not exist before begin play, and the collision
///   the mover uses lives on the host side, not with the world.
/// - `PHYS_NavMeshWalking` (Gearbox's), `PHYS_Rotating` and every other mode: `NotYet` (not read).
pub fn physics_rule(mode: &str, class: &str, driver: Driver) -> Result<(), ow_core::NotYet> {
    use ow_core::NotYet;
    match mode {
        "PHYS_None" => Ok(()),
        "PHYS_Interpolating" => match driver {
            Driver::Unbound => Ok(()),
            Driver::Bound => Err(NotYet::layer_absent(
                format!("the Matinee playback that moves a bound interpolating {class}"),
                "Kismet and Matinee playback in the world",
                KISMET_RECORD,
            )),
            Driver::Unknown => Err(NotYet::not_read(
                format!(
                    "whether a Matinee op names an interpolating {class} (an op links a variable the scan cannot read)"
                ),
                KISMET_RECORD,
            )),
        },
        "PHYS_Walking" | "PHYS_Falling" => Err(NotYet::not_read(
            format!(
                "the {mode} physics of a placed {class}: the walking and falling step of a pawn that is not the \
                 player's, its movement values and its controller's acceleration"
            ),
            PHYSICS_RECORD,
        )),
        "PHYS_NavMeshWalking" => Err(NotYet::not_read(
            format!("the nav-mesh walking physics (a Gearbox mode) of a placed {class}"),
            ENGINE_RECORD,
        )),
        "PHYS_Rotating" => Err(NotYet::not_read(
            format!(
                "the rotating physics of a placed {class}: how RotationRate x DeltaTime becomes rotator units, \
                 DesiredRotation, and a turn that collision blocks"
            ),
            ENGINE_RECORD,
        )),
        other => Err(NotYet::not_read(format!("the {other} physics of a placed {class}"), ENGINE_RECORD)),
    }
}

/// Installs the host's physics step on `world` (`World::set_physics_step`): [`physics_rule`] for each actor, with
/// `bindings` from [`matinee_bound`] over the loaded map. While any linked variable is one that could hold an actor
/// and was not read ([`MatineeBindings::unreadable`]), every interpolating actor is [`Driver::Unknown`].
pub fn install_physics(world: &mut World, bindings: MatineeBindings) -> Result<(), String> {
    let bindings = Rc::new(bindings);
    world
        .set_physics_step(Rc::new(move |vm: &mut Vm, actor: ObjRef, _dt: f32| {
            let mode = physics_name(&vm.space, actor);
            let driver = if bindings.unreadable > 0 {
                Driver::Unknown
            } else if bindings.bound.contains(&actor) {
                Driver::Bound
            } else {
                Driver::Unbound
            };
            let class = ow_attr::eval::class_path(&vm.space, actor);
            physics_rule(&mode, &class, driver).map_err(VmError::NotYet)
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
    let name = PackageName::parse(&map).map_err(|e| format!("{map}: {e}"))?;
    let report = load_map(&mut world, &name).map_err(|e| format!("loading {map} into the world: {e}"))?;
    let bindings = matinee_bound(&mut world.vm).map_err(|e| format!("reading {map}'s Matinee bindings: {e}"))?;
    install_physics(&mut world, bindings)?;
    let actors = world.actors().borrow().list().len();
    let mut summary = LoadSummary::from_report(&map, mode, &report, actors);
    summary.load_time = started.elapsed();
    Ok((world, summary))
}

#[cfg(test)]
mod tests;
