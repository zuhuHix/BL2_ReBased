//! Owner: lane IMPL-PICKUP. AI-assisted (Claude Code). The headless pickup of `--slice-gate`'s links 6 and 7
//! (NATIVE_PICKUP.md; decision 2026-11-19-68). The pickup is installed script and the VM runs it; this module only
//! makes the objects and calls the script's entry points, in the note's order:
//! - `make`: a preflight first, before any write (review r2 of #452): the weapon class's `DroppedPickupClass` (read
//!   from its class default object), its `Owner` and `Instigator` fields, the pickup class's `InitializeFromInventory`
//!   signature (an object, an object, a bool) and its `Pickup` state, the globals the spawn and the initialisation
//!   reach (lane IMPL-WILLOW-GLOBALS: the world's globals object, its definition, the declared schemas of
//!   `PickupList` and `RarityLevelColors`, and the four globals natives bound), and the binding of
//!   `WillowInventory.GetInventoryDefinition`, which the pickup's initialisation reaches on the fixed fixture. An
//!   unbound one is answered as the VM answers it (an unbound native), before anything is made. Then Maya (the
//!   `--equip-run` weapon's owner, at the world origin facing +X) and her gun: the gun leaves its owner, and its
//!   `DroppedPickupClass` is spawned 150 uu ahead of her, initialised from it with pickup enabled (its script
//!   `InitializeFromInventory`) and put in its `Pickup` state. That is what section 1 says the drop does. Link 7 also
//!   drops a decoy gun of the same line, with no owner, 45 uu to her side (section 4's fixture);
//! - `touch`: the pickup's `Touch` by Maya's pawn, the `Pickup` state's handler (checked before the dispatch). Link 6
//!   touches its one gun; link 7 touches the decoy;
//! - `look` (link 7): first, Maya's controller must hold the decoy as `CurrentTouchedPickupable` (the fixture's own
//!   check); then one pass of the per-tick pickup targeting (`ow_inventory::pickup_targeting`, lane
//!   IMPL-PICKUP-TARGETING, decision 2026-11-19-81) on her HUD, with the headless world's `Headless` layers. In
//!   NATIVE_PICKUP_TARGETING.md's look order: no HUD is a named `NotYet { LayerAbsent }` (the headless world spawns
//!   none); a HUD with no owner or `HUDDef` ends the pass, and the look then fails as an ordinary error (nothing seen);
//!   otherwise the camera is the next `NotYet { LayerAbsent }`. A pass that does run must choose the aimed gun, and
//!   Maya's `CurrentSeenPickupable` must then hold it (her `SawPickupable` handler);
//! - `key`: the controller's `PickupSomething` (section 5).
//!
//! **Writes and refusals** (the hub's rule, reviews r2 and r3 of #452): the run's first write is the first mutation
//! that succeeds in `make` (the gun's creation, tracked through the typed fixture preparation of `equip_run`). From
//! then on a `NotYet` is terminal and its text starts "stopped after partial writes", its kind and record kept; any
//! other error passes through unchanged. Errors stay `VmError`s, and the gate grades their variant, not their text
//! (`slice_gate::classify_error`).
//!
//! Harness stand-ins, none a rule: the owner links of `--equip-run`; the pickup's spawn and state entry in place of
//! `DropFrom` (no mission or can-drop check, no physics or toss velocity, and `bNoCollisionFail`, since the world has
//! no collision: what the drop's own spawn passes is not read); the places (the world origin and the note's
//! offsets).

use ow_core::NotYet;
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::Vm;
use ow_vm::error::VmError;

use crate::cmd::inventory::equip_run;

/// The text that marks a refusal after the run's first write.
pub const PARTIAL_WRITES: &str = "stopped after partial writes";

pub(crate) const WEAPON_CLASS: &str = "WillowGame.WillowWeapon";

