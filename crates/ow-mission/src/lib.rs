//! Missions and world logic: the mission tracker, the mission-script bridge, Kismet, Matinee hooks, dialog, movers,
//! interactive objects, vendors and the Fire slice (port of `src/mission*.cpp`, `src/kismet.*`, `src/dialog.*`,
//! `src/mover.*`, `src/slice.*`).
//!
//! Engine-agnostic gameplay rules. Each rule keeps the evidence label it has in `docs/verification/` (verified
//! how, or UNVERIFIED); porting a rule does not change what is known about it.
//!
//! Scaffold (W0-SCAFFOLD): every module file exists with its owner lane named in its header; lanes fill their
//! own files and never edit this `lib.rs` (architecture section 2.3).

pub mod bridge;
pub mod dialog;
pub mod interactive;
pub mod kismet;
pub mod mission_list;
pub mod mover;
pub mod side_missions;
pub mod balance_refresh;
pub mod slice;
pub mod stations;
pub mod tracker;
pub mod vendors;
