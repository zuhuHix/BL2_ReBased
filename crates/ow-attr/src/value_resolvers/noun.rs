//! `GearboxFramework.NounAttributeValueResolver`, the "AI parameter" resolver. The class declares only
//! `DefaultValue` (initialization data) and `ReplicationStrategy`. **Every rule here is UNVERIFIED in the running
//! game.**
//!
//! **Parity modes** (architecture R3):
//!
//! - `ParityMode::Reference`, the C++ reading (`src/progression.cpp` `nounValue`, [`reference_read`]): the
//!   `BaseValue` of the context's `AIClass.AttributeStartingValues` entry for this attribute, else `DefaultValue`,
//!   both evaluated with the context as the source; nothing is written. Get and get-base are the same read.
//! - `ParityMode::Off`, the native rule of NATIVE_NOUN_ATTRIBUTES.md, implemented in the clean room from that note
//!   alone (lane FIX-NOUN-OFF, decision 2026-11-16-43; docs/LANE_BRIEF.md, "Clean room (native code)"): the noun
//!   store on the context ("Off mode" below).
//!
//! **The attribute.** The C++ reading is handed the attribute it serves; [`ValueResolver`] is not. Every installed
//! Noun resolver (15) is a subobject of its one attribute definition and listed in that attribute's
//! `ValueResolverChain` (the packages), so the resolver's outer is taken as the attribute. The assumption is checked:
//! an outer that is not an attribute definition, or whose `ValueResolverChain` does not list this resolver, is
//! `NotYet { NotRead }`.
//!
//! **Stricter than the C++.** Where `nounValue` falls back or skips, this reading answers `BadData`: an `AIClass`
//! that is not an object reference (the C++ reads no starting values and falls back to `DefaultValue`), and a
//! starting-value entry with no `Attribute` field (the C++ skips that entry). Neither shape is in the installed data.
//!
//! **Writes** (add, remove, set-base) in Reference mode answer `NotYet { NotPorted }`: the C++ has no noun write.
//!
//! # Off mode (note sections 3 to 5)
//!
//! - **Key:** the name of the resolver's outer object (`NameId` equality, the number included). No outer: no store
//!   (reads 0.0, writes false). Unlike the Reference reading, the outer need not be an attribute.
//! - **Provider:** a context whose class declares `NounState` (checklist item 2; `ow-object` models no interfaces).
//!   Anything else: no store.
//! - **Strategy 0** (every installed mind and weapon noun): the entry is the first `NounState` element whose
//!   `NounName` is the key. A missing one is created by reads and writes alike: `DefaultValue` is evaluated on the
//!   context first (`d`), then an element (key, `Value` = `BaseValue` = `d`, empty stack, the strategy, slot -1)
//!   goes on the end of the array as it stands after that evaluation. Reads give the entry's `Value` / `BaseValue`;
//!   the chain's previous value is ignored. The writes are [`add`], [`remove`] and [`set_base`] on
//!   [`Storage::Noun`], with the recompute of section 5 and no change notification.
//! - **The providers' arrays** (note section 10, lane IMPL-NOUN-REPLICATION, decision 2026-11-19-NNN, coded from
//!   section 10 alone; analyst lane ANALYST-NOUN-REPLICATION, not this thread). The classes read, each by its exact
//!   class path: `WillowPawn`, `WillowAIPawn`, `WillowPlayerPawn` and `WillowPlayerReplicationInfo` hand over
//!   `NounState` on authority (`Role` 3) and `ReplicatedNouns` always (20 slots, `NumReplicatedNouns`, the change call
//!   sets `bNetDirty`); `GearboxMind` hands over `NounState` on authority and no replicated array; `WillowWeapon`
//!   hands over `NounState` always and no replicated array. Their fields and count are preflighted before `d` is
//!   evaluated (section 10.5, steps 4 to 6), each failure an `AttrError::BadData` with the note's text.
//! - **An unread class** (any other class declaring `NounState`, `WillowMind` among them) keeps the strategy-0 path
//!   above, with no `Role` test. Strategy 1 or 2 on it, or a write to one of its entries with a replicated slot, is
//!   `NotYet { NotRead }` (section 10.5, step 3).
//! - **Strategy 1 or 2 on a read class:** a pawn or a replication info on authority creates a missing entry with a
//!   replicated slot (section 10.3: `d`, the count read again, the slot and `bNetDirty`, then the `NounState`
//!   append; no slot when the 20 are taken); off authority it reads `ReplicatedNouns` alone and writes nothing. A
//!   mind or a weapon has no store for them (0.0, false).
//! - **Mirror:** a write to an entry with a slot on a pawn or a replication info writes the entry, then the slot's
//!   `Value`, then `bNetDirty`. Each write is planned and checked first ([`NounWrite`]), then signalled with
//!   `EvalCx::note_writes` and made ([`commit`]); a creation during a read signals too.
//! - **Review r1 of #523.** The probe (`probe_storage`, with #518's `ProbeOp`) makes non-loading reads only: a cold
//!   resolver or context passes, and the default is validated without being evaluated (#518's `preflight_init`).
//!   Execution counts the resolver and context loads it makes as writes. A creation or a mirror builds and checks every
//!   replacement before its first write, then writes in the note's order. Get-base on the replicated-only row answers
//!   0.0 without reading a slot. The Off add, remove and set-base run in a write scope (`effects.rs`), so a `NotYet`
//!   after a committed write is prefixed.
//! - **Net mode:** the attribute worlds are standalone (the slice is single player), so the client row of the table
//!   is not implemented (checklist item 6); every row ported here is the same in every net mode.
//! - **A null modifier** cannot reach the resolver: `AttributeSystem::add_modifier` and `remove_modifier` take an
//!   object reference, so the note's "a null modifier returns false" is the caller's to answer.

