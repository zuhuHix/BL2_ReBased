//! Owner: lane W3-ATTRCTX. The resolvers whose rule needs what this layer cannot do yet (an interface test, script)
//! and the classes not read at all. Each answers `NotYet`, naming what is missing; a None source is None where the
//! record says the resolver works on a source.

use ow_core::NotYet;
use ow_object::ids::ObjRef;

use super::RECORD;
use crate::api::{AttrError, ContextResolver, EvalCx};
use crate::registry::AttributeSystem;

/// The answer of a resolver whose rule needs an engine query the contract does not have yet.
fn needs_query(what: &'static str) -> AttrError {
    NotYet::not_ported(
        what,
        "architecture.md section 3.5 (EngineQuery; W3-ATTRCTX handoff)",
    )
    .into()
}

/// `BalancedActorAttributeContextResolver` (confidence medium-low): the source if it implements the balanced-actor
/// interface, else None. The class table holds no interface list and [`crate::api::EngineQuery`] has no interface
/// test yet, so a source is `NotYet { NotPorted }`.
#[derive(Debug, Clone, Copy, Default)]
pub struct BalancedActor;

impl ContextResolver for BalancedActor {
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
            Some(_) => Err(needs_query(
                "BalancedActorAttributeContextResolver: an engine query for the balanced-actor interface",
            )),
        }
    }
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
/// form first checks `HasDesignerAttribute`. The interface calls need an engine query the contract does not have:
/// `NotYet { NotPorted }` for a source.
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
            (Some(_), false) => Err(needs_query(
                "DesignerAttributeContextResolver: an engine query for the designer attribute provider",
            )),
            (Some(_), true) => Err(needs_query(
                "DesignerAttributeContextResolverByName: an engine query for the designer attribute provider",
            )),
        }
    }
}

/// `SkillAttributeContextResolver` (script-only). `Skill.GetAttributeContexts` does not call it: a chain that starts
/// with this class means "the context is the skill" there (NATIVE_ATTRIBUTES.md section 10). The class's own
/// script body is not read, so the resolver is `NotYet { NotRead }` for a source. Package not recorded: not
/// registered ([`super::PACKAGE_NOT_RECORDED`]).
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

/// A context resolver class the notes do not read ([`super::UNREAD`]): every call is `NotYet { NotRead }`, a None
/// source included (whether it needs one is not read either).
#[derive(Debug, Clone, Copy, Default)]
pub struct Unread;

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
        Err(NotYet::not_read(
            format!("context resolver class {class}"),
            "NATIVE_ATTRIBUTES.md \"Not read yet\"",
        )
        .into())
    }
}
