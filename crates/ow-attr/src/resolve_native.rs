//! Owner: lane IMPL-RESOLVE-CONTEXT. The shared rule of the script native `AttributeDefinitionBase.ResolveContext`
//! in `ParityMode::Off`, coded from NATIVE_RESOLVE_CONTEXT.md section 5 alone (clean room; the analyst lane was
//! ANALYST-RESOLVE-CONTEXT). AI-assisted (Claude Code). **UNVERIFIED** in the running game.
//!
//! [`AttributeSystem::resolve_context_native`] holds the preflight (R1, R2, R3) and the walk; the script native's
//! wrapper (`ow_inventory::equip::natives`) checks the receiver and the argument first, and both the equip pack and
//! the action-skill pack bind that one wrapper.
//!
//! **The preflight** (write-free, load-free, RNG-free; plain object-space reads, nothing that loads), in order:
//! 1. **R1:** the definition is an unloaded definition handle: `NotYet { NotRead }`;
//! 2. the chain: `ContextResolverChain` declared, in the class layout, as an array of object references (the declared
//!    element type, so an empty array of another type is malformed too; review r1 of #615), else
//!    [`ResolveError::Malformed`] (the wrapper's `VmError::Script`);
//! 3. **R2, every entry first:** the first non-None entry that is an unloaded handle: `NotYet { NotRead }`;
//! 4. **R3, every entry after that:** the first non-None entry whose class is not registered with the context
//!    resolver registry: the registry's own `NotYet { NotRead }`, "context resolver class <path>".
//!
//! **The walk:** [`crate::eval::resolve_context`] (the note's section 2). Once it starts, any `NotYet` is terminal
//! (the hub's ruling, the `GetValue` precedent, decision 2026-11-19-58): it goes through
//! `NotYet::after_partial_writes` and nothing is put back; any other error is returned unchanged.

use ow_core::NotYet;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;

use crate::api::{AttrError, EvalCx};
use crate::registry::AttributeSystem;

/// The unit every text names.
pub const UNIT: &str = "AttributeDefinitionBase.ResolveContext";

/// The note this module implements.
pub const RECORD: &str = "NATIVE_RESOLVE_CONTEXT.md";

/// The chain the walk reads.
pub const CHAIN: &str = "ContextResolverChain";

/// The tail of R1's and R2's texts.
pub const UNLOADED: &str = "is an unloaded handle (a load is a write)";

/// Why the shared rule did not answer.
#[derive(Debug)]
pub enum ResolveError {
    /// The definition's chain is not an array of object references (the wrapper's `VmError::Script`).
    Malformed(String),
    /// R1 to R3, or anything the walk returned (a `NotYet` from the walk already marked terminal).
    Attr(AttrError),
}

impl From<AttrError> for ResolveError {
    fn from(e: AttrError) -> Self {
        ResolveError::Attr(e)
    }
}

/// The `<path>` R1's and R2's texts name (review r1 of #615): an unloaded handle's path from the store's own record
/// (it has no outer yet), otherwise the object's full path by its outers.
fn name_of(parts: &crate::api::WorldParts<'_>, o: ObjRef) -> String {
    parts.defs.as_deref().and_then(|d| d.handle_path(o)).unwrap_or_else(|| crate::eval::object_path(parts.space, o))
}

impl AttributeSystem {
    /// `AttributeDefinitionBase.ResolveContext(ContextSource)` on `attr` after the wrapper's receiver and argument
    /// checks: the preflight (module documentation), then the walk. The answer is the context or None.
    pub fn resolve_context_native(
        &self,
        attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, ResolveError> {
        {
            let parts = cx.world.parts();
            let space = &*parts.space;
            let unloaded = |o: ObjRef| parts.defs.as_deref().is_some_and(|d| d.is_unloaded(space, o));
            // R1.
            if unloaded(attr) {
                return Err(AttrError::from(NotYet::not_read(
                    format!("{UNIT}: the definition {} {UNLOADED}", name_of(&parts, attr)),
                    RECORD,
                ))
                .into());
            }
            // The chain's shape.
            let malformed = || ResolveError::Malformed(format!("{UNIT}: {CHAIN} is not declared as an array of object references"));
            let class = space.heap.get(attr).map_err(AttrError::from)?.class;
            let Some(prop) = space.prop(class, CHAIN) else {
                return Err(malformed());
            };
            // The declared element type, not the values (review r1 of #615): an empty array of ints is malformed.
            let declared = space.classes.decl(prop).map(|d| &d.kind);
            if !matches!(declared, Some(PropKind::Array(inner)) if matches!(**inner, PropKind::Object { .. })) {
                return Err(malformed());
            }
            let chain = space.read(attr, PropAddr::scalar(prop)).map_err(AttrError::from)?;
            let Some(items) = chain.elements() else {
                return Err(malformed());
            };
            let mut entries = Vec::with_capacity(items.len());
            for item in items {
                match item {
                    Value::Object(o) => entries.extend(*o),
                    _ => return Err(malformed()),
                }
            }
            // R2, every entry.
            if let Some(&r) = entries.iter().find(|&&r| unloaded(r)) {
                return Err(AttrError::from(NotYet::not_read(
                    format!("{UNIT}: the context resolver {} {UNLOADED}", name_of(&parts, r)),
                    RECORD,
                ))
                .into());
            }
            // R3, every entry.
            for &r in &entries {
                self.context_resolver(space, r)?;
            }
        }
        // The walk: any NotYet from here on is terminal.
        crate::eval::resolve_context(self, attr, source, cx).map_err(|e| match e {
            AttrError::NotYet(n) => ResolveError::Attr(AttrError::NotYet(n.after_partial_writes())),
            other => ResolveError::Attr(other),
        })
    }
}
