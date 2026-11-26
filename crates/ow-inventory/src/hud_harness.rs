//! Owner: lane IMPL-LINK7-HUD-TARGETING (decision 2026-11-26-66). The inputs link 7's pickup targeting needs in the
//! headless pickup world, as the hub's four rulings on NATIVE_LINK7_HUD_TARGETING.md (#806) allow them: each Off only,
//! logged as a harness input, **UNVERIFIED**. AI-assisted (Claude Code).
//!
//! **Clean room.** Implemented from docs/verification/NATIVE_LINK7_HUD_TARGETING.md sections 3 to 5 (#806 as landed)
//! alone; this lane did not read the executable. The targeting pass itself is `crate::pickup_targeting`.
//!
//! 1. **The harness HUD** ([`harness_hud`], ruling 1, section 3): the world's `GameInfo.HUDType` spawned owned by the
//!    controller into `myHUD`, as `ClientSetHUD` spawns it; then the data steps only: `PlayerOwner` := the controller
//!    (`HUD.PostBeginPlay`), `WPlayerOwner` := the controller (`WillowHUD.PostBeginPlay`), `HUDDef` := the globals
//!    definition's `HUDDefinition` (`CreateHUDMovie`'s first step). `InitHud`, `LoadBuildInfo`, the timer, the movies
//!    and the material instances are skipped. Before any write: the HUD class (a `HUDType` of None, or one not in the
//!    class table, is [`HUD_CLASS`] `NotRead`), the definition resident ([`HUD_DEFINITION`] `NotRead`), each field
//!    declared as an object, else `VmError::Script`. A controller that already has a HUD is left as it is.
//! 2. **The pickups' registration** ([`register_pickup`], ruling 2, section 4, its narrower form): a dropped pickup is
//!    added to the world's globals' `PickupList` through the bound `WillowGlobals.AddPickup`, as its `PostBeginPlay`
//!    would, since the pickup world never begins play. The rest of `PostBeginPlay` is not run.
//! 3. **The view** ([`HarnessHost`], ruling 3, section 5): the owner's pawn's eye point (`Location` + (0, 0,
//!    `BaseEyeHeight`)) with the controller's `Rotation`, by fit the first-person view. Off, the camera's `LayerAbsent`
//!    stands (`crate::pickup_targeting::Headless`).
//! 4. **The line check** ([`HarnessHost`], ruling 4, as the hub refined it on review r1 of #806; its own setting,
//!    `pickup_targeting::NoLevelLineCheck`): under the world's `NoLevel` collision service (no level geometry) it
//!    hits nothing, passing over the owner's pawn, the target pickup and the target's `Base`. Any other live actor
//!    holding collision (a `CollisionComponent` set) is [`LINE_ACTOR`] `NotRead`, unless geometry clears it: the
//!    segment's own box is disjoint from the actor's bounds box by the bounding-cylinder rule (the bounds step; #807's
//!    vertical-line test does not fit a slanted line). A world with geometry is [`LINE_GEOMETRY`] `NotRead`; one with
//!    no collision service keeps the headless `LayerAbsent`.
//!
//! **The settings.** Rulings 2 and 3 are one Off-only harness setting, [`HudTargetingHarness`] (a VM service; the
//! pickup world's set-up turns it on, logging [`SETTING_INPUT`], and makes the harness HUD there, ruling 1). Without
//! it no pickup is registered and the view answers as [`Headless`] does. Ruling 4 has its own setting (above). **Ruling 2's form:** the
//! note's choice, begun play at set-up, was tried on the installed gate and broke link 6 (its `make` stopped at "the
//! base search after PostBeginPlay"), so this is the note's fallback, the `AddPickup` registration.
//!
//! The bounds come from the port's own rule (`ow_world::world::bounding_cylinder::answer`, twice the half height), not
//! a stand-in.

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::pickup_targeting::{Headless, Host, View};