use ow_core::{NotYet, ParityMode};
use ow_object::heap::ObjectSpace;
use ow_object::ids::{NameId, ObjRef, PropAddr, PropId, StructId};
use ow_object::layout::{LayoutError, PropKind};
use ow_object::value::Value;

use crate::api::{
    ProbeOp,
    AddModifierOutcome, AttrError, EvalCx, RemoveModifierOutcome, Storage, ValueResolver,
};
use crate::eval::{
    byte, class_path, elements, ensure_loaded, evaluate_value, field, name as name_of, number,
    object, property, required,
};
use crate::registry::AttributeSystem;
use crate::stack::{CombineForm, NumKind};
use crate::storage::stack_value;

/// Where the native rule is written down.
pub(crate) const NOTE: &str = "NATIVE_NOUN_ATTRIBUTES.md";
/// The C++ reading Reference mode follows.
const CPP: &str = "src/progression.cpp nounValue";
/// The property that makes a context a noun provider and holds its entries.
const NOUN_STATE: &str = "NounState";

pub(crate) struct Noun;

/// The world's parity mode ([`crate::eval::parity`]).
fn mode(cx: &mut EvalCx<'_>) -> ParityMode {
    crate::eval::parity(cx)
}

/// Reference mode's writes: the C++ reading has none.
fn reference_write() -> AttrError {
    NotYet::not_ported(
        "NounAttributeValueResolver add, remove or set-base in the reference reading: the C++ has no noun write",
        CPP,
    )
    .into()
}

fn bad(reason: String) -> AttrError {
    AttrError::BadData {
        reason: reason.into(),
    }
}

/// The attribute this resolver serves, for the C++ reading, which is handed it: the resolver's outer, which must be
/// an attribute definition whose `ValueResolverChain` lists this resolver (module documentation).
fn attribute(cx: &mut EvalCx<'_>, resolver: ObjRef) -> Result<ObjRef, AttrError> {
    ensure_loaded(cx, resolver)?;
    let owner = cx.world.parts().space.heap.get(resolver)?.outer;
    if let Some(a) = owner
        && let Some(chain) = property(cx, a, "ValueResolverChain")?
    {
        for entry in elements("ValueResolverChain", &chain)? {
            if object("ValueResolverChain entry", entry)? == Some(resolver) {
                return Ok(a);
            }
        }
    }
    let space = cx.world.parts().space;
    Err(NotYet::not_read(
        format!(
            "NounAttributeValueResolver (reference reading) whose outer {} is not an attribute definition listing \
             it in its ValueResolverChain: the resolver is not told which attribute it serves",
            owner.map_or_else(|| "None".to_string(), |o| class_path(space, o))
        ),
        CPP,
    )
    .into())
}

/// Reference mode, the C++ reading (module documentation). The store gives one object per path, so object identity
/// is the C++ path match. A context without an `AIClass` property, or with None there, reads `DefaultValue`, as the
/// C++ does for any resolved object.
fn reference_read(
    sys: &AttributeSystem,
    resolver: ObjRef,
    context: ObjRef,
    cx: &mut EvalCx<'_>,
) -> Result<f32, AttrError> {
    let attr = attribute(cx, resolver)?;
    let class = match property(cx, context, "AIClass")? {
        Some(v) => object("AIClass", &v)?,
        None => None,
    };
    if let Some(class) = class
        && let Some(starting) = property(cx, class, "AttributeStartingValues")?
    {
        let chosen = {
            let space = cx.world.parts().space;
            let mut chosen = None;
            for entry in elements("AttributeStartingValues", &starting)? {
                if object("Attribute", field(space, entry, "Attribute")?)? == Some(attr) {
                    chosen = Some(field(space, entry, "BaseValue")?.clone());
                    break;
                }
            }
            chosen
        };
        if let Some(data) = chosen {
            return evaluate_value(sys, &data, Some(context), None, cx);
        }
    }
    let data = required(cx, resolver, "DefaultValue")?;
    evaluate_value(sys, &data, Some(context), None, cx)
}

// ---------------------------------------------------------------------------------------------------------------
// Off mode: the noun store (module documentation, "Off mode")
// ---------------------------------------------------------------------------------------------------------------

/// The noun-provider classes the note reads (section 10.2), each by its exact class path, and what each hands over.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Provider {
    /// `WillowPawn`, `WillowAIPawn`, `WillowPlayerPawn`: `NounState` on authority, `ReplicatedNouns` always.
    Pawn,
    /// `WillowPlayerReplicationInfo`: as a pawn.
    ReplicationInfo,
    /// `GearboxMind`: `NounState` on authority, no replicated array.
    Mind,
    /// `WillowWeapon`: `NounState` always, no replicated array.
    Weapon,
    /// Any other class that declares `NounState` (`WillowMind` among them): today's strategy-0 path, and strategies 1
    /// and 2 refused (section 10.5).
    Unread,
}

