//! Owner: lane IMPL-WILLOW-GLOBALS. AI-assisted (Claude Code). The globals natives on the pickup path (slice links 6
//! and 7, the `make` step), coded in `ParityMode::Off` from NATIVE_WILLOW_GLOBALS.md alone (#469; decision
//! 2026-11-19-94). This lane did not read the natives. Every rule is the note's reading of the executable,
//! **UNVERIFIED** in the running game.
//!
//! - **`WillowGlobals.GetWillowGlobals()` and `GearboxGlobals.GetGearboxGlobals()`** (static): the world's one globals
//!   object (`WorldQuery::set_globals`), the same on every call. A world without one is the world's own
//!   `NotYet { LayerAbsent }`, never None (None would let `PostBeginPlay` skip `AddPickup` silently). They write
//!   nothing.
//! - **`WillowGlobals.AddPickup(Pickup)`:** appends the argument, None included, at the end of `PickupList`, with no
//!   duplicate check and no event. A `PickupList` the class does not declare as a dynamic array of object references is
//!   a data error, raised before the append.
//! - **`WillowGlobals.GetGlobalsDefinition()`:** `TheGlobalsDefinition` (declared on `GearboxGlobals`) when it holds a
//!   `GlobalsDefinition` or a subclass. Otherwise `NotYet { NotRead }`: the native's first load numbers the challenge
//!   list in a way the note did not read, so Off does not run it. World set-up fills the field with the installed
//!   definition (the slice's pickup world, `ow-cli`), so the call is a read.
//! - **`GlobalsDefinition.GetDropLifeSpanType(RarityLevel)`:** the `DropLifeSpanType` of the first `RarityLevelColors`
//!   entry with `MinLevel <= RarityLevel <= MaxLevel`, both ends included; 3 when none holds it (an empty array
//!   included). A pure read. A `RarityLevelColors` not declared as a dynamic array of that struct is a data error.
//! - **`GearboxGlobals.GetPopulationMaster()`:** the globals' `ThePopulationMaster`, as it is (None included).
//! - **`WillowGlobals.RemovePickup(Pickup)`** stays `ow_inventory::pickup`'s `NotYet { NotRead }`, raised before any
//!   change: the two global events it fires first are not ported (the note's section 6, step 8).
//!
//! Each native is bound by its function path when the VM declares it, and skipped when its key is already bound (the
//! mission bridge binds the two getters in its own world). Reference binds nothing here (no C++ stand-in; R3).

use std::rc::Rc;

use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ClassId, ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, NativeFn, Vm};
use ow_vm::error::VmError;

use crate::equip::{bad, field, object_of, required};

/// The record these natives follow.
pub const RECORD: &str = "NATIVE_WILLOW_GLOBALS.md";

pub const GET_WILLOW_GLOBALS: &str = "WillowGame.WillowGlobals.GetWillowGlobals";
pub const GET_GEARBOX_GLOBALS: &str = "GearboxFramework.GearboxGlobals.GetGearboxGlobals";
pub const ADD_PICKUP: &str = "WillowGame.WillowGlobals.AddPickup";
pub const GET_GLOBALS_DEFINITION: &str = "WillowGame.WillowGlobals.GetGlobalsDefinition";
pub const GET_DROP_LIFE_SPAN_TYPE: &str = "WillowGame.GlobalsDefinition.GetDropLifeSpanType";
pub const GET_POPULATION_MASTER: &str = "GearboxFramework.GearboxGlobals.GetPopulationMaster";
/// The class `TheGlobalsDefinition` must hold for the getter to be a read.
pub const GLOBALS_DEFINITION_CLASS: &str = "WillowGame.GlobalsDefinition";
/// The answer when no band holds the level.
pub const NO_BAND: u8 = 3;

/// The natives [`register`] binds in Off, by path.
pub const PATHS: [&str; 6] =
    [GET_WILLOW_GLOBALS, GET_GEARBOX_GLOBALS, ADD_PICKUP, GET_GLOBALS_DEFINITION, GET_DROP_LIFE_SPAN_TYPE, GET_POPULATION_MASTER];

