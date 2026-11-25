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
//! - **`GearboxGlobals.GetPopulationMaster()`:** the globals' `ThePopulationMaster`, as it is (None included; the
//!   supported fixture's value). On a non-None master, `Destroyed` calls `RemoveSpawnedActor`, which is bound here to a
//!   `NotYet { NotRead }` ("PopulationMaster.RemoveSpawnedActor", not read).
//! - **`WillowGlobals.RemovePickup(Pickup)`** stays `ow_inventory::pickup`'s `NotYet { NotRead }`, "the pickup events of
//!   RemovePickup", raised before any dispatch and any change to `PickupList` (section 6.3, step 7).
//! - **The preflight's schemas** ([`globals_schemas`]): the declared reference types the note names, checked before any
//!   value is read, each mismatch a data error (`VmError::Script`).
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

/// `WillowPlayerController.UnlockAchievementIfConditionsMet`, which reads the globals definition (lane
/// IMPL-LINK12-UNLOCK-ACHIEVEMENT).
pub mod achievement;
pub mod class_achievement;

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

/// The population master's `RemoveSpawnedActor`, which `Destroyed` calls on a non-None master: not read (section 6.3,
/// step 6).
pub const REMOVE_SPAWNED_ACTOR: [&str; 2] =
    ["GearboxFramework.PopulationMaster.RemoveSpawnedActor", "WillowGame.WillowPopulationMaster.RemoveSpawnedActor"];

/// The natives [`register`] binds in Off, by path (and the two of [`REMOVE_SPAWNED_ACTOR`]).
pub const PATHS: [&str; 6] =
    [GET_WILLOW_GLOBALS, GET_GEARBOX_GLOBALS, ADD_PICKUP, GET_GLOBALS_DEFINITION, GET_DROP_LIFE_SPAN_TYPE, GET_POPULATION_MASTER];

/// The world's globals object (the getters' answer). None recorded is the note's *layer absent*, "the world's globals
/// object" (section 6.3, step 1); any other error passes unchanged.
pub fn world_globals(vm: &mut Vm) -> Result<ObjRef, VmError> {
    let cx = ow_attr::api::EvalCx::new(vm);
    match cx.world.globals() {
        Ok(o) => Ok(o),
        Err(e) => match VmError::from(e) {
            VmError::NotYet(n) if matches!(n.missing, ow_core::Missing::LayerAbsent(_)) => Err(NotYet::layer_absent(
                "the world's globals object",
                "the world set-up that makes it (WorldQuery::set_globals)",
                RECORD,
            )
            .into()),
            other => Err(other),
        },
    }
}

/// The declared reference types the route needs (section 6.2, step 4, and the note's last round): the field `name` on
/// `o`'s class, found through the parent chain, declared as one object reference whose class is exactly `class` (or
/// any object reference when `class` is None). A mismatch is a data error (`VmError::Script`), raised before any
/// value is read.
///
/// The declaration is checked first, then the declared class is compared with `class` by its own path, so nothing is
/// looked up and nothing loads (lane FOLLOWUP-PICKUP-RB-VALIDATION: `Vm::find_class` can load, and a load is a write
/// in an Off preflight). A declared class with no layout is `NotYet { NotRead }`.
pub fn declared_reference(vm: &Vm, o: ObjRef, name: &str, class: Option<&str>, what: &str) -> Result<(), VmError> {
    let declared = declared_reference_to(vm, o, name, None, class.unwrap_or("object"), what).and_then(|()| {
        let own = vm.space.heap.get(o)?.class;
        match vm.space.prop(own, name).and_then(|p| vm.space.classes.decl(p)).map(|d| &d.kind) {
            Some(PropKind::Object { class: c }) => Ok(*c),
            other => Err(bad(what, format!("{name} is declared as {other:?}, not one object reference"))),
        }
    })?;
    match class {
        None => Ok(()),
        Some(path) => class_named(vm, declared, path, name, what),
    }
}

/// Whether `c`, the class a declaration names, is the class at `path`: the class table's read-only `by_path` (which
/// parses the path, so its separator equivalence holds, and neither loads nor interns) compared by id (#625 r1, item
/// 3). A different class, or a path the table does not hold, is a data error naming `name`; a declared class with no
/// layout is `NotYet { NotRead }`.
pub fn class_named(vm: &Vm, c: ClassId, path: &str, name: &str, what: &str) -> Result<(), VmError> {
    match vm.space.classes.get_layout(c) {
        Some(_) if vm.space.classes.by_path(path) == Some(c) => Ok(()),
        Some(l) => Err(bad(what, format!("{name} is declared as a reference to {}, not one {path}", l.path().as_str()))),
        None => Err(NotYet::not_read(
            format!("the class {name} is declared with, not loaded (a class load is a write)"),
            RECORD,
        )
        .into()),
    }
}

