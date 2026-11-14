//! Skills: skills, skill trees, `SkillEffectManager`, action skill start and refusals, cooldown pool use and the
//! Phaselock rules (from `tools/skill_stats.py`, the `prepare_skill_*` rules and the `NATIVE_SKILLS`,
//! `NATIVE_ACTION_SKILL` and `NATIVE_PHASELOCK_*` notes). Above `ow-combat`: skills read the held weapon, deal damage
//! and lift targets.
//!
//! Engine-agnostic gameplay rules. Each rule keeps the evidence label it has in `docs/verification/` (verified
//! how, or UNVERIFIED); porting a rule does not change what is known about it.
//!
//! Scaffold (W0-SCAFFOLD): every module file exists with its owner lane named in its header; lanes fill their
//! own files and never edit this `lib.rs` (architecture section 2.3).

pub mod action_skill;
pub mod phaselock;
pub mod skills;
pub mod tree;
