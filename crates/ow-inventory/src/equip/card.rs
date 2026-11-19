//! Owner: lane IMPL-CACHE-WEAPON-CARD-BODY. `WillowWeapon.CacheWeaponCard()` in `ParityMode::Off`, coded from
//! NATIVE_CACHE_WEAPON_CARD_BODY.md (#491 at its review-r1 head) and the outline NATIVE_CACHE_WEAPON_CARD.md section 3
//! alone (clean room: the implementer did not read the native code). AI-assisted (Claude Code). **Every rule is
//! UNVERIFIED in the running game**; the body note's last section lists the bl-sdk checks.
//!
//! [`cache_weapon_card`] computes the whole card first, writing nothing, then writes it:
//! 1. **The globals definition** through the Off getter's rule ([`crate::globals::world_globals`],
//!    [`crate::globals::globals_definition`]): no globals is the world's `NotYet { LayerAbsent }`, an unset
//!    definition the getter's `NotYet { NotRead }`. A None `WeaponTypeDefinition` then does nothing more.
//! 2. **The slot count** (body note section 4, step 3): which definition supplies it is not read, and its clamp always
//!    visits slot 0. So an activated slot from 1 to 18 is `NotYet { NotRead }` [`SLOT_COUNT`]; otherwise only slot 0
//!    can contribute. Slot 0, when `bActivated`, looks up the presentations of its `AttributeToModify` and its
//!    `ConstraintAttribute`; when it is `bIncludeInModifierText` it gives a slot entry (its `ComputedModifierValue`,
//!    displayed), and a None presentation clears its `bIncludeInModifierText` (a planned write).
//! 3. **The presentation lookup** ([`presentation`], section 2): the balance's DLC set's override then default list,
//!    then the globals definition's; the first non-None `Attributes` entry whose `Attribute` is the attribute; a
//!    `WeaponStatusEffectAttributePresentationDefinition` (by class name, through the parent chain) is replaced by the
//!    `Presentation` of the first `MappingData` entry whose `DamageType` is the weapon's (plain identity, None included),
//!    and by None when no entry matches.
//! 4. **The modifier records** (section 3.1): from the type, then each definition slot (the nine parts, then the two
//!    name parts; None skipped), `WeaponAttributeEffects` into the first map and `ExternalAttributeEffects` into the
//!    second, each `BaseModifierValue` evaluated with the weapon as context. A record is made on an attribute's first
//!    sight; a negative scale goes to its own sum, a zero value adds nothing.
//! 5. **The visits** (section 3.2): the type and the eleven slots, each `WeaponCardAttributes` entry's presentation added
//!    with its `PriorityIncrease` and a record required, then its `CustomPresentations`; then the damage type's
//!    `WeaponCardPresentations`. A visited definition that is neither a weapon type nor a weapon part is `NotYet
//!    { NotRead }` [`VISIT`] (its own visit is not read). Adding: an existing entry gains the increase; a new one takes
//!    its record by the presentation's `Attribute` (the first map's, else the second's), is skipped when a record is
//!    required and none exists, and starts at `BasePriority` plus the increase.
//! 6. **The order** (section 3.3): entries with a priority of at least 0.0001 are appended, largest priority first. Two
//!    appended entries of exactly equal priority are `NotYet { NotRead }` [`TIE`]: the sort is not stable. An appended
//!    entry with a record needs the display conversion for `bShouldDisplay`, which is read in outline only: `NotYet
//!    { NotRead }` [`DISPLAY`]. So every appended entry the port writes has no record: `ModifierValue` 0 and
//!    `bShouldDisplay` true. The value formula of section 3.3 is therefore never reached in Off and is not coded.
//! 7. **The writes**, in order: `WeaponCardModifierStats` emptied; slot 0's flag cleared when planned; the slot entry
//!    and the appended entries written; on the authority (`Role` is `ROLE_Authority`), the displayed entries after the
//!    slot entries fill the first `ReplicatedWeaponCardModifierValues` (at most 5), the rest left as they were.
//!
//! **The partial-writes mark** (the hub's rule, review r1 of #506): a `NotYet` met while computing (steps 1 to 6) comes
//! back unprefixed, since the native has written nothing; the caller layer that wrote before it (link 8's equip
//! driver) marks it. Only a `NotYet` after the native's own first write (step 7) is marked here, through
//! `ow_world::pools::after_partial_writes`. The computation's lazy definition loads are writes of the store; they run
//! inside the caller's init, after its writes, so the caller's mark covers them.
//!
//! **The preflight** ([`preflight`], body note section 4) replaces the outline's card refusal: NATIVE_WILLOW_GLOBALS.md
//! section 6.2 steps 1 to 3 (the globals instance, its class, `TheGlobalsDefinition` holding a `GlobalsDefinition`,
//! on a world set up per its section 6.1), the name-part schema, then the card schema ([`card_schema`]), each schema
//! failure a `VmError::Script` naming the field. Nothing is written.
//!
//! Classes the note names without a package are matched by their short name through the parent chain (the declared
//! reference types); the port invents no package path. **Reference** binds nothing (architecture rule R3).