/// The class at `path` when it is ready with no load: a defined layout and loaded function and state code (lane
/// IMPL-PICKUP-RB, review r1 of #587; the test of #579's `class_ready`). `Vm::find_class` can register code even
/// when the layout is there, so a write-free preflight uses this instead. A class that is not ready is a named
/// `NotYet { NotRead }`: a class load is a write.
pub fn ready_class(vm: &Vm, path: &str) -> Result<ClassId, VmError> {
    match vm.space.classes.by_path(path) {
        Some(c) if vm.space.classes.get_layout(c).is_some_and(|l| l.is_defined()) && vm.class_code_loaded(c) => Ok(c),
        _ => Err(NotYet::not_read(format!("the class {path}, not loaded (a class load is a write)"), RECORD).into()),
    }
}

/// [`declared_reference`] with no load: the wanted class through [`ready_class`] (review r1 of #587).
pub fn declared_reference_loaded(vm: &Vm, o: ObjRef, name: &str, class: Option<&str>, what: &str) -> Result<(), VmError> {
    let want = match class {
        Some(path) => Some(ready_class(vm, path)?),
        None => None,
    };
    declared_reference_to(vm, o, name, want, class.unwrap_or("object"), what)
}

/// [`declared_reference`] with the class already found (`label` names it in the error): it loads nothing.
pub fn declared_reference_to(
    vm: &Vm,
    o: ObjRef,
    name: &str,
    want: Option<ClassId>,
    label: &str,
    what: &str,
) -> Result<(), VmError> {
    let own = vm.space.heap.get(o)?.class;
    let p = vm
        .space
        .prop(own, name)
        .ok_or_else(|| bad(what, format!("{} declares no {name}", ow_attr::eval::class_path(&vm.space, o))))?;
    let declared = vm.space.classes.decl(p).map(|d| (d.kind.clone(), d.array_dim));
    match declared {
        Some((PropKind::Object { class: c }, 1)) if want.is_none_or(|w| w == c) => Ok(()),
        other => Err(bad(what, format!("{name} is declared as {other:?}, not one {label}"))),
    }
}

/// The declared struct of `RarityLevelColors`' entries.
pub const RARITY_LEVEL_COLOR: &str = "WillowGame.GlobalsDefinition.RarityLevelColor";

/// The declared type of `TheGlobalsDefinition` (on `GearboxGlobals`) and of `PickupList`'s elements.
pub const GEARBOX_GLOBALS_DEFINITION: &str = "GearboxFramework.GearboxGlobalsDefinition";
pub const PICKUP_CLASS: &str = "WillowGame.WillowPickup";

/// The schemas of section 6.2 step 4 on the globals object, checked before any value is read: `TheGlobalsDefinition`
/// (through the parent chain, declared as a `GearboxGlobalsDefinition`), `ThePopulationMaster` (an object reference)
/// and `PickupList` (a dynamic array of `WillowPickup` references).
pub fn globals_schemas(vm: &Vm, globals: ObjRef, what: &str) -> Result<(), VmError> {
    declared_reference(vm, globals, "TheGlobalsDefinition", Some(GEARBOX_GLOBALS_DEFINITION), what)?;
    declared_reference(vm, globals, "ThePopulationMaster", None, what)?;
    pickup_list_declared(vm, globals, what)
}

/// `PickupList`'s declaration on the globals' class: a dynamic array whose elements are `WillowPickup` references.
/// The declaration is checked first, then its element class by its own path ([`class_named`]; nothing loads).
pub fn pickup_list_declared(vm: &Vm, globals: ObjRef, what: &str) -> Result<(), VmError> {
    let class = vm.space.heap.get(globals)?.class;
    let p = vm.space.prop(class, "PickupList").ok_or_else(|| bad(what, "the globals declare no PickupList"))?;
    let declared = vm.space.classes.decl(p).map(|d| (d.kind.clone(), d.array_dim));
    let element = match &declared {
        Some((PropKind::Array(inner), 1)) => match inner.as_ref() {
            PropKind::Object { class } => Some(*class),
            _ => None,
        },
        _ => None,
    };
    let Some(element) = element else {
        return Err(bad(what, format!("PickupList is declared as {declared:?}, not a dynamic array of WillowPickup")));
    };
    class_named(vm, element, PICKUP_CLASS, "PickupList's elements", what)
        .map_err(|e| match e {
            VmError::Script { function, message } => VmError::Script {
                function,
                message: format!("{message} (PickupList must be a dynamic array of WillowPickup)"),
            },
            other => other,
        })
}