/// The record of the rulings.
pub const RECORD: &str = "NATIVE_LINK7_HUD_TARGETING.md";
pub const NO_GAME: &str = "the harness HUD: the world has no game";
pub const HUD_CLASS: &str = "the harness HUD: HUDType is not a WillowHUD";
pub const SECOND_SET_UP: &str = "the harness HUD: a second set-up";
pub const BEGUN_PLAY: &str = "the harness HUD: the world has begun play (the spawn would run the HUD's begin-play events)";
pub const HUD_DEFINITION: &str = "the harness HUD: the globals definition's HUDDefinition, not loaded (a load is a write)";
pub const LINE_GEOMETRY: &str = "the pickup targeting's line check against level geometry (not read)";
pub const LINE_ACTOR: &str = "the pickup targeting's line check: another actor's collision";
pub const HUD_INPUT: &str = "harness input: the player's HUD made with its data steps only (PlayerOwner, WPlayerOwner, \
     HUDDef; no movie natives), the hub's ruling 1 on #806 (UNVERIFIED)";
pub const REGISTER_INPUT: &str = "harness input: the dropped pickup registered with WillowGlobals.AddPickup (its \
     PostBeginPlay's registration; the pickup world never begins play), the hub's ruling 2 on #806 (UNVERIFIED)";
pub const VIEW_INPUT: &str = "harness input: the view is the pawn's eye point (Location + BaseEyeHeight) with the \
     controller's Rotation, the hub's ruling 3 on #806 (UNVERIFIED)";
pub const LINE_INPUT: &str = "harness input: the pickup targeting's line check hit nothing (a world with no level \
     geometry; the owner's pawn, the target pickup and its Base passed over; UNVERIFIED stand-in, the hub's ruling on \
     review of #806)";
pub const SETTING_INPUT: &str = "harness input: the pickup world turns on link 7's HUD targeting harness (the hub's \
     four rulings on #806; UNVERIFIED)";
const WHAT: &str = "the link 7 harness";

/// The Off-only harness setting of the four rulings (module documentation; a VM service, its presence turns it on).
#[derive(Debug, Clone, Copy, Default)]
pub struct HudTargetingHarness;

/// Turns [`HudTargetingHarness`] on, logging [`SETTING_INPUT`] (Off only, else a registration refusal; twice is a
/// registration refusal too).
pub fn enable_hud_targeting_harness(vm: &mut Vm) -> Result<(), VmError> {
    off_only(vm, "HudTargetingHarness")?;
    vm.add_service(HudTargetingHarness)?;
    vm.log.push(SETTING_INPUT.to_string());
    Ok(())
}

/// Whether the setting is on.
pub fn harness_on(vm: &Vm) -> bool {
    vm.parity == ParityMode::Off && vm.service::<HudTargetingHarness>().is_some()
}

fn bad(message: impl Into<String>) -> VmError {
    VmError::Script { function: WHAT.to_string(), message: message.into() }
}

fn off_only(vm: &Vm, what: &'static str) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Err(VmError::Registration { key: what.to_string(), reason: "an Off harness input; Reference is unchanged" });
    }
    Ok(())
}

fn read(vm: &Vm, o: ObjRef, name: &str) -> Result<(Value, PropKind), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| bad(format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, o))))?;
    let kind = vm.space.classes.decl(p).map(|d| d.kind.clone()).ok_or_else(|| bad(format!("{name} undeclared")))?;
    Ok((vm.space.read(o, PropAddr::scalar(p))?.clone(), kind))
}

fn object(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    match read(vm, o, name)? {
        (Value::Object(x), PropKind::Object { .. }) => Ok(x),
        _ => Err(bad(format!("{name} is not declared as an object"))),
    }
}

/// `name` declared on `class` as an object, else a data error.
fn declared_object(vm: &Vm, class: ClassId, name: &str) -> Result<(), VmError> {
    let p = vm.space.prop(class, name).ok_or_else(|| bad(format!("{} declares no {name}", vm.space.classes.layout(class).path().as_str())))?;
    match vm.space.classes.decl(p).map(|d| &d.kind) {
        Some(PropKind::Object { .. }) => Ok(()),
        _ => Err(bad(format!("{name} is not declared as an object"))),
    }
}