use std::collections::HashMap;
use std::rc::Rc;

use ow_attr::stack::ModifierType;
use ow_core::{NotYet, ParityMode};
use ow_object::ids::{ClassId, ObjRef, PropAddr, StructId};
use ow_object::layout::PropKind;
use ow_object::value::Value;
use ow_vm::api::{NativeCall, Vm};
use ow_vm::error::VmError;

use super::name_parts::{WEAPON_CLASS, WEAPON_PART_CLASS, WEAPON_TYPE_CLASS, loaded};
use super::{WEAPON_PART_FIELDS, bad, effects_in, field, object_of, required};

/// The record of this module.
pub const RECORD: &str = "docs/verification/NATIVE_CACHE_WEAPON_CARD_BODY.md";

/// The native's script function.
pub const CACHE_WEAPON_CARD: &str = "WillowGame.WillowWeapon.CacheWeaponCard";

/// The refusal texts (body note section 4, step 3; each names the native).
pub const SLOT_COUNT: &str = "WillowWeapon.CacheWeaponCard: the card's slot count";
pub const TIE: &str = "WillowWeapon.CacheWeaponCard: the card order of equal-priority entries";
pub const DISPLAY: &str = "WillowWeapon.CacheWeaponCard: the presentation's displayed value";
pub const VISIT: &str =
    "WillowWeapon.CacheWeaponCard: the card visit of a definition that is neither a weapon type nor a weapon part";

const NATIVE: &str = "WillowWeapon.CacheWeaponCard";
const SCHEMA: &str = "WillowWeapon.CacheWeaponCard (the card schema)";

/// The lowest priority an appended entry has (section 3.3).
pub const THRESHOLD: f32 = 0.0001;
/// How many entries replication fills at most (the outline's step 7).
pub const REPLICATED: usize = 5;
/// The size the slot count's clamp falls back to (the outline's step 3).
const SLOT_CLAMP: usize = 19;

const GLOBALS_CLASS: &str = "WillowGame.WillowGlobals";
const STATUS_PRESENTATION: &str = "WeaponStatusEffectAttributePresentationDefinition";
const PRESENTATION: &str = "AttributePresentationDefinition";
const ATTRIBUTE: &str = "AttributeDefinition";
const LISTS: [&str; 2] = ["WeaponAttributePresentationOverride", "DefaultAttributePresentation"];

fn not_read(what: &'static str) -> VmError {
    NotYet::not_read(what, RECORD).into()
}

// ---- the preflight and the card schema -------------------------------------------------------------------------

/// The equip driver's preflight before the weapon is made (module documentation): the globals steps 1 to 3, the
/// name-part schema, then the card schema. Unprefixed: nothing is written.
pub fn preflight(vm: &mut Vm) -> Result<(), VmError> {
    const WHAT: &str = "the equip driver's globals";
    let globals = crate::globals::world_globals(vm)?;
    let willow = vm.find_class(GLOBALS_CLASS)?;
    if !vm.space.classes.is_child_of(vm.space.heap.get(globals)?.class, willow) {
        return Err(bad(
            WHAT,
            format!("the world's globals are a {}, not a WillowGlobals", ow_attr::eval::class_path(&vm.space, globals)),
        ));
    }
    crate::globals::globals_definition(vm, globals, WHAT)?;
    super::name_parts::name_part_schema(vm)?;
    card_schema(vm)
}

/// Whether class `c` or one of its parents has the short name `short` (the last segment of its path).
fn named(vm: &Vm, mut c: ClassId, short: &str) -> bool {
    loop {
        let Some(layout) = vm.space.classes.get_layout(c) else { return false };
        if layout.path().as_str().rsplit('.').next() == Some(short) {
            return true;
        }
        match layout.super_class() {
            Some(s) => c = s,
            None => return false,
        }
    }
}

fn class_name(vm: &Vm, c: ClassId) -> String {
    vm.space.classes.get_layout(c).map(|l| l.path().as_str().to_string()).unwrap_or_default()
}

fn schema(message: String) -> VmError {
    bad(SCHEMA, message)
}

/// The declared kind and dimension of `class`'s property `name` (through the parent chain); none is a schema error.
fn declared(vm: &Vm, class: ClassId, name: &str) -> Result<(PropKind, i32), VmError> {
    let p = vm.space.prop(class, name).ok_or_else(|| schema(format!("{} declares no {name}", class_name(vm, class))))?;
    let d = vm.space.classes.decl(p).ok_or_else(|| schema(format!("{}.{name} has no declaration", class_name(vm, class))))?;
    Ok((d.kind.clone(), d.array_dim))
}

