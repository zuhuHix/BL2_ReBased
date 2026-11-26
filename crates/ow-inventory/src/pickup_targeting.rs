//! Owner: lane IMPL-PICKUP-TARGETING. AI-assisted (Claude Code). The per-tick pickup targeting of the player's HUD
//! (slice link 7: the aimed pickup is the seen one), coded in `ParityMode::Off` from NATIVE_PICKUP_TARGETING.md alone
//! (decision 2026-11-19-81; the note as accepted after review r3 of #461). This lane did not read the native. Every
//! rule is the note's reading of the executable, **UNVERIFIED** in the running game.
//!
//! [`tick`] is one pass of a `WillowHUD` (sections 1-8 of the note). It reads the HUD's `PlayerOwner`, `HUDDef` and
//! `WPlayerOwner` (the events' recipient), and the two pickups it keeps between ticks: `CachedHoverPickup` (`H`) and
//! `CachedBestPickup` (`S`). It writes `ClosestPickup`, `SavedLookAtInventory`, `H` and `S`, and each scored pickup's
//! `bHasPickupParticleOffset`. What the world must supply comes through a [`Host`]: the owner's view point and
//! rotation, a pickup's component bounds and the world line check. The headless world has none of these, so
//! [`Headless`] answers each with a named `NotYet { LayerAbsent }` (the hub's ruling: never a fitted choice).
//! [`hud_of`] finds the player's HUD (`PlayerController.myHUD`); the headless world spawns none, which is a named
//! `NotYet { LayerAbsent }` too.
//!
//! **Plan first, then write** (the note's section 10, the hub's rulings). The pass runs the whole tick read-only first,
//! in the native's order: every read, every decision and every refusal (`NotYet`) on the path it takes. The plan uses
//! read-only accessors only (review r1 of #467): it dispatches nothing through the VM.
//! - The getters it needs are answered by field reads. `GetInventoryDefinition` uses the exact-class rule of
//!   NATIVE_INVENTORY_DEFINITION.md, the same field reads as #462's native. `GetGlobalsDefinition` is the globals'
//!   `TheGlobalsDefinition` field (declared on `GearboxGlobals`), as NATIVE_WILLOW_GLOBALS.md (#469) reads it.
//! - The plan checks that the receiver's method resolves to the native declaration. A script override, or any other
//!   function, is a named `NotYet { NotRead }`.
//! - The `Host` gets a shared view of the VM. The globals come from a [`Globals`] provider that also gets a shared
//!   view. Its Off form, [`WorldGlobals`], is prepared before the pass: it reads the world's object and its active
//!   definition field, with no script, no binding and no definition-store lookup (review r3 of #467).
//! - The planned handler call is built with its exact arguments and validated with `Vm::check_call`, then stored for
//!   the commit.
//! - The plan's mutable access is for resolving classes and methods (loading their code) and for def-store lookups,
//!   which change no object the pass reads.
//! - The planned event's handler is resolved, its parameters checked and its binding prepared, all in the plan.
//!
//! Only then does it write, in the native's order:
//! - `ClosestPickup` (emptied, and refilled while scoring), then each scored pickup's `bHasPickupParticleOffset`;
//! - no winner (section 6): `SavedLookAtInventory` emptied, `ClearSeenPickupable` raised when `S` is set and then `S`
//!   emptied, then `H` emptied;
//! - a winner (section 7): `SavedLookAtInventory`, then `H`, then `SawPickupable` raised and then `S` written.
//!
//! The note's summary list puts `H` before the events in both branches; this module follows sections 6 and 7, which
//! give the no-winner order as the clear first and `H` last. An event runs script, so it counts as a write. A `NotYet`
//! after a write is terminal: its text starts "stopped after partial writes", its kind and record are kept, and the
//! writes made stay (no rollback). Other errors pass through unchanged.
//!
//! `WPlayerOwner` is read when the pass starts; a None value is refused (`NotYet { NotRead }`) only when the plan raises
//! an event.
//!
//! Choices of this port, none a rule of the game:
//! - **Suspension:** the owner's `Dead` state is checked first; when it holds, the pawn's suspend query is not asked.
//!   The query is answered only for the concrete class `WillowPlayerPawn` driving nothing (the note: "no"). Any other
//!   pawn, a subclass included, and a driven vehicle are refused (open question 1).
//! - **The shrink bookkeeping** (section 8; its rule read in NATIVE_LINK7_HUD_TARGETING.md section 7, lane
//!   IMPL-LINK7-HUD-TARGETING) runs whenever the pass has a pawn, suspended or not, over every counted pickup. It is
//!   enabled when the HUD has a `WorldInfo` whose `NetMode` is not the client's (3) and the owner is not a split-screen
//!   player (a `Player` of None: no; a set one needs the engine's local-player count, [`SPLIT_SCREEN`] `NotRead`).
//!   Each counted pickup whose `LifeSpan` is above the `WillowPickup` default's `PickupShrinkDuration` counts; the one
//!   with the smallest such `LifeSpan` is tracked. When the count is above the HUD's `MaxNumPickups` (by fit),
//!   `BeginShrinking` is raised on it, resolved and checked in the plan and raised after the other writes.
//! - **No pawn:** the native refills `ClosestPickup` over the HUD's `PostRenderedActors`. That slot has no reader on the
//!   targeting path (the note: "not needed"), so Off leaves it emptied.
//! - **The pickup's rotation** for the particle offset uses the same sine table as the view (the engine's rotation
//!   matrix convention, UE3's forward, right and up axes). The note names the table for the view only. UNVERIFIED.
//! - **Precision:** single precision throughout, with exact square roots where the native uses a refined fast
//!   reciprocal square root (the note's Off answer). The sine table's entries are double sines rounded to single. A
//!   case within a few units in the last place of a cutoff is not decided bit for bit (UNVERIFIED).