/// What the pickup world's set-up prepared for the preflight (lane IMPL-WILLOW-GLOBALS, review r1 of #476): the
/// weapon class's default object, loaded at set-up, and the concrete class of the globals definition it installed.
/// The preflight validates the declarations it will read on these before any value is read, and loads nothing.
#[derive(Debug, Clone, Copy)]
pub struct PickupSetup {
    pub weapon_defaults: ObjRef,
    pub definition_class: ClassId,
}
const GET_INVENTORY_DEFINITION: &str = "Engine.WillowInventory.GetInventoryDefinition";
/// The note's fixture places (section 4), Maya at the origin facing +X.
const AIMED_AT: [f32; 3] = [150.0, 0.0, 0.0];
const DECOY_AT: [f32; 3] = [0.0, 45.0, 0.0];

/// The objects of one pickup run.
#[derive(Debug, Clone, Copy)]
pub struct Pickup {
    /// The gun the run means to take (link 7's aimed gun).
    pub weapon: ObjRef,
    pub pawn: ObjRef,
    pub controller: ObjRef,
    /// The gun's pickup actor.
    pub pickup: ObjRef,
    /// Link 7's decoy: its gun and its pickup actor, the one Maya touches.
    pub decoy: Option<(ObjRef, ObjRef)>,
    /// Whether the run has written anything yet (module documentation).
    pub wrote: bool,
}

fn error(message: impl Into<String>) -> VmError {
    VmError::Script { function: "slice-pickup".into(), message: message.into() }
}

/// `e` as a refusal after a write (module documentation): a `NotYet` marked once, its kind and record kept; any other
/// error unchanged.
pub fn after_writes(e: VmError) -> VmError {
    match e {
        VmError::NotYet(n) if !n.what.starts_with(PARTIAL_WRITES) => {
            NotYet::new(format!("{PARTIAL_WRITES}: {}", n.what), n.missing, n.record).into()
        }
        other => other,
    }
}

fn read(vm: &Vm, o: ObjRef, name: &str) -> Result<Value, VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm
        .space
        .prop(class, name)
        .ok_or_else(|| error(format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, o))))?;
    Ok(vm.space.read(o, PropAddr::scalar(p))?.clone())
}

fn write(vm: &mut Vm, o: ObjRef, name: &str, v: Value) -> Result<(), VmError> {
    let class = vm.space.heap.get(o)?.class;
    let p = vm
        .space
        .prop(class, name)
        .ok_or_else(|| error(format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, o))))?;
    Ok(vm.space.write(o, PropAddr::scalar(p), v)?)
}

fn object(v: &Value) -> Option<ObjRef> {
    match v {
        Value::Object(o) | Value::Interface(o) => *o,
        _ => None,
    }
}

/// The zero value of each parameter of `f`, with `given` in place of the first ones.
fn args(vm: &Vm, f: ow_object::ids::FuncId, given: &[Value]) -> Result<Vec<Value>, VmError> {
    let params = vm.function(f).map(|d| d.params.clone()).unwrap_or_default();
    params
        .iter()
        .enumerate()
        .map(|(i, p)| match given.get(i) {
            Some(v) => Ok(v.clone()),
            None => vm.space.classes.default_value(&p.kind).map_err(|e| error(e.to_string())),
        })
        .collect()
}

/// `name` on `o` as its class and current state see it.
fn method(vm: &mut Vm, o: ObjRef, name: &str) -> Result<ow_object::ids::FuncId, VmError> {
    let (class, state) = {
        let h = vm.space.heap.get(o)?;
        (h.class, h.state)
    };
    vm.find_method(class, name, state)?
        .ok_or_else(|| error(format!("{} has no {name}", ow_attr::eval::class_path(&vm.space, o))))
}

fn call(vm: &mut Vm, o: ObjRef, name: &str, given: &[Value]) -> Result<Value, VmError> {
    let f = method(vm, o, name)?;
    let a = args(vm, f, given)?;
    Ok(vm.call(f, Some(o), a)?.ret)
}

fn state_name(vm: &mut Vm) -> ow_object::ids::NameId {
    vm.space.names.intern("Pickup")
}

/// Whether `o` is in its `Pickup` state.
pub fn in_pickup_state(vm: &mut Vm, o: ObjRef) -> Result<bool, VmError> {
    let wanted = state_name(vm);
    Ok(vm.space.heap.get(o)?.state == Some(wanted))
}