fn this(c: &NativeCall<'_>, what: &str) -> Result<ObjRef, VmError> {
    c.this.ok_or_else(|| bad(what, "called without an object"))
}

/// `PickupList` of `globals` as declared: a dynamic array of object references, else a data error. (The preflight
/// also checks the element class, [`globals_schemas`].)
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
    if vm.defs.is_unloaded(&vm.space, definition) {
        return Err(lazy_definition());
    }
    let class = vm.space.heap.get(definition)?.class;
    let p = rarity_schema(vm, class, what)?;
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

/// `RarityLevelColors`' declaration on `class`, checked without reading any value (the preflight runs it on the
/// declared `GlobalsDefinition` class before `TheGlobalsDefinition` is read; #469 r2, section 6.2 step 4): a dynamic
/// array of `RarityLevelColor`, whose `MinLevel` and `MaxLevel` are ints and `DropLifeSpanType` a byte.
pub fn rarity_schema(vm: &Vm, class: ClassId, what: &str) -> Result<ow_object::ids::PropId, VmError> {
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
    // The declared struct itself, not any struct with the same field names (review r1 of #476).
    if vm.space.classes.struct_by_path(RARITY_LEVEL_COLOR) != Some(ty) {
        return Err(bad(what, format!("RarityLevelColors holds a struct that is not {RARITY_LEVEL_COLOR}")));
    }
    let layout = vm.space.classes.struct_layout(ty).ok_or_else(|| bad(what, "RarityLevelColor has no layout"))?;
    for (name, want) in [("MinLevel", "one int"), ("MaxLevel", "one int"), ("DropLifeSpanType", "one byte")] {
        let id = vm.space.names.find(name);
        let decl = id.and_then(|id| layout.field(id)).map(|(_, d)| (d.kind.clone(), d.array_dim));
        let fits = match (name, &decl) {
            ("DropLifeSpanType", Some((PropKind::Byte { .. }, 1))) => true,
            ("MinLevel" | "MaxLevel", Some((PropKind::Int, 1))) => true,
            _ => false,
        };
        if !fits {
            return Err(bad(what, format!("RarityLevelColor.{name} is declared as {decl:?}, not {want}")));
        }
    }
    Ok(p)
}

/// `GetDropLifeSpanType`'s rule (module documentation).
pub fn drop_life_span_type(bands: &[(i32, i32, u8)], level: i32) -> u8 {
    bands.iter().find(|(min, max, _)| *min <= level && level <= *max).map_or(NO_BAND, |b| b.2)
}

/// `TheGlobalsDefinition` of `globals` when it holds a `GlobalsDefinition`, else the named `NotYet` (module
/// documentation).
pub fn globals_definition(vm: &mut Vm, globals: ObjRef, what: &str) -> Result<ObjRef, VmError> {
    let gearbox = vm.find_class(GEARBOX_GLOBALS_DEFINITION)?;
    let class = vm.find_class(GLOBALS_DEFINITION_CLASS)?;
    globals_definition_of(vm, globals, gearbox, class, what)
}

/// [`globals_definition`] with its two classes already found (the declared `GearboxGlobalsDefinition` and the
/// `GlobalsDefinition` it must hold): it loads nothing and writes nothing, so a preflight may call it (review r2 of
/// #521, item 2).
pub fn globals_definition_of(
    vm: &Vm,
    globals: ObjRef,
    gearbox: ClassId,
    class: ClassId,
    what: &str,
) -> Result<ObjRef, VmError> {
    declared_reference_to(vm, globals, "TheGlobalsDefinition", Some(gearbox), GEARBOX_GLOBALS_DEFINITION, what)?;
    let d = held_definition(vm, globals, what)?;
    definition_of_class(vm, d, class)
}

