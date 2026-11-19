//! Owner: lane W3-ATTRCTX; the Off rules are lane IMPL-INVENTORY-CONTEXT's (decision 2026-11-19-67), coded from
//! NATIVE_INVENTORY_CONTEXT.md alone (the analyst was lane ANALYST-INVENTORY-CONTEXT). The inventory resolvers:
//! EquippedInventory and Inventory. UNVERIFIED in the running game.
//!
//! - **`ParityMode::Off`.** `InventoryAttributeContextResolver.GetAttributeContext` is script (the note's section 2),
//!   so Off runs it through the world (`AttrWorld::script_context`), a None source included; what it reaches, the
//!   manager's `ItemActors` iterator among it, answers as the VM answers any call. The equipped resolver has no script
//!   override (section 4): [`equipped_off`] is its rule.
//! - **`ParityMode::Reference`** keeps what these resolvers did before that lane: the inventory resolver answers
//!   `NotYet { NotRead }` for a source and None for a None source, and the equipped resolver asks the world's
//!   [`EngineQuery::EquippedInventory`].

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;
use ow_object::value::Value;

use ow_defs::store::DefError;
use ow_object::reflect::ReflectError;
use ow_package::store::StoreError;

use super::{RECORD, ensure_loaded, is_a, native_stand_in, not_declared};
use crate::api::{AttrError, ContextResolver, EngineQuery, EvalCx};
use crate::registry::AttributeSystem;

/// The record of the Off rules.
pub const RECORD_INVENTORY_CONTEXT: &str = "NATIVE_INVENTORY_CONTEXT.md section 4";

/// The interface the equipped resolver asks a source for (`WillowGame.upk` exports the interface class `IBodyPawn`).
pub const BODY_PAWN_INTERFACE: &str = "WillowGame.IBodyPawn";

/// The class whose objects the equipped resolver answers as themselves.
pub const EQUIP_ABLE_ITEM: &str = "WillowGame.WillowEquipAbleItem";

/// The slots of `WillowPawn.EquippedItems` (a static array of four); a location of this or more is a data error.
pub const EQUIPPED_SLOTS: u8 = 4;

const WHO: &str = "EquippedInventoryAttributeContextResolver";

/// `EquippedInventoryAttributeContextResolver` (module documentation). Registered as
/// `WillowGame.EquippedInventoryAttributeContextResolver` ([`super::READ_FROM_PACKAGES`]).
#[derive(Debug, Clone, Copy, Default)]
pub struct EquippedInventory;

/// The resolver's `EquipmentLocation` byte.
fn equipment_location(cx: &mut EvalCx<'_>, resolver: ObjRef) -> Result<u8, AttrError> {
    let parts = cx.world.parts();
    let space = &*parts.space;
    let class = space.heap.get(resolver)?.class;
    let Some(prop) = space.prop(class, "EquipmentLocation") else {
        return Err(not_declared(space, resolver, "EquipmentLocation", WHO));
    };
    match space.read(resolver, prop.into())? {
        Value::Byte(b) => Ok(*b),
        other => Err(AttrError::BadData {
            reason: format!("EquipmentLocation holds {}, not a byte", other.kind_name()).into(),
        }),
    }
}