/// Puts `o` in its `Pickup` state through its own `GotoState`, and checks that it is there.
pub fn enter_pickup_state(vm: &mut Vm, o: ObjRef) -> Result<(), VmError> {
    let name = state_name(vm);
    call(vm, o, "GotoState", &[Value::Name(name)])?;
    if !in_pickup_state(vm, o)? {
        return Err(error(format!("{} did not enter its Pickup state", ow_attr::eval::class_path(&vm.space, o))));
    }
    Ok(())
}

/// The pickup's `Touch` by `pawn`, the `Pickup` state's handler: refused unless the pickup is in that state.
pub fn touch(vm: &mut Vm, pickup: ObjRef, pawn: ObjRef) -> Result<(), VmError> {
    if !in_pickup_state(vm, pickup)? {
        return Err(error("the pickup is not in its Pickup state, so its Touch is not the Pickup state's handler"));
    }
    call(vm, pickup, "Touch", &[Value::Object(Some(pawn))]).map(|_| ())
}

/// The pickup class the weapon class drops (from its class default object), with every check of the preflight
/// (module documentation).
pub fn preflight(vm: &mut Vm, setup: &PickupSetup) -> Result<ClassId, VmError> {
    let weapon = vm.find_class(WEAPON_CLASS)?;
    for field in ["Owner", "Instigator"] {
        if vm.space.prop(weapon, field).is_none() {
            return Err(error(format!("{WEAPON_CLASS} declares no {field}")));
        }
    }
    // The default object was loaded at set-up; the preflight only reads it.
    let class = match read(vm, setup.weapon_defaults, "DroppedPickupClass")? {
        Value::Class(Some(c)) => c,
        other => return Err(error(format!("{WEAPON_CLASS}'s DroppedPickupClass is {other:?}"))),
    };
    let init = vm
        .find_method(class, "InitializeFromInventory", None)?
        .ok_or_else(|| error("the pickup class has no InitializeFromInventory"))?;
    let kinds: Vec<PropKind> = vm.function(init).map(|d| d.params.iter().map(|p| p.kind.clone()).collect()).unwrap_or_default();
    if !matches!(kinds.as_slice(), [PropKind::Object { .. }, PropKind::Object { .. }, PropKind::Bool]) {
        return Err(error(format!("InitializeFromInventory's parameters are {kinds:?}, not (object, object, bool)")));
    }
    let state = state_name(vm);
    if !vm.class_declares_state(class, state)? {
        return Err(error("the pickup class declares no Pickup state"));
    }
    globals_preflight(vm, setup.definition_class)?;
    // The fixed fixture's initialisation reaches it: it must resolve, be declared native and be bound (review r3 of
    // #452). A failed lookup is that error; an unbound one is the VM's unbound-native error.
    bound_native(vm, GET_INVENTORY_DEFINITION)?;
    Ok(class)
}

/// `path` resolves, is declared native and is bound; else that lookup's error, an ordinary error, or the VM's
/// unbound-native error.
fn bound_native(vm: &mut Vm, path: &str) -> Result<(), VmError> {
    let f = vm.find_function(path)?;
    let key = vm.function(f).and_then(|d| d.native_key.clone()).ok_or_else(|| error(format!("{path} is not declared native")))?;
    if !vm.is_bound(f) {
        return Err(VmError::UnboundNative(key));
    }
    Ok(())
}