/// The declared class `c`, with its own declarations loaded (a property's declared class can be only reserved until
/// the class is found; finding a class is a load of the code package, not a write).
fn defined(vm: &mut Vm, c: ClassId) -> Result<ClassId, VmError> {
    let path = class_name(vm, c);
    vm.find_class(&path)
}

/// `class.name`: one reference to a class named `target` (or a child of it); the declared class, loaded.
fn reference(vm: &mut Vm, class: ClassId, name: &str, target: &str) -> Result<ClassId, VmError> {
    match declared(vm, class, name)? {
        (PropKind::Object { class: c }, 1) if named(vm, c, target) => defined(vm, c),
        other => Err(schema(format!("{}.{name} is {other:?}, not one {target} reference", class_name(vm, class)))),
    }
}

/// `class.name`: a dynamic array of references to `target`; the declared class, loaded.
fn references(vm: &mut Vm, class: ClassId, name: &str, target: &str) -> Result<ClassId, VmError> {
    match declared(vm, class, name)? {
        (PropKind::Array(inner), 1) if matches!(*inner, PropKind::Object { class: c } if named(vm, c, target)) => {
            let PropKind::Object { class: c } = *inner else { unreachable!() };
            defined(vm, c)
        }
        other => Err(schema(format!("{}.{name} is {other:?}, not an array of {target} references", class_name(vm, class)))),
    }
}

/// Whether struct `s`'s path ends with `short`.
fn struct_named(vm: &Vm, s: StructId, short: &str) -> bool {
    vm.space.classes.struct_layout(s).is_some_and(|l| l.path().as_str().rsplit('.').next() == Some(short))
}

/// `class.name`: a dynamic array of the struct named `short`; the struct.
fn structs(vm: &Vm, class: ClassId, name: &str, short: &str) -> Result<StructId, VmError> {
    match declared(vm, class, name)? {
        (PropKind::Array(inner), 1) if matches!(*inner, PropKind::Struct(s) if struct_named(vm, s, short)) => {
            let PropKind::Struct(s) = *inner else { unreachable!() };
            Ok(s)
        }
        other => Err(schema(format!("{}.{name} is {other:?}, not an array of {short}", class_name(vm, class)))),
    }
}

