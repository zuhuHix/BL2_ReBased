//! `WillowGame.ResourcePoolStateAttributeValueResolver` (NATIVE_ATTRIBUTES.md section 6, high confidence): 1.0 if
//! the context pool is in `PoolState` (`EResourcePoolState`: 0 `RPS_Depleted`, 1 `RPS_Filled`, 2
//! `RPS_Regenerating`), else 0.
//!
//! What makes a pool "in" a state is not read: `ResourcePool` has no state property, and the native test (a value
//! at the minimum? at the maximum? a regeneration flag?) is not in the record. The resolver checks its own field and
//! answers `NotYet { NotRead }` naming the state, never a guessed comparison.

use ow_core::NotYet;
use ow_object::ids::ObjRef;

use super::RECORD;
use crate::api::{AttrError, EvalCx, ValueResolver};
use crate::eval::{byte, required};
use crate::registry::AttributeSystem;

pub(crate) struct ResourcePoolState;

const STATES: [&str; 3] = ["RPS_Depleted", "RPS_Filled", "RPS_Regenerating"];

impl ValueResolver for ResourcePoolState {
    fn value(
        &self,
        _sys: &AttributeSystem,
        resolver: ObjRef,
        _context: ObjRef,
        _previous: f32,
        cx: &mut EvalCx<'_>,
    ) -> Result<f32, AttrError> {
        let raw = byte("PoolState", &required(cx, resolver, "PoolState")?)?;
        let state = STATES
            .get(usize::from(raw))
            .ok_or_else(|| AttrError::BadData {
                reason: format!(
                    "ResourcePoolStateAttributeValueResolver PoolState {raw} is outside the enum"
                )
                .into(),
            })?;
        Err(NotYet::not_read(
            format!("ResourcePoolStateAttributeValueResolver: how a pool is found to be in state {state}"),
            RECORD,
        )
        .into())
    }
}