/// The world's globals object (the getters' answer).
pub fn world_globals(vm: &mut Vm) -> Result<ObjRef, VmError> {
    let cx = ow_attr::api::EvalCx::new(vm);
    Ok(cx.world.globals()?)
}

fn this(c: &NativeCall<'_>, what: &str) -> Result<ObjRef, VmError> {
    c.this.ok_or_else(|| bad(what, "called without an object"))
}

/// `PickupList` of `globals` as declared: a dynamic array of object references, else a data error.
pub fn pickup_list(vm: &Vm, globals: ObjRef, what: &str) -> Result<Vec<Value>, VmError> {
    let class = vm.space.heap.get(globals)?.class;
    let p = vm.space.prop(class, "PickupList").ok_or_else(|| bad(what, "the globals declare no PickupList"))?;
    match vm.space.classes.decl(p).map(|d| (&d.kind, d.array_dim)) {
        Some((PropKind::Array(inner), 1)) if matches!(**inner, PropKind::Object { .. }) => {}
        other => return Err(bad(what, format!("PickupList is declared as {other:?}, not a dynamic array of objects"))),
    }
    match vm.space.read(globals, PropAddr::scalar(p))? {
        Value::Array(a) => Ok(a.as_ref().clone()),
        other => Err(bad(what, format!("PickupList holds {}, not an array", other.kind_name()))),
    }
}

/// `RarityLevelColors` of `definition` as declared: a dynamic array of structs with `MinLevel`, `MaxLevel` (ints) and
/// `DropLifeSpanType` (a byte), else a data error.
pub fn rarity_bands(vm: &Vm, definition: ObjRef, what: &str) -> Result<Vec<(i32, i32, u8)>, VmError> {
    let class = vm.space.heap.get(definition)?.class;
    let p = vm
        .space
        .prop(class, "RarityLevelColors")
        .ok_or_else(|| bad(what, "the globals definition declares no RarityLevelColors"))?;
    let ty = match vm.space.classes.decl(p).map(|d| (&d.kind, d.array_dim)) {
        Some((PropKind::Array(inner), 1)) => match **inner {
            PropKind::Struct(ty) => ty,
            ref k => return Err(bad(what, format!("RarityLevelColors holds {k:?}, not a struct"))),
        },
        other => return Err(bad(what, format!("RarityLevelColors is declared as {other:?}, not a dynamic array"))),
    };
    let layout = vm.space.classes.struct_layout(ty).ok_or_else(|| bad(what, "RarityLevelColor has no layout"))?;
    for (name, want) in [("MinLevel", "an int"), ("MaxLevel", "an int"), ("DropLifeSpanType", "a byte")] {
        let id = vm.space.names.find(name);
        let kind = id.and_then(|id| layout.field(id)).map(|(_, d)| d.kind.clone());
        let fits = match (name, &kind) {
            ("DropLifeSpanType", Some(PropKind::Byte { .. })) => true,
            ("MinLevel" | "MaxLevel", Some(PropKind::Int)) => true,
            _ => false,
        };
        if !fits {
            return Err(bad(what, format!("RarityLevelColor.{name} is {kind:?}, not {want}")));
        }
    }
    let entries = match vm.space.read(definition, PropAddr::scalar(p))? {
        Value::Array(a) => a.as_ref().clone(),
        other => return Err(bad(what, format!("RarityLevelColors holds {}, not an array", other.kind_name()))),
    };
    entries
        .iter()
        .map(|e| {
            let int = |name: &str| match field(vm, e, name)? {
                Value::Int(i) => Ok(i),
                other => Err(bad(what, format!("RarityLevelColor.{name} holds {}", other.kind_name()))),
            };
            let kind = match field(vm, e, "DropLifeSpanType")? {
                Value::Byte(b) => b,
                other => return Err(bad(what, format!("DropLifeSpanType holds {}", other.kind_name()))),
            };
            Ok((int("MinLevel")?, int("MaxLevel")?, kind))
        })
        .collect()
}

