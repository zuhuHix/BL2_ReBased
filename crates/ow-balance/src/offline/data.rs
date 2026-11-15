//! Owner: lane W3-BALANCE. Readers of definition data the balance rules share: a property of a definition (loaded
//! through the world's `DefStore` first), a field of a struct value, and the conversions of stored values.
//!
//! These mirror the private readers of `ow-attr`'s `eval.rs` (which are `pub(crate)` there): a definition handle the
//! store has not loaded yet is loaded in place before it is read; a world without a store answers
//! `NotYet { LayerAbsent("ow-defs") }` for a slotless handle, never a guessed value. A property the class does not
//! declare is bad data where the rule needs it ([`required`]) and "absent" where the record says what absence means
//! ([`property`]).

use ow_attr::api::{AttrError, EvalCx, InitData, WorldParts};
use ow_core::NotYet;
use ow_object::heap::{ObjectSpace, Origin};
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::value::Value;

use crate::types::BalanceError;

/// The record the readers name.
const RECORD: &str = "NATIVE_PROGRESSION.md section 1";

fn bad(reason: String) -> BalanceError {
    BalanceError::Attr(AttrError::BadData {
        reason: reason.into(),
    })
}

/// Makes sure `obj` can be read: an unloaded definition handle (`Origin::Resource`) is loaded through the world's
/// store; any other live object is read as it is.
pub(crate) fn ensure_loaded(cx: &mut EvalCx<'_>, obj: ObjRef) -> Result<(), BalanceError> {
    let WorldParts { space, defs, .. } = cx.world.parts();
    let (resource, slotless) = {
        let o = space.heap.get(obj).map_err(AttrError::from)?;
        (
            matches!(o.origin, Origin::Resource { .. }),
            o.slots.is_empty(),
        )
    };
    if !resource {
        return Ok(());
    }
    match defs {
        Some(defs) => defs
            .ensure_loaded(space, obj)
            .map_err(|e| BalanceError::from(AttrError::from(e))),
        None if slotless => Err(NotYet::layer_absent(
            format!("loading the unloaded definition {obj:?}"),
            "ow-defs",
            RECORD,
        )
        .into()),
        None => Ok(()),
    }
}

/// The class path of `obj`, for messages.
pub(crate) fn class_path(space: &ObjectSpace, obj: ObjRef) -> String {
    match space.heap.get(obj) {
        Ok(o) => match space.classes.get_layout(o.class) {
            Some(l) => l.path().as_str().to_string(),
            None => format!("{:?}", o.class),
        },
        Err(_) => format!("{obj:?}"),
    }
}

/// Whether `obj` is of the class at `base_path` or a subclass; a space without that class has no such object.
pub(crate) fn is_a(
    space: &ObjectSpace,
    obj: ObjRef,
    base_path: &str,
) -> Result<bool, BalanceError> {
    let Some(base) = space.classes.by_path(base_path) else {
        return Ok(false);
    };
    let class = space.heap.get(obj).map_err(AttrError::from)?.class;
    Ok(space.classes.is_child_of(class, base))
}

/// Property `name` of `obj` (element 0), loaded first; `Ok(None)` when the class chain declares no such property.
pub(crate) fn property(
    cx: &mut EvalCx<'_>,
    obj: ObjRef,
    name: &str,
) -> Result<Option<Value>, BalanceError> {
    ensure_loaded(cx, obj)?;
    let space = cx.world.parts().space;
    let class = space.heap.get(obj).map_err(AttrError::from)?.class;
    let Some(p) = space.prop(class, name) else {
        return Ok(None);
    };
    Ok(Some(
        space
            .read(obj, PropAddr::scalar(p))
            .map_err(AttrError::from)?
            .clone(),
    ))
}

/// Property `name` of `obj`, which the class must declare (bad data naming both otherwise).
pub(crate) fn required(
    cx: &mut EvalCx<'_>,
    obj: ObjRef,
    name: &str,
) -> Result<Value, BalanceError> {
    match property(cx, obj, name)? {
        Some(v) => Ok(v),
        None => {
            let class = class_path(cx.world.parts().space, obj);
            Err(bad(format!("{class} has no property {name}")))
        }
    }
}

