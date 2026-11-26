//! Combat: firing maths, the hit chain, damage pipeline natives, death, down state (FFYL), respawn, kill-XP glue,
//! loot on death, the scripted combat run and the damage-dealing behavior
//! handlers (port of `src/weapon_firing.*` and `src/combat*.cpp`).
//!
//! Engine-agnostic gameplay rules. Each rule keeps the evidence label it has in `docs/verification/` (verified
//! how, or UNVERIFIED); porting a rule does not change what is known about it.
//!
//! Scaffold (W0-SCAFFOLD): every module file exists with its owner lane named in its header; lanes fill their
//! own files and never edit this `lib.rs` (architecture section 2.3).

pub mod damage;
pub mod death;
pub mod default_physics;
pub mod down;
pub mod firing;
pub mod go_down;
pub mod hit;
pub mod hit_region_cap;
pub mod holds;
pub mod bcyl_tail;
pub mod combat_music;
pub mod link34;
pub mod shield_stage;
pub mod links34_tail;
pub mod links1314;
pub mod loot;
pub mod maya9;
pub mod pawn_interface;
pub mod run;
pub mod spawn_balance;
pub mod starting_values;
pub mod spawn_positions;
pub mod stance;
pub mod xp;