/// `GetDropLifeSpanType`'s rule (module documentation).
pub fn drop_life_span_type(bands: &[(i32, i32, u8)], level: i32) -> u8 {
    bands.iter().find(|(min, max, _)| *min <= level && level <= *max).map_or(NO_BAND, |b| b.2)
}

/// `TheGlobalsDefinition` of `globals` when it holds a `GlobalsDefinition`, else the named `NotYet` (module
/// documentation).
pub fn globals_definition(vm: &mut Vm, globals: ObjRef, what: &str) -> Result<ObjRef, VmError> {
    let class: ClassId = vm.find_class(GLOBALS_DEFINITION_CLASS)?;
    let held = object_of(&required(vm, globals, "TheGlobalsDefinition", what)?, what)?;
    match held {
        Some(d) if vm.space.classes.is_child_of(vm.space.heap.get(d)?.class, class) => Ok(d),
        _ => Err(NotYet::not_read(
            "GetGlobalsDefinition: TheGlobalsDefinition holds no GlobalsDefinition, and the native's first load, which \
             numbers the challenge list, was not read (section 4)",
            RECORD,
        )
        .into()),
    }
}

fn add_pickup(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "WillowGlobals.AddPickup";
    let globals = this(c, WHAT)?;
    let pickup = c.arg(0)?.clone();
    let mut list = pickup_list(c.vm, globals, WHAT)?;
    let class = c.vm.space.heap.get(globals)?.class;
    let p = c.vm.space.prop(class, "PickupList").ok_or_else(|| bad(WHAT, "the globals declare no PickupList"))?;
    let entry = match pickup {
        Value::Object(o) | Value::Interface(o) => Value::Object(o),
        other => return Err(bad(WHAT, format!("the pickup is {}, not an object", other.kind_name()))),
    };
    list.push(entry);
    c.vm.space.write(globals, PropAddr::scalar(p), Value::array(list))?;
    Ok(Value::None)
}

fn get_drop_life_span_type(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "GlobalsDefinition.GetDropLifeSpanType";
    let definition = this(c, WHAT)?;
    let level = match c.arg(0)? {
        Value::Int(i) => *i,
        other => return Err(bad(WHAT, format!("the rarity level is {}, not an int", other.kind_name()))),
    };
    let bands = rarity_bands(c.vm, definition, WHAT)?;
    Ok(Value::Byte(drop_life_span_type(&bands, level)))
}

/// Binds the module's natives in `ParityMode::Off` (module documentation).
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let natives: [(&str, NativeFn); 6] = [
        (GET_WILLOW_GLOBALS, Rc::new(|c: &mut NativeCall<'_>| Ok(Value::Object(Some(world_globals(c.vm)?))))),
        (GET_GEARBOX_GLOBALS, Rc::new(|c: &mut NativeCall<'_>| Ok(Value::Object(Some(world_globals(c.vm)?))))),
        (ADD_PICKUP, Rc::new(add_pickup)),
        (
            GET_GLOBALS_DEFINITION,
            Rc::new(|c: &mut NativeCall<'_>| {
                let globals = this(c, "WillowGlobals.GetGlobalsDefinition")?;
                Ok(Value::Object(Some(globals_definition(c.vm, globals, "WillowGlobals.GetGlobalsDefinition")?)))
            }),
        ),
        (GET_DROP_LIFE_SPAN_TYPE, Rc::new(get_drop_life_span_type)),
        (
            GET_POPULATION_MASTER,
            Rc::new(|c: &mut NativeCall<'_>| {
                let globals = this(c, "GearboxGlobals.GetPopulationMaster")?;
                required(c.vm, globals, "ThePopulationMaster", "GearboxGlobals.GetPopulationMaster")
            }),
        ),
    ];
    for (path, f) in natives {
        let Ok(id) = vm.find_function(path) else { continue };
        let Some(key) = vm.function(id).and_then(|d| d.native_key.clone()) else { continue };
        if !vm.has_native(&key) {
            vm.register_native(&key, f)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "globals_tests.rs"]
mod tests;