fn write(vm: &mut Vm, o: ObjRef, name: &str, v: Value) -> Result<(), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| bad(format!("no {name}")))?;
    vm.space.write(o, PropAddr::scalar(p), v)?;
    Ok(())
}

fn resident(vm: &Vm, o: ObjRef) -> bool {
    let Ok(x) = vm.space.heap.get(o) else { return false };
    let short = vm.space.classes.get_layout(x.class).is_some_and(|l| x.slots.len() < l.slot_count() as usize);
    !(vm.defs.is_unloaded(&vm.space, o) || vm.defs.is_failed(&vm.space, o) || short)
}

// --- 1. The harness HUD ---------------------------------------------------------------------------------------------

/// The plan of [`harness_hud`]: the HUD class and the definition.
fn hud_plan(vm: &mut Vm, controller: ObjRef) -> Result<(ClassId, ObjRef), VmError> {
    if object(vm, controller, "myHUD")?.is_some() {
        return Err(NotYet::not_read(SECOND_SET_UP, RECORD).into());
    }
    let info = object(vm, controller, "WorldInfo")?.ok_or_else(|| bad("the controller has no WorldInfo"))?;
    let game = object(vm, info, "Game")?.ok_or_else(|| NotYet::not_read(NO_GAME, RECORD))?;
    let class = match read(vm, game, "HUDType")? {
        (Value::Class(Some(c)), PropKind::Class { .. }) => c,
        (Value::Class(None), PropKind::Class { .. }) => return Err(NotYet::not_read(HUD_CLASS, RECORD).into()),
        _ => return Err(bad("HUDType is not declared as a class")),
    };
    // A WillowHUD or a subclass, by the class table only (the hub's ruling 3 on #806).
    let willow = vm.space.classes.by_path("WillowGame.WillowHUD");
    if vm.space.classes.get_layout(class).is_none_or(|l| !l.is_defined())
        || !willow.is_some_and(|w| vm.space.classes.is_child_of(class, w))
    {
        return Err(NotYet::not_read(HUD_CLASS, RECORD).into());
    }
    for f in ["PlayerOwner", "WPlayerOwner", "HUDDef"] {
        declared_object(vm, class, f)?;
    }
    let globals = crate::globals::world_globals(vm)?;
    let definition = crate::globals::globals_definition_resident(vm, globals, WHAT)?;
    let hud_definition = object(vm, definition, "HUDDefinition")?.ok_or_else(|| NotYet::not_read(HUD_DEFINITION, RECORD))?;
    if !resident(vm, hud_definition) {
        return Err(NotYet::not_read(HUD_DEFINITION, RECORD).into());
    }
    Ok((class, hud_definition))
}

/// Ruling 1 (module documentation): the controller's harness HUD, made at set-up before the world could begin play
/// (so no begin-play event runs on it), once: a second set-up is [`SECOND_SET_UP`] `NotRead`. Without the setting,
/// nothing is made and the targeting answers as it does (its no-HUD `LayerAbsent`).
///
/// It checks its own conditions (review of #810, item 3): with the setting off it answers as the headless world does
/// (the targeting's own `myHUD`, else its no-HUD `LayerAbsent`), nothing made; in a world that has begun play it is
/// [`BEGUN_PLAY`] `NotRead` before any write, since the spawn would then run the HUD's begin-play events.
pub fn harness_hud(vm: &mut Vm, controller: ObjRef) -> Result<ObjRef, VmError> {
    off_only(vm, "HarnessHud")?;
    if !harness_on(vm) {
        return crate::pickup_targeting::hud_of(vm, controller);
    }
    let begun = match vm.service::<ow_world::actors::Actors>() {
        Some(a) => a.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.begun_play(),
        None => false,
    };
    if begun {
        return Err(NotYet::not_read(BEGUN_PLAY, RECORD).into());
    }
    let (class, definition) = hud_plan(vm, controller)?;
    let request = ow_world::actors::SpawnRequest::new(Some(class)).with_owner(Some(controller)).with_no_collision_fail(true);
    let hud = ow_world::actors::spawn(vm, controller, &request)?.ok_or_else(|| bad("the HUD class did not spawn"))?;
    let rest = (|| -> Result<(), VmError> {
        write(vm, controller, "myHUD", Value::Object(Some(hud)))?;
        write(vm, hud, "PlayerOwner", Value::Object(Some(controller)))?;
        write(vm, hud, "WPlayerOwner", Value::Object(Some(controller)))?;
        write(vm, hud, "HUDDef", Value::Object(Some(definition)))
    })();
    rest.map_err(ow_world::pools::after_partial_writes)?;
    vm.log.push(HUD_INPUT.to_string());
    Ok(hud)
}

