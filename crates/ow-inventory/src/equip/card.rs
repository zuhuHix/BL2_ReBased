//! Owner: lane IMPL-CACHE-WEAPON-CARD-BODY. `WillowWeapon.CacheWeaponCard()` in `ParityMode::Off`, coded from
//! NATIVE_CACHE_WEAPON_CARD_BODY.md (#491 at its review-r1 head) and the outline NATIVE_CACHE_WEAPON_CARD.md section 3
//! alone (clean room: the implementer did not read the native code). AI-assisted (Claude Code). **Every rule is
//! UNVERIFIED in the running game**; the body note's last section lists the bl-sdk checks.
//!
//! [`cache_weapon_card`] computes the whole card first, writing nothing, then writes it:
//! 1. **The globals definition** through the Off getter's rule ([`crate::globals::world_globals`],
//!    [`crate::globals::globals_definition`]): no globals is the world's `NotYet { LayerAbsent }`, an unset
//!    definition the getter's `NotYet { NotRead }`. A None `WeaponTypeDefinition` then does nothing more.
//! 2. **The weapon type and the slots** (lane IMPL-CARD-SLOT-COUNT, NATIVE_CARD_SLOT_COUNT.md sections 2 to 4). A
//!    weapon type that is an unloaded definition handle is `NotYet { NotRead }` ([`unloaded_type`]), before any load
//!    and before the balance, damage-type and slot reads. The count is the type's `AttributeSlotMaxActivated` (its
//!    class default when not stored), 1 to 19 as it is and 19 otherwise ([`super::natives::clamped_slot_max`], the
//!    `GetAttributeSlotMaxActivated` rule), and never more than the weapon's declared `AttributeSlots`. Slots 0 to
//!    count - 1 run the same step, in index order: a slot that is not `bActivated` is skipped; otherwise the
//!    presentations of its `AttributeToModify` and its `ConstraintAttribute` are looked up, and when it is
//!    `bIncludeInModifierText` it gives a slot entry (its `ComputedModifierValue`, displayed), a None presentation
//!    clearing its `bIncludeInModifierText` (a planned write). Slots at or past the count are not read.
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
//! 6. **The order** (NATIVE_CARD_BODY_REST.md section 2, lane IMPL-CARD-BODY-REST): a NaN priority among all the
//!    entries is `NotYet { NotRead }` [`UNORDERED`]; otherwise the whole list, in first-insertion order, is sorted by
//!    [`sort_entries`] (the engine's unstable sort, as the note's rules give it), and only then are the entries with a
//!    priority of at least 0.0001 appended, in the sorted order.
//! 7. **The values** (body note section 3.3; NATIVE_CARD_BODY_REST.md section 3): each appended entry's base value B,
//!    then with no record `ModifierValue` 0 and `bShouldDisplay` true; with a record the value V ([`value_of`]) and
//!    `bShouldDisplay` from the presentation's displayed form of V ([`shows`]), with the rest note's typed refusals
//!    ([`REMAP_CONTEXT`], [`RANGE`], [`PRECISION`], [`CUTOFF`]), all raised before the first card write.
//! 8. **The writes**, in order: `WeaponCardModifierStats` emptied; each planned slot flag cleared; the slot entries
//!    and the appended entries written; on the authority (`Role` is `ROLE_Authority`), the displayed entries after the
//!    slot entries fill the first `ReplicatedWeaponCardModifierValues` (at most 5), the rest left as they were.
//!
//! **The partial-writes mark** (the hub's rule, review r1 of #506): a `NotYet` met while computing (steps 1 to 7) comes
//! back unprefixed, since the native has written nothing; the caller layer that wrote before it (link 8's equip
//! driver) marks it. Only a `NotYet` after the native's own first write (step 8) is marked here, through
//! `ow_world::pools::after_partial_writes`. The computation's lazy definition loads are writes of the store; they run
//! inside the caller's init, after its writes, so the caller's mark covers them.
//!
//! **The preflight** ([`preflight`], body note section 4) replaces the outline's card refusal: NATIVE_WILLOW_GLOBALS.md
//! section 6.2 steps 1 to 3 (the globals instance, its class, `TheGlobalsDefinition` holding a `GlobalsDefinition`,
//! on a world set up per its section 6.1), the name-part schema, then the card schema ([`card_schema`]), each schema
//! failure a `VmError::Script` naming the field. Nothing is written, and nothing is loaded: classes are found with
//! non-loading lookups. [`setup`] loads them first, at world set-up, together with every class a checked field
//! declares and its parents (review r1 of #521).
//!
//! Classes the note names without a package are matched by their short name through the parent chain (the declared
//! reference types); the port invents no package path. **Reference** binds nothing (architecture rule R3).

use ow_core::DetMap;
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
/// The record of the slot count and the unloaded weapon type (lane IMPL-CARD-SLOT-COUNT).
pub const SLOT_COUNT_RECORD: &str = "docs/verification/NATIVE_CARD_SLOT_COUNT.md";

/// The native's script function.
pub const CACHE_WEAPON_CARD: &str = "WillowGame.WillowWeapon.CacheWeaponCard";

