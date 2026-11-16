//! `GearboxFramework.NounAttributeValueResolver`, the "AI parameter" resolver. The class declares only
//! `DefaultValue` (initialization data) and `ReplicationStrategy`. **Every rule here is UNVERIFIED in the running
//! game.**
//!
//! **Parity modes** (architecture R3):
//!
//! - `ParityMode::Reference`, the C++ reading (`src/progression.cpp` `nounValue`, [`reference_read`]): the
//!   `BaseValue` of the context's `AIClass.AttributeStartingValues` entry for this attribute, else `DefaultValue`,
//!   both evaluated with the context as the source; nothing is written. Get and get-base are the same read.
//! - `ParityMode::Off`: `NotYet { NotPorted }`. The native rule is written down in NATIVE_NOUN_ATTRIBUTES.md and
//!   waits for a clean-room implementation from that note (docs/LANE_BRIEF.md, "Clean room (native code)").
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
//! **Writes** (add, remove, set-base) answer `NotYet { NotPorted }` in both modes: the C++ has no noun write, and the
//! native one waits with the Off rule.

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;

use crate::api::{AttrError, EvalCx, Storage, ValueResolver};
use crate::eval::{
    class_path, elements, ensure_loaded, evaluate_value, field, object, property, required,
};
use crate::registry::AttributeSystem;

/// Where the native rule is written down.
pub(crate) const NOTE: &str = "NATIVE_NOUN_ATTRIBUTES.md";
/// The C++ reading Reference mode follows.
const CPP: &str = "src/progression.cpp nounValue";
/// Why the Off rule and the writes are not here yet.
const AWAITING: &str =
    "native rule in NATIVE_NOUN_ATTRIBUTES.md, awaiting clean-room implementation";

pub(crate) struct Noun;

/// The world's parity mode: its store's. A world without a store runs the Rust rules (`Off`).
fn mode(cx: &mut EvalCx<'_>) -> ParityMode {
    cx.world.parts().defs.map_or(ParityMode::Off, |d| d.mode())
}

/// `NotYet { NotPorted }` for the native rule.
fn awaiting(what: &str) -> AttrError {
    NotYet::not_ported(
        format!("NounAttributeValueResolver {what}: {AWAITING}"),
        NOTE,
    )
    .into()
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
            ParityMode::Off => Err(awaiting("get")),
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
            ParityMode::Off => Err(awaiting("get-base")),
            ParityMode::Reference => reference_read(sys, resolver, context, cx),
        }
    }

    /// Both modes: `NotYet { NotPorted }` (module documentation), never a quiet `None`.
    fn storage(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _context: ObjRef,
        _cx: &mut EvalCx<'_>,
    ) -> Result<Option<Storage>, AttrError> {
        Err(awaiting("add, remove or set-base"))
    }
}