// --- 2. The pickups' registration -----------------------------------------------------------------------------------

/// Ruling 2 (module documentation): `pickup` added to the world's globals through the bound `AddPickup`, logged.
/// Without the setting, nothing.
pub fn register_pickup(vm: &mut Vm, pickup: ObjRef) -> Result<(), VmError> {
    if !harness_on(vm) {
        return Ok(());
    }
    let globals = crate::globals::world_globals(vm)?;
    let f = vm.find_function(crate::globals::ADD_PICKUP)?;
    vm.call(f, Some(globals), vec![Value::Object(Some(pickup))])?;
    if !vm.log.iter().any(|l| l == REGISTER_INPUT) {
        vm.log.push(REGISTER_INPUT.to_string());
    }
    Ok(())
}

// --- 3 and 4. The view and the line check ---------------------------------------------------------------------------

/// The link 7 host (module documentation): the eye-point view (ruling 3), the port's bounds rule, and the line check
/// under `NoLevel` (ruling 4). Each ruling is used only when its flag is set; otherwise [`Headless`] answers. The lines
/// it used are collected in [`HarnessHost::inputs`], for the caller to log.
#[derive(Debug, Clone, Default)]
pub struct HarnessHost {
    pub eye_point_view: bool,
    pub no_level_line: bool,
    pub inputs: Vec<&'static str>,
}

impl HarnessHost {
    /// The host the settings give: the eye-point view with [`HudTargetingHarness`], the no-hit line check with
    /// `pickup_targeting::NoLevelLineCheck`.
    pub fn for_world(vm: &Vm) -> Self {
        let line = vm.parity == ParityMode::Off && vm.service::<crate::pickup_targeting::NoLevelLineCheck>().is_some();
        HarnessHost { eye_point_view: harness_on(vm), no_level_line: line, inputs: Vec::new() }
    }

    fn note(&mut self, line: &'static str) {
        if !self.inputs.contains(&line) {
            self.inputs.push(line);
        }
    }
}

fn vector(vm: &Vm, o: ObjRef, name: &str) -> Result<[f32; 3], VmError> {
    let v = read(vm, o, name)?.0;
    let mut out = [0.0; 3];
    for (slot, axis) in out.iter_mut().zip(["X", "Y", "Z"]) {
        *slot = match crate::equip::field(vm, &v, axis)? {
            Value::Float(x) => x,
            other => return Err(bad(format!("{name}.{axis} is {}", other.kind_name()))),
        };
    }
    Ok(out)
}

impl Host for HarnessHost {
    fn view(&mut self, vm: &Vm, owner: ObjRef) -> Result<View, VmError> {
        if !self.eye_point_view || vm.parity != ParityMode::Off {
            return Headless.view(vm, owner);
        }
        // No pawn: the camera's LayerAbsent stays (the hub's ruling 3 on #806).
        let Some(pawn) = object(vm, owner, "Pawn")? else { return Headless.view(vm, owner) };
        let at = vector(vm, pawn, "Location")?;
        let eye = match read(vm, pawn, "BaseEyeHeight")? {
            (Value::Float(f), PropKind::Float) => f,
            _ => return Err(bad("BaseEyeHeight is not declared as a float")),
        };
        let (rotation, _) = read(vm, owner, "Rotation")?;
        let part = |p: &str| -> Result<i32, VmError> {
            // An undeclared member is a data error of this harness (review of #810, item 5).
            match crate::equip::field(vm, &rotation, p).map_err(|e| bad(format!("the controller's Rotation.{p}: {e}")))? {
                Value::Int(i) => Ok(i),
                other => Err(bad(format!("the controller's Rotation.{p} is {}", other.kind_name()))),
            }
        };
        let (pitch, yaw) = (part("Pitch")?, part("Yaw")?);
        self.note(VIEW_INPUT);
        Ok(View { location: [at[0], at[1], at[2] + eye], pitch, yaw })
    }