use ow_core::NotYet;
use ow_object::ids::{ClassId, FuncId, ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::equip::{bad, field, object_of, object_prop, required, write};

/// The record these rules follow.
pub const RECORD: &str = "NATIVE_PICKUP_TARGETING.md";
/// The split-screen test's refusal for an owner with a `Player` (NATIVE_LINK7_HUD_TARGETING.md section 7).
pub const SPLIT_SCREEN: &str = "the shrink bookkeeping's split-screen test: an owner with a Player needs the engine's \
     local-player count";
/// The record of the shrink rule.
pub const RECORD_SHRINK: &str = "NATIVE_LINK7_HUD_TARGETING.md (section 7)";

/// The hub's ruling on review of #806 (NATIVE_LINK7_HUD_TARGETING.md section 8): the line check under a `NoLevel`
/// service answers no hit, `crate::hud_harness::HarnessHost` applying it (a VM service; its presence turns it on). Off
/// only; the slice's two `NoLevel` worlds turn it on.
#[derive(Debug, Clone, Copy, Default)]
pub struct NoLevelLineCheck;

/// The set-up line of [`enable_no_level_line_check`].
pub const NO_LEVEL_LINE_INPUT: &str = "harness input: the world turns on the pickup targeting's no-hit line check for a \
     world with no level geometry (the hub's ruling on review of #806; UNVERIFIED)";

/// Turns [`NoLevelLineCheck`] on, logging [`NO_LEVEL_LINE_INPUT`] (Off only, else a registration refusal; twice is a
/// registration refusal too).
pub fn enable_no_level_line_check(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ow_core::ParityMode::Off {
        return Err(VmError::Registration {
            key: "NoLevelLineCheck".to_string(),
            reason: "the no-hit line check is an Off harness setting; Reference keeps the refusal",
        });
    }
    vm.add_service(NoLevelLineCheck)?;
    vm.log.push(NO_LEVEL_LINE_INPUT.to_string());
    Ok(())
}

/// The text that marks a refusal after the pass's first write.
pub const PARTIAL_WRITES: &str = "stopped after partial writes";

const WHAT: &str = "the pickup targeting";
const PLAYER_PAWN: &str = "WillowGame.WillowPlayerPawn";
const PICKUP: &str = "WillowGame.WillowPickup";

/// The owner's player view point: a location and a rotation (rotator units, 65536 to the turn).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub location: [f32; 3],
    pub pitch: i32,
    pub yaw: i32,
}

/// What the pass did this tick.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// No owner or no HUD definition: the pass ends (only `ClosestPickup` is emptied).
    Idle,
    /// The owner has no pawn: no event, `H` and `S` unchanged.
    NoPawn,
    /// No winner (section 6): `ClearSeenPickupable` raised when a pickup was reported; `H` and `S` emptied.
    NoWinner { cleared: bool },
    /// A winner, but no definition or the 0.5 view check failed (section 7): `H` and `S` unchanged.
    Unchanged { winner: ObjRef },
    /// A winner, now `H`; `reported` when `SawPickupable` was raised for it this tick.
    Hover { winner: ObjRef, reported: bool },
}

/// What the world supplies to the pass (module documentation), through a shared, read-only view of the VM (review r1
/// of #467).
pub trait Host {
    /// The owner's player view point and rotation.
    fn view(&mut self, vm: &Vm, owner: ObjRef) -> Result<View, VmError>;
    /// The height of the bounding box of `pickup`'s components.
    fn bounds_height(&mut self, vm: &Vm, pickup: ObjRef) -> Result<f32, VmError>;
    /// The first actor the world line check from `from` to `to` hits, None when it hits nothing.
    fn line_blocker(&mut self, vm: &Vm, from: [f32; 3], to: [f32; 3]) -> Result<Option<ObjRef>, VmError>;
    /// [`Host::line_blocker`] for the line check of section 5, told the owner's `pawn` and the `target` pickup (a
    /// host that must ignore them; lane IMPL-LINK7-HUD-TARGETING). By default the plain line check.
    fn line_blocker_for(
        &mut self,
        vm: &Vm,
        from: [f32; 3],
        to: [f32; 3],
        _pawn: ObjRef,
        _target: ObjRef,
    ) -> Result<Option<ObjRef>, VmError> {
        self.line_blocker(vm, from, to)
    }
}

/// The headless world: no camera, no component bounds, no collision (module documentation).
#[derive(Debug, Clone, Copy, Default)]
pub struct Headless;

impl Host for Headless {
    fn view(&mut self, _: &Vm, _: ObjRef) -> Result<View, VmError> {
        Err(NotYet::layer_absent(
            "the pickup targeting needs the owner's player view point and rotation (section 1)",
            "a camera (the headless world has none)",
            RECORD,
        )
        .into())
    }

    fn bounds_height(&mut self, _: &Vm, _: ObjRef) -> Result<f32, VmError> {
        Err(NotYet::layer_absent(
            "the pickup targeting's score needs the pickup's component bounds (section 4)",
            "component bounds (the headless world has none)",
            RECORD,
        )
        .into())
    }

    fn line_blocker(&mut self, _: &Vm, _: [f32; 3], _: [f32; 3]) -> Result<Option<ObjRef>, VmError> {
        Err(NotYet::layer_absent(
            "the pickup targeting's line check beyond a quarter of PickupRange (section 5)",
            "collision (the headless world has none)",
            RECORD,
        )
        .into())
    }
}

/// The player's HUD (`PlayerController.myHUD`); none is a named `NotYet { LayerAbsent }` (the note's look order,
/// step 2).
pub fn hud_of(vm: &Vm, controller: ObjRef) -> Result<ObjRef, VmError> {
    object_prop(vm, controller, "myHUD", WHAT)?.ok_or_else(|| {
        NotYet::layer_absent(
            "the pickup targeting is a per-tick pass of the player's HUD, its owner and its HUDDef (section 1)",
            "a HUD (the headless world spawns none)",
            RECORD,
        )
        .into()
    })
}

