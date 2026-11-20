//! Owner: lane IMPL-ADD-ATTRIBUTE-MODIFIER. The shared rule of the script native
//! `AttributeDefinitionBase.AddAttributeModifier` in `ParityMode::Off`, coded from NATIVE_ADD_ATTRIBUTE_MODIFIER.md
//! section 3 at 82e8d02e alone (clean room; the analyst lane was ANALYST-ADD-ATTRIBUTE-MODIFIER). AI-assisted (Claude
//! Code). **UNVERIFIED** in the running game.
//!
//! [`AttributeSystem::add_modifier_native`] holds steps 3 to 9 of the note's preflight and the write. The script
//! native's wrapper (`ow_inventory::equip::natives`) does steps 1 and 2 first (the receiver, the None cases and the
//! argument shapes), and both the equip pack and the action-skill pack bind that one wrapper.
//!
//! **Steps, in order** (write-free, load-free, RNG-free up to step 9; the first failure is the answer):
//! 3. **A1:** the definition is an unloaded handle: `NotYet { NotRead }`;
//! 4. **A0:** the context is an unloaded handle: `NotYet { NotRead }`;
//! 5. the chain: `ValueResolverChain` declared, in the class layout, as an array of object references (the declared
//!    element type, so an empty array of another type is malformed too), else [`AddError::Malformed`]. An
//!    empty chain, or a None first entry, answers false. Only the entries before the first None entry are checked;
//! 6. **A2, every checked entry first:** an unloaded value resolver handle: `NotYet { NotRead }`;
//! 7. **A3, every checked entry after that:** a class the value resolver registry does not register: the registry's
//!    own `NotYet { NotRead }`;
//! 8. **the storage probes, every checked entry last:** each resolver's read-only `probe_storage` for an add, inside a
//!    load-free scope ([`crate::eval::load_free`]): a handle it would load (a missing noun entry's default) refuses
//!    `NotYet { NotRead }` instead. The probes' own refusals (the replication table's among them) pass unchanged;
//! 9. the write: [`crate::effects::add_modifier`], answering `succeeded()`.
//!
//! **Once the write starts, any `NotYet` is terminal** (NATIVE_RESOLVE_CONTEXT.md's rule, word for word): it goes
//! through `NotYet::after_partial_writes` and nothing is put back; any other error is returned unchanged.

use ow_core::NotYet;
use ow_object::ids::{ObjRef, PropAddr};
use ow_object::layout::PropKind;
use ow_object::value::Value;

use crate::api::{AttrError, EvalCx, ProbeOp};
use crate::registry::AttributeSystem;

/// The unit every text names.
pub const UNIT: &str = "AttributeDefinitionBase.AddAttributeModifier";

/// The note this module implements.
pub const RECORD: &str = "NATIVE_ADD_ATTRIBUTE_MODIFIER.md";

/// The chain the add walks.
pub const CHAIN: &str = "ValueResolverChain";

/// The tail of A0's, A1's and A2's texts.
pub const UNLOADED: &str = "is an unloaded handle (a load is a write)";

/// Why the shared rule did not answer.
#[derive(Debug)]
pub enum AddError {
    /// The definition's chain is not an array of object references (the wrapper's `VmError::Script`).
    Malformed(String),
    /// A0 to A3, a probe's refusal, or anything the write returned (a `NotYet` from the write already terminal).
    Attr(AttrError),
}

impl From<AttrError> for AddError {
    fn from(e: AttrError) -> Self {
        AddError::Attr(e)
    }
}

/// The `<path>` A0's to A2's texts name (review r1 of #616): an unloaded handle's path from the store's own record (it
/// has no outer yet), otherwise the object's full path by its outers.
fn name_of(parts: &crate::api::WorldParts<'_>, o: ObjRef) -> String {
    parts.defs.as_deref().and_then(|d| d.handle_path(o)).unwrap_or_else(|| crate::eval::object_path(parts.space, o))
}

fn unloaded(what: &str, name: String) -> AddError {
    AttrError::from(NotYet::not_read(format!("{UNIT}: the {what} {name} {UNLOADED}"), RECORD)).into()
}

impl AttributeSystem {
    /// `AttributeDefinitionBase.AddAttributeModifier(Context, Modifier, bSuppressNotify)` on `attr`, after the wrapper's
    /// steps 1 and 2 (module documentation). The answer is the add's success.
    pub fn add_modifier_native(
        &self,
        attr: ObjRef,
        context: ObjRef,
        modifier: ObjRef,
        suppress_notify: bool,
        cx: &mut EvalCx<'_>,
    ) -> Result<bool, AddError> {
        {
            let parts = cx.world.parts();
            let space = &*parts.space;
            let is_unloaded = |o: ObjRef| parts.defs.as_deref().is_some_and(|d| d.is_unloaded(space, o));
            // 3. A1, 4. A0.
            if is_unloaded(attr) {
                return Err(unloaded("definition", name_of(&parts, attr)));
            }
            if is_unloaded(context) {
                return Err(unloaded("context", name_of(&parts, context)));
            }
            // 5. The chain: the entries before the first None.
            let malformed = || AddError::Malformed(format!("{UNIT}: {CHAIN} is not declared as an array of object references"));
            let class = space.heap.get(attr).map_err(AttrError::from)?.class;
            let Some(prop) = space.prop(class, CHAIN) else {
                return Err(malformed());
            };
            // The declared element type, not the values (review r1 of #616): an empty array of ints is malformed.
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
                    Value::Object(Some(o)) => entries.push(*o),
                    Value::Object(None) => break,
                    _ => return Err(malformed()),
                }
            }
            if entries.is_empty() {
                return Ok(false);
            }
            // 6. A2, every entry.
            if let Some(&r) = entries.iter().find(|&&r| is_unloaded(r)) {
                return Err(unloaded("value resolver", name_of(&parts, r)));
            }
            // 7. A3, every entry.
            for &r in &entries {
                self.value_resolver(space, r)?;
            }
        }
        // 8. The storage probes, load-free.
        crate::eval::load_free(UNIT, || crate::effects::check_modifier_chain(self, attr, context, ProbeOp::Add, cx))?;
        // 9. The write: any NotYet from here on is terminal.
        crate::effects::add_modifier(self, attr, Some(context), modifier, suppress_notify, cx)
            .map(|outcome| outcome.succeeded())
            .map_err(|e| match e {
                AttrError::NotYet(n) => AddError::Attr(AttrError::NotYet(n.after_partial_writes())),
                other => AddError::Attr(other),
            })
    }
}