impl Provider {
    fn replicated(self) -> bool {
        matches!(self, Provider::Pawn | Provider::ReplicationInfo)
    }

    /// Whether `NounState` is handed over only when `Role` is 3.
    fn needs_authority(self) -> bool {
        matches!(self, Provider::Pawn | Provider::ReplicationInfo | Provider::Mind)
    }
}

/// The classes read (section 10.2, "each by its exact class").
const READ_CLASSES: [(&str, Provider); 6] = [
    ("WillowGame.WillowPawn", Provider::Pawn),
    ("WillowGame.WillowAIPawn", Provider::Pawn),
    ("WillowGame.WillowPlayerPawn", Provider::Pawn),
    ("WillowGame.WillowPlayerReplicationInfo", Provider::ReplicationInfo),
    ("GearboxFramework.GearboxMind", Provider::Mind),
    ("WillowGame.WillowWeapon", Provider::Weapon),
];

/// `ROLE_Authority`.
const AUTHORITY: u8 = 3;
/// The declared size of `ReplicatedNouns`.
const CAPACITY: i32 = 20;

fn provider_of(space: &ObjectSpace, context: ObjRef) -> Provider {
    let path = class_path(space, context);
    READ_CLASSES
        .iter()
        .find(|(p, _)| p.eq_ignore_ascii_case(&path))
        .map_or(Provider::Unread, |&(_, k)| k)
}

/// A diagnostic of section 10.5 ("The diagnostics").
fn diag(reason: String) -> AttrError {
    bad(format!("NounAttributeValueResolver: {reason}"))
}

/// The object path of `o` (its outer chain's names), for the diagnostics.
fn object_path(space: &ObjectSpace, o: ObjRef) -> String {
    let mut parts = Vec::new();
    let mut cur = Some(o);
    while let Some(x) = cur {
        match space.heap.get(x) {
            Ok(obj) => {
                parts.push(space.names.get(obj.name).map(|t| t.to_string()).unwrap_or_default());
                cur = obj.outer;
            }
            Err(_) => {
                parts.push(format!("{x:?}"));
                break;
            }
        }
    }
    parts.reverse();
    parts.join(".")
}

/// Preflight step 3's refusal: an unread class with strategy 1 or 2, or a write to a slotted entry on one.
fn unread_refusal(space: &ObjectSpace, context: ObjRef, strategy: u8) -> AttrError {
    NotYet::not_read(
        format!(
            "NounAttributeValueResolver: the noun arrays a {} hands over, and its replicated-copy-changed call, are \
             not read (ReplicationStrategy {strategy})",
            class_path(space, context)
        ),
        NOTE,
    )
    .into()
}

/// The last segment of a struct's path (`NounAttributeState`, `ReplicatedNounAttributeState`).
fn struct_name(space: &ObjectSpace, s: StructId) -> String {
    space
        .classes
        .struct_layout(s)
        .map(|l| l.path().as_str().rsplit(['.', ':']).next().unwrap_or("").to_string())
        .unwrap_or_default()
}

/// The replicated half of a pawn's or a replication info's shape: `ReplicatedNouns`, `NumReplicatedNouns`,
/// `bNetDirty`, and the count as read.
#[derive(Debug, Clone, Copy)]
struct Replicated {
    nouns: PropId,
    num: PropId,
    dirty: PropId,
    count: i32,
}

/// What preflight steps 4 and 5 read on a read class.
#[derive(Debug, Clone, Copy)]
struct Shape {
    authority: bool,
    replicated: Option<Replicated>,
}

/// Field `name` of `class`, or the missing-declaration diagnostic.
fn declared(space: &ObjectSpace, context: ObjRef, name: &str) -> Result<PropId, AttrError> {
    let class = space.heap.get(context)?.class;
    space
        .prop(class, name)
        .ok_or_else(|| diag(format!("{} has no {name}", class_path(space, context))))
}