/// A field of struct `s` and what it must be.
#[derive(Clone, Copy)]
enum Want<'a> {
    Ref(&'a str),
    AnyRef,
    Float,
    Bool,
    Byte,
    /// An int, or a byte.
    Index,
    Struct(&'a str),
}

fn kind_is(vm: &Vm, kind: &PropKind, want: Want<'_>) -> bool {
    match (want, kind) {
        (Want::Ref(t), PropKind::Object { class }) => named(vm, *class, t),
        (Want::AnyRef, PropKind::Object { .. }) => true,
        (Want::Float, PropKind::Float) | (Want::Bool, PropKind::Bool) => true,
        (Want::Byte, PropKind::Byte { .. }) => true,
        (Want::Index, PropKind::Int | PropKind::Byte { .. }) => true,
        (Want::Struct(t), PropKind::Struct(s)) => struct_named(vm, *s, t),
        _ => false,
    }
}

/// Struct `s` declares each of `fields` as wanted (dimension 1).
fn struct_fields(vm: &Vm, s: StructId, fields: &[(&str, Want<'_>)]) -> Result<(), VmError> {
    let layout = vm.space.classes.struct_layout(s).ok_or_else(|| schema("a card struct has no layout".into()))?;
    let path = layout.path().as_str().to_string();
    for (name, want) in fields {
        let d = vm.space.names.find(name).and_then(|id| layout.field(id)).map(|(_, d)| (d.kind.clone(), d.array_dim));
        match d {
            Some((k, 1)) if kind_is(vm, &k, *want) => {}
            other => return Err(schema(format!("{path}.{name} is {other:?}, not the field the card reads"))),
        }
    }
    Ok(())
}

/// `class` declares each of `fields` as wanted (dimension 1).
fn class_fields(vm: &Vm, class: ClassId, fields: &[(&str, Want<'_>)]) -> Result<(), VmError> {
    for (name, want) in fields {
        match declared(vm, class, name)? {
            (k, 1) if kind_is(vm, &k, *want) => {}
            other => return Err(schema(format!("{}.{name} is {other:?}, not the field the card reads", class_name(vm, class)))),
        }
    }
    Ok(())
}

/// `class.name`: a static array of the struct named `short`; the struct.
fn static_structs(vm: &Vm, class: ClassId, name: &str, short: &str) -> Result<StructId, VmError> {
    match declared(vm, class, name)? {
        (PropKind::Struct(s), dim) if dim >= 1 && struct_named(vm, s, short) => Ok(s),
        other => Err(schema(format!("{}.{name} is {other:?}, not a static array of {short}", class_name(vm, class)))),
    }
}

/// The card schema of the body note's section 4, step 1, on the declared classes, before any write.
pub fn card_schema(vm: &mut Vm) -> Result<(), VmError> {
    let weapon = vm.find_class(WEAPON_CLASS)?;
    let weapon_type = vm.find_class(WEAPON_TYPE_CLASS)?;
    let part = vm.find_class(WEAPON_PART_CLASS)?;
    let globals_definition = vm.find_class(crate::globals::GLOBALS_DEFINITION_CLASS)?;
    let status = vm.find_class(&format!("WillowGame.{STATUS_PRESENTATION}"))?;
    let presentation = vm.find_class(&format!("WillowGame.{PRESENTATION}"))?;
    // The balance (DefinitionData.BalanceDefinition, the outline's read) and its DLC set.
    let (data, _) = declared(vm, weapon, "DefinitionData")?;
    let PropKind::Struct(data) = data else {
        return Err(schema(format!("{WEAPON_CLASS}.DefinitionData is not a struct")));
    };
    struct_fields(vm, data, &[("BalanceDefinition", Want::Ref("InventoryBalanceDefinition"))])?;
    let balance = vm
        .space
        .classes
        .struct_layout(data)
        .and_then(|l| vm.space.names.find("BalanceDefinition").and_then(|id| l.field(id)))
        .and_then(|(_, d)| match d.kind {
            PropKind::Object { class } => Some(class),
            _ => None,
        })
        .ok_or_else(|| schema("WeaponDefinitionData.BalanceDefinition has no class".into()))?;
    let balance = defined(vm, balance)?;
    let mut balance_root = balance;
    while !vm.space.classes.get_layout(balance_root).is_some_and(|l| l.path().as_str().ends_with(".InventoryBalanceDefinition")) {
        match vm.space.classes.get_layout(balance_root).and_then(|l| l.super_class()) {
            Some(s) => balance_root = s,
            None => return Err(schema("BalanceDefinition is not an InventoryBalanceDefinition".into())),
        }
    }
    let dlc = reference(vm, balance_root, "DlcItemSet", "DownloadableItemSetDefinition")?;
    // The presentation lists on the DLC set and on the globals definition.
    for holder in [dlc, globals_definition] {
        for list in LISTS {
            let l = reference(vm, holder, list, "AttributePresentationListDefinition")?;
            references(vm, l, "Attributes", PRESENTATION)?;
        }
    }
    // The status-effect presentation's mapping.
    let mapping = structs(vm, status, "MappingData", "AttributePresentationDamageTypeMapping")?;
    struct_fields(vm, mapping, &[("Presentation", Want::Ref(PRESENTATION)), ("DamageType", Want::Ref("DamageTypeDefinition"))])?;
    // The type's and the part's card fields.
    for holder in [weapon_type, part] {
        let priority = structs(vm, holder, "WeaponCardAttributes", "AttributePriorityData")?;
        struct_fields(vm, priority, &[("Attribute", Want::Ref(ATTRIBUTE)), ("PriorityIncrease", Want::Float)])?;
        references(vm, holder, "CustomPresentations", PRESENTATION)?;
        for list in ["WeaponAttributeEffects", "ExternalAttributeEffects"] {
            let effect = structs(vm, holder, list, "AttributeEffectData")?;
            struct_fields(
                vm,
                effect,
                &[
                    ("AttributeToModify", Want::Ref(ATTRIBUTE)),
                    ("ModifierType", Want::Byte),
                    ("BaseModifierValue", Want::Struct("AttributeInitializationData")),
                ],
            )?;
        }
    }
    let damage = reference(vm, weapon_type, "DefaultDamageTypeDefinition", "WillowDamageTypeDefinition")?;
    reference(vm, part, "CustomDamageTypeDefinition", "WillowDamageTypeDefinition")?;
    references(vm, damage, "WeaponCardPresentations", PRESENTATION)?;
    // The presentation's own fields.
    class_fields(
        vm,
        presentation,
        &[
            ("BasePriority", Want::Float),
            ("Attribute", Want::Ref(ATTRIBUTE)),
            ("bDisplayAsPercentage", Want::Bool),
            ("bUseOneAsBaseValueForPresentationIfContextFailsToResolve", Want::Bool),
            ("bUseTwoAsBaseValueForPresentationIfContextFailsToResolve", Want::Bool),
        ],
    )?;
    // The weapon's card, its slots and its replicated values.
    let stats = structs(vm, weapon, "WeaponCardModifierStats", "ModifierValuePresentationData")?;
    struct_fields(
        vm,
        stats,
        &[
            ("AttributePresentation", Want::Ref(PRESENTATION)),
            ("OptionalConstraintPresentation", Want::Ref(PRESENTATION)),
            ("ModifierValue", Want::Float),
            ("bShouldDisplay", Want::Bool),
        ],
    )?;
    let slot = static_structs(vm, weapon, "AttributeSlots", "AttributeSlotData")?;
    struct_fields(
        vm,
        slot,
        &[
            ("bActivated", Want::Bool),
            ("bIncludeInModifierText", Want::Bool),
            ("AttributeToModify", Want::AnyRef),
            ("ConstraintAttribute", Want::AnyRef),
            ("ComputedModifierValue", Want::Float),
        ],
    )?;
    let replicated = match declared(vm, weapon, "ReplicatedWeaponCardModifierValues")? {
        (PropKind::Struct(s), dim) if dim >= 1 => s,
        other => return Err(schema(format!("{WEAPON_CLASS}.ReplicatedWeaponCardModifierValues is {other:?}, not a static array of structs"))),
    };
    struct_fields(vm, replicated, &[("ModifierValue", Want::Float), ("ModifierStatIndex", Want::Index)])?;
    class_fields(vm, weapon, &[("Role", Want::Byte)])
}

// ---- the computation ---------------------------------------------------------------------------------------------

/// One attribute's accumulated modifiers (section 3.1).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Record {
    pub scale: f32,
    pub pre_add: f32,
    pub post_add: f32,
    pub negative_scale: f32,
}

/// One card entry (sections 3.2 and 3.3).
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub presentation: ObjRef,
    pub record: Option<Record>,
    pub priority: f32,
}

/// One written `ModifierValuePresentationData`.
#[derive(Debug, Clone, PartialEq)]
pub struct Stat {
    pub presentation: Option<ObjRef>,
    pub constraint: Option<ObjRef>,
    pub value: f32,
    pub display: bool,
}

/// The whole card, computed before any write.
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    /// Slot 0's `bIncludeInModifierText` is cleared.
    pub clear_slot_flag: bool,
    /// The slot entries (S of them), then the appended entries.
    pub stats: Vec<Stat>,
    pub slot_entries: usize,
}

fn obj(vm: &Vm, o: ObjRef, name: &str) -> Result<Option<ObjRef>, VmError> {
    object_of(&required(vm, o, name, NATIVE)?, NATIVE)
}

fn objs(vm: &Vm, o: ObjRef, name: &str) -> Result<Vec<Option<ObjRef>>, VmError> {
    let v = required(vm, o, name, NATIVE)?;
    ow_attr::eval::elements(name, &v)?.iter().map(|e| object_of(e, NATIVE)).collect()
}

fn values(vm: &Vm, o: ObjRef, name: &str) -> Result<Vec<Value>, VmError> {
    let v = required(vm, o, name, NATIVE)?;
    Ok(ow_attr::eval::elements(name, &v)?.to_vec())
}

fn float(v: Value, name: &str) -> Result<f32, VmError> {
    match v {
        Value::Float(x) => Ok(x),
        other => Err(bad(NATIVE, format!("{name} holds {}, not a float", other.kind_name()))),
    }
}

fn flag(v: Value, name: &str) -> Result<bool, VmError> {
    match v {
        Value::Bool(b) => Ok(b),
        other => Err(bad(NATIVE, format!("{name} holds {}, not a bool", other.kind_name()))),
    }
}

/// What the lookup reads besides the attribute.
struct Lookup {
    dlc: Option<ObjRef>,
    globals_definition: ObjRef,
    damage: Option<ObjRef>,
}

fn search(vm: &mut Vm, list: Option<ObjRef>, a: ObjRef) -> Result<Option<ObjRef>, VmError> {
    let Some(list) = list else { return Ok(None) };
    loaded(vm, list)?;
    for p in objs(vm, list, "Attributes")?.into_iter().flatten() {
        loaded(vm, p)?;
        if obj(vm, p, "Attribute")? == Some(a) {
            return Ok(Some(p));
        }
    }
    Ok(None)
}

/// The presentation of attribute `a` (module documentation, step 3).
fn presentation(vm: &mut Vm, lk: &Lookup, a: Option<ObjRef>) -> Result<Option<ObjRef>, VmError> {
    let Some(a) = a else { return Ok(None) };
    let mut found = None;
    for holder in [lk.dlc, Some(lk.globals_definition)].into_iter().flatten() {
        for list in LISTS {
            if found.is_none() {
                let l = obj(vm, holder, list)?;
                found = search(vm, l, a)?;
            }
        }
    }
    let Some(p) = found else { return Ok(None) };
    let class = vm.space.heap.get(p)?.class;
    if !named(vm, class, STATUS_PRESENTATION) {
        return Ok(Some(p));
    }
    for m in values(vm, p, "MappingData")? {
        if object_of(&field(vm, &m, "DamageType")?, NATIVE)? == lk.damage {
            return object_of(&field(vm, &m, "Presentation")?, NATIVE);
        }
    }
    Ok(None)
}

/// The weapon's damage type: the elemental part's `CustomDamageTypeDefinition` when both are set, else the type's
/// `DefaultDamageTypeDefinition`.
fn damage_type(vm: &mut Vm, data: &Value, weapon_type: ObjRef) -> Result<Option<ObjRef>, VmError> {
    if let Some(e) = object_of(&field(vm, data, "ElementalPartDefinition")?, NATIVE)? {
        loaded(vm, e)?;
        if let Some(d) = obj(vm, e, "CustomDamageTypeDefinition")? {
            return Ok(Some(d));
        }
    }
    obj(vm, weapon_type, "DefaultDamageTypeDefinition")
}

/// The two maps of section 3.1 from `holders`, in order.
fn accumulate(
    vm: &mut Vm,
    weapon: ObjRef,
    holders: &[ObjRef],
) -> Result<[HashMap<ObjRef, Record>; 2], VmError> {
    let mut maps: [HashMap<ObjRef, Record>; 2] = [HashMap::new(), HashMap::new()];
    for &h in holders {
        for (list, map) in ["WeaponAttributeEffects", "ExternalAttributeEffects"].into_iter().zip(maps.iter_mut()) {
            let v = required(vm, h, list, NATIVE)?;
            for e in effects_in(vm, &v)? {
                let Some(a) = e.attribute_to_modify else { continue };
                let sys = vm.attrs.clone();
                let value =
                    sys.evaluate_init(&e.base_modifier_value, Some(weapon), None, &mut ow_attr::api::EvalCx::new(vm))?;
                let r = map.entry(a).or_default();
                match e.modifier_type {
                    ModifierType::Scale if value < 0.0 => r.negative_scale += value,
                    _ if value == 0.0 => {}
                    ModifierType::Scale => r.scale += value,
                    ModifierType::PreAdd => r.pre_add += value,
                    ModifierType::PostAdd => r.post_add += value,
                }
            }
        }
    }
    Ok(maps)
}

/// Adds presentation `p` with increase `inc` (section 3.2, "Adding a presentation").
fn add(
    vm: &mut Vm,
    entries: &mut Vec<Entry>,
    maps: &[HashMap<ObjRef, Record>; 2],
    p: Option<ObjRef>,
    inc: f32,
    record_required: bool,
) -> Result<(), VmError> {
    let Some(p) = p else { return Ok(()) };
    if let Some(e) = entries.iter_mut().find(|e| e.presentation == p) {
        e.priority += inc;
        return Ok(());
    }
    loaded(vm, p)?;
    let record = obj(vm, p, "Attribute")?.and_then(|a| maps[0].get(&a).or_else(|| maps[1].get(&a)).copied());
    if record_required && record.is_none() {
        return Ok(());
    }
    let base = float(required(vm, p, "BasePriority", NATIVE)?, "BasePriority")?;
    entries.push(Entry { presentation: p, record, priority: base + inc });
    Ok(())
}

/// The card of `weapon`, computed with no write; `None` when the type is None (module documentation, steps 1 to 6).
pub fn compute(vm: &mut Vm, weapon: ObjRef) -> Result<Option<Card>, VmError> {
    // 1. The globals definition, through the Off getter's rule.
    let globals = crate::globals::world_globals(vm)?;
    let globals_definition = crate::globals::globals_definition(vm, globals, NATIVE)?;
    let data = required(vm, weapon, "DefinitionData", NATIVE)?;
    let Some(weapon_type) = object_of(&field(vm, &data, "WeaponTypeDefinition")?, NATIVE)? else {
        return Ok(None);
    };
    loaded(vm, weapon_type)?;
    let dlc = match object_of(&field(vm, &data, "BalanceDefinition")?, NATIVE)? {
        Some(b) => {
            loaded(vm, b)?;
            let d = obj(vm, b, "DlcItemSet")?;
            if let Some(d) = d {
                loaded(vm, d)?;
            }
            d
        }
        None => None,
    };
    let damage = damage_type(vm, &data, weapon_type)?;
    let lk = Lookup { dlc, globals_definition, damage };
    // 2. The slot count, then slot 0.
    let class = vm.space.heap.get(weapon)?.class;
    let p = vm.space.prop(class, "AttributeSlots").ok_or_else(|| bad(NATIVE, "the weapon declares no AttributeSlots"))?;
    let dim = vm.space.classes.decl(p).map(|d| d.array_dim).unwrap_or(0);
    let dim = usize::try_from(dim).unwrap_or(0).min(SLOT_CLAMP);
    let slot = |vm: &Vm, i: usize| -> Result<Value, VmError> {
        Ok(vm.space.read(weapon, PropAddr::element(p, u32::try_from(i).unwrap_or(u32::MAX)))?.clone())
    };
    for i in 1..dim {
        if flag(field(vm, &slot(vm, i)?, "bActivated")?, "bActivated")? {
            return Err(not_read(SLOT_COUNT));
        }
    }
    let mut stats = Vec::new();
    let mut clear_slot_flag = false;
    if dim >= 1 {
        let s = slot(vm, 0)?;
        if flag(field(vm, &s, "bActivated")?, "bActivated")? {
            let modify = presentation(vm, &lk, object_of(&field(vm, &s, "AttributeToModify")?, NATIVE)?)?;
            let constraint = presentation(vm, &lk, object_of(&field(vm, &s, "ConstraintAttribute")?, NATIVE)?)?;
            if flag(field(vm, &s, "bIncludeInModifierText")?, "bIncludeInModifierText")? {
                clear_slot_flag = modify.is_none();
                let value = float(field(vm, &s, "ComputedModifierValue")?, "ComputedModifierValue")?;
                stats.push(Stat { presentation: modify, constraint, value, display: true });
            }
        }
    }
    let slot_entries = stats.len();
    // 4. The records: the type, then each definition slot.
    let mut visited = vec![weapon_type];
    for name in WEAPON_PART_FIELDS.iter().copied().chain(["PrefixPartDefinition", "TitlePartDefinition"]) {
        if let Some(d) = object_of(&field(vm, &data, name)?, NATIVE)? {
            loaded(vm, d)?;
            visited.push(d);
        }
    }
    let maps = accumulate(vm, weapon, &visited)?;
    // 5. The visits.
    let type_class = vm.find_class(WEAPON_TYPE_CLASS)?;
    let part_class = vm.find_class(WEAPON_PART_CLASS)?;
    let mut entries = Vec::new();
    for &d in &visited {
        let c = vm.space.heap.get(d)?.class;
        if !vm.space.classes.is_child_of(c, type_class) && !vm.space.classes.is_child_of(c, part_class) {
            return Err(not_read(VISIT));
        }
        for row in values(vm, d, "WeaponCardAttributes")? {
            let a = object_of(&field(vm, &row, "Attribute")?, NATIVE)?;
            let inc = float(field(vm, &row, "PriorityIncrease")?, "PriorityIncrease")?;
            let p = presentation(vm, &lk, a)?;
            add(vm, &mut entries, &maps, p, inc, true)?;
        }
        for p in objs(vm, d, "CustomPresentations")? {
            add(vm, &mut entries, &maps, p, 0.0, false)?;
        }
    }
    if let Some(dt) = damage {
        loaded(vm, dt)?;
        for p in objs(vm, dt, "WeaponCardPresentations")? {
            add(vm, &mut entries, &maps, p, 0.0, false)?;
        }
    }
    // 6. The order, the tie and the display refusals, on the appended entries only.
    let mut appended: Vec<Entry> = entries.into_iter().filter(|e| e.priority >= THRESHOLD).collect();
    appended.sort_by(|a, b| b.priority.total_cmp(&a.priority));
    if appended.windows(2).any(|w| w[0].priority == w[1].priority) {
        return Err(not_read(TIE));
    }
    if appended.iter().any(|e| e.record.is_some()) {
        return Err(not_read(DISPLAY));
    }
    stats.extend(appended.into_iter().map(|e| Stat { presentation: Some(e.presentation), constraint: None, value: 0.0, display: true }));
    Ok(Some(Card { clear_slot_flag, stats, slot_entries }))
}

/// `ModifierValuePresentationData` values of `stats`, of the declared element struct `ty`.
fn stat_values(vm: &Vm, ty: StructId, stats: &[Stat]) -> Result<Vec<Value>, VmError> {
    let mut out = Vec::new();
    for s in stats {
        let mut v = vm.space.classes.struct_defaults(ty).map_err(|e| bad(NATIVE, e.to_string()))?;
        set(vm, &mut v, "AttributePresentation", Value::Object(s.presentation))?;
        set(vm, &mut v, "OptionalConstraintPresentation", Value::Object(s.constraint))?;
        set(vm, &mut v, "ModifierValue", Value::Float(s.value))?;
        set(vm, &mut v, "bShouldDisplay", Value::Bool(s.display))?;
        out.push(v);
    }
    Ok(out)
}

/// Struct value `v`'s field `name` set to `x` (declared, else a data error).
fn set(vm: &Vm, v: &mut Value, name: &str, x: Value) -> Result<(), VmError> {
    let Value::Struct(s) = v else { return Err(bad(NATIVE, format!("{name}: not a struct"))) };
    let layout = vm.space.classes.struct_layout(s.ty).ok_or_else(|| bad(NATIVE, "a struct with no layout"))?;
    let (i, _) = vm
        .space
        .names
        .find(name)
        .and_then(|id| layout.field(id))
        .ok_or_else(|| bad(NATIVE, format!("the struct declares no {name}")))?;
    let slot = Rc::make_mut(s).fields.get_mut(i as usize).ok_or_else(|| bad(NATIVE, format!("{name} is out of range")))?;
    *slot = x;
    Ok(())
}

/// The writes of a computed card (module documentation, step 7).
fn write_card(vm: &mut Vm, weapon: ObjRef, card: &Card) -> Result<(), VmError> {
    let class = vm.space.heap.get(weapon)?.class;
    let prop = |vm: &Vm, name: &str| vm.space.prop(class, name).ok_or_else(|| bad(NATIVE, format!("the weapon declares no {name}")));
    let stats_prop = prop(vm, "WeaponCardModifierStats")?;
    let ty = match vm.space.classes.decl(stats_prop).map(|d| d.kind.clone()) {
        Some(PropKind::Array(inner)) => match *inner {
            PropKind::Struct(s) => s,
            _ => return Err(bad(NATIVE, "WeaponCardModifierStats is not an array of structs")),
        },
        _ => return Err(bad(NATIVE, "WeaponCardModifierStats is not an array")),
    };
    let values = stat_values(vm, ty, &card.stats)?;
    // 1. The clear.
    vm.space.write(weapon, PropAddr::scalar(stats_prop), Value::array(Vec::new()))?;
    // 2. Slot 0's flag.
    if card.clear_slot_flag {
        let slots = prop(vm, "AttributeSlots")?;
        let mut s = vm.space.read(weapon, PropAddr::element(slots, 0))?.clone();
        set(vm, &mut s, "bIncludeInModifierText", Value::Bool(false))?;
        vm.space.write(weapon, PropAddr::element(slots, 0), s)?;
    }
    // 3. The entries.
    vm.space.write(weapon, PropAddr::scalar(stats_prop), Value::array(values))?;
    // 4. Replication on the authority.
    if !super::slots::is_authority(vm, weapon, NATIVE)? {
        return Ok(());
    }
    let rep = prop(vm, "ReplicatedWeaponCardModifierValues")?;
    let dim = vm.space.classes.decl(rep).map(|d| usize::try_from(d.array_dim).unwrap_or(0)).unwrap_or(0);
    let mut next = 0;
    for (index, stat) in card.stats.iter().enumerate().skip(card.slot_entries) {
        if next >= REPLICATED.min(dim) {
            break;
        }
        if !stat.display {
            continue;
        }
        let at = PropAddr::element(rep, u32::try_from(next).map_err(|_| bad(NATIVE, "an index past u32"))?);
        let mut v = vm.space.read(weapon, at)?.clone();
        set(vm, &mut v, "ModifierValue", Value::Float(stat.value))?;
        let slot_index = match field(vm, &v, "ModifierStatIndex")? {
            Value::Byte(_) => Value::Byte(u8::try_from(index).map_err(|_| bad(NATIVE, "ModifierStatIndex past a byte"))?),
            _ => Value::Int(i32::try_from(index).map_err(|_| bad(NATIVE, "ModifierStatIndex past an int"))?),
        };
        set(vm, &mut v, "ModifierStatIndex", slot_index)?;
        vm.space.write(weapon, at, v)?;
        next += 1;
    }
    Ok(())
}

/// `CacheWeaponCard` on `weapon` in Off (module documentation): compute, writing nothing (a `NotYet` unprefixed), then
/// write (a `NotYet` after the first write marked).
pub fn cache_weapon_card(vm: &mut Vm, weapon: ObjRef) -> Result<(), VmError> {
    match compute(vm, weapon)? {
        Some(card) => write_card(vm, weapon, &card).map_err(ow_world::pools::after_partial_writes),
        None => Ok(()),
    }
}

/// Binds the native in `ParityMode::Off` by its declaration's own native key (a package without the function binds
/// nothing); Reference binds nothing.
pub fn register(vm: &mut Vm) -> Result<(), VmError> {
    if vm.parity != ParityMode::Off {
        return Ok(());
    }
    let Ok(id) = vm.find_function(CACHE_WEAPON_CARD) else { return Ok(()) };
    let Some(key) = vm.function(id).and_then(|d| d.native_key.clone()) else { return Ok(()) };
    vm.register_native(
        &key,
        Rc::new(|c: &mut NativeCall<'_>| {
            let me = c.this.ok_or_else(|| bad(NATIVE, "called without a weapon"))?;
            cache_weapon_card(c.vm, me)?;
            Ok(Value::None)
        }),
    )?;
    Ok(())
}

#[cfg(test)]
mod tests;