/// [`globals_definition`] with no load and no code registration (review r1 of #587): both classes through
/// [`ready_class`], the same refusals otherwise, in the same order.
pub fn globals_definition_loaded(vm: &Vm, globals: ObjRef, what: &str) -> Result<ObjRef, VmError> {
    declared_reference_loaded(vm, globals, "TheGlobalsDefinition", Some(GEARBOX_GLOBALS_DEFINITION), what)?;
    let d = held_definition(vm, globals, what)?;
    let class = ready_class(vm, GLOBALS_DEFINITION_CLASS)?;
    definition_of_class(vm, d, class)
}

/// [`globals_definition`] with no class lookup that loads (lane COLD-LOAD-FIXES), for a read-only path: both classes
/// are taken from the class table by path, because only their identity and the class chain are used (a declared type
/// and `is_child_of`, layout questions), not their code. A class the table does not hold cannot be one an existing
/// declaration or object names, so the same refusals follow without a load:
/// - no `GearboxGlobalsDefinition` in the table: the declaration names another class, a data error;
/// - no `GlobalsDefinition`: the held definition is not one, the first-load refusal.
pub fn globals_definition_resident(vm: &Vm, globals: ObjRef, what: &str) -> Result<ObjRef, VmError> {
    let gearbox = vm.space.classes.by_path(GEARBOX_GLOBALS_DEFINITION);
    declared_reference_to(vm, globals, "TheGlobalsDefinition", gearbox, GEARBOX_GLOBALS_DEFINITION, what)?;
    if gearbox.is_none() {
        return Err(bad(
            what,
            format!("TheGlobalsDefinition is declared as a reference to a class other than {GEARBOX_GLOBALS_DEFINITION}"),
        ));
    }
    let d = held_definition(vm, globals, what)?;
    match vm.space.classes.by_path(GLOBALS_DEFINITION_CLASS) {
        Some(class) => definition_of_class(vm, d, class),
        None => Err(first_load()),
    }
}

fn definition_of_class(vm: &Vm, d: ObjRef, class: ClassId) -> Result<ObjRef, VmError> {
    if vm.space.classes.is_child_of(vm.space.heap.get(d)?.class, class) { Ok(d) } else { Err(first_load()) }
}

fn first_load() -> VmError {
    NotYet::not_read("the globals definition's first load, which numbers the challenge list", RECORD).into()
}

/// The validated holder's definition: None is the first load, a handle not loaded yet the lazy load (review r1 of
/// #476).
fn held_definition(vm: &Vm, globals: ObjRef, what: &str) -> Result<ObjRef, VmError> {
    let first_load =
        || -> VmError { NotYet::not_read("the globals definition's first load, which numbers the challenge list", RECORD).into() };
    // The validated holder first (review r1 of #476): None is the first load.
    let Some(d) = object_of(&required(vm, globals, "TheGlobalsDefinition", what)?, what)? else {
        return Err(first_load());
    };
    // A definition handle not loaded yet is not loaded here: a lazy load is a write.
    if vm.defs.is_unloaded(&vm.space, d) {
        return Err(lazy_definition());
    }
    Ok(d)
}

/// The globals definition held as a handle the definition store has not loaded: a named `NotYet { NotRead }`, raised
/// without loading it (a lazy load is a write; the world's set-up preloads the installed definition).
fn lazy_definition() -> VmError {
    NotYet::not_read("the globals definition's lazy load (the handle is not loaded, and a load is a write)", RECORD).into()
}

fn add_pickup(c: &mut NativeCall<'_>) -> Result<Value, VmError> {
    const WHAT: &str = "WillowGlobals.AddPickup";
    let globals = this(c, WHAT)?;
    let pickup = c.arg(0)?.clone();
    // The declared element class, exactly WillowPickup, before the list is read (review r1 of #476).
    pickup_list_declared(c.vm, globals, WHAT)?;
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
    let refused: NativeFn = Rc::new(|_: &mut NativeCall<'_>| {
        Err(NotYet::not_read("PopulationMaster.RemoveSpawnedActor", RECORD).into())
    });
    let natives = natives.into_iter().chain(REMOVE_SPAWNED_ACTOR.map(|p| (p, refused.clone())));
    for (path, f) in natives {
        let Ok(id) = vm.find_function(path) else { continue };
        if !vm.function(id).is_some_and(|d| d.is_native()) {
            continue;
        }
        if !vm.is_bound(id) {
            vm.register_native_for(id, f)?;
        }
    }
    achievement::register(vm)?;
    class_achievement::register(vm)
}

#[cfg(test)]
#[path = "globals_tests.rs"]
mod tests;