/// Preflight steps 4 and 5 (section 10.5) on a read class: the declarations, the `Role` byte and the count. Reads
/// only.
fn shape(space: &ObjectSpace, context: ObjRef, provider: Provider, state: PropId) -> Result<Shape, AttrError> {
    let path = class_path(space, context);
    let mistyped = |field: &str, what: &str| diag(format!("{path}.{field} is not {what}"));
    let role = declared(space, context, "Role")?;
    let decl = space.classes.decl(role).ok_or(LayoutError::NotAProperty(role))?;
    if !matches!(decl.kind, PropKind::Byte { .. }) || decl.array_dim > 1 {
        return Err(mistyped("Role", "a byte"));
    }
    let decl = space.classes.decl(state).ok_or(LayoutError::NotAProperty(state))?;
    let state_ok = match &decl.kind {
        PropKind::Array(inner) => {
            matches!(inner.as_ref(), PropKind::Struct(s) if struct_name(space, *s) == "NounAttributeState")
        }
        _ => false,
    };
    if !state_ok {
        return Err(mistyped(NOUN_STATE, "an array of NounAttributeState"));
    }
    let replicated = if provider.replicated() {
        let nouns = declared(space, context, "ReplicatedNouns")?;
        let decl = space.classes.decl(nouns).ok_or(LayoutError::NotAProperty(nouns))?;
        let ok = matches!(&decl.kind, PropKind::Struct(s) if struct_name(space, *s) == "ReplicatedNounAttributeState")
            && decl.array_dim == CAPACITY;
        if !ok {
            return Err(mistyped("ReplicatedNouns", "a fixed array of 20 ReplicatedNounAttributeState"));
        }
        let num = declared(space, context, "NumReplicatedNouns")?;
        let decl = space.classes.decl(num).ok_or(LayoutError::NotAProperty(num))?;
        if !matches!(decl.kind, PropKind::Int) || decl.array_dim > 1 {
            return Err(mistyped("NumReplicatedNouns", "an int"));
        }
        let dirty = declared(space, context, "bNetDirty")?;
        let decl = space.classes.decl(dirty).ok_or(LayoutError::NotAProperty(dirty))?;
        if !matches!(decl.kind, PropKind::Bool) || decl.array_dim > 1 {
            return Err(mistyped("bNetDirty", "a bool"));
        }
        Some(Replicated { nouns, num, dirty, count: count_of(space, context, num)? })
    } else {
        None
    };
    let authority = match space.read(context, PropAddr::scalar(role))? {
        Value::Byte(r) => *r == AUTHORITY,
        other => return Err(bad(format!("Role holds a {}, not a byte", other.kind_name()))),
    };
    Ok(Shape { authority, replicated })
}

/// `NumReplicatedNouns`, checked to lie in 0..=20 (preflight step 5).
fn count_of(space: &ObjectSpace, context: ObjRef, num: PropId) -> Result<i32, AttrError> {
    let n = match space.read(context, PropAddr::scalar(num))? {
        Value::Int(n) => *n,
        other => {
            return Err(bad(format!("NumReplicatedNouns holds a {}, not an int", other.kind_name())));
        }
    };
    if !(0..=CAPACITY).contains(&n) {
        return Err(diag(format!(
            "{} holds NumReplicatedNouns {n}, outside 0..=20",
            object_path(space, context)
        )));
    }
    Ok(n)
}

/// The element struct of the provider property `state`, which must be an array of structs.
fn entry_struct(space: &ObjectSpace, state: PropId) -> Result<StructId, AttrError> {
    let decl = space
        .classes
        .decl(state)
        .ok_or(LayoutError::NotAProperty(state))?;
    match &decl.kind {
        PropKind::Array(inner) => match inner.as_ref() {
            PropKind::Struct(s) => Ok(*s),
            other => Err(bad(format!(
                "{NOUN_STATE} is an array of {}, not of NounAttributeState",
                other.kind_name()
            ))),
        },
        other => Err(bad(format!(
            "{NOUN_STATE} is a {}, not an array",
            other.kind_name()
        ))),
    }
}

/// The provider's `NounState` elements.
fn entries(space: &ObjectSpace, owner: ObjRef, state: PropId) -> Result<Vec<Value>, AttrError> {
    let v = space.read(owner, PropAddr::scalar(state))?;
    Ok(elements(NOUN_STATE, v)?.to_vec())
}

/// Element `index` of `items`.
fn element(items: &[Value], index: usize) -> Result<&Value, AttrError> {
    items
        .get(index)
        .ok_or_else(|| bad(format!("{NOUN_STATE} has no element {index}")))
}

/// Sets field `name` of the struct value `v`.
fn set_field(space: &ObjectSpace, v: &mut Value, name: &str, x: Value) -> Result<(), AttrError> {
    let s = v
        .struct_mut()
        .ok_or_else(|| bad(format!("a noun element is not a struct (setting {name})")))?;
    let layout = space
        .classes
        .struct_layout(s.ty)
        .ok_or_else(|| bad(format!("struct {:?} has no layout", s.ty)))?;
    let (entry, decl) = space
        .names
        .find(name)
        .and_then(|n| layout.field(n))
        .ok_or_else(|| {
            bad(format!(
                "struct {} has no field {name}",
                layout.path().as_str()
            ))
        })?;
    // The replacement must fit the field (review r1 of #523, item 2: every destination checked before a write).
    let fits = match (&decl.kind, &x) {
        (PropKind::Name, Value::Name(_))
        | (PropKind::Float, Value::Float(_))
        | (PropKind::Bool, Value::Bool(_))
        | (PropKind::Int, Value::Int(_))
        | (PropKind::Byte { .. }, Value::Byte(_))
        | (PropKind::Array(_), Value::Array(_)) => true,
        (
            PropKind::Name | PropKind::Float | PropKind::Bool | PropKind::Int | PropKind::Byte { .. } | PropKind::Array(_),
            _,
        ) => false,
        _ => true,
    };
    if !fits || decl.array_dim > 1 {
        return Err(bad(format!(
            "struct {} field {name} is a {}, which does not take a {}",
            layout.path().as_str(),
            decl.kind.kind_name(),
            x.kind_name()
        )));
    }
    let slot = usize::try_from(entry)
        .ok()
        .and_then(|i| s.fields.get_mut(i))
        .ok_or_else(|| bad(format!("a noun element lacks field {name}")))?;
    *slot = x;
    Ok(())
}