/// The refusal texts (body note section 4, step 3; each names the native).
/// The record of the order and the displayed value (#532), and its typed refusals (its section 4, item 3; each text
/// is the note's, with the class name before it as #521's are).
pub const REST_RECORD: &str = "docs/verification/NATIVE_CARD_BODY_REST.md";
pub const UNORDERED: &str = "WillowWeapon.CacheWeaponCard: an unordered card priority";
pub const REMAP_CONTEXT: &str =
    "WillowWeapon.CacheWeaponCard: the remap's default context (the first local player's controller, by fit)";
pub const RANGE: &str = "WillowWeapon.CacheWeaponCard: a displayed value outside the integer range";
pub const PRECISION: &str = "WillowWeapon.CacheWeaponCard: the display rounding's intermediate precision";
pub const CUTOFF: &str = "WillowWeapon.CacheWeaponCard: a displayed value at the display cutoff";
pub const VISIT: &str =
    "WillowWeapon.CacheWeaponCard: the card visit of a definition that is neither a weapon type nor a weapon part";

const NATIVE: &str = "WillowWeapon.CacheWeaponCard";
const SCHEMA: &str = "WillowWeapon.CacheWeaponCard (the card schema)";

/// The lowest priority an appended entry has (section 3.3).
pub const THRESHOLD: f32 = 0.0001;
/// How many entries replication fills at most (the outline's step 7).
pub const REPLICATED: usize = 5;

const GLOBALS_CLASS: &str = "WillowGame.WillowGlobals";
const STATUS_PRESENTATION: &str = "WeaponStatusEffectAttributePresentationDefinition";
const PRESENTATION: &str = "AttributePresentationDefinition";
const ATTRIBUTE: &str = "AttributeDefinition";
const REMAP_STRUCT: &str = "AttributePresentationValueRemappingData";
const LISTS: [&str; 2] = ["WeaponAttributePresentationOverride", "DefaultAttributePresentation"];

fn not_read(what: &'static str) -> VmError {
    NotYet::not_read(what, RECORD).into()
}

fn rest(what: &'static str) -> VmError {
    NotYet::not_read(what, REST_RECORD).into()
}

// ---- the setup, the preflight and the card schema --------------------------------------------------------------

/// The classes the schema walk starts from.
const ROOTS: [&str; 9] = [
    WEAPON_CLASS,
    WEAPON_TYPE_CLASS,
    WEAPON_PART_CLASS,
    super::name_parts::NAME_PART_CLASS,
    crate::globals::GLOBALS_DEFINITION_CLASS,
    crate::globals::GEARBOX_GLOBALS_DEFINITION,
    "WillowGame.WeaponStatusEffectAttributePresentationDefinition",
    "WillowGame.AttributePresentationDefinition",
    GLOBALS_CLASS,
];

/// What stops the schema walk: a class a checked field declares that is only reserved (its declarations and parents
/// not loaded yet), or a schema error.
enum Walk {
    Reserved(ClassId),
    Bad(VmError),
}

impl From<VmError> for Walk {
    fn from(e: VmError) -> Self {
        Walk::Bad(e)
    }
}

/// The card's explicit setup (the hub's ruling on #521: class and code loading happens at world set-up or pack bind,
/// never in a preflight). It finds the [`ROOTS`], then each class a checked field declares, with its parents, until the
/// schema walk meets no reserved class. It validates nothing: the preflight does that, read-only. A failed class
/// lookup is returned as the VM gave it.
pub fn setup(vm: &mut Vm) -> Result<(), VmError> {
    for root in ROOTS {
        vm.find_class(root)?;
    }
    let mut lookups = 0;
    let mut resolve = |vm: &mut Vm, c: ClassId| -> Result<(), VmError> {
        lookups += 1;
        if lookups > 512 {
            return Err(schema("the card setup met more than 512 reserved classes".into()));
        }
        let path = class_name(vm, c);
        let id = vm.find_class(&path)?;
        if is_defined(vm, id) { Ok(()) } else { Err(schema(format!("{path} stays reserved after its lookup"))) }
    };
    loop {
        match walk(vm) {
            Ok(()) | Err(Walk::Bad(_)) => break,
            Err(Walk::Reserved(c)) => resolve(vm, c)?,
        }
    }
    // The classes the name-part schema's references declare, with every parent (review r2 of #521, item 4): its
    // `is_child_of` test needs the whole chain, and a reserved class has no parent yet.
    for start in super::name_parts::name_part_references(vm) {
        let mut c = Some(start);
        while let Some(class) = c {
            if !is_defined(vm, class) {
                resolve(vm, class)?;
            }
            c = vm.space.classes.get_layout(class).and_then(|l| l.super_class());
        }
    }
    Ok(())
}

