//! Owner: lane W3-DECODE. Class default objects and instantiated exports (architecture section 3.3): the port of the
//! C++ `Runtime::defaultsOf` / `buildDefaults` and `instantiateExport` (`src/vm.cpp`).
//!
//! **A class default object** ([`class_default`]) starts with every property at its default value (zero, struct
//! defaults applied: [`ClassTable::default_slots`](crate::layout::ClassTable::default_slots)). Then, from the root
//! class down to the class itself, each class's own default object export, `Default__<Class>` in the class's package
//! whose class reference is that class, applies its tags ([`ValueCx::apply_tags`] with the 4-byte prefix): each class's
//! default object records only what it changes. A class whose default object export is missing adds nothing; one
//! whose tags fail part way keeps what was applied and logs `defaults: Default__<Class>: <error>`, as the C++ does.
//! Each class's default object is built once per [`DecodeState`](crate::decode::DecodeState) and has
//! [`Origin::Default`].
//!
//! **An instantiated export** ([`instantiate_export`]) is a new object of the export's class, named after the export,
//! starting from a copy of the class default object, with the export's own tags applied over it from the given
//! [`PropertyPrefix`]. A failure there is an error (the C++ does not catch it). The object has [`Origin::Export`]
//! and no outer: identity, outers and lazy loading of definitions are W3-DEFS's.
//!
//! Status: parity with the C++ VM. Archetypes are not followed (the C++ does not either): an export's archetype
//! defaults do not apply, which is UNVERIFIED against the game.

use ow_package::PackageId;

use crate::decode::{DecodeError, ValueCx};
use crate::heap::Origin;
use crate::ids::{ObjRef, StructId};
use crate::layout::PropKind;
use crate::prefix::PropertyPrefix;
use crate::reflect::ClassKey;

/// The class default object of loaded class `class`, built on first use (module documentation). A call made while
/// it is being built (through a resolver that instantiates an export of this class) returns the object being built,
/// with the tags applied so far.
pub fn class_default(cx: &mut ValueCx<'_>, class: ClassKey) -> Result<ObjRef, DecodeError> {
    if let Some(&o) = cx.state.class_defaults.get(&class) {
        return Ok(o);
    }
    let id = cx.layout(class)?;
    // Every struct a property holds by value takes its defaults before the object is allocated, so the slots start
    // from them.
    let structs: Vec<StructId> = cx
        .space
        .classes
        .props(id)?
        .into_iter()
        .filter_map(|(_, decl)| match decl.kind {
            PropKind::Struct(s) => Some(s),
            _ => None,
        })
        .collect();
    for s in structs {
        // A default stream that does not decode leaves the zero value without an error (the C++ rule); an error here
        // means the struct's export or package could not be read, which the C++ cannot meet.
        cx.ensure_struct_defaults(s)?;
    }
    let name = format!("Default__{}", cx.code.class(class).name);
    let object = cx
        .space
        .alloc(id, &name, None, Origin::Default { class: id })?;
    // Registered before any tag is applied, as the C++ `buildDefaults` sets `defaultsBuilt` first: a resolver that
    // instantiates an export of this class while these defaults are read gets this object, still being built,
    // instead of starting another one.
    cx.state.class_defaults.insert(class, object);
    let mut chain: Vec<ClassKey> = cx.code.super_chain(class).collect();
    chain.reverse();
    for cursor in chain {
        let info = cx.code.class(cursor);
        let (package, class_export) = (info.package, info.export);
        let wanted = format!("Default__{}", info.name);
        let Some(export) = default_export(cx, package, class_export, &wanted) else {
            continue;
        };
        if let Err(error) = cx.apply_tags(
            object,
            cursor,
            package,
            export,
            PropertyPrefix::Ordinary.bytes(),
        ) {
            cx.state.log.push(format!("defaults: {wanted}: {error}"));
        }
    }
    Ok(object)
}

/// The first export of `package` named exactly `wanted` whose class reference is `class_export` (the C++ looks the
/// name up folded, then checks the spelling and the class).
fn default_export(
    cx: &mut ValueCx<'_>,
    package: PackageId,
    class_export: i32,
    wanted: &str,
) -> Option<i32> {
    let candidates = cx.exports_named(package, wanted);
    let pkg = cx.store.get(package);
    candidates.into_iter().find(|&index| {
        let e = &pkg.exports[index as usize - 1];
        e.name == wanted && e.class == class_export
    })
}

/// An instantiated export: the new object, and where its tag stream ended (just past the `None` tag, from the
/// export's payload start: the start of any untagged tail).
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Instance {
    pub object: ObjRef,
    pub tags_end: usize,
}

/// Instantiates export `index` of `package` (module documentation): a new object of the export's class with the class
/// defaults and the export's tags from `prefix` on. Errors: an index that is not positive ("unsupported exported
/// object request"), an index outside the export table, a class reference that resolves to nothing ("cannot
/// instantiate a null class") or not to a class, and a failure applying the export's tags. On a failure after the
/// object was allocated it is freed again, as the C++ drops its local object: nothing is left behind but the class
/// default object and the stand-ins the tags reached, which stay cached as in the C++.
pub fn instantiate_export(
    cx: &mut ValueCx<'_>,
    package: PackageId,
    index: i32,
    prefix: PropertyPrefix,
) -> Result<Instance, DecodeError> {
    if index <= 0 {
        return Err(DecodeError::UnsupportedRequest);
    }
    let pkg = cx.store.shared(package);
    let exported = pkg.object(index)?;
    let class = cx
        .code
        .class_at(cx.store, package, exported.class)?
        .ok_or(DecodeError::NullClass)?;
    let defaults = class_default(cx, class)?;
    let id = cx.layout(class)?;
    let object = cx
        .space
        .alloc(id, &exported.name, None, Origin::Export { package, index })?;
    let applied = cx
        .space
        .heap
        .get(defaults)
        .map(|d| d.slots.clone())
        .map_err(DecodeError::from)
        .and_then(|slots| {
            cx.space.heap.get_mut(object)?.slots = slots;
            cx.apply_tags(object, class, package, index, prefix.bytes())
        });
    match applied {
        Ok(tags_end) => Ok(Instance { object, tags_end }),
        Err(error) => {
            // Nothing refers to the new object yet; freeing it only gives its slot back.
            let _ = cx.space.free(object);
            Err(error)
        }
    }
}

#[cfg(test)]
mod tests;