/// The index of the first element whose `NounName` is `key` (note section 4, "entry").
fn find(space: &ObjectSpace, items: &[Value], key: NameId) -> Result<Option<usize>, AttrError> {
    for (i, e) in items.iter().enumerate() {
        if name_of("NounName", field(space, e, "NounName")?)? == key {
            return Ok(Some(i));
        }
    }
    Ok(None)
}

/// An entry's `IndexInReplicationArray`.
fn slot_of(space: &ObjectSpace, e: &Value) -> Result<i32, AttrError> {
    match field(space, e, "IndexInReplicationArray")? {
        Value::Int(i) => Ok(*i),
        other => Err(bad(format!(
            "IndexInReplicationArray holds a {}, not an int",
            other.kind_name()
        ))),
    }
}

/// Preflight step 6 on a read class: an entry's slot is -1 or one of the occupied slots (none for a mind or a
/// weapon).
fn check_slot(
    space: &ObjectSpace,
    context: ObjRef,
    e: &Value,
    replicated: Option<Replicated>,
) -> Result<i32, AttrError> {
    let slot = slot_of(space, e)?;
    let n = replicated.map_or(0, |r| r.count);
    if slot != -1 && !(0..n).contains(&slot) {
        let key = name_of("NounName", field(space, e, "NounName")?)?;
        let key = space.names.get(key).map(|t| t.to_string()).unwrap_or_default();
        return Err(diag(format!(
            "the entry {key} on {} holds IndexInReplicationArray {slot}, outside -1 and the occupied slots 0..{n}",
            object_path(space, context)
        )));
    }
    Ok(slot)
}

/// The heap's own checks of a planned write, made before any write (review r2 of #523, item 1): the address against
/// the object's class, and the whole value against the declaration it lands in, as `ObjectSpace::write` makes them.
/// A failure is malformed noun data: `AttrError::BadData`.
fn precheck(space: &ObjectSpace, o: ObjRef, a: PropAddr, v: &Value, what: &str) -> Result<(), AttrError> {
    let bad_write = |e: String| bad(format!("NounAttributeValueResolver: the write to {what} would be refused: {e}"));
    let decl = space.classes.decl(a.prop).ok_or(LayoutError::NotAProperty(a.prop))?;
    space.read(o, a).map_err(|e| bad_write(e.to_string()))?;
    space.heap.check_value(&space.classes, &decl.kind, v).map_err(|e| bad_write(e.to_string()))
}

/// What a lookup found (section 10.5's storage-selection table).
enum Located {
    /// No store: reads 0.0, writes false.
    Nothing,
    /// A `NounState` entry (found or created).
    Entry(Storage),
    /// `ReplicatedNouns` alone: the slot's `Value` (0.0 when none matches); get-base 0.0, writes false.
    Replicated(f32),
}

/// What the lookup is for: a get, a get-base, a write (`storage`), or the read-only probe of a write's operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    Read,
    /// Get-base: the replicated-only row answers 0.0 without reading a slot (review r1 of #523, item 5).
    ReadBase,
    Write,
    Probe(ProbeOp),
}

/// The value of the first of the first `count` replicated slots whose `NounName` is `key`, or 0.0 (section 10.4).
fn replicated_read(space: &ObjectSpace, context: ObjRef, r: Replicated, key: NameId) -> Result<f32, AttrError> {
    for i in 0..r.count {
        let addr = PropAddr::element(r.nouns, u32::try_from(i).unwrap_or(0));
        let slot = space.read(context, addr)?;
        if name_of("NounName", field(space, slot, "NounName")?)? == key {
            return number("Value", field(space, slot, "Value")?);
        }
    }
    Ok(0.0)
}