/// The equip driver's preflight before the weapon is made (module documentation): the globals steps 1 to 3, the
/// name-part schema, then the card schema. Read-only and unprefixed: class lookups do not load (the setup did), and
/// nothing is written. The globals definition's own class check finds a class the setup loaded.
pub fn preflight(vm: &mut Vm) -> Result<(), VmError> {
    const WHAT: &str = "the equip driver's globals";
    let globals = crate::globals::world_globals(vm)?;
    let willow = defined_class(vm, GLOBALS_CLASS, WHAT)?;
    if !vm.space.classes.is_child_of(vm.space.heap.get(globals)?.class, willow) {
        return Err(bad(
            WHAT,
            format!("the world's globals are a {}, not a WillowGlobals", ow_attr::eval::class_path(&vm.space, globals)),
        ));
    }
    // Both classes by `by_path` only (review r2 of #521, item 2): the setup found them.
    let gearbox = defined_class(vm, crate::globals::GEARBOX_GLOBALS_DEFINITION, WHAT)?;
    let class = defined_class(vm, crate::globals::GLOBALS_DEFINITION_CLASS, WHAT)?;
    crate::globals::globals_definition_of(vm, globals, gearbox, class, WHAT)?;
    super::name_parts::name_part_schema(vm)?;
    card_schema(vm)
}

fn is_defined(vm: &Vm, c: ClassId) -> bool {
    vm.space.classes.get_layout(c).is_some_and(|l| l.is_defined())
}

/// The defined class at `path`, without loading anything; a class not loaded yet is a data error of `what` (the
/// card's setup loads the ones the card reads).
pub(crate) fn defined_class(vm: &Vm, path: &str, what: &str) -> Result<ClassId, VmError> {
    match vm.space.classes.by_path(path) {
        Some(c) if is_defined(vm, c) => Ok(c),
        _ => Err(bad(what, format!("class {path} is not loaded (equip::card::setup loads it before the preflight)"))),
    }
}

/// Class `c`, defined, or the walk's [`Walk::Reserved`].
fn need(vm: &Vm, c: ClassId) -> Result<ClassId, Walk> {
    if is_defined(vm, c) { Ok(c) } else { Err(Walk::Reserved(c)) }
}

