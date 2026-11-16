//! Owner: lane W3-ATTRCTX. The resolvers whose rule this layer cannot run yet (an unread proxy test, script)
//! and the classes not read at all. Each answers `NotYet`, naming what is missing; a None source is None where the
//! record says the resolver works on a source.

use ow_core::NotYet;
use ow_object::ids::ObjRef;

use super::RECORD;
use crate::api::{AttrError, ContextResolver, EvalCx};
use crate::registry::AttributeSystem;

/// The answer of a resolver whose rule is script: the VM runs it, so the layer missing is `ow-vm`.
fn script(what: &'static str) -> AttrError {
    NotYet::layer_absent(what, "ow-vm", RECORD).into()
}

/// `StatusEffectChanceModifierResolver` (confidence medium-low, "not read in detail"): the source if it is a
/// status-effect proxy, otherwise through the proxy's interface. Neither the proxy class nor the interface call is
/// recorded: `NotYet { NotRead }` for a source.
#[derive(Debug, Clone, Copy, Default)]
pub struct StatusEffectChance;

impl ContextResolver for StatusEffectChance {
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
                "StatusEffectChanceModifierResolver: the status-effect proxy test and its interface call",
                RECORD,
            )
            .into()),
        }
    }
}

/// `DesignerAttributeContextResolver` and its `ByName` form (script, confidence medium): the source's (or its
/// owner's) `IDesignerAttributeProvider`'s `InstancedDesignerAttribute` for the definition's `ValueName`; the ByName
/// form first checks `HasDesignerAttribute`. Both are script, which the VM runs: `NotYet { LayerAbsent("ow-vm") }`
/// for a source. (The C++ oracle keeps the previous object instead: decision 2026-11-15-12.)
#[derive(Debug, Clone, Copy, Default)]
pub struct Designer {
    /// The `DesignerAttributeContextResolverByName` form.
    pub by_name: bool,
}

impl ContextResolver for Designer {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        _resolver: ObjRef,
        _attr: ObjRef,
        source: Option<ObjRef>,
        _cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        match (source, self.by_name) {
            (None, _) => Ok(None),
            (Some(_), false) => Err(script(
                "DesignerAttributeContextResolver.GetAttributeContext (script)",
            )),
            (Some(_), true) => Err(script(
                "DesignerAttributeContextResolverByName.GetAttributeContext (script)",
            )),
        }
    }
}

/// `SkillAttributeContextResolver` (script-only). `Skill.GetAttributeContexts` does not call it: a chain that starts
/// with this class means "the context is the skill" there (NATIVE_ATTRIBUTES.md section 10). The class's own
/// script body is not read, so the resolver is `NotYet { NotRead }` for a source. Registered as
/// `WillowGame.SkillAttributeContextResolver` ([`super::READ_FROM_PACKAGES`]).
#[derive(Debug, Clone, Copy, Default)]
pub struct Skill;

impl ContextResolver for Skill {
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
                "SkillAttributeContextResolver.GetAttributeContext (script)",
                "NATIVE_ATTRIBUTES.md section 10",
            )
            .into()),
        }
    }
}

/// A context resolver class whose rule is not read ([`super::UNREAD`]): every call is `NotYet { NotRead }` naming
/// `record`, the place the class is known from, a None source included (whether it needs one is not read either).
#[non_exhaustive]
#[derive(Debug, Clone, Copy)]
pub struct Unread {
    pub record: &'static str,
}

impl Unread {
    pub fn new(record: &'static str) -> Self {
        Unread { record }
    }
}

impl ContextResolver for Unread {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        _attr: ObjRef,
        _source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        let parts = cx.world.parts();
        let class = parts
            .space
            .heap
            .get(resolver)
            .ok()
            .and_then(|o| parts.space.classes.get_layout(o.class))
            .map(|l| l.path().as_str().to_string())
            .unwrap_or_default();
        Err(NotYet::not_read(format!("context resolver class {class}"), self.record).into())
    }
}
