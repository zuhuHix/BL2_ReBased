//! Owner: lane IMPL-PICKUP. AI-assisted (Claude Code). The headless pickup of `--slice-gate`'s links 6 and 7
//! (NATIVE_PICKUP.md; decision 2026-11-19-68). The pickup is installed script and the VM runs it; this module only
//! makes the objects and calls the script's entry points, in the note's order:
//! - `make`: `--equip-run`'s weapon and owner (`equip_run::prepare_line`); the weapon leaves its owner, and the
//!   weapon's `DroppedPickupClass` is spawned and initialised from it with pickup enabled (its script
//!   `InitializeFromInventory`). That is what section 1 says the drop does. The drop's own script, `DropFrom`, stops at
//!   the unbound `WillowInventory.GetInventoryDefinition()`, which no note reads, so the run does not call it;
//! - `touch`: the pickup's `Touch` by the pawn, in the pickup's `Pickup` state (section 3). Link 6 touches its one gun;
//!   link 7 touches the decoy, the second gun `make` drops (the slice fixtures of section 4);
//! - `look` (link 7): the per-tick pickup targeting (section 4), a native with no script name. It needs the player's
//!   view point and direction, which the headless world does not have (no camera): `NotYet { LayerAbsent }`, never a
//!   fitted choice;
//! - `key`: the controller's `PickupSomething` (section 5).
//!
//! The two guns' places (150 uu ahead, and 45 uu to the side) are not set: every step that would read them refuses
//! first in Off (the touch trace, the look).
//!
//! Three harness stand-ins, none a rule: the owner links of `--equip-run`; the pickup's spawn in place of `DropFrom`
//! (no mission or can-drop check, no physics or toss velocity, and `bNoCollisionFail`, since the world has no
//! collision: what the drop's own spawn passes is not read); and the spawn's location (the spawner's).

use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;
use ow_vm::api::Vm;

use crate::cmd::inventory::equip_run;

/// The objects of one pickup run.
#[derive(Debug, Clone, Copy)]
pub struct Pickup {
    /// The gun the run means to take (link 7's aimed gun).
    pub weapon: ObjRef,
    pub pawn: ObjRef,
    pub controller: ObjRef,
    /// The gun's pickup actor.
    pub pickup: Option<ObjRef>,
    /// Link 7's decoy: its gun and its pickup actor, the one the pawn touches.
    pub decoy: Option<(ObjRef, ObjRef)>,
}

fn text(e: &ow_vm::error::VmError) -> String {
    ow_combat::hit::access::error_text(e)
}

fn read(vm: &Vm, o: ObjRef, name: &str) -> Result<Value, String> {
    let class = vm.space.heap.get(o).map_err(|e| e.to_string())?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, o)))?;
    Ok(vm.space.read(o, PropAddr::scalar(p)).map_err(|e| e.to_string())?.clone())
}

fn write(vm: &mut Vm, o: ObjRef, name: &str, v: Value) -> Result<(), String> {
    let class = vm.space.heap.get(o).map_err(|e| e.to_string())?.class;
    let p = vm.space.prop(class, name).ok_or_else(|| format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, o)))?;
    vm.space.write(o, PropAddr::scalar(p), v).map_err(|e| e.to_string())
}

fn object(v: &Value) -> Option<ObjRef> {
    match v {
        Value::Object(o) | Value::Interface(o) => *o,
        _ => None,
    }
}

/// The zero value of each parameter of `f`, with `first` in place of the first.
fn args(vm: &Vm, f: ow_object::ids::FuncId, given: &[Value]) -> Result<Vec<Value>, String> {
    let params = vm.function(f).map(|d| d.params.clone()).unwrap_or_default();
    params
        .iter()
        .enumerate()
        .map(|(i, p)| match given.get(i) {
            Some(v) => Ok(v.clone()),
            None => vm.space.classes.default_value(&p.kind).map_err(|e| e.to_string()),
        })
        .collect()
}