/// The lookup of `resolver` on `context` (note sections 3, 4 and 10.5), creating a missing entry for reads and
/// writes where the table says so; the probe creates nothing.
///
/// **Off assumption, UNVERIFIED (review r2 of #483):** that `DefaultValue`'s evaluation writes nothing during the
/// probe. Data that names a `BaseValueAttribute` or an `InitializationDefinition` could lead to another noun attribute
/// on the same context, whose read goes through this lookup with creation on. No data known to the port does this.
fn locate(
    sys: &AttributeSystem,
    resolver: ObjRef,
    context: ObjRef,
    cx: &mut EvalCx<'_>,
    op: Op,
) -> Result<Located, AttrError> {
    // Loads are allowed in the probe and in execution; in Off each is tracked as a write (review r2 of #523, the
    // hub's ruling; `eval::ensure_loaded`).
    ensure_loaded(cx, resolver)?;
    ensure_loaded(cx, context)?;
    // Preflight step 1: the key and the provider.
    let (key, state) = {
        let space = cx.world.parts().space;
        let Some(outer) = space.heap.get(resolver)?.outer else {
            return Ok(Located::Nothing);
        };
        let key = space.heap.get(outer)?.name;
        let class = space.heap.get(context)?.class;
        let Some(state) = space.prop(class, NOUN_STATE) else {
            return Ok(Located::Nothing);
        };
        (key, state)
    };
    // Step 2: the strategy.
    let strategy = byte(
        "ReplicationStrategy",
        &required(cx, resolver, "ReplicationStrategy")?,
    )?;
    if strategy > 2 {
        return Err(NotYet::not_read(
            format!("NounAttributeValueResolver with ReplicationStrategy {strategy}, outside the note's table"),
            NOTE,
        )
        .into());
    }
    let (provider, shape) = {
        let space = cx.world.parts().space;
        let provider = provider_of(space, context);
        // Step 3: an unread class with strategy 1 or 2.
        if provider == Provider::Unread && strategy != 0 {
            return Err(unread_refusal(space, context, strategy));
        }
        // Steps 4 and 5: the fields and the count of a read class.
        let shape = match provider {
            Provider::Unread => None,
            _ => Some(shape(space, context, provider, state)?),
        };
        (provider, shape)
    };
    let hands_state = match shape {
        Some(s) => !provider.needs_authority() || s.authority,
        None => true,
    };
    let replicated = shape.and_then(|s| s.replicated);
    let mirrored = strategy != 0;
    if mirrored {
        match (replicated, hands_state) {
            // A mind or a weapon: no replicated array, so no store.
            (None, _) => return Ok(Located::Nothing),
            // Off authority: `ReplicatedNouns` alone, never created, writes false.
            (Some(r), false) => {
                return Ok(match op {
                    Op::Read => Located::Replicated(replicated_read(cx.world.parts().space, context, r, key)?),
                    Op::ReadBase => Located::Replicated(0.0),
                    Op::Write | Op::Probe(_) => Located::Nothing,
                });
            }
            (Some(_), true) => {}
        }
    } else if !hands_state {
        return Ok(Located::Nothing);
    }
    // The `NounState` entry.
    {
        let space = cx.world.parts().space;
        let items = entries(space, context, state)?;
        if let Some(index) = find(space, &items, key)? {
            let e = element(&items, index)?;
            match provider {
                // Step 3's second half: a write to a slotted entry on an unread class.
                Provider::Unread => {
                    // Only an operation that rewrites the entry is refused: a write, an add, or the removal of a
                    // modifier the entry's stack holds (#518's ProbeOp, F6).
                    // Execution (`Op::Write`) refuses where its rewrite is planned (`plan`, and `add` before its duplicate
                    // test), so a removal of an absent modifier answers NotPresent in both (review r2 of #523, item 4).
                    let rewrites = match op {
                        Op::Read | Op::ReadBase | Op::Write => false,
                        Op::Probe(ProbeOp::Add) => true,
                        Op::Probe(ProbeOp::Remove(m)) => {
                            stack_of(space, context, state, index)?.contains(&Value::Object(Some(m)))
                        }
                    };
                    if rewrites && slot_of(space, e)? != -1 {
                        return Err(unread_refusal(space, context, strategy));
                    }
                }
                // Step 6: the slot, before any mutation.
                _ => {
                    check_slot(space, context, e, replicated)?;
                }
            }
            return Ok(Located::Entry(Storage::Noun {
                owner: context,
                state,
                index,
            }));
        }
    }
    // Missing: `d` first (section 4; it may create other entries and take slots). The probe validates the default
    // without evaluating it (#518's `preflight_init`): evaluating could create another noun's entry or slot.
    let default = required(cx, resolver, "DefaultValue")?;
    if let Op::Probe(_) = op {
        crate::eval::preflight_init(sys, &default, cx)?;
        return Ok(Located::Nothing);
    }
    let d = evaluate_value(sys, &default, Some(context), None, cx)?;
    // Everything is built and checked before the first write (review r1 of #523, item 2), then written in the note's
    // order: the count, the slot and `bNetDirty`, then the `NounState` append.
    let (slot_write, items, index) = {
        let space = cx.world.parts().space;
        let mut slot = -1;
        let mut slot_write = None;
        if mirrored && let Some(r) = replicated {
            // The count is read again after `d` (section 10.5, "Creation with a slot", step 2).
            let count = count_of(space, context, r.num)?;
            if count < CAPACITY {
                let addr = PropAddr::element(r.nouns, u32::try_from(count).unwrap_or(0));
                let mut entry = space.read(context, addr)?.clone();
                set_field(space, &mut entry, "NounName", Value::Name(key))?;
                set_field(space, &mut entry, "Value", Value::Float(d))?;
                set_field(space, &mut entry, "bOnlyRelevantToOwner", Value::Bool(strategy == 1))?;
                slot = count;
                slot_write = Some((r, count + 1, addr, entry));
            }
            // At 20 there is no slot: nothing is written for it (the note's log line is the port's to choose; none).
        }
        let mut items = entries(space, context, state)?;
        let mut e = space.classes.struct_defaults(entry_struct(space, state)?)?;
        set_field(space, &mut e, "NounName", Value::Name(key))?;
        set_field(space, &mut e, "Value", Value::Float(d))?;
        set_field(space, &mut e, "BaseValue", Value::Float(d))?;
        set_field(space, &mut e, "ModifierStack", Value::array(Vec::new()))?;
        set_field(space, &mut e, "ReplicationStrategy", Value::Byte(strategy))?;
        set_field(space, &mut e, "IndexInReplicationArray", Value::Int(slot))?;
        let index = items.len();
        items.push(e);
        let items_value = Value::array(items.clone());
        if let Some((r, count, addr, entry)) = &slot_write {
            precheck(space, context, PropAddr::scalar(r.num), &Value::Int(*count), "NumReplicatedNouns")?;
            precheck(space, context, *addr, entry, "the replicated slot")?;
            precheck(space, context, PropAddr::scalar(r.dirty), &Value::Bool(true), "bNetDirty")?;
        }
        precheck(space, context, PropAddr::scalar(state), &items_value, NOUN_STATE)?;
        (slot_write, items, index)
    };
    cx.note_writes();
    let space = cx.world.parts().space;
    if let Some((r, count, addr, entry)) = slot_write {
        space.write(context, PropAddr::scalar(r.num), Value::Int(count))?;
        space.write(context, addr, entry)?;
        space.write(context, PropAddr::scalar(r.dirty), Value::Bool(true))?;
    }
    space.write(context, PropAddr::scalar(state), Value::array(items))?;
    Ok(Located::Entry(Storage::Noun {
        owner: context,
        state,
        index,
    }))
}