/// The globals the pickup's spawn and initialisation reach (NATIVE_WILLOW_GLOBALS.md section 1, lane
/// IMPL-WILLOW-GLOBALS, decision 2026-11-19-94), checked before any write:
/// - the world's one globals object (none is the world's `NotYet { LayerAbsent }`), a `WillowGlobals`;
/// - its `TheGlobalsDefinition`, a `GlobalsDefinition` (else the getter's named `NotYet { NotRead }`);
/// - the declared schemas, before any value is read: `TheGlobalsDefinition` (a `GearboxGlobalsDefinition`, through the
///   parent chain), `ThePopulationMaster` (an object), `PickupList` (of `WillowPickup`), and the definition's
///   `RarityLevelColors` (else a data error);
/// - the four natives on the route (`GetWillowGlobals`, `AddPickup`, `GetGlobalsDefinition`, `GetDropLifeSpanType`)
///   declared native and bound.
fn globals_preflight(vm: &mut Vm, definition_class: ClassId) -> Result<(), VmError> {
    use ow_inventory::globals as g;
    const WHAT: &str = "the pickup's globals";
    let globals = g::world_globals(vm)?;
    let willow = vm.find_class("WillowGame.WillowGlobals")?;
    if !vm.space.classes.is_child_of(vm.space.heap.get(globals)?.class, willow) {
        return Err(error(format!("the world's globals are a {}, not a WillowGlobals", ow_attr::eval::class_path(&vm.space, globals))));
    }
    // The declared schemas first, before any field value is read (section 6.2, step 4): the globals' own, and
    // RarityLevelColors on the concrete definition class the set-up installed (review r1 of #476).
    g::globals_schemas(vm, globals, WHAT)?;
    g::rarity_schema(vm, definition_class, WHAT)?;
    let definition = g::globals_definition(vm, globals, WHAT)?;
    let held = vm.space.heap.get(definition)?.class;
    if held != definition_class {
        return Err(error(format!(
            "the globals definition is a {}, not the set-up's {}",
            ow_attr::eval::class_path(&vm.space, definition),
            vm.space.classes.layout(definition_class).path().as_str()
        )));
    }
    g::pickup_list(vm, globals, WHAT)?;
    g::rarity_bands(vm, definition, WHAT)?;
    for path in [g::GET_WILLOW_GLOBALS, g::ADD_PICKUP, g::GET_GLOBALS_DEFINITION, g::GET_DROP_LIFE_SPAN_TYPE] {
        bound_native(vm, path)?;
    }
    Ok(())
}

/// A location value of `pawn`'s `Location` type at `at`.
fn place(vm: &Vm, pawn: ObjRef, at: [f32; 3]) -> Result<Value, VmError> {
    let mut v = read(vm, pawn, "Location")?;
    for (axis, x) in ["X", "Y", "Z"].into_iter().zip(at) {
        if !ow_combat::hit::access::set_field(vm, &mut v, axis, Value::Float(x)) {
            return Err(error(format!("Location has no {axis}")));
        }
    }
    Ok(v)
}

/// The gun leaves any owner, and its pickup is spawned at `at`, initialised from it and put in its `Pickup` state.
fn drop_at(vm: &mut Vm, class: ClassId, weapon: ObjRef, spawner: ObjRef, at: [f32; 3]) -> Result<ObjRef, VmError> {
    write(vm, weapon, "Owner", Value::Object(None))?;
    write(vm, weapon, "Instigator", Value::Object(None))?;
    let location = place(vm, spawner, at)?;
    let request = ow_world::actors::SpawnRequest::new(Some(class)).with_no_collision_fail(true).with_location(location);
    let pickup = ow_world::actors::spawn(vm, spawner, &request)?.ok_or_else(|| error("the pickup class did not spawn"))?;
    call(vm, pickup, "InitializeFromInventory", &[Value::Object(Some(weapon)), Value::Object(None), Value::Bool(true)])?;
    enter_pickup_state(vm, pickup)?;
    Ok(pickup)
}

/// How `make` prepares one gun: its weapons-file line, whether it gets an owner (Maya) or none (the decoy), and the
/// flag its first successful mutation sets.
pub type Prepare<'a> = dyn FnMut(&mut Vm, &str, bool, &mut bool) -> Result<ObjRef, VmError> + 'a;

/// What `make` runs on each gun once it is made and before its drop: the gun's init (lane IMPL-LINK67-PICKUP-GUN-INIT,
/// `equip_run::pickup_gun_init`).
pub type Init<'a> = dyn FnMut(&mut Vm, ObjRef) -> Result<(), VmError> + 'a;

/// How a gun is dropped: (the pickup class, the gun, the spawner, the place) to its pickup actor; `make` uses
/// [`drop_at`].
pub type DropGun<'a> = dyn FnMut(&mut Vm, ClassId, ObjRef, ObjRef, [f32; 3]) -> Result<ObjRef, VmError> + 'a;

/// The init `make` gives each gun: the gun's init before its drop (review of #761: a test compares it with
/// `equip_run::pickup_gun_init`).
pub const MAKE_INIT: fn(&mut Vm, ObjRef) -> Result<(), VmError> = equip_run::pickup_gun_init;

