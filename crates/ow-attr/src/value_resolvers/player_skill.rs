//! `WillowGame.PlayerSkillAttributeValueResolver` (NATIVE_ATTRIBUTES.md section 6, medium confidence): the grade of
//! `AssociatedSkill` in the player's skill tree, 0 when the player or the tree is missing.
//!
//! The skill tree is engine state this crate has no way to reach: no [`crate::api::EngineQuery`] names it, and the
//! tree's layout is not read. So the resolver answers `NotYet { LayerAbsent }` (after checking its own field), never
//! the 0 that only a missing player or tree gives. Proposed contract addition (handoff): an
//! `EngineQuery::PlayerSkill { definition }` answering the player's skill object for the definition, whose `Grade`
//! the resolver then reads.

use ow_core::NotYet;
use ow_object::ids::ObjRef;

use super::RECORD;
use crate::api::{AttrError, EvalCx, ValueResolver};
use crate::eval::{object, required};
use crate::registry::AttributeSystem;

pub(crate) struct PlayerSkill;

impl ValueResolver for PlayerSkill {
    fn value(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        _context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        object(
            "AssociatedSkill",
            &required(cx, resolver, "AssociatedSkill")?,
        )?;
        Err(NotYet::layer_absent(
            "PlayerSkillAttributeValueResolver: the grade of a skill in the player's skill tree",
            "an engine query for the player's skill tree (ow-world / ow-skills)",
            RECORD,
        )
        .into())
    }
}