    fn bounds_height(&mut self, vm: &Vm, pickup: ObjRef) -> Result<f32, VmError> {
        let (_, half) = ow_world::world::bounding_cylinder::answer(vm, pickup)?;
        Ok(2.0 * half)
    }

    fn line_blocker(&mut self, vm: &Vm, from: [f32; 3], to: [f32; 3]) -> Result<Option<ObjRef>, VmError> {
        Headless.line_blocker(vm, from, to)
    }

    fn line_blocker_for(
        &mut self,
        vm: &Vm,
        from: [f32; 3],
        to: [f32; 3],
        pawn: ObjRef,
        target: ObjRef,
    ) -> Result<Option<ObjRef>, VmError> {
        if !self.no_level_line || vm.parity != ParityMode::Off {
            return Headless.line_blocker(vm, from, to);
        }
        match ow_world::placement::collision_model(vm)? {
            None => Headless.line_blocker(vm, from, to),
            Some(m) if m.holds_no_geometry() => {
                let base = object(vm, target, "Base")?;
                let actors = ow_world::actors::actors_of(vm)?;
                let list = actors.try_borrow().map_err(|_| VmError::ServiceBorrowed("Actors"))?.list().to_vec();
                for other in list {
                    if other == pawn || other == target || Some(other) == base {
                        continue;
                    }
                    if ow_world::actors::is_destroyed(&vm.space, other)? {
                        continue;
                    }
                    let class = vm.space.heap.get(other)?.class;
                    let Some(p) = vm.space.prop(class, "CollisionComponent") else { continue };
                    let Value::Object(Some(component)) = *vm.space.read(other, PropAddr::scalar(p))? else { continue };
                    if let Some(why) = not_cleared(vm, other, from, to)? {
                        let who = format!(
                            "{LINE_ACTOR}: a {} with a {} ({why})",
                            ow_attr::eval::class_path(&vm.space, other),
                            ow_attr::eval::class_path(&vm.space, component)
                        );
                        return Err(NotYet::not_read(who, RECORD).into());
                    }
                }
                self.note(LINE_INPUT);
                Ok(None)
            }
            Some(_) => Err(NotYet::not_read(LINE_GEOMETRY, RECORD).into()),
        }
    }
}

/// Why geometry does not clear `other` for the segment `from`-`to`, or `None` when it does (module documentation,
/// ruling 4 as refined on review r1 of #806): cleared when the segment's own box (its two ends) is disjoint from
/// `other`'s bounds box, `Location` ± (radius, radius, half height) by the bounding-cylinder rule. A refusal of that
/// rule clears nothing; touching boxes are not disjoint. Reads only.
fn not_cleared(vm: &Vm, other: ObjRef, from: [f32; 3], to: [f32; 3]) -> Result<Option<String>, VmError> {
    let (radius, half) = match ow_world::world::bounding_cylinder::answer(vm, other) {
        Ok(b) => b,
        Err(VmError::NotYet(n)) => return Ok(Some(format!("its bounds are not answered: {}", n.what))),
        Err(e) => return Err(e),
    };
    if !(radius.is_finite() && half.is_finite() && radius >= 0.0 && half >= 0.0) {
        return Ok(Some(format!("its bounds are radius {radius}, half height {half}")));
    }
    let at = vector(vm, other, "Location")?;
    let extent = [radius, radius, half];
    let disjoint = (0..3).any(|i| {
        let (lo, hi) = (from[i].min(to[i]), from[i].max(to[i]));
        hi < at[i] - extent[i] || lo > at[i] + extent[i]
    });
    Ok((!disjoint).then(|| format!("its bounds box, {at:?} ± ({radius}, {radius}, {half}), meets the line's box {from:?} to {to:?}")))
}

#[cfg(test)]
#[path = "hud_harness_tests.rs"]
mod tests;