/// `make` (module documentation): Maya and her gun, and with `decoy` an unowned second gun of the same line. Each gun
/// runs its init before its drop (NATIVE_LINK67_PICKUP_GUN_INIT.md section 3), as a dropped weapon in the game was
/// initialised when it was made.
pub fn make(vm: &mut Vm, setup: &PickupSetup, line: &str, decoy: bool) -> Result<Pickup, VmError> {
    let class = preflight(vm, setup)?;
    let mut init = MAKE_INIT;
    build(
        vm,
        class,
        line,
        decoy,
        &mut |vm, line, owned, wrote| {
            if owned {
                equip_run::prepare_line_typed(vm, line, wrote)
            } else {
                equip_run::prepare_unowned_typed(vm, line, wrote)
            }
        },
        &mut init,
    )
}

/// The fixture after the preflight, each gun made by `prepare` and then given `init`, before its drop: a refusal after
/// the first successful mutation is marked (module documentation), one before it is answered as it is.
pub fn build(
    vm: &mut Vm,
    class: ClassId,
    line: &str,
    decoy: bool,
    prepare: &mut Prepare<'_>,
    init: &mut Init<'_>,
) -> Result<Pickup, VmError> {
    build_dropping(vm, class, line, decoy, prepare, init, &mut drop_at)
}

/// [`build`] with the drop given (its tests run the whole fixture, the decoy's path included, with a toy drop; review
/// of #761).
pub fn build_dropping(
    vm: &mut Vm,
    class: ClassId,
    line: &str,
    decoy: bool,
    prepare: &mut Prepare<'_>,
    init: &mut Init<'_>,
    drop: &mut DropGun<'_>,
) -> Result<Pickup, VmError> {
    let mut wrote = false;
    let made = build_with(vm, class, line, decoy, prepare, init, drop, &mut wrote);
    made.map_err(|e| if wrote { after_writes(e) } else { e })
}

#[allow(clippy::too_many_arguments)] // the fixture's three seams and its write flag, each one named
fn build_with(
    vm: &mut Vm,
    class: ClassId,
    line: &str,
    decoy: bool,
    prepare: &mut Prepare<'_>,
    init: &mut Init<'_>,
    drop: &mut DropGun<'_>,
    wrote: &mut bool,
) -> Result<Pickup, VmError> {
    let weapon = gun(vm, line, true, prepare, init, wrote)?;
    let pawn = object(&read(vm, weapon, "Owner")?).ok_or_else(|| error("the weapon has no owner"))?;
    let controller = object(&read(vm, pawn, "Controller")?).ok_or_else(|| error("the pawn has no controller"))?;
    let origin = place(vm, pawn, [0.0, 0.0, 0.0])?;
    write(vm, pawn, "Location", origin)?;
    *wrote = true;
    let pickup = drop(vm, class, weapon, pawn, AIMED_AT)?;
    level_world_info(vm, pickup, &[controller, pawn])?;
    let decoy = if decoy {
        // The decoy's gun, after the first gun's drop: the same steps.
        let w = gun(vm, line, false, prepare, init, wrote)?;
        Some((w, drop(vm, class, w, pawn, DECOY_AT)?))
    } else {
        None
    };
    Ok(Pickup { weapon, pawn, controller, pickup, decoy, wrote: true })
}

/// The log line of [`level_world_info`].
pub const WORLD_INFO_INPUT: &str = "harness input: the pickup world's controller and pawn take the level's WorldInfo (the pickup's; an UNVERIFIED stand-in: in the game every spawned actor has its level's WorldInfo, NATIVE_UPDATE_AMMO_COUNTS_AND_TAKE.md section 4.1)";