/// Whether the class of `source` implements [`BODY_PAWN_INTERFACE`] by its package declaration, by the strict lookup
/// (`CodeIndex::implements_declared`, review r1 of #455): confirmed present, confirmed absent, or unreadable. It has no
/// structural fallback and drops no unresolved table entry. Unreadable, and anything else that keeps the answer from
/// being established, is the note's named `NotYet { NotRead }`: "whether the source class implements `IBodyPawn`",
/// with the reason after it, never a "no". Those cases are: no definition store, a class with no package export
/// behind it, the interface's package missing from a store that holds packages, a table that cannot be read, and an
/// ancestry through an engine-native stand-in. A store with packages but no `IBodyPawn` class has no implementer.
fn declares_body_pawn(cx: &mut EvalCx<'_>, source: ObjRef) -> Result<bool, AttrError> {
    let unknown = |why: String| -> AttrError {
        NotYet::not_read(
            format!("{WHO}: whether the source class implements IBodyPawn ({why})"),
            RECORD_INVENTORY_CONTEXT,
        )
        .into()
    };
    let parts = cx.world.parts();
    let space = &*parts.space;
    let class = space.heap.get(source)?.class;
    let class_path = crate::eval::class_path(space, source);
    let Some(defs) = parts.defs else {
        return Err(unknown("this world has no definition store".into()));
    };
    let (code, store) = defs.code_mut();
    let Some(key) = code.key_of(class) else {
        return Err(unknown(format!("the class {class_path} has no package export behind it")));
    };
    let iface = match code.find_class(store, BODY_PAWN_INTERFACE) {
        Ok(iface) => iface,
        Err(ReflectError::ClassNotFound(_)) => return Ok(false),
        Err(ReflectError::Store(StoreError::PackageNotFound(_))) if store.indexed_package_count() == 0 => {
            return Ok(false);
        }
        Err(ReflectError::Store(StoreError::PackageNotFound(name))) => {
            return Err(unknown(format!("the package {name} is not in the store")));
        }
        Err(e) => return Err(DefError::from(e).into()),
    };
    match code.implements_declared(store, key, iface) {
        Some(true) => Ok(true),
        None => Err(unknown(format!("the interface tables on the ancestry of {class_path} cannot be read"))),
        Some(false) => match native_stand_in(space, class) {
            Some(path) => Err(unknown(format!("the ancestry runs through the engine-native stand-in {path}"))),
            None => Ok(false),
        },
    }
}

/// The Off rule (NATIVE_INVENTORY_CONTEXT.md section 4): a None source is None; a `WillowEquipAbleItem` (or below) is
/// itself, at any location; any other source must implement `IBodyPawn`, and one whose class does not is None. One
/// that does would be read through an `IBodyPawn` accessor, and which of the three the native calls is not settled, so
/// that is `NotYet { NotRead }` ("which IBodyPawn accessor the equipped resolver calls") after the location is checked
/// (a location of 4 or more is a data error, the note's "refuse rather than guess").
pub fn equipped_off(
    resolver: ObjRef,
    source: Option<ObjRef>,
    cx: &mut EvalCx<'_>,
) -> Result<Option<ObjRef>, AttrError> {
    let Some(s) = source else { return Ok(None) };
    ensure_loaded(cx, resolver)?;
    if is_a(cx.world.parts().space, s, EQUIP_ABLE_ITEM)? {
        return Ok(Some(s));
    }
    if !declares_body_pawn(cx, s)? {
        return Ok(None);
    }
    let location = equipment_location(cx, resolver)?;
    if location >= EQUIPPED_SLOTS {
        return Err(AttrError::BadData {
            reason: format!(
                "{WHO}: EquipmentLocation {location} is past WillowPawn.EquippedItems' {EQUIPPED_SLOTS} slots"
            )
            .into(),
        });
    }
    Err(NotYet::not_read(
        format!("{WHO}: which IBodyPawn accessor the equipped resolver calls"),
        RECORD_INVENTORY_CONTEXT,
    )
    .into())
}

impl ContextResolver for EquippedInventory {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        _attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        if crate::eval::parity(cx) == ParityMode::Off {
            return equipped_off(resolver, source, cx);
        }
        let Some(s) = source else { return Ok(None) };
        ensure_loaded(cx, resolver)?;
        let location = equipment_location(cx, resolver)?;
        cx.world
            .engine_query(EngineQuery::EquippedInventory { location }, s)
    }
}

/// `InventoryAttributeContextResolver` (module documentation): Off runs the class's script; Reference answers
/// `NotYet { NotRead }` for a source and None for a None source, as before.
#[derive(Debug, Clone, Copy, Default)]
pub struct Inventory;

impl ContextResolver for Inventory {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        if crate::eval::parity(cx) == ParityMode::Off {
            return cx.world.script_context(resolver, attr, source);
        }
        match source {
            None => Ok(None),
            Some(_) => Err(NotYet::not_read(
                "InventoryAttributeContextResolver: the property of its equipped-only flag",
                RECORD,
            )
            .into()),
        }
    }
}