/// Calls `name` on `o` as its class and current state see it.
fn call(vm: &mut Vm, o: ObjRef, name: &str, given: &[Value]) -> Result<Value, String> {
    let (class, state) = {
        let h = vm.space.heap.get(o).map_err(|e| e.to_string())?;
        (h.class, h.state)
    };
    let f = vm
        .find_method(class, name, state)
        .map_err(|e| text(&e))?
        .ok_or_else(|| format!("{} has no {name}", ow_attr::eval::class_path(&vm.space, o)))?;
    let a = args(vm, f, given)?;
    vm.call(f, Some(o), a).map(|r| r.ret).map_err(|e| text(&e))
}

/// `make` (module documentation): one gun, and with `decoy` a second one of the same line.
pub fn make(vm: &mut Vm, line: &str, decoy: bool) -> Result<Pickup, String> {
    let (weapon, pickup, pawn, controller) = drop_one(vm, line)?;
    let decoy = if decoy {
        let (w, p, _, _) = drop_one(vm, line)?;
        Some((w, p))
    } else {
        None
    };
    Ok(Pickup { weapon, pawn, controller, pickup: Some(pickup), decoy })
}

/// One gun of `line` with its own owner (whose pawn and controller are answered), dropped as a pickup.
fn drop_one(vm: &mut Vm, line: &str) -> Result<(ObjRef, ObjRef, ObjRef, ObjRef), String> {
    let weapon = equip_run::prepare_line(vm, line)?;
    let pawn = object(&read(vm, weapon, "Owner")?).ok_or("the weapon has no owner")?;
    let controller = object(&read(vm, pawn, "Controller")?).ok_or("the pawn has no controller")?;
    write(vm, weapon, "Owner", Value::Object(None))?;
    write(vm, weapon, "Instigator", Value::Object(None))?;
    let class = match read(vm, weapon, "DroppedPickupClass")? {
        Value::Class(Some(c)) => c,
        other => return Err(format!("the weapon's DroppedPickupClass is {other:?}")),
    };
    let request = ow_world::actors::SpawnRequest::new(Some(class)).with_no_collision_fail(true);
    let pickup = ow_world::actors::spawn(vm, pawn, &request)
        .map_err(|e| text(&e))?
        .ok_or("the pickup class did not spawn")?;
    call(vm, pickup, "InitializeFromInventory", &[Value::Object(Some(weapon)), Value::Object(None), Value::Bool(true)])?;
    Ok((weapon, pickup, pawn, controller))
}

/// What the take writes that the run can see: the weapon's `Owner` and the pickup's `Inventory`.
fn take_state(vm: &Vm, p: &Pickup) -> (Option<Value>, Option<Value>) {
    (
        read(vm, p.weapon, "Owner").ok(),
        p.pickup.and_then(|o| read(vm, o, "Inventory").ok()),
    )
}

/// One step after `make` (module documentation); its error text, if any. A refusal after the step already changed
/// what the take writes is terminal and says so ("stopped after partial writes", the hub's rule; the run has no undo).
pub fn step(vm: &mut Vm, name: &str, p: &Pickup) -> Option<String> {
    let before = take_state(vm, p);
    let r = match name {
        "touch" => p.decoy.map(|d| d.1).or(p.pickup).ok_or_else(|| "no pickup".to_string()).and_then(|pickup| {
            call(vm, pickup, "Touch", &[Value::Object(Some(p.pawn))]).map(|_| ())
        }),
        "look" => Err(ow_core::NotYet::layer_absent(
            "the per-tick pickup targeting (NATIVE_PICKUP.md section 4) needs the player's view point and direction",
            "a camera (the headless world has none)",
            "NATIVE_PICKUP.md",
        )
        .to_string()),
        "key" => call(vm, p.controller, "PickupSomething", &[Value::Bool(false)]).map(|_| ()),
        other => Err(format!("{other}: not a step of the pickup run")),
    };
    let e = r.err()?;
    if take_state(vm, p) != before {
        return Some(format!("stopped after partial writes: {e}"));
    }
    Some(e)
}

/// Whether the gun is the pawn's own afterwards (its `Owner`), and the decoy is not: the links' evidence.
pub fn taken(vm: &Vm, p: &Pickup) -> bool {
    let owner = |w: ObjRef| read(vm, w, "Owner").ok().and_then(|v| object(&v));
    owner(p.weapon) == Some(p.pawn) && p.decoy.is_none_or(|(w, _)| owner(w) != Some(p.pawn))
}