/// Whether class `c` or one of its parents has the short name `short` (its own name, after the last `.` or `:`).
fn named(vm: &Vm, mut c: ClassId, short: &str) -> bool {
    loop {
        let Some(layout) = vm.space.classes.get_layout(c) else { return false };
        if layout.path().name() == short {
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

/// `class.name`: one reference to a class named `target` (or a child of it); the declared class.
fn reference(vm: &Vm, class: ClassId, name: &str, target: &str) -> Result<ClassId, Walk> {
    let found = declared(vm, class, name)?;
    if let (PropKind::Object { class: c }, 1) = found
        && named(vm, need(vm, c)?, target)
    {
        return Ok(c);
    }
    Err(schema(format!("{}.{name} is {found:?}, not one {target} reference", class_name(vm, class))).into())
}

/// `class.name`: a dynamic array of references to `target`; the declared class.
fn references(vm: &Vm, class: ClassId, name: &str, target: &str) -> Result<ClassId, Walk> {
    let found = declared(vm, class, name)?;
    if let (PropKind::Array(inner), 1) = &found
        && let PropKind::Object { class: c } = **inner
        && named(vm, need(vm, c)?, target)
    {
        return Ok(c);
    }
    Err(schema(format!("{}.{name} is {found:?}, not an array of {target} references", class_name(vm, class))).into())
}

/// Whether struct `s`'s own name is `short`.
fn struct_named(vm: &Vm, s: StructId, short: &str) -> bool {
    vm.space.classes.struct_layout(s).is_some_and(|l| l.path().name() == short)
}

/// `class.name`: a dynamic array of the struct named `short`; the struct.
fn structs(vm: &Vm, class: ClassId, name: &str, short: &str) -> Result<StructId, Walk> {
    match declared(vm, class, name)? {
        (PropKind::Array(inner), 1) if matches!(*inner, PropKind::Struct(s) if struct_named(vm, s, short)) => {
            let PropKind::Struct(s) = *inner else { unreachable!() };
            Ok(s)
        }
        other => Err(schema(format!("{}.{name} is {other:?}, not an array of {short}", class_name(vm, class))).into()),
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
    Int,
    Struct(&'a str),
    /// A byte of an enum that names `ROLE_Authority` (what `slots::is_authority` reads; review r1 of #521, item 7).
    Role,
}

fn kind_is(vm: &Vm, kind: &PropKind, want: Want<'_>) -> Result<bool, Walk> {
    Ok(match (want, kind) {
        (Want::Ref(t), PropKind::Object { class }) => named(vm, need(vm, *class)?, t),
        (Want::AnyRef, PropKind::Object { .. }) => true,
        (Want::Float, PropKind::Float) | (Want::Bool, PropKind::Bool) => true,
        (Want::Byte, PropKind::Byte { .. }) => true,
        (Want::Index, PropKind::Int | PropKind::Byte { .. }) => true,
        (Want::Int, PropKind::Int) => true,
        (Want::Struct(t), PropKind::Struct(s)) => struct_named(vm, *s, t),
        (Want::Role, PropKind::Byte { enum_: Some(e) }) => vm
            .space
            .classes
            .enum_layout(*e)
            .and_then(|l| vm.space.names.find("ROLE_Authority").and_then(|n| l.index_of(n)))
            .is_some(),
        _ => false,
    })
}

/// Struct `s` declares each of `fields` as wanted (dimension 1).
fn struct_fields(vm: &Vm, s: StructId, fields: &[(&str, Want<'_>)]) -> Result<(), Walk> {
    let layout = vm.space.classes.struct_layout(s).ok_or_else(|| schema("a card struct has no layout".into()))?;
    let path = layout.path().as_str().to_string();
    for (name, want) in fields {
        let d = vm.space.names.find(name).and_then(|id| layout.field(id)).map(|(_, d)| (d.kind.clone(), d.array_dim));
        let ok = match &d {
            Some((k, 1)) => kind_is(vm, k, *want)?,
            _ => false,
        };
        if !ok {
            return Err(schema(format!("{path}.{name} is {d:?}, not the field the card reads")).into());
        }
    }
    Ok(())
}

/// `class` declares each of `fields` as wanted (dimension 1).
fn class_fields(vm: &Vm, class: ClassId, fields: &[(&str, Want<'_>)]) -> Result<(), Walk> {
    for (name, want) in fields {
        let found = declared(vm, class, name)?;
        let ok = match &found {
            (k, 1) => kind_is(vm, k, *want)?,
            _ => false,
        };
        if !ok {
            return Err(schema(format!("{}.{name} is {found:?}, not the field the card reads", class_name(vm, class))).into());
        }
    }
    Ok(())
}

/// `class.name`: a static array of the struct named `short`; the struct.
fn static_structs(vm: &Vm, class: ClassId, name: &str, short: &str) -> Result<StructId, Walk> {
    match declared(vm, class, name)? {
        (PropKind::Struct(s), dim) if dim >= 1 && struct_named(vm, s, short) => Ok(s),
        other => Err(schema(format!("{}.{name} is {other:?}, not a static array of {short}", class_name(vm, class))).into()),
    }
}

/// The card schema of the body note's section 4, step 1, on the declared classes, before any write. Read-only: a
/// class a checked field declares that is not loaded yet is a schema error naming it ([`setup`] loads it).
pub fn card_schema(vm: &Vm) -> Result<(), VmError> {
    walk(vm).map_err(|w| match w {
        Walk::Bad(e) => e,
        Walk::Reserved(c) => {
            schema(format!("{} is not loaded (equip::card::setup loads it before the preflight)", class_name(vm, c)))
        }
    })
}

/// The schema walk: the checks of [`card_schema`], stopping at the first reserved class.
fn walk(vm: &Vm) -> Result<(), Walk> {
    let weapon = defined_class(vm, WEAPON_CLASS, SCHEMA)?;
    let weapon_type = defined_class(vm, WEAPON_TYPE_CLASS, SCHEMA)?;
    let part = defined_class(vm, WEAPON_PART_CLASS, SCHEMA)?;
    let globals_definition = defined_class(vm, crate::globals::GLOBALS_DEFINITION_CLASS, SCHEMA)?;
    let status = defined_class(vm, &format!("WillowGame.{STATUS_PRESENTATION}"), SCHEMA)?;
    let presentation = defined_class(vm, &format!("WillowGame.{PRESENTATION}"), SCHEMA)?;
    // The balance (DefinitionData.BalanceDefinition, the outline's read) and its DLC set.
    let (data, _) = declared(vm, weapon, "DefinitionData")?;
    let PropKind::Struct(data) = data else {
        return Err(schema(format!("{WEAPON_CLASS}.DefinitionData is not a struct")).into());
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
    let mut balance_root = need(vm, balance)?;
    while vm.space.classes.get_layout(balance_root).is_none_or(|l| l.path().name() != "InventoryBalanceDefinition") {
        match vm.space.classes.get_layout(balance_root).and_then(|l| l.super_class()) {
            Some(s) => balance_root = s,
            None => return Err(schema("BalanceDefinition is not an InventoryBalanceDefinition".into()).into()),
        }
    }
    let dlc = reference(vm, balance_root, "DlcItemSet", "DownloadableItemSetDefinition")?;
    // The presentation lists on the DLC set and on the globals definition.
    for holder in [dlc, globals_definition] {
        let holder = need(vm, holder)?;
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
    class_fields(vm, weapon_type, &[("AttributeSlotMaxActivated", Want::Int)])?;
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
            // The displayed form (NATIVE_CARD_BODY_REST.md section 3; its section 4 puts these shapes here).
            ("bValueRemappingEnabled", Want::Bool),
            ("bDisplayPercentAsFloat", Want::Bool),
            ("bBiggerIsBetter", Want::Bool),
            ("SignStyle", Want::Byte),
            ("RoundingMode", Want::Byte),
            ("FloatPrecision", Want::Int),
            ("RemappingData", Want::Struct(REMAP_STRUCT)),
        ],
    )?;
    if let (PropKind::Struct(remap), _) = declared(vm, presentation, "RemappingData")? {
        let init = Want::Struct("AttributeInitializationData");
        struct_fields(
            vm,
            remap,
            &[
                ("bUseContextOverride", Want::Bool),
                ("InputValueMn", init),
                ("InputValueMx", init),
                ("OutputValueMn", init),
                ("OutputValueMx", init),
            ],
        )?;
    }
    // The weapon's card, its slots, its replicated values and its role.
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
        other => {
            return Err(schema(format!(
                "{WEAPON_CLASS}.ReplicatedWeaponCardModifierValues is {other:?}, not a static array of structs"
            ))
            .into());
        }
    };
    struct_fields(vm, replicated, &[("ModifierValue", Want::Float), ("ModifierStatIndex", Want::Index)])?;
    class_fields(vm, weapon, &[("Role", Want::Role)])
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
    /// The slots whose `bIncludeInModifierText` is cleared, in index order.
    pub clear_slot_flags: Vec<u32>,
    /// The slot entries (S of them), then the appended entries.
    pub stats: Vec<Stat>,
    pub slot_entries: usize,
    /// The weapon's `Role` is `ROLE_Authority`, resolved before any write (review r1 of #521, item 7).
    pub authority: bool,
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
) -> Result<[DetMap<ObjRef, Record>; 2], VmError> {
    let mut maps: [DetMap<ObjRef, Record>; 2] = [DetMap::new(), DetMap::new()];
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
    maps: &[DetMap<ObjRef, Record>; 2],
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
    // 2. A weapon type whose load already failed gives that stored error again, which changes nothing (review r1 of
    // #534); any other unloaded weapon type is refused here, loading nothing (NATIVE_CARD_SLOT_COUNT.md section 4,
    // step 3).
    if vm.defs.is_failed(&vm.space, weapon_type) {
        loaded(vm, weapon_type)?;
    }
    if vm.defs.is_unloaded(&vm.space, weapon_type) {
        return Err(unloaded_type(vm, weapon_type));
    }
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
    // The slot count (section 2 of the slot-count note), then the slots 0 to count - 1.
    let count = slot_count(vm, weapon_type)?;
    let class = vm.space.heap.get(weapon)?.class;
    let p = vm.space.prop(class, "AttributeSlots").ok_or_else(|| bad(NATIVE, "the weapon declares no AttributeSlots"))?;
    let dim = vm.space.classes.decl(p).map(|d| d.array_dim).unwrap_or(0);
    let count = count.min(u32::try_from(dim).unwrap_or(0));
    let mut stats = Vec::new();
    let mut clear_slot_flags = Vec::new();
    for i in 0..count {
        let s = vm.space.read(weapon, PropAddr::element(p, i))?.clone();
        if !flag(field(vm, &s, "bActivated")?, "bActivated")? {
            continue;
        }
        let modify = presentation(vm, &lk, object_of(&field(vm, &s, "AttributeToModify")?, NATIVE)?)?;
        let constraint = presentation(vm, &lk, object_of(&field(vm, &s, "ConstraintAttribute")?, NATIVE)?)?;
        if flag(field(vm, &s, "bIncludeInModifierText")?, "bIncludeInModifierText")? {
            if modify.is_none() {
                clear_slot_flags.push(i);
            }
            let value = float(field(vm, &s, "ComputedModifierValue")?, "ComputedModifierValue")?;
            stats.push(Stat { presentation: modify, constraint, value, display: true });
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
    let type_class = defined_class(vm, WEAPON_TYPE_CLASS, NATIVE)?;
    let part_class = defined_class(vm, WEAPON_PART_CLASS, NATIVE)?;
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
    // 6. The order (NATIVE_CARD_BODY_REST.md section 2): a NaN priority is refused over all entries, before the sort;
    // the whole list is sorted in first-insertion order, and only then filtered (the note's example H).
    if entries.iter().any(|e| e.priority.is_nan()) {
        return Err(rest(UNORDERED));
    }
    sort_entries(&mut entries, |e| e.priority);
    // 7. In append order: the base value B (body note section 3.3, step 1), read for each entry so a refusal of its
    // read passes through typed (review r1 of #521, item 5); then V and `bShouldDisplay` (the rest note's section 3).
    for e in entries.into_iter().filter(|e| e.priority >= THRESHOLD) {
        let b = base_value(vm, weapon, e.presentation)?;
        let (value, display) = match e.record {
            None => (0.0, true),
            Some(r) => {
                let percent = flag(required(vm, e.presentation, "bDisplayAsPercentage", NATIVE)?, "bDisplayAsPercentage")?;
                let v = value_of(r, b, percent);
                let d = display_of(vm, e.presentation, percent)?;
                (v, shows(&d, v)?)
            }
        };
        stats.push(Stat { presentation: Some(e.presentation), constraint: None, value, display });
    }
    let authority = super::slots::is_authority(vm, weapon, NATIVE)?;
    Ok(Some(Card { clear_slot_flags, stats, slot_entries, authority }))
}

/// The weapon type `t` held as an unloaded definition handle: a named `NotYet { NotRead }`, raised without loading it
/// (NATIVE_CARD_SLOT_COUNT.md section 4, step 3).
fn unloaded_type(vm: &Vm, t: ObjRef) -> VmError {
    let path = vm.space.heap.get(t).map(|o| vm.space.names.text(o.name).into_owned()).unwrap_or_else(|_| "?".into());
    NotYet::not_read(format!("WillowWeapon.CacheWeaponCard: the weapon type {path} is an unloaded definition handle"), SLOT_COUNT_RECORD)
        .into()
}

/// The card's slot count on weapon type `t` (NATIVE_CARD_SLOT_COUNT.md section 2): its `AttributeSlotMaxActivated`,
/// clamped as `GetAttributeSlotMaxActivated` answers it. It writes nothing.
fn slot_count(vm: &Vm, t: ObjRef) -> Result<u32, VmError> {
    match required(vm, t, "AttributeSlotMaxActivated", NATIVE)? {
        Value::Int(i) => Ok(u32::try_from(super::natives::clamped_slot_max(i)).unwrap_or(0)),
        other => Err(bad(NATIVE, format!("AttributeSlotMaxActivated holds {}, not an int", other.kind_name()))),
    }
}

/// The base value B of presentation `p` (section 3.3, step 1): 1 with no `Attribute`; else the attribute's base
/// value with the weapon as context; when that context does not resolve, 1 or 2 by the presentation's two flags,
/// else the no-context answer 0.
fn base_value(vm: &mut Vm, weapon: ObjRef, p: ObjRef) -> Result<f32, VmError> {
    let Some(a) = obj(vm, p, "Attribute")? else { return Ok(1.0) };
    let sys = vm.attrs.clone();
    let v = sys.base_value_of(a, Some(weapon), None, &mut ow_attr::api::EvalCx::new(vm))?;
    if let Some(v) = v {
        return Ok(v.value);
    }
    let one = "bUseOneAsBaseValueForPresentationIfContextFailsToResolve";
    let two = "bUseTwoAsBaseValueForPresentationIfContextFailsToResolve";
    Ok(if flag(required(vm, p, one, NATIVE)?, one)? {
        1.0
    } else if flag(required(vm, p, two, NATIVE)?, two)? {
        2.0
    } else {
        0.0
    })
}

// ---- the order and the displayed form (NATIVE_CARD_BODY_REST.md, #532; UNVERIFIED in the running game) ----------

/// "a is lower than b" (the rest note's section 2.1).
fn lower(a: f32, b: f32) -> bool {
    a < b
}

/// The card's sort (the rest note's section 2.1): descending by `p` for ordered values, not stable. `p(a) < p(b)` is
/// "a is lower than b". A range of 8 or fewer: repeatedly the first lowest entry goes to the range's last place, and
/// the range shrinks by one from the right. A range of 9 or more: the entry at offset `count / 2` is the pivot, moved
/// to the front; a left cursor stops at entries strictly lower than the pivot, a right cursor (from the end) at
/// entries not lower, each pair exchanged until the cursors cross; the pivot goes where the right cursor stopped, and
/// the entries before it and those from the left cursor's stop onward are each sorted by the same rules. The caller
/// refuses a NaN first ([`UNORDERED`]), so every comparison here is ordered.
pub fn sort_entries<T>(v: &mut [T], p: impl Fn(&T) -> f32 + Copy) {
    let n = v.len();
    if n <= 1 {
        return;
    }
    if n <= 8 {
        for last in (1..n).rev() {
            let mut low = 0;
            for i in 1..=last {
                if lower(p(&v[i]), p(&v[low])) {
                    low = i;
                }
            }
            v.swap(low, last);
        }
        return;
    }
    v.swap(0, n / 2);
    let pivot = p(&v[0]);
    let (mut left, mut right) = (1, n - 1);
    loop {
        while left < n && !lower(p(&v[left]), pivot) {
            left += 1;
        }
        while right > 0 && lower(p(&v[right]), pivot) {
            right -= 1;
        }
        if left >= right {
            break;
        }
        v.swap(left, right);
    }
    v.swap(0, right);
    let (before, after) = v.split_at_mut(left);
    sort_entries(&mut before[..right], p);
    sort_entries(after, p);
}

/// The value V of an entry with record `r` and base value `b` (body note section 3.3, step 2), in single precision:
/// ((scale + 1) / (1 − negative scale)) × (pre-add + B) + post-add − B, then divided by B for a percentage when
/// |B| is at least 0.00000001.
pub fn value_of(r: Record, b: f32, percent: bool) -> f32 {
    let v = ((r.scale + 1.0) / (1.0 - r.negative_scale)) * (r.pre_add + b) + r.post_add - b;
    if percent && b.abs() >= SIZE { v / b } else { v }
}

/// The smallest size that shows, and the zero snap's bound (the rest note's section 3).
const SIZE: f32 = 0.000_000_01;

/// What the displayed form reads from a presentation (the rest note's section 3). `remap` is the four evaluated
/// remap values `a`, `b`, `c`, `d` when the remap applies (enabled, not a percentage), else `None`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Display {
    pub percent: bool,
    pub remap: Option<[f32; 4]>,
    pub sign_style: u8,
    pub bigger_is_better: bool,
    pub rounding: u8,
    pub precision: i32,
    pub percent_as_float: bool,
}

fn byte_of(v: Value, name: &str) -> Result<u8, VmError> {
    match v {
        Value::Byte(b) => Ok(b),
        other => Err(bad(NATIVE, format!("{name} holds {}, not a byte", other.kind_name()))),
    }
}

fn int_of(v: Value, name: &str) -> Result<i32, VmError> {
    match v {
        Value::Int(i) => Ok(i),
        other => Err(bad(NATIVE, format!("{name} holds {}, not an int", other.kind_name()))),
    }
}

/// Presentation `p`'s display fields, the remap evaluated when it applies (the rest note's section 3, "The remap").
/// The card passes no context object, so a remap without `bUseContextOverride` needs the engine's default source,
/// which is not read: [`REMAP_CONTEXT`]. With the override, the source is the call's context, None, and the four
/// fields are evaluated in the order `InputValueMn`, `InputValueMx`, `OutputValueMn`, `OutputValueMx`, each refusal of
/// the attribute layer passed through.
fn display_of(vm: &mut Vm, p: ObjRef, percent: bool) -> Result<Display, VmError> {
    let get = |vm: &Vm, name: &str| required(vm, p, name, NATIVE);
    let enabled = flag(get(vm, "bValueRemappingEnabled")?, "bValueRemappingEnabled")?;
    let remap = if enabled && !percent {
        let data = get(vm, "RemappingData")?;
        if !flag(field(vm, &data, "bUseContextOverride")?, "bUseContextOverride")? {
            return Err(rest(REMAP_CONTEXT));
        }
        let mut out = [0.0; 4];
        for (slot, name) in out.iter_mut().zip(["InputValueMn", "InputValueMx", "OutputValueMn", "OutputValueMx"]) {
            let init = ow_attr::eval::init_data(&vm.space, &field(vm, &data, name)?)?;
            let sys = vm.attrs.clone();
            *slot = sys.evaluate_init(&init, None, None, &mut ow_attr::api::EvalCx::new(vm))?;
        }
        Some(out)
    } else {
        None
    };
    Ok(Display {
        percent,
        remap,
        sign_style: byte_of(get(vm, "SignStyle")?, "SignStyle")?,
        bigger_is_better: flag(get(vm, "bBiggerIsBetter")?, "bBiggerIsBetter")?,
        rounding: byte_of(get(vm, "RoundingMode")?, "RoundingMode")?,
        precision: int_of(get(vm, "FloatPrecision")?, "FloatPrecision")?,
        percent_as_float: flag(get(vm, "bDisplayPercentAsFloat")?, "bDisplayPercentAsFloat")?,
    })
}

/// The remap of `v` by `a`, `b`, `c`, `d` (the rest note's section 3): computed in double, rounded to single; the
/// comparisons are between single values, so with a NaN among the four the last form applies.
pub fn remap(v: f32, [a, b, c, d]: [f32; 4]) -> f32 {
    let (a64, b64, c64, d64) = (f64::from(a), f64::from(b), f64::from(c), f64::from(d));
    let slope = if a < b { (d64 - c64) / (b64 - a64) } else { 0.0 };
    if v < a {
        c
    } else if v < b {
        ((f64::from(v) - a64) * slope + c64) as f32
    } else {
        ((b64 - a64) * slope + c64) as f32
    }
}

/// 10 to the power 0 to 10, each exact in single precision.
const POW10: [f64; 11] = [1.0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10];

/// The integer n of rounding modes 0 to 3 from `w` at precision `p` (the rest note's section 3, step 5): the doubled
/// sum converted to the nearest integer (ties to even), halved toward minus infinity, and negated for mode 3. With
/// `single`, every step is rounded to single precision (the product of mode 0, then the doubled sum). `None` when the
/// converted value is NaN, infinite or outside the signed 32-bit range. The double-precision sum is not exact for a
/// small X (in mode 0 below about 1/32, in modes 1 to 3 below about 1e-9): it rounds, and that rounding does not move n
/// except for |X| below about 1e-16 (the rest note's precision notes).
fn rounded(mode: u8, w: f32, p: usize, single: bool) -> Option<i64> {
    let sum = if single {
        let x = if mode == 0 { POW10[p] as f32 * w } else { w };
        f64::from(match mode {
            0 | 1 => 2.0 * x + 0.5,
            2 => 2.0 * x - 0.5,
            _ => -0.5 - 2.0 * x,
        })
    } else {
        let x = if mode == 0 { POW10[p] * f64::from(w) } else { f64::from(w) };
        match mode {
            0 | 1 => 2.0 * x + 0.5,
            2 => 2.0 * x - 0.5,
            _ => -0.5 - 2.0 * x,
        }
    };
    let k = sum.round_ties_even();
    if k.is_nan() || k < f64::from(i32::MIN) || k > f64::from(i32::MAX) {
        return None;
    }
    let half = (k as i64).div_euclid(2);
    Some(if mode == 3 { -half } else { half })
}

/// Whether value `v` shows under presentation `d` in the card's mode (the rest note's section 3, steps 1 to 6, and the
/// typed refusals of its section 4, item 3). Reads only.
pub fn shows(d: &Display, v: f32) -> Result<bool, VmError> {
    // 1. The zero snap (a NaN fails the test and snaps too).
    let zero = v.is_nan() || v.abs() < SIZE;
    let mut w = if zero { 0.0 } else { v };
    // 2. Scale or remap.
    if d.percent {
        if !zero {
            w = (f64::from(w) * 100.0) as f32;
        }
    } else if let Some(r) = d.remap {
        w = remap(w, r);
    }
    // 3. The sign.
    let negate = match d.sign_style {
        3 => false,
        4 => true,
        1 if w < 0.0 => true,
        2 if w > 0.0 => true,
        _ => !d.bigger_is_better,
    };
    if negate {
        w = -w;
    }
    // 5. The rounding (4, the inverse, is not in the card's mode; 6, no minimum of 1, leaves n as it is).
    let mode = if d.percent && d.rounding == 0 && !d.percent_as_float { 1 } else { d.rounding };
    let p = usize::try_from(d.precision.clamp(0, 10)).unwrap_or(0);
    if mode > 3 {
        return Ok(w.abs() >= SIZE);
    }
    let n = rounded(mode, w, p, false).ok_or_else(|| rest(RANGE))?;
    let single = rounded(mode, w, p, true);
    judge(mode, p, n, single)
}

/// Where n puts a value in the card's mode (the rest note's section 3, the cutoff).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Hidden,
    /// Mode 0 at precision 8 or more, |n| strictly between half and twice 10^(P−8): the division's precision decides.
    Band,
    Shown,
}

fn outcome(mode: u8, p: usize, n: i64) -> Outcome {
    if mode != 0 || p <= 7 {
        return if n != 0 { Outcome::Shown } else { Outcome::Hidden };
    }
    // Mode 0 at precision 8 or more: the size |n| / 10^P against 0.00000001, boundary t = 10^(P−8).
    let t = POW10[p - 8] as i64;
    let m = n.abs();
    if m >= 2 * t {
        Outcome::Shown
    } else if 2 * m <= t {
        Outcome::Hidden
    } else {
        Outcome::Band
    }
}

/// The display decision from `n`, the double-precision rule's integer, and `single`, the same with every step rounded to
/// single (`None` when that conversion leaves the integer range). The precision refusal ([`PRECISION`]) comes when the
/// two disagree on the outcome: in modes 1 to 3, and in mode 0 below precision 8, that is whether n is 0; in mode 0 at
/// precision 8 or more it is hidden, in the band or shown (the hub's ruling on #532: single rounding can move n by one
/// into or out of the band without crossing 0). Then a value in the band is the cutoff refusal ([`CUTOFF`]).
fn judge(mode: u8, p: usize, n: i64, single: Option<i64>) -> Result<bool, VmError> {
    let found = outcome(mode, p, n);
    if single.is_none_or(|s| outcome(mode, p, s) != found) {
        return Err(rest(PRECISION));
    }
    match found {
        Outcome::Shown => Ok(true),
        Outcome::Hidden => Ok(false),
        Outcome::Band => Err(rest(CUTOFF)),
    }
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
    // 2. The planned slot flags.
    for &i in &card.clear_slot_flags {
        let slots = prop(vm, "AttributeSlots")?;
        let mut s = vm.space.read(weapon, PropAddr::element(slots, i))?.clone();
        set(vm, &mut s, "bIncludeInModifierText", Value::Bool(false))?;
        vm.space.write(weapon, PropAddr::element(slots, i), s)?;
    }
    // 3. The entries.
    vm.space.write(weapon, PropAddr::scalar(stats_prop), Value::array(values))?;
    // 4. Replication on the authority.
    if !card.authority {
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

/// The write tracker (the hub's design, review r2 of #521; #518's counters): the heap's committed writes, the random
/// streams' draws and the store's load transitions, three counts that only grow. Each is a write in Off.
pub fn load_mark(vm: &Vm) -> (u64, u64, u64) {
    (vm.space.heap.committed_writes(), vm.rng.draws(), vm.defs.load_transitions())
}

/// [`load_mark`] with the VM's code loads added (lane COUNTERS-MISS-CLASS-CODE-LOADS), for the tests' write-free
/// checks; [`marked_after_loads`] still reads only [`load_mark`]'s three.
pub fn load_and_code_mark(vm: &Vm) -> (u64, u64, u64, u64) {
    let (w, d, l) = load_mark(vm);
    (w, d, l, vm.class_code_loads())
}

/// `f`'s result, a failure marked "stopped after partial writes" when any of the three counts of [`load_mark`] moved
/// before it (review r2 of #521, item 1): a heap write, a random draw (an effect's variance, for one) or a definition
/// load, the ones attribute evaluation makes included.
pub(crate) fn marked_after_loads<T>(
    vm: &mut Vm,
    f: impl FnOnce(&mut Vm) -> Result<T, VmError>,
) -> Result<T, VmError> {
    let before = load_mark(vm);
    f(vm).map_err(|e| if load_mark(vm) == before { e } else { ow_world::pools::after_partial_writes(e) })
}

/// `CacheWeaponCard` on `weapon` in Off (module documentation): compute, then write. A `NotYet` with nothing written is
/// unprefixed; one after a load of the computation or after the first card write is marked.
pub fn cache_weapon_card(vm: &mut Vm, weapon: ObjRef) -> Result<(), VmError> {
    match marked_after_loads(vm, |vm| compute(vm, weapon))? {
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
    // First wins: a key bound before is kept (review r1 of #521, item 6).
    if vm.has_native(&key) {
        return Ok(());
    }
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