/// Field `name` of the struct value `v`; `Ok(None)` when the struct type has no such field.
pub(crate) fn field_opt<'v>(
    space: &ObjectSpace,
    v: &'v Value,
    name: &str,
) -> Result<Option<&'v Value>, BalanceError> {
    let s = v.as_struct().ok_or_else(|| {
        bad(format!(
            "expected a struct with field {name}, found {}",
            v.kind_name()
        ))
    })?;
    let layout = space
        .classes
        .struct_layout(s.ty)
        .ok_or_else(|| bad(format!("struct {:?} has no layout", s.ty)))?;
    let Some(entry) = space
        .names
        .find(name)
        .and_then(|n| layout.field(n))
        .map(|(entry, _)| entry)
    else {
        return Ok(None);
    };
    usize::try_from(entry)
        .ok()
        .and_then(|i| s.fields.get(i))
        .map(Some)
        .ok_or_else(|| {
            bad(format!(
                "struct {} value lacks field {name}",
                layout.path().as_str()
            ))
        })
}

/// Field `name` of the struct value `v`, which the struct type must have.
pub(crate) fn field<'v>(
    space: &ObjectSpace,
    v: &'v Value,
    name: &str,
) -> Result<&'v Value, BalanceError> {
    field_opt(space, v, name)?.ok_or_else(|| {
        let ty = v
            .as_struct()
            .and_then(|s| space.classes.struct_layout(s.ty))
            .map(|l| l.path().as_str().to_string())
            .unwrap_or_default();
        bad(format!("struct {ty} has no field {name}"))
    })
}

fn wrong(what: &str, wanted: &str, v: &Value) -> BalanceError {
    bad(format!(
        "{what}: expected {wanted}, found {}",
        v.kind_name()
    ))
}

/// A stored number as a float: a float as it is, an int or a byte converted (NATIVE_ATTRIBUTES.md section 3).
pub(crate) fn number(what: &str, v: &Value) -> Result<f32, BalanceError> {
    match v {
        Value::Float(f) => Ok(*f),
        // int -> float rounds to nearest, as the C++ float(int).
        Value::Int(i) => Ok(*i as f32),
        Value::Byte(b) => Ok(f32::from(*b)),
        other => Err(wrong(what, "a number", other)),
    }
}

pub(crate) fn flag(what: &str, v: &Value) -> Result<bool, BalanceError> {
    match v {
        Value::Bool(b) => Ok(*b),
        other => Err(wrong(what, "a flag", other)),
    }
}

/// An object reference; `Value::None` reads as no object.
pub(crate) fn object(what: &str, v: &Value) -> Result<Option<ObjRef>, BalanceError> {
    match v {
        Value::Object(o) => Ok(*o),
        Value::None => Ok(None),
        other => Err(wrong(what, "an object reference", other)),
    }
}

/// The elements of an array value.
pub(crate) fn elements<'v>(what: &str, v: &'v Value) -> Result<&'v [Value], BalanceError> {
    v.elements().ok_or_else(|| wrong(what, "an array", v))
}

/// An `AttributeInitializationData` struct value as [`InitData`].
pub(crate) fn init_data(space: &ObjectSpace, v: &Value) -> Result<InitData, BalanceError> {
    Ok(InitData {
        base_value_constant: number("BaseValueConstant", field(space, v, "BaseValueConstant")?)?,
        base_value_attribute: object("BaseValueAttribute", field(space, v, "BaseValueAttribute")?)?,
        initialization_definition: object(
            "InitializationDefinition",
            field(space, v, "InitializationDefinition")?,
        )?,
        base_value_scale_constant: number(
            "BaseValueScaleConstant",
            field(space, v, "BaseValueScaleConstant")?,
        )?,
    })
}
