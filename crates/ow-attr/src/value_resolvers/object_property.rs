//! `Engine.ObjectPropertyAttributeValueResolver`: the property `PropertyName` of the context (NATIVE_ATTRIBUTES.md
//! sections 3 and 6, high confidence). The one writable resolver: [`ValueResolver::storage`] tells the effects lane
//! (W3-ATTREFFECTS) where the number lives.
//!
//! - **Value:** a stack attribute (the stack flag) or a plain attribute (the plain flag) returns its stored number,
//!   an int or byte converted to float; a property that is neither returns 0.
//! - **Base:** a stack attribute reads its base companion; a plain attribute reads itself; neither: 0.
//! - **Storage:** a stack attribute is `Storage::Stack` on the context, a plain one `Storage::Plain`; neither is
//!   `None` (every operation answers false, section 3).
//!
//! **Inherited properties** (real-game question 12): the name is looked up over the context's whole class chain
//! (`ClassTable::prop` walks the super classes), which is what the data needs (`ExpLevel` on weapons). The native
//! lookup as read walks the own member list and caches per owner class; whether it reaches superclasses is
//! UNVERIFIED.
//!
//! A context whose class chain has no property of that name is not read (the record says what happens for a
//! property that is not an attribute, not for a missing one): `NotYet { NotRead }`. A stack attribute without its
//! companions is the engine's "Modifier stack not found" data bug: `BadData`. Status: UNVERIFIED.

use ow_core::NotYet;
use ow_object::ids::{ObjRef, PropAddr, PropId};
use ow_object::layout::{AttrCompanions, PropKind};

use super::RECORD;
use crate::api::{AttrError, EvalCx, Storage, ValueResolver};
use crate::eval::{class_path, ensure_loaded, name, number, required};
use crate::registry::AttributeSystem;
use crate::stack::NumKind;

pub(crate) struct ObjectProperty;

/// What the named property is.
enum Target {
    Stack {
        prop: PropId,
        companions: AttrCompanions,
        kind: NumKind,
    },
    Plain {
        prop: PropId,
        kind: NumKind,
    },
    Neither,
}

fn num_kind(kind: &PropKind, what: &str) -> Result<NumKind, AttrError> {
    match kind {
        PropKind::FloatAttribute | PropKind::Float => Ok(NumKind::Float),
        PropKind::IntAttribute | PropKind::Int => Ok(NumKind::Int),
        PropKind::ByteAttribute { .. } | PropKind::Byte { .. } => Ok(NumKind::Byte),
        other => Err(AttrError::BadData {
            reason: format!(
                "{what}: an attribute flag on a {} property",
                other.kind_name()
            )
            .into(),
        }),
    }
}

fn target(cx: &mut EvalCx<'_>, resolver: ObjRef, context: ObjRef) -> Result<Target, AttrError> {
    let property = name("PropertyName", &required(cx, resolver, "PropertyName")?)?;
    ensure_loaded(cx, context)?;
    let space = cx.world.parts().space;
    let class = space.heap.get(context)?.class;
    let Some(prop) = space.classes.prop(class, property) else {
        return Err(NotYet::not_read(
            format!(
                "ObjectPropertyAttributeValueResolver on a {} context, which has no property {}",
                class_path(space, context),
                space.names.text(property)
            ),
            RECORD,
        )
        .into());
    };
    let decl = space.classes.decl(prop).ok_or_else(|| AttrError::BadData {
        reason: format!("property {prop:?} has no declaration").into(),
    })?;
    let what = space.names.text(property).into_owned();
    if decl.is_stack_attribute() {
        let companions = decl.attr.ok_or_else(|| AttrError::BadData {
            reason: format!("Modifier stack not found for attribute {what}").into(),
        })?;
        Ok(Target::Stack {
            prop,
            companions,
            kind: num_kind(&decl.kind, &what)?,
        })
    } else if decl.is_plain_attribute() {
        Ok(Target::Plain {
            prop,
            kind: num_kind(&decl.kind, &what)?,
        })
    } else {
        Ok(Target::Neither)
    }
}

fn read(cx: &mut EvalCx<'_>, context: ObjRef, prop: PropId) -> Result<f32, AttrError> {
    let space = cx.world.parts().space;
    number(
        "an attribute property",
        space.read(context, PropAddr::scalar(prop))?,
    )
}

impl ValueResolver for ObjectProperty {
    fn value(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        match target(cx, resolver, context)? {
            Target::Stack { prop, .. } | Target::Plain { prop, .. } => read(cx, context, prop),
            Target::Neither => Ok(0.0),
        }
    }

    fn base_value(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        match target(cx, resolver, context)? {
            Target::Stack { companions, .. } => read(cx, context, companions.base),
            Target::Plain { prop, .. } => read(cx, context, prop),
            Target::Neither => Ok(0.0),
        }
    }

    fn storage(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<Storage>, AttrError> {
        Ok(match target(cx, resolver, context)? {
            Target::Stack {
                companions, kind, ..
            } => Some(Storage::Stack {
                owner: context,
                companions,
                kind,
            }),
            Target::Plain { prop, kind } => Some(Storage::Plain {
                owner: context,
                prop,
                kind,
            }),
            Target::Neither => None,
        })
    }
}
