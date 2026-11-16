//! Owner: lane FIX-ATTR-CONTEXTS (decision 2026-11-16-49). `Engine.BalancedActorAttributeContextResolver`: the
//! source if its class implements the balanced-actor interface, else None (NATIVE_ATTRIBUTES.md section 5,
//! confidence medium-low). UNVERIFIED in the running game.

use ow_core::NotYet;
use ow_defs::store::DefError;
use ow_object::ids::ObjRef;
use ow_object::reflect::ReflectError;

use super::{RECORD, native_stand_in};
use crate::api::{AttrError, ContextResolver, EvalCx, WorldParts};
use crate::registry::AttributeSystem;

/// The balanced-actor interface. NATIVE_CONTROLLER_HELPERS.md and NATIVE_INVENTORY_SORT.md name it `IBalancedActor`;
/// its package, `Engine`, was read from the installed code packages' export tables (a top-level class export of
/// `Engine.upk` whose super class is `Core.Interface`; decision 2026-11-16-49).
pub const BALANCED_ACTOR_INTERFACE: &str = "Engine.IBalancedActor";

const WHO: &str = "BalancedActorAttributeContextResolver";

/// `BalancedActorAttributeContextResolver` (native; NATIVE_ATTRIBUTES.md section 5, confidence medium-low): the source
/// if its class implements [`BALANCED_ACTOR_INTERFACE`], else None; a None source is None.
///
/// The interface test is the class index's (`CodeIndex::implements`, the one `Vm::implements` and the VM's
/// `InterfaceCast` use): the interface table of the source's class or of a super class names the interface or a
/// class derived from it. As there, a class with no package export behind it implements nothing, and a "no" over an
/// ancestry through an engine-native stand-in class is not proven, so it is `NotYet { NotRead }`. The test needs the
/// code packages: a world without a definition store is `NotYet { LayerAbsent("ow-defs") }`. A store whose code
/// packages hold no such interface class has no implementer (None).
///
/// The C++ evaluator refuses the class ("unsupported attribute context resolver", `src/progression.cpp`), so there is
/// no Reference reading: both parity modes run this rule.
#[derive(Debug, Clone, Copy, Default)]
pub struct BalancedActor;

impl ContextResolver for BalancedActor {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        let Some(s) = source else { return Ok(None) };
        let implements = implements_interface(cx.world.parts(), s, BALANCED_ACTOR_INTERFACE, WHO)?;
        Ok(implements.then_some(s))
    }
}

/// Whether the class of `o` implements the interface class at `interface` (the class index's
/// `CodeIndex::implements`, the test `Vm::implements` and the VM's `InterfaceCast` use): the interface table of the
/// class or of a super class names the interface or a class derived from it. A class with no package export behind it
/// implements nothing; a store whose code packages hold no such interface (or no package that could) has no implementer
/// (false); a "no" over an
/// ancestry through an engine-native stand-in class is not proven, so it is `NotYet { NotRead }`; a world without a
/// definition store is `NotYet { LayerAbsent("ow-defs") }`. `who` names the caller in those texts. Shared by
/// [`BalancedActor`] and `ow-world`'s player-controller query (lane FIX-PHASELOCK-CHAIN, decision 2026-11-16-69).
pub fn implements_interface(
    parts: WorldParts<'_>,
    o: ObjRef,
    interface: &str,
    who: &str,
) -> Result<bool, AttrError> {
    let space = &*parts.space;
    let class = space.heap.get(o)?.class;
    let Some(defs) = parts.defs else {
        return Err(NotYet::layer_absent(
            format!("{who}: the interface table of the source's class (this world has no definition store)"),
            "ow-defs",
            RECORD,
        )
        .into());
    };
    let (code, store) = defs.code_mut();
    let key = match code.find_class(store, interface) {
        Ok(key) => Some(key),
        // No such interface class, or no package that could hold it: nothing implements it.
        Err(ReflectError::ClassNotFound(_))
        | Err(ReflectError::Store(ow_package::store::StoreError::PackageNotFound(_))) => None,
        Err(e) => return Err(DefError::from(e).into()),
    };
    let implements = match (key, code.key_of(class)) {
        (Some(key), Some(class)) => code.implements(store, class, key),
        _ => false,
    };
    if implements {
        return Ok(true);
    }
    match native_stand_in(space, class) {
        Some(path) => Err(NotYet::not_read(
            format!("{who}: whether an object of the native stand-in class {path} implements {interface}"),
            RECORD,
        )
        .into()),
        None => Ok(false),
    }
}
