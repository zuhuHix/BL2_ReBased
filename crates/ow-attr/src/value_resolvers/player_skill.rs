//! `WillowGame.PlayerSkillAttributeValueResolver` (NATIVE_ATTRIBUTES.md section 6, medium confidence): the grade of
//! `AssociatedSkill` in the player's skill tree, 0 when the player or the tree is missing.
//!
//! The skill tree is engine state, reached through [`EngineQuery::PlayerSkill`] with the context as the source: the
//! world answers the skill object for the definition, whose `Grade` is read as stored (class-mod bonuses included,
//! as they are modifiers on `Skill.Grade`, NATIVE_ATTRIBUTES.md section 4); `Ok(None)` (no player, no tree, no such
//! skill) reads 0. A world without the engine side answers the query's `NotYet { LayerAbsent }`, which is what every
//! world gives until the skill lanes land, never a guessed 0. That the grade is the skill object's `Grade` is
//! UNVERIFIED (the tree's layout is not read).

use ow_core::NotYet;
use ow_object::ids::ObjRef;

use crate::api::{AttrError, EngineQuery, EvalCx, ValueResolver};
use crate::eval::{number, object, required};
use crate::registry::AttributeSystem;

pub(crate) struct PlayerSkill;

impl ValueResolver for PlayerSkill {
    fn value(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        let Some(definition) = object(
            "AssociatedSkill",
            &required(cx, resolver, "AssociatedSkill")?,
        )?
        else {
            return Err(NotYet::not_read(
                "PlayerSkillAttributeValueResolver with no AssociatedSkill",
                super::RECORD,
            )
            .into());
        };
        match cx
            .world
            .engine_query(EngineQuery::PlayerSkill { definition }, context)?
        {
            None => Ok(0.0),
            Some(skill) => number("Grade", &required(cx, skill, "Grade")?),
        }
    }
}