/// The game's globals object and its `GlobalsDefinition`, as the pass needs them when it reaches the pickup list. The
/// plan only reads: the provider answers from what was prepared before the pass (review r3 of #467).
pub trait Globals {
    fn resolve(&self, vm: &Vm) -> Result<(ObjRef, ObjRef), VmError>;
}

/// The Off world's globals, prepared before the pass ([`WorldGlobals::prepare`]): the world's `WillowGlobals` and its
/// active definition, the `TheGlobalsDefinition` field (declared on `GearboxGlobals`; the world's set-up fills it,
/// NATIVE_WILLOW_GLOBALS.md section 6). Whatever refused while preparing is answered only when the pass reaches the
/// pickup list.
#[derive(Debug, Clone)]
pub struct WorldGlobals(Result<(ObjRef, ObjRef), VmError>);

/// The class `TheGlobalsDefinition` must hold for the getter to be a field read.
pub const GLOBALS_DEFINITION_CLASS: &str = "WillowGame.GlobalsDefinition";

impl WorldGlobals {
    /// Resolves, before the pass:
    /// - the world's globals object (the world's `NotYet { LayerAbsent }` without one);
    /// - that its `GetGlobalsDefinition` is the native declaration;
    /// - its active definition. That field holding no `GlobalsDefinition` is a named `NotYet { NotRead }`: the native's
    ///   first load numbers the challenge list, which was not read (section 4).
    ///
    /// It reads fields and resolves classes and methods. It runs no script, binds nothing and looks nothing up in the
    /// definition store.
    pub fn prepare(vm: &mut Vm) -> Self {
        WorldGlobals((|| {
            let globals = {
                let cx = ow_attr::api::EvalCx::new(vm);
                cx.world.globals()?
            };
            native_getter(vm, globals, "GetGlobalsDefinition", "WillowGame.WillowGlobals.GetGlobalsDefinition")?;
            let class = table_class(vm, GLOBALS_DEFINITION_CLASS)?;
            match object_field(vm, globals, "TheGlobalsDefinition")? {
                Some(d) if vm.space.classes.is_child_of(class_of(vm, d)?, class) => Ok((globals, d)),
                _ => Err(refused(
                    "the globals' TheGlobalsDefinition holds no GlobalsDefinition, and the getter's first load (which \
                     numbers the challenge list) was not read (NATIVE_WILLOW_GLOBALS.md section 4)",
                )),
            }
        })())
    }
}

impl Globals for WorldGlobals {
    fn resolve(&self, _: &Vm) -> Result<(ObjRef, ObjRef), VmError> {
        self.0.clone()
    }
}

/// Link 7's `look` after the fixture's own check (the note's look order, steps 2-4): the player's HUD (none: a named
/// `NotYet`), then one pass of it with `host`, the Off world's globals resolved only when the pass reaches them.
pub fn look(vm: &mut Vm, controller: ObjRef, host: &mut dyn Host) -> Result<Outcome, VmError> {
    let hud = hud_of(vm, controller)?;
    let globals = WorldGlobals::prepare(vm);
    tick(vm, hud, &globals, host)
}

/// `e` as a refusal after a write: a `NotYet` marked once (at the start of its text), its kind and record kept; any
/// other error unchanged.
pub fn after_writes(e: VmError) -> VmError {
    match e {
        VmError::NotYet(n) if !n.what.starts_with(PARTIAL_WRITES) => {
            NotYet::new(format!("{PARTIAL_WRITES}: {}", n.what), n.missing, n.record).into()
        }
        other => other,
    }
}

type V3 = [f32; 3];