/// NATIVE_UPDATE_AMMO_COUNTS_AND_TAKE.md section 4.1 (lane IMPL-UPDATEAMMOCOUNTS): the pickup world's controller and
/// pawn had no `WorldInfo`, so `PickupPickupable`'s `WorldInfo.Game.PickupQuery` on the controller read through None.
/// In the game every spawned actor has its level's `WorldInfo`; here each of `actors` whose `WorldInfo` is None takes
/// the pickup's, logged once. Nothing when the pickup's class declares no `WorldInfo` or holds none (a toy fixture).
fn level_world_info(vm: &mut Vm, pickup: ObjRef, actors: &[ObjRef]) -> Result<(), VmError> {
    let class = vm.space.heap.get(pickup)?.class;
    if vm.space.prop(class, "WorldInfo").is_none() {
        return Ok(());
    }
    let Some(info) = object(&read(vm, pickup, "WorldInfo")?) else {
        return Ok(());
    };
    let mut wrote = false;
    for &a in actors {
        if object(&read(vm, a, "WorldInfo")?).is_none() {
            write(vm, a, "WorldInfo", Value::Object(Some(info)))?;
            wrote = true;
        }
    }
    if wrote {
        vm.log.push(WORLD_INFO_INPUT.to_string());
    }
    Ok(())
}

/// One gun of `make`: made by `prepare` (owned by Maya, or none for the decoy), then given `init`, before its drop.
pub fn gun(
    vm: &mut Vm,
    line: &str,
    owned: bool,
    prepare: &mut Prepare<'_>,
    init: &mut Init<'_>,
    wrote: &mut bool,
) -> Result<ObjRef, VmError> {
    let weapon = prepare(vm, line, owned, wrote)?;
    init(vm, weapon)?;
    Ok(weapon)
}

/// One step after `make` (module documentation); its error, if any.
pub fn step(vm: &mut Vm, name: &str, p: &Pickup) -> Option<VmError> {
    let r = match name {
        "touch" => touch(vm, p.decoy.map_or(p.pickup, |d| d.1), p.pawn),
        "look" => look(vm, p),
        "key" => call(vm, p.controller, "PickupSomething", &[Value::Bool(false)]).map(|_| ()),
        other => Err(error(format!("{other}: not a step of the pickup run"))),
    };
    let e = r.err()?;
    Some(if p.wrote { after_writes(e) } else { e })
}

fn look(vm: &mut Vm, p: &Pickup) -> Result<(), VmError> {
    let Some((_, decoy)) = p.decoy else {
        return Err(error("look: this run has no decoy"));
    };
    let touched = object(&read(vm, p.controller, "CurrentTouchedPickupable")?);
    if touched != Some(decoy) {
        return Err(error(format!("look: Maya's CurrentTouchedPickupable is {touched:?}, not the decoy")));
    }
    use ow_inventory::pickup_targeting::{Headless, Outcome, look as target};
    match target(vm, p.controller, &mut Headless)? {
        Outcome::Hover { winner, .. } if winner == p.pickup => {}
        other => return Err(error(format!("look: the targeting did not choose the aimed gun ({other:?})"))),
    }
    let seen = object(&read(vm, p.controller, "CurrentSeenPickupable")?);
    if seen != Some(p.pickup) {
        return Err(error(format!("look: Maya's CurrentSeenPickupable is {seen:?}, not the aimed gun")));
    }
    Ok(())
}

/// The links' evidence, from successful reads only: the gun is in Maya's inventory chain, its pickup is gone, and the
/// decoy (link 7) is still a live pickup holding its own gun and still enabled for pickup (`bPickupable`).
pub fn taken(vm: &Vm, p: &Pickup) -> bool {
    let held = || -> Result<bool, VmError> {
        let Some(manager) = object(&read(vm, p.pawn, "InvManager")?) else { return Ok(false) };
        let mut cursor = object(&read(vm, manager, "InventoryChain")?);
        for _ in 0..256 {
            let Some(item) = cursor else { return Ok(false) };
            if item == p.weapon {
                return Ok(true);
            }
            cursor = object(&read(vm, item, "Inventory")?);
        }
        Ok(false)
    };
    let consumed = || ow_world::actors::is_destroyed(&vm.space, p.pickup);
    let decoy_left = || -> Result<bool, VmError> {
        let Some((gun, pickup)) = p.decoy else { return Ok(true) };
        let enabled = matches!(read(vm, pickup, "bPickupable")?, Value::Bool(true));
        Ok(!ow_world::actors::is_destroyed(&vm.space, pickup)? && object(&read(vm, pickup, "Inventory")?) == Some(gun) && enabled)
    };
    matches!((held(), consumed(), decoy_left()), (Ok(true), Ok(true), Ok(true)))
}

#[cfg(test)]
#[path = "slice_pickup_tests.rs"]
mod tests;
