//! Owner: lane W3-ATTRCTX. The pool resolvers: ResourcePool and WeaponResourcePool (NATIVE_ATTRIBUTES.md section 5,
//! confidence medium). Finding the owner (the resource definition's own owner resolver) and its pool for the resource
//! is the engine's, through [`EngineQuery::ResourcePool`]. UNVERIFIED in the running game.

use ow_core::NotYet;
use ow_object::ids::ObjRef;
use ow_object::value::Value;

use super::{RECORD, ensure_loaded, is_a, not_declared, object_prop};
use crate::api::{AttrError, ContextResolver, EngineQuery, EvalCx};
use crate::registry::AttributeSystem;

/// The resource the resolver object names in property `name`: `Ok(None)` for a None reference, `NotYet { NotRead }`
/// when the resolver's class declares no such property.
fn resource_of(
    cx: &mut EvalCx<'_>,
    resolver: ObjRef,
    name: &str,
    who: &'static str,
) -> Result<Option<ObjRef>, AttrError> {
    ensure_loaded(cx, resolver)?;
    let parts = cx.world.parts();
    match object_prop(parts.space, resolver, name)? {
        Some(r) => Ok(r),
        None => Err(not_declared(parts.space, resolver, name, who)),
    }
}

/// `ResourcePoolAttributeContextResolver`: the source's pool for the resolver's `Resource`
/// ([`EngineQuery::ResourcePool`]); None for a None source, a None `Resource` (as the C++ oracle,
/// `src/progression.cpp`) or no such pool.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResourcePool;

impl ContextResolver for ResourcePool {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        _attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        let Some(s) = source else { return Ok(None) };
        let who = "ResourcePoolAttributeContextResolver";
        match resource_of(cx, resolver, "Resource", who)? {
            Some(resource) => cx
                .world
                .engine_query(EngineQuery::ResourcePool { resource }, s),
            None => Ok(None),
        }
    }
}

/// `WeaponResourcePoolAttributeContextResolver`: as [`ResourcePool`] with the resolver's `PrimaryHandResource`, or
/// its `OffHandResource` when the source weapon's `bOffHand` is set. That the two resources are the resolver's
/// fields is this port's reading of the record. A source that is not a weapon is `NotYet { NotRead }`.
/// Registered as `Engine.WeaponResourcePoolAttributeContextResolver` ([`super::READ_FROM_PACKAGES`]).
#[derive(Debug, Clone, Copy, Default)]
pub struct WeaponResourcePool;

impl ContextResolver for WeaponResourcePool {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        _attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        const WHO: &str = "WeaponResourcePoolAttributeContextResolver";
        let Some(s) = source else { return Ok(None) };
        let off_hand = {
            let parts = cx.world.parts();
            let space = &*parts.space;
            if !is_a(space, s, "Engine.Weapon")? {
                return Err(NotYet::not_read(
                    format!("{WHO} on a source that is not a weapon"),
                    RECORD,
                )
                .into());
            }
            let class = space.heap.get(s)?.class;
            let Some(prop) = space.prop(class, "bOffHand") else {
                return Err(not_declared(space, s, "bOffHand", WHO));
            };
            // The installed `Engine.Weapon` declares `bOffHand` as a byte, and its script only compares it with 0
            // (lane FIX-EQUIP-NATIVES): nonzero is the off hand. A bool is taken too (an invented class may declare
            // one).
            match space.read(s, prop.into())? {
                Value::Bool(b) => *b,
                Value::Byte(b) => *b != 0,
                other => {
                    return Err(AttrError::BadData {
                        reason: format!("bOffHand holds {}, not a byte or bool", other.kind_name()).into(),
                    });
                }
            }
        };
        let field = if off_hand {
            "OffHandResource"
        } else {
            "PrimaryHandResource"
        };
        match resource_of(cx, resolver, field, WHO)? {
            Some(resource) => cx
                .world
                .engine_query(EngineQuery::ResourcePool { resource }, s),
            None => Ok(None),
        }
    }
}
