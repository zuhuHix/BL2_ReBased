//! Owner: lane W3-ATTRCTX. The resolvers whose rule this layer cannot run yet (an unread proxy test, script)
//! and the classes not read at all. Each answers `NotYet`, naming what is missing; a None source is None where the
//! record says the resolver works on a source.

use ow_core::{NotYet, ParityMode};
use ow_object::ids::ObjRef;

use super::RECORD;
use crate::api::{AttrError, ContextResolver, EvalCx};
use crate::registry::AttributeSystem;

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
/// form first checks `HasDesignerAttribute`. Lane FIX-DESIGNER-RESOLVER (decision 2026-11-16-62):
///
/// - The rule is the class's own script, which the world runs ([`AttrWorld::script_context`]; the VM runs it, a world
///   that runs no script answers `NotYet { LayerAbsent("ow-vm") }`). What the script reaches (the provider interface,
///   `GetInstancedDesignerAttribute` and the natives under it) answers as the VM answers any call.
/// - `ParityMode::Reference`, the plain form only: the C++ stand-in, which treats the class as always resolving and
///   keeps the object the chain held before it (`src/progression.cpp:85-87`): the source is the answer. The C++ has no
///   reading of the `ByName` form (it refuses the class), so that form runs the script in both modes.
/// - A None source is None in both modes (the script's interface query on None answers None, and the C++ keeps None).
///
/// [`AttrWorld::script_context`]: crate::api::AttrWorld::script_context
#[derive(Debug, Clone, Copy, Default)]
pub struct Designer {
    /// The `DesignerAttributeContextResolverByName` form.
    pub by_name: bool,
}

impl ContextResolver for Designer {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        let Some(source) = source else {
            return Ok(None);
        };
        let mode = cx.world.parts().defs.map_or(ParityMode::Off, |d| d.mode());
        if !self.by_name && mode == ParityMode::Reference {
            return Ok(Some(source));
        }
        cx.world.script_context(resolver, attr, Some(source))
    }
}

/// A context resolver class whose `GetAttributeContext` is script in the installed code packages ([`super::SCRIPTED`],
/// lane FIX-DESIGNER-RESOLVER, decision 2026-11-16-62): the rule is that script, which the world runs
/// ([`AttrWorld::script_context`]), a None source included (the script decides what None gives). The C++ has no
/// reading of these classes (it refuses them), so both parity modes run the script. A world that runs no script
/// answers `NotYet { LayerAbsent("ow-vm") }`.
///
/// [`AttrWorld::script_context`]: crate::api::AttrWorld::script_context
#[derive(Debug, Clone, Copy, Default)]
pub struct Script;

impl ContextResolver for Script {
    fn resolve(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        attr: ObjRef,
        source: Option<ObjRef>,
        cx: &mut EvalCx<'_>,
    ) -> Result<Option<ObjRef>, AttrError> {
        cx.world.script_context(resolver, attr, source)
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
