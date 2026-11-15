//! Owner: lane W3-ATTRCTX. The inventory resolvers: EquippedInventory (confidence medium) and Inventory (confidence
//! low; NATIVE_ATTRIBUTES.md section 5). The inventory manager's lookups are the engine's
//! ([`EngineQuery::EquippedInventory`], [`EngineQuery::Inventory`]). UNVERIFIED in the running game.

use ow_core::NotYet;
use ow_object::ids::ObjRef;
use ow_object::value::Value;

use super::{RECORD, ensure_loaded, not_declared};
use crate::api::{AttrError, ContextResolver, EngineQuery, EvalCx};
use crate::registry::AttributeSystem;

/// `EquippedInventoryAttributeContextResolver`: the item the source's inventory manager holds at the resolver's
/// `EquipmentLocation` byte ([`EngineQuery::EquippedInventory`]); None for a None source. Package not recorded: not
/// registered ([`super::PACKAGE_NOT_RECORDED`]).
#[derive(Debug, Clone, Copy, Default)]
pub struct EquippedInventory;

impl ContextResolver for EquippedInventory {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        _attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        const WHO: &str = "EquippedInventoryAttributeContextResolver";
        let Some(s) = source else { return Ok(None) };
        ensure_loaded(cx, resolver)?;
        let location = {
            let parts = cx.world.parts();
            let space = &*parts.space;
            let class = space.heap.get(resolver)?.class;
            let Some(prop) = space.prop(class, "EquipmentLocation") else {
                return Err(not_declared(space, resolver, "EquipmentLocation", WHO));
            };
            match space.read(resolver, prop.into())? {
                Value::Byte(b) => *b,
                other => {
                    return Err(AttrError::BadData {
                        reason: format!(
                            "EquipmentLocation holds {}, not a byte",
                            other.kind_name()
                        )
                        .into(),
                    });
                }
            }
        };
        cx.world
            .engine_query(EngineQuery::EquippedInventory { location }, s)
    }
}

/// `InventoryAttributeContextResolver`: a lookup through the source's interface for the item matching the
/// resolver's `InventoryDefinition`, optionally only equipped items. The property that carries the "equipped only"
/// flag is not recorded, so the resolver answers `NotYet { NotRead }` for a source (None for a None source) rather
/// than guess the flag. Once the property is known, the body is one [`EngineQuery::Inventory`].
#[derive(Debug, Clone, Copy, Default)]
pub struct Inventory;

impl ContextResolver for Inventory {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _attr: ObjRef,
        source: Option<ObjRef>,
        _cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
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