fn sub(a: V3, b: V3) -> V3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}
fn add(a: V3, b: V3) -> V3 {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}
fn scale(a: V3, k: f32) -> V3 {
    [a[0] * k, a[1] * k, a[2] * k]
}
fn dot(a: V3, b: V3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn sqrt(x: f32) -> f32 {
    // A double square root rounded to single is the correctly rounded single square root.
    ow_core::num::sqrt_f64(f64::from(x)) as f32
}
fn length(a: V3) -> f32 {
    sqrt(dot(a, a))
}
fn horizontal(a: V3) -> V3 {
    [a[0], a[1], 0.0]
}

/// The engine's sine table entry for `units` (16384 steps to the turn: the units shifted right by two).
fn table_sin(units: i32) -> f32 {
    let step = (units >> 2) & 16383;
    ow_core::num::sin_f64(f64::from(step) * std::f64::consts::TAU / 16384.0) as f32
}

/// The table's cosine: the sine a quarter turn on.
fn table_cos(units: i32) -> f32 {
    table_sin(units.wrapping_add(16384))
}

/// The full view direction (pitch and yaw), from the sine table.
pub fn view_direction(view: &View) -> V3 {
    let (sp, cp) = (table_sin(view.pitch), table_cos(view.pitch));
    let (sy, cy) = (table_sin(view.yaw), table_cos(view.yaw));
    [cp * cy, cp * sy, sp]
}

/// `a` turned by the rotator `(pitch, yaw, roll)`: its x, y and z along the rotator's forward, right and up axes.
pub fn rotate(a: V3, pitch: i32, yaw: i32, roll: i32) -> V3 {
    let (sp, cp) = (table_sin(pitch), table_cos(pitch));
    let (sy, cy) = (table_sin(yaw), table_cos(yaw));
    let (sr, cr) = (table_sin(roll), table_cos(roll));
    let forward = [cp * cy, cp * sy, sp];
    let right = [sr * sp * cy - cr * sy, sr * sp * sy + cr * cy, -sr * cp];
    let up = [-(cr * sp * cy + sr * sy), cy * sr - cr * sp * sy, cr * cp];
    add(add(scale(forward, a[0]), scale(right, a[1])), scale(up, a[2]))
}

/// `a` made a unit vector when its squared length is above 1e-8, else as it is (section 3's cutoff).
fn unit_or_as_is(a: V3) -> V3 {
    let squared = dot(a, a);
    if squared > 1e-8 { scale(a, 1.0 / sqrt(squared)) } else { a }
}

/// The cosine of section 3 and section 7: `look` is a unit vector, the other direction is normalised by the cutoff.
fn cosine(look: V3, to: V3) -> f32 {
    dot(look, unit_or_as_is(to))
}

/// The distance between the closest points of the segment `bottom`-`top` and the ray piece `q0`-`q1`, None when there
/// is no `d` (section 4's cutoffs: a near-zero piece, parallel lines, a closest point outside its piece).
pub fn segment_distance(bottom: V3, top: V3, q0: V3, q1: V3) -> Option<f32> {
    let (s, r) = (sub(top, bottom), sub(q1, q0));
    let near_zero = |v: V3| v.iter().all(|c| c.abs() < 0.0001);
    if near_zero(s) || near_zero(r) {
        return None;
    }
    let (a, b, c) = (dot(s, s), dot(s, r), dot(r, r));
    let denominator = a * c - b * b;
    if denominator.abs() < 1e-8 {
        return None;
    }
    let w0 = sub(bottom, q0);
    let (d, e) = (dot(s, w0), dot(r, w0));
    let along_s = (b * e - c * d) / denominator;
    let along_r = (a * e - b * d) / denominator;
    if !(0.0..=1.0).contains(&along_s) || !(0.0..=1.0).contains(&along_r) {
        return None;
    }
    Some(length(sub(add(bottom, scale(s, along_s)), add(q0, scale(r, along_r)))))
}

/// The score of section 4 for a distance `d` and a capture radius `radius`: 0 without a `d`, beyond the radius, or with
/// a radius of zero or less.
pub fn score(d: Option<f32>, radius: f32) -> f32 {
    match d {
        Some(d) if radius > 0.0 && d <= radius => (radius - d) / radius,
        _ => 0.0,
    }
}

fn refused(what: impl Into<String>) -> VmError {
    NotYet::not_read(what.into(), RECORD).into()
}

fn float(vm: &Vm, o: ObjRef, name: &str) -> Result<f32, VmError> {
    match required(vm, o, name, WHAT)? {
        Value::Float(x) => Ok(x),
        other => Err(bad(WHAT, format!("{name} is {}, not a float", other.kind_name()))),
    }
}

fn flag(vm: &Vm, o: ObjRef, name: &str) -> Result<bool, VmError> {
    match required(vm, o, name, WHAT)? {
        Value::Bool(b) => Ok(b),
        other => Err(bad(WHAT, format!("{name} is {}, not a bool", other.kind_name()))),
    }
}

/// An object property that the class must declare (None when it holds none).
fn object_field(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    object_of(&required(vm, o, name, WHAT)?, WHAT)
}

fn vector_of(vm: &Vm, v: &Value) -> Result<V3, VmError> {
    let mut out = [0.0; 3];
    for (slot, axis) in out.iter_mut().zip(["X", "Y", "Z"]) {
        *slot = match field(vm, v, axis)? {
            Value::Float(x) => x,
            other => return Err(bad(WHAT, format!("a vector's {axis} is {}", other.kind_name()))),
        };
    }
    Ok(out)
}

fn location(vm: &Vm, o: ObjRef) -> Result<V3, VmError> {
    vector_of(vm, &required(vm, o, "Location", WHAT)?)
}

fn rotation(vm: &Vm, o: ObjRef) -> Result<(i32, i32, i32), VmError> {
    let r = required(vm, o, "Rotation", WHAT)?;
    let mut out = [0; 3];
    for (slot, axis) in out.iter_mut().zip(["Pitch", "Yaw", "Roll"]) {
        *slot = match field(vm, &r, axis)? {
            Value::Int(i) => i,
            other => return Err(bad(WHAT, format!("a rotator's {axis} is {}", other.kind_name()))),
        };
    }
    Ok((out[0], out[1], out[2]))
}

fn class_of(vm: &Vm, o: ObjRef) -> Result<ClassId, VmError> {
    Ok(vm.space.heap.get(o)?.class)
}

/// `name` on `o` as its class and current state resolve it, read-only (review of #810, item 2): a class of the chain
/// whose code is not loaded is a named `NotYet { NotRead }` (a load is a write), no function a data error.
fn method_of(vm: &Vm, o: ObjRef, name: &str) -> Result<FuncId, VmError> {
    let (class, state) = {
        let h = vm.space.heap.get(o)?;
        (h.class, h.state)
    };
    match vm.find_loaded_method(class, name, state) {
        None => Err(refused(format!(
            "the code of {name} on {} is not loaded (a load is a write)",
            ow_attr::eval::class_path(&vm.space, o)
        ))),
        Some(f) => f.ok_or_else(|| bad(WHAT, format!("{} has no {name}", ow_attr::eval::class_path(&vm.space, o)))),
    }
}

/// The class at `path` in the class table, read-only: not there is a named `NotYet { NotRead }` (a class load is a
/// write; review of #810, item 2).
fn table_class(vm: &Vm, path: &str) -> Result<ClassId, VmError> {
    vm.space
        .classes
        .by_path(path)
        .ok_or_else(|| refused(format!("the class {path} is not in the class table (a class load is a write)")))
}

/// `class`'s class default object when it is already built with all its slots, read-only: none is a named
/// `NotYet { NotRead }` (building it is a write; review of #810, item 2).
fn built_default(vm: &Vm, class: ClassId) -> Result<ObjRef, VmError> {
    let slots = vm.space.classes.get_layout(class).map_or(0, |l| l.slot_count() as usize);
    vm.space
        .heap
        .handles()
        .into_iter()
        .find(|&o| {
            vm.space.heap.get(o).is_ok_and(|x| {
                x.class == class && matches!(x.origin, ow_object::heap::Origin::Default { .. }) && x.slots.len() >= slots
            })
        })
        .ok_or_else(|| {
            refused(format!(
                "the class default of {} is not built (building it is a write)",
                vm.space.classes.get_layout(class).map_or_else(String::new, |l| l.path().to_string())
            ))
        })
}

/// Checks, without running anything, that `name` on `o` resolves to the native declaration `declaration` (review r1
/// of #467): an override in script, or any other function, is a named `NotYet { NotRead }`, since the plan answers
/// the getter by field reads only.
fn native_getter(vm: &mut Vm, o: ObjRef, name: &str, declaration: &str) -> Result<(), VmError> {
    let f = method_of(vm, o, name)?;
    let d = vm.function(f).ok_or_else(|| bad(WHAT, format!("{name} has no declaration")))?;
    if d.native_key.is_none() || !d.path.as_str().eq_ignore_ascii_case(declaration) {
        return Err(refused(format!(
            "{name} on a {} resolves to {}, not the native {declaration}: the pass reads that getter's fields only",
            ow_attr::eval::class_path(&vm.space, o),
            d.path.as_str()
        )));
    }
    Ok(())
}

/// The weapon classes whose `GetInventoryDefinition` reads `DefinitionData.WeaponTypeDefinition`
/// (NATIVE_INVENTORY_DEFINITION.md section 2): #462's allowlist at 8e59258.
const WEAPON_CLASSES: [&str; 4] = [
    "WillowGame.WillowWeapon",
    "WillowGame.WillowBuzzaxeWeapon",
    "WillowGame.WillowTurretWeapon",
    "WillowGame.WillowVehicleWeapon",
];
/// The item classes whose `GetInventoryDefinition` reads `DefinitionData.ItemDefinition` (as above).
const ITEM_CLASSES: [&str; 9] = [
    "WillowGame.WillowItem",
    "WillowGame.WillowEquipAbleItem",
    "WillowGame.WillowArtifact",
    "WillowGame.WillowClassMod",
    "WillowGame.WillowGrenadeMod",
    "WillowGame.WillowShield",
    "WillowGame.WillowUsableItem",
    "WillowGame.WillowMissionItem",
    "WillowGame.WillowUsableCustomizationItem",
];
const GET_INVENTORY_DEFINITION: &str = "Engine.WillowInventory.GetInventoryDefinition";
/// #462's record for `GetInventoryDefinition`.
const DEFINITION_RECORD: &str = "docs/verification/NATIVE_INVENTORY_DEFINITION.md (GetInventoryDefinition, the Off answers)";

/// The definition of the pickup's inventory, read as `GetInventoryDefinition` answers it but by field reads only. None
/// without an inventory. The receiver's method must resolve to the native declaration (the guard kept from review r1
/// of #467). Nothing is run.
fn definition_of(vm: &mut Vm, pickup: ObjRef) -> Result<Option<ObjRef>, VmError> {
    let Some(inventory) = object_field(vm, pickup, "Inventory")? else { return Ok(None) };
    native_getter(vm, inventory, "GetInventoryDefinition", GET_INVENTORY_DEFINITION)?;
    inventory_definition_of(vm, inventory)
}

/// A copy of #462's read-only accessor `ow_inventory::definition::inventory_definition_of` (fix/rust-inventory-definition
/// at 8e59258, NATIVE_INVENTORY_DEFINITION.md section 3), with the same rule, texts and record. #462 is not on main,
/// which this branch now targets; once it lands, this copy is replaced by a call to that function (review r3 of #467,
/// item 5).
/// - The exact class chooses the field. Exactly `Engine.WillowInventory` answers None. Any other class is a named
///   `NotYet { NotRead }`.
/// - A missing layout is a named `NotYet { NotRead }`; a malformed one is a data error.
fn inventory_definition_of(vm: &Vm, me: ObjRef) -> Result<Option<ObjRef>, VmError> {
    const NATIVE: &str = "GetInventoryDefinition";
    let not_read = |what: String| -> VmError { NotYet::not_read(what, DEFINITION_RECORD).into() };
    let class = vm.space.heap.get(me)?.class;
    let classes = &vm.space.classes;
    let is = |paths: &[&str]| paths.iter().any(|p| classes.by_path(p) == Some(class));
    let name = if is(&WEAPON_CLASSES) {
        "WeaponTypeDefinition"
    } else if is(&ITEM_CLASSES) {
        "ItemDefinition"
    } else if is(&["Engine.WillowInventory"]) {
        return Ok(None);
    } else {
        let path = classes.layout(class).path().as_str().to_string();
        return Err(not_read(format!("{NATIVE} on a {path}: a class whose engine rule was not read")));
    };
    let prop = vm
        .space
        .prop(class, "DefinitionData")
        .ok_or_else(|| not_read(format!("{NATIVE} on a class without DefinitionData")))?;
    let decl = classes.decl(prop).ok_or_else(|| bad(NATIVE, "DefinitionData has no declaration"))?;
    let ty = match (&decl.kind, decl.array_dim) {
        (PropKind::Struct(ty), 1) => *ty,
        (kind, dim) => return Err(bad(NATIVE, format!("DefinitionData is declared as {kind:?} x {dim}, not one struct"))),
    };
    let field_decl = vm
        .space
        .names
        .find(name)
        .and_then(|id| classes.struct_layout(ty).and_then(|l| l.field(id).map(|(_, d)| (d.kind.clone(), d.array_dim))));
    match field_decl {
        None => return Err(not_read(format!("{NATIVE} on a DefinitionData without {name}"))),
        Some((PropKind::Object { .. }, 1)) => {}
        Some((kind, dim)) => {
            return Err(bad(
                NATIVE,
                format!("DefinitionData.{name} is declared as {kind:?} x {dim}, not one object reference"),
            ));
        }
    }
    let data = vm.space.read(me, PropAddr::scalar(prop))?.clone();
    match field(vm, &data, name)? {
        Value::Object(o) => Ok(o),
        other => Err(bad(NATIVE, format!("DefinitionData.{name} holds {}, not an object reference", other.kind_name()))),
    }
}

/// The pickup's particle offset (section 4) and whether it is non-zero (so the pass sets `bHasPickupParticleOffset`).
pub fn particle_offset(vm: &mut Vm, pickup: ObjRef) -> Result<(V3, bool), VmError> {
    let Some(definition) = definition_of(vm, pickup)? else { return Ok(([0.0; 3], false)) };
    let offset = vector_of(vm, &required(vm, definition, "PickupFlagOffset", WHAT)?)?;
    if offset == [0.0; 3] {
        return Ok((offset, false));
    }
    let (p, y, r) = rotation(vm, pickup)?;
    Ok((rotate(offset, p, y, r), true))
}

/// Whether scoring is suspended this tick (section 2; module documentation).
fn suspended(vm: &mut Vm, owner: ObjRef, pawn: ObjRef) -> Result<bool, VmError> {
    // The state name compared by its text, as names compare (without case), so nothing is interned on a pass (lane
    // COLD-LOAD-FIXES; the audit's side finding on #701).
    if vm.space.heap.get(owner)?.state.is_some_and(|s| vm.space.names.text(s).eq_ignore_ascii_case("Dead")) {
        return Ok(true);
    }
    if object_field(vm, pawn, "DrivenVehicle")?.is_some() {
        return Err(refused("the suspend query of a driven vehicle (open question 1)"));
    }
    // The class table by path, read only (lane COLD-LOAD-FIXES): a resident WillowPlayerPawn pawn means the table holds
    // that path, so no class code is loaded for the comparison.
    if vm.space.classes.by_path(PLAYER_PAWN) != Some(class_of(vm, pawn)?) {
        return Err(refused(format!(
            "the suspend query of a {} (only a plain WillowPlayerPawn is answered; open question 1)",
            ow_attr::eval::class_path(&vm.space, pawn)
        )));
    }
    Ok(false)
}

/// An event the plan raises on the recipient, with the handler it resolved and checked.
#[derive(Debug, Clone, PartialEq)]
enum Event {
    Saw(ObjRef, Checked),
    ClearSeen(Checked),
}

/// A handler call built and checked in the plan (`Vm::check_call`): the function and its exact arguments.
#[derive(Debug, Clone, PartialEq)]
struct Checked {
    f: FuncId,
    args: Vec<Value>,
}

/// Everything the tick writes, decided before the first write.
struct Plan {
    closest: Option<ObjRef>,
    flagged: Vec<ObjRef>,
    /// The tail after the scoring writes (sections 6 and 7); None when the pass ends earlier.
    tail: Option<Tail>,
    /// The shrink step's event: the pickup and its checked `BeginShrinking` call, raised last.
    shrink: Option<(ObjRef, Checked)>,
    outcome: Outcome,
}

/// Sections 6 and 7, in write order: the seen-item slot, `H` (a winner), the event and its write to `S`, `H` (no
/// winner), and an out-of-range winner's second write of the seen-item slot.
struct Tail {
    seen_item: Option<ObjRef>,
    hover_before: Option<Option<ObjRef>>,
    event: Option<Event>,
    hover_after: Option<Option<ObjRef>>,
    seen_item_after: Option<Option<ObjRef>>,
}

/// The HUD's state the pass reads first.
struct HudState {
    hud: ObjRef,
    owner: Option<ObjRef>,
    definition: Option<ObjRef>,
    recipient: Option<ObjRef>,
    hover: Option<ObjRef>,
    reported: Option<ObjRef>,
}

fn hud_state(vm: &Vm, hud: ObjRef) -> Result<HudState, VmError> {
    Ok(HudState {
        hud,
        owner: object_field(vm, hud, "PlayerOwner")?,
        definition: object_field(vm, hud, "HUDDef")?,
        recipient: object_field(vm, hud, "WPlayerOwner")?,
        hover: object_field(vm, hud, "CachedHoverPickup")?,
        reported: object_field(vm, hud, "CachedBestPickup")?,
    })
}

/// One pass of `hud` (module documentation), the globals resolved by `globals` when the pass reaches the pickup list.
pub fn tick(vm: &mut Vm, hud: ObjRef, globals: &dyn Globals, host: &mut dyn Host) -> Result<Outcome, VmError> {
    let state = hud_state(vm, hud)?;
    // A refusal carries the partial-writes prefix only when a write really came before it (review of #810, item 1):
    // the counters (heap writes, random draws, load transitions, class code loads) taken before the plan.
    let marks = |vm: &Vm| (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions(), vm.class_code_loads());
    let entry = marks(vm);
    let run = (|| {
        let plan = plan(vm, &state, globals, host)?;
        let mut wrote = false;
        commit(vm, hud, plan, &mut wrote)
    })();
    run.map_err(|e| if marks(vm) != entry { after_writes(e) } else { e })
}

fn plan(vm: &mut Vm, h: &HudState, globals: &dyn Globals, host: &mut dyn Host) -> Result<Plan, VmError> {
    let ended = |outcome| Plan { closest: None, flagged: Vec::new(), tail: None, shrink: None, outcome };
    let (Some(owner), Some(hud_definition)) = (h.owner, h.definition) else { return Ok(ended(Outcome::Idle)) };
    let view = host.view(vm, owner)?;
    let Some(pawn) = object_field(vm, owner, "Pawn")? else { return Ok(ended(Outcome::NoPawn)) };
    let suspended = suspended(vm, owner, pawn)?;

    let (globals, gd) = globals.resolve(vm)?;
    let list = match required(vm, globals, "PickupList", WHAT)? {
        Value::Array(a) => a.as_ref().clone(),
        other => return Err(bad(WHAT, format!("PickupList is {}, not an array", other.kind_name()))),
    };
    let hover_particle = object_field(vm, gd, "PartItemHover")?.is_some();
    let (width, height) = (float(vm, gd, "HoverWidthPickup")?, float(vm, gd, "ItemHoverProjectionHeight")?);
    let range = float(vm, hud_definition, "PickupRange")?;
    // Read-only (review of #810, item 2): the pickup class from the table, its default already built.
    let pickup_class = table_class(vm, PICKUP)?;
    let shrink_duration = float(vm, built_default(vm, pickup_class)?, "PickupShrinkDuration")?;

    let v = view.location;
    let look = view_direction(&view);
    let look_flat = [table_cos(view.yaw), table_sin(view.yaw), 0.0];
    let mut closest: Option<(f32, ObjRef)> = None;
    let mut flagged = Vec::new();
    let mut best: (f32, Option<ObjRef>) = (0.0, None);
    // The shrink step's count and the tracked pickup (smallest LifeSpan above the duration).
    let mut counted = 0i64;
    let mut tracked: Option<(f32, ObjRef)> = None;
    for entry in &list {
        let Some(p) = object_of(entry, WHAT)? else { continue };
        if !vm.space.classes.is_child_of(class_of(vm, p)?, pickup_class) || !flag(vm, p, "bPickupable")? {
            continue;
        }
        let life = float(vm, p, "LifeSpan")?;
        if life > shrink_duration {
            counted += 1;
            if tracked.is_none_or(|(t, _)| life < t) {
                tracked = Some((life, p));
            }
        }
        if suspended {
            continue;
        }
        let l = location(vm, p)?;
        let to = sub(l, v);
        let distance = length(to);
        if object_field(vm, p, "Inventory")?.is_some() && closest.is_none_or(|(d, _)| distance < d) {
            closest = Some((distance, p));
        }
        if !(hover_particle || dot(to, to) < range * range) {
            continue;
        }
        if cosine(look_flat, horizontal(to)) <= 0.9 {
            continue;
        }
        let current = h.hover == Some(p);
        let (offset, nonzero) = particle_offset(vm, p)?;
        if nonzero {
            flagged.push(p);
        }
        let centre = add(l, offset);
        let half = host.bounds_height(vm, p)? / 2.0;
        let top = add(centre, [0.0, 0.0, height * if current { 1.25 } else { 1.0 }]);
        let bottom = sub(centre, [0.0, 0.0, half * if current { 1.5 } else { 0.75 }]);
        let ray_end = add(v, scale(look, 100.0 * length(sub(top, v))));
        let radius = width * 0.1 * if current { 1.5 } else { 1.0 };
        let s = score(segment_distance(bottom, top, v, ray_end), radius);
        if s <= 0.0 {
            continue;
        }
        let value = (s + if current { 0.25 } else { 0.0 }) / distance;
        if value > best.0 {
            best = (value, Some(p));
        }
    }
    let winner = match best.1 {
        Some(w) => checked_winner(vm, pawn, w, v, range, host)?,
        None => None,
    };
    let (tail, outcome) = match winner {
        None => {
            let event = match h.reported {
                Some(_) => Some(Event::ClearSeen(handler(vm, h.recipient, "ClearSeenPickupable", None)?)),
                None => None,
            };
            let tail =
                Tail { seen_item: None, hover_before: None, event: None, hover_after: Some(None), seen_item_after: None };
            let cleared = event.is_some();
            let tail = Tail { event, ..tail };
            (tail, Outcome::NoWinner { cleared })
        }
        Some(w) => winner_tail(vm, h, w, v, look, look_flat, range, hover_particle)?,
    };
    // The shrink step after the winner's and the event's checks (review r1 of #467, the note's plan order).
    let shrink = shrink_step(vm, h, owner, counted, tracked.map(|t| t.1))?;
    Ok(Plan { closest: closest.map(|c| c.1), flagged, tail: Some(tail), shrink, outcome })
}

/// The shrink bookkeeping (module documentation): the `BeginShrinking` event to raise, if any. Every field it reads is
/// declared with its kind, else a data error. Reads only.
fn shrink_step(
    vm: &mut Vm,
    h: &HudState,
    owner: ObjRef,
    counted: i64,
    tracked: Option<ObjRef>,
) -> Result<Option<(ObjRef, Checked)>, VmError> {
    // Whether shrinking is enabled first (review of #810, item 4); MaxNumPickups only when it is.
    let Some(info) = object_field(vm, h.hud, "WorldInfo")? else { return Ok(None) };
    let client = match required(vm, info, "NetMode", WHAT)? {
        Value::Byte(b) => b == 3,
        other => return Err(bad(WHAT, format!("NetMode is {}, not a byte", other.kind_name()))),
    };
    if client {
        return Ok(None);
    }
    if object_field(vm, owner, "Player")?.is_some() {
        return Err(NotYet::not_read(SPLIT_SCREEN, RECORD_SHRINK).into());
    }
    let max = match required(vm, h.hud, "MaxNumPickups", WHAT)? {
        Value::Int(i) => i64::from(i),
        other => return Err(bad(WHAT, format!("MaxNumPickups is {}, not an int", other.kind_name()))),
    };
    let Some(p) = tracked.filter(|_| counted > max) else { return Ok(None) };
    let f = method_of(vm, p, "BeginShrinking")?;
    vm.check_prepared(f)?;
    vm.check_call(f, Vec::new())?;
    Ok(Some((p, Checked { f, args: Vec::new() })))
}

/// The recipient's handler `name`, resolved and checked in the plan (reviews r1 and r3 of #467):
/// - a None recipient is a named `NotYet` (the native raises without a check);
/// - the handler takes no argument, or `pickup` as its one object or interface parameter;
/// - the exact call, its arguments as the commit passes them, is validated with `Vm::check_call` (the binding and
///   entry checks), so a refusal at call entry comes before any write.
fn handler(vm: &mut Vm, recipient: Option<ObjRef>, name: &str, pickup: Option<ObjRef>) -> Result<Checked, VmError> {
    let recipient = recipient.ok_or_else(|| {
        refused(format!("the HUD's WPlayerOwner is None and the tick raises {name} (the native raises without a check)"))
    })?;
    let f = method_of(vm, recipient, name)?;
    let kinds: Vec<PropKind> = vm.function(f).map(|d| d.params.iter().map(|p| p.kind.clone()).collect()).unwrap_or_default();
    let want = usize::from(pickup.is_some());
    let fits = kinds.len() == want
        && kinds.iter().all(|k| matches!(k, PropKind::Object { .. } | PropKind::Interface { .. }));
    if !fits {
        return Err(bad(WHAT, format!("{name}'s parameters are {kinds:?}, not {want} object reference(s)")));
    }
    vm.check_prepared(f)?;
    let args = match pickup {
        Some(w) => vec![handler_arg(vm, f, w)],
        None => Vec::new(),
    };
    vm.check_call(f, args.clone())?;
    Ok(Checked { f, args })
}

/// Section 5: the winner after the pawn test and the line check, None when either refuses it.
fn checked_winner(
    vm: &mut Vm,
    pawn: ObjRef,
    w: ObjRef,
    v: V3,
    range: f32,
    host: &mut dyn Host,
) -> Result<Option<ObjRef>, VmError> {
    // A WillowPlayerPawn here: any other pawn was refused at the suspend query.
    for name in ["bDeleteMe", "bIsInjured", "bIsDead"] {
        if flag(vm, pawn, name)? {
            return Ok(None);
        }
    }
    let to_w = sub(location(vm, w)?, v);
    if length(horizontal(to_w)) > range * 0.25 {
        let from = sub(add(v, to_w), scale(unit_or_as_is(to_w), 25.0));
        if let Some(hit) = host.line_blocker_for(vm, from, v, pawn, w)?
            && hit != w
            && Some(hit) != object_field(vm, w, "Base")?
        {
            return Ok(None);
        }
    }
    Ok(Some(w))
}

/// Section 7: what a winner's tick writes, and its outcome.
#[allow(clippy::too_many_arguments)]
fn winner_tail(
    vm: &mut Vm,
    h: &HudState,
    w: ObjRef,
    v: V3,
    look: V3,
    look_flat: V3,
    range: f32,
    hover_particle: bool,
) -> Result<(Tail, Outcome), VmError> {
    let seen_item = object_field(vm, w, "Inventory")?;
    let unchanged = Tail { seen_item, hover_before: None, event: None, hover_after: None, seen_item_after: None };
    if definition_of(vm, w)?.is_none() {
        return Ok((unchanged, Outcome::Unchanged { winner: w }));
    }
    let to_w = sub(location(vm, w)?, v);
    let c = if hover_particle { cosine(look_flat, horizontal(to_w)) } else { cosine(look, to_w) };
    if c <= 0.5 {
        return Ok((unchanged, Outcome::Unchanged { winner: w }));
    }
    let hover_before = (h.hover != Some(w)).then_some(Some(w));
    let in_range = dot(to_w, to_w) < range * range;
    let event = if in_range && h.reported != Some(w) {
        Some(Event::Saw(w, handler(vm, h.recipient, "SawPickupable", Some(w))?))
    } else {
        None
    };
    // Out of range: the seen-item slot took the winner's inventory first, and is emptied after `H` (section 7).
    let seen_item_after = (!in_range).then_some(None);
    let tail = Tail { seen_item, hover_before, event: None, hover_after: None, seen_item_after };
    let reported = event.is_some();
    let tail = Tail { event, ..tail };
    Ok((tail, Outcome::Hover { winner: w, reported }))
}

fn commit(vm: &mut Vm, hud: ObjRef, plan: Plan, wrote: &mut bool) -> Result<Outcome, VmError> {
    let obj = |o: Option<ObjRef>| Value::Object(o);
    write(vm, hud, "ClosestPickup", obj(plan.closest), WHAT)?;
    *wrote = true;
    for p in plan.flagged {
        write(vm, p, "bHasPickupParticleOffset", Value::Bool(true), WHAT)?;
    }
    // The shrink step's event, after the nearest-pickup and particle-offset writes and before the seen-item, hover and
    // reporting writes (the note's order; review of #810, item 1).
    if let Some((p, call)) = plan.shrink {
        vm.call(call.f, Some(p), call.args)?;
    }
    let Some(tail) = plan.tail else { return Ok(plan.outcome) };
    write(vm, hud, "SavedLookAtInventory", obj(tail.seen_item), WHAT)?;
    if let Some(hover) = tail.hover_before {
        write(vm, hud, "CachedHoverPickup", obj(hover), WHAT)?;
    }
    if let Some(event) = tail.event {
        let recipient = object_field(vm, hud, "WPlayerOwner")?.ok_or_else(|| bad(WHAT, "no recipient for the event"))?;
        match event {
            Event::Saw(w, call) => {
                vm.call(call.f, Some(recipient), call.args)?;
                write(vm, hud, "CachedBestPickup", obj(Some(w)), WHAT)?;
            }
            Event::ClearSeen(call) => {
                vm.call(call.f, Some(recipient), call.args)?;
                write(vm, hud, "CachedBestPickup", obj(None), WHAT)?;
            }
        }
    }
    if let Some(hover) = tail.hover_after {
        write(vm, hud, "CachedHoverPickup", obj(hover), WHAT)?;
    }
    if let Some(seen) = tail.seen_item_after {
        write(vm, hud, "SavedLookAtInventory", obj(seen), WHAT)?;
    }
    Ok(plan.outcome)
}

/// `SawPickupable`'s argument as its declared kind takes it (an interface or an object reference).
fn handler_arg(vm: &Vm, f: FuncId, w: ObjRef) -> Value {
    match vm.function(f).and_then(|d| d.params.first()).map(|p| &p.kind) {
        Some(PropKind::Interface { .. }) => Value::Interface(Some(w)),
        _ => Value::Object(Some(w)),
    }
}

#[cfg(test)]
#[path = "pickup_targeting_tests.rs"]
mod tests;