/// Field `name` (`Value` or `BaseValue`) of the entry found, the replicated slot's value for `Value`, or 0.0.
fn read_located(cx: &mut EvalCx<'_>, found: Located, name: &str) -> Result<f32, AttrError> {
    match found {
        Located::Entry(Storage::Noun {
            owner,
            state,
            index,
        }) => entry_number(cx.world.parts().space, owner, state, index, name),
        Located::Replicated(v) if name == "Value" => Ok(v),
        _ => Ok(0.0),
    }
}

/// Field `name` of element `index` of `owner`'s noun store, as a float.
pub(crate) fn entry_number(
    space: &ObjectSpace,
    owner: ObjRef,
    state: PropId,
    index: usize,
    name: &str,
) -> Result<f32, AttrError> {
    let items = entries(space, owner, state)?;
    number(name, field(space, element(&items, index)?, name)?)
}

/// The `ModifierStack` of element `index`.
fn stack_of(
    space: &ObjectSpace,
    owner: ObjRef,
    state: PropId,
    index: usize,
) -> Result<Vec<Value>, AttrError> {
    let items = entries(space, owner, state)?;
    let e = element(&items, index)?;
    Ok(elements("ModifierStack", field(space, e, "ModifierStack")?)?.to_vec())
}

/// The mirror of a write (section 10.3, "The mirror"): the slot's replacement (its `Value` set, built and checked in
/// the plan), then `bNetDirty`.
#[derive(Debug, Clone)]
struct Mirror {
    nouns: PropId,
    dirty: PropId,
    slot: i32,
    replacement: Value,
}

/// A planned write to a noun entry, computed and checked before anything changes; [`commit`] makes it.
#[derive(Debug, Clone)]
pub(crate) struct NounWrite {
    owner: ObjRef,
    state: PropId,
    items: Vec<Value>,
    mirror: Option<Mirror>,
}

/// Plans a rewrite of element `index` with a new stack or base and the recomputed value (note section 5), with its
/// mirror. Everything is read and checked here, nothing written: a slotted entry on an unread class is refused, a
/// slot outside the occupied ones is the step-6 diagnostic.
fn plan(
    space: &ObjectSpace,
    owner: ObjRef,
    state: PropId,
    index: usize,
    stack: Option<Vec<Value>>,
    base: Option<f32>,
) -> Result<NounWrite, AttrError> {
    let mut items = entries(space, owner, state)?;
    let mut e = element(&items, index)?.clone();
    let slot = slot_of(space, &e)?;
    let provider = provider_of(space, owner);
    let mut target = None;
    if slot != -1 {
        if provider == Provider::Unread {
            let strategy = byte("ReplicationStrategy", field(space, &e, "ReplicationStrategy")?)?;
            return Err(unread_refusal(space, owner, strategy));
        }
        let replicated = shape(space, owner, provider, state)?.replicated;
        check_slot(space, owner, &e, replicated)?;
        target = replicated;
    }
    let stack = match stack {
        Some(s) => s,
        None => elements("ModifierStack", field(space, &e, "ModifierStack")?)?.to_vec(),
    };
    let base = match base {
        Some(b) => b,
        None => number("BaseValue", field(space, &e, "BaseValue")?)?,
    };
    // Section 5: single precision, sums in stack order, other types skipped, no truncation, no clamp. That is the
    // VM's opcode 5F form on a float, whatever combine form the call carries.
    let value = stack_value(space, base, &stack, CombineForm::Vm5F, NumKind::Float)?;
    set_field(space, &mut e, "ModifierStack", Value::array(stack))?;
    set_field(space, &mut e, "BaseValue", Value::Float(base))?;
    set_field(space, &mut e, "Value", value.clone())?;
    items[index] = e;
    let mirror = match target {
        Some(r) => {
            let addr = PropAddr::element(r.nouns, u32::try_from(slot).unwrap_or(0));
            let mut replacement = space.read(owner, addr)?.clone();
            set_field(space, &mut replacement, "Value", value)?;
            precheck(space, owner, addr, &replacement, "the replicated slot")?;
            precheck(space, owner, PropAddr::scalar(r.dirty), &Value::Bool(true), "bNetDirty")?;
            Some(Mirror { nouns: r.nouns, dirty: r.dirty, slot, replacement })
        }
        None => None,
    };
    precheck(space, owner, PropAddr::scalar(state), &Value::array(items.clone()), NOUN_STATE)?;
    Ok(NounWrite {
        owner,
        state,
        items,
        mirror,
    })
}

