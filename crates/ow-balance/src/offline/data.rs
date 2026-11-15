//! Owner: lane W3-BALANCE. The two readers of definition data the balance rules need beyond `ow-attr`'s public ones
//! (`ow_attr::eval::{property, required, field, number, flag, object, elements, init_data, class_path}`, which the
//! rules use directly): a class test and a struct field the record allows to be absent.

use ow_attr::api::AttrError;
use ow_object::heap::ObjectSpace;
use ow_object::ids::ObjRef;
use ow_object::value::Value;

use crate::types::BalanceError;

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

/// Field `name` of the struct value `v`, or `Ok(None)` when the struct type declares no such field (where the record
/// says what absence means). A value that is not a struct, or lacks a declared field, is bad data.
pub(crate) fn field_opt<'v>(
    space: &ObjectSpace,
    v: &'v Value,
    name: &str,
) -> Result<Option<&'v Value>, BalanceError> {
    let bad = |reason: String| {
        BalanceError::Attr(AttrError::BadData {
            reason: reason.into(),
        })
    };
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