/// Makes a planned write (section 10.5, "A successful mirror"): the entry, then the slot's `Value`, then `bNetDirty`.
/// The caller signals the write (`EvalCx::note_writes`) first.
pub(crate) fn commit(space: &mut ObjectSpace, w: NounWrite) -> Result<(), AttrError> {
    space.write(w.owner, PropAddr::scalar(w.state), Value::array(w.items))?;
    if let Some(m) = w.mirror {
        let addr = PropAddr::element(m.nouns, u32::try_from(m.slot).unwrap_or(0));
        space.write(w.owner, addr, m.replacement)?;
        space.write(w.owner, PropAddr::scalar(m.dirty), Value::Bool(true))?;
    }
    Ok(())
}

/// Add (note section 5): the same modifier object already on the entry's stack is refused; otherwise it is
/// appended and the value recomputed. Returns the outcome and the write to commit.
pub(crate) fn add(
    space: &ObjectSpace,
    owner: ObjRef,
    state: PropId,
    index: usize,
    modifier: ObjRef,
) -> Result<(AddModifierOutcome, Option<NounWrite>), AttrError> {
    // An add on a slotted entry of an unread class is refused whether or not the modifier is already there, as its
    // probe refuses it (review r2 of #523, item 4).
    if provider_of(space, owner) == Provider::Unread {
        let items = entries(space, owner, state)?;
        let e = element(&items, index)?;
        if slot_of(space, e)? != -1 {
            let strategy = byte("ReplicationStrategy", field(space, e, "ReplicationStrategy")?)?;
            return Err(unread_refusal(space, owner, strategy));
        }
    }
    let mut stack = stack_of(space, owner, state, index)?;
    let held = Value::Object(Some(modifier));
    if stack.contains(&held) {
        return Ok((AddModifierOutcome::Duplicate, None));
    }
    stack.push(held);
    let w = plan(space, owner, state, index, Some(stack), None)?;
    Ok((AddModifierOutcome::Added, Some(w)))
}

/// Remove (note section 5): every copy of the modifier object; none there is refused and changes nothing.
pub(crate) fn remove(
    space: &ObjectSpace,
    owner: ObjRef,
    state: PropId,
    index: usize,
    modifier: ObjRef,
) -> Result<(RemoveModifierOutcome, Option<NounWrite>), AttrError> {
    let stack = stack_of(space, owner, state, index)?;
    let held = Value::Object(Some(modifier));
    let kept: Vec<Value> = stack.iter().filter(|v| **v != held).cloned().collect();
    let count = stack.len().saturating_sub(kept.len());
    if count == 0 {
        return Ok((RemoveModifierOutcome::NotPresent, None));
    }
    let w = plan(space, owner, state, index, Some(kept), None)?;
    Ok((RemoveModifierOutcome::Removed { count }, Some(w)))
}

/// Set-base (note section 5): the base, then the recompute.
pub(crate) fn set_base(
    space: &ObjectSpace,
    owner: ObjRef,
    state: PropId,
    index: usize,
    v: f32,
) -> Result<NounWrite, AttrError> {
    plan(space, owner, state, index, None, Some(v))
}

impl ValueResolver for Noun {
    fn value(
        &self,
        sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        match mode(cx) {
            ParityMode::Off => {
                let found = locate(sys, resolver, context, cx, Op::Read)?;
                read_located(cx, found, "Value")
            }
            ParityMode::Reference => reference_read(sys, resolver, context, cx),
        }
    }

    fn base_value(
        &self,
        sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        match mode(cx) {
            ParityMode::Off => {
                let found = locate(sys, resolver, context, cx, Op::ReadBase)?;
                read_located(cx, found, "BaseValue")
            }
            ParityMode::Reference => reference_read(sys, resolver, context, cx),
        }
    }

    /// Off: the entry (created when missing), or `None` where there is no store or only the replicated array (every
    /// write then answers false). Reference: `NotYet { NotPorted }` (module documentation), never a quiet `None`.
    fn storage(
        &self,
        sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<Storage>, AttrError> {
        match mode(cx) {
            ParityMode::Off => Ok(match locate(sys, resolver, context, cx, Op::Write)? {
                Located::Entry(s) => Some(s),
                Located::Nothing | Located::Replicated(_) => None,
            }),
            ParityMode::Reference => Err(reference_write()),
        }
    }

    /// The read-only probe (review r1 of #483): the lookup without creating; the preflight's refusals are raised here.
    fn probe_storage(
        &self,
        sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        op: ProbeOp,
        cx: &mut EvalCx<'_>,
    ) -> Result<(), AttrError> {
        match mode(cx) {
            ParityMode::Off => locate(sys, resolver, context, cx, Op::Probe(op)).map(|_| ()),
            ParityMode::Reference => Err(reference_write()),
        }
    }
}
