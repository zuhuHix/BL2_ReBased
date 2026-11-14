//! Saves: the `PlayerSaveGame` model, load order and save triggers (lane W7-SAVE). The on-disk format waits on
//! maintainer decision D9. Nothing in `src/` to port.
//!
//! Engine-agnostic gameplay rules. Each rule keeps the evidence label it has in `docs/verification/` (verified
//! how, or UNVERIFIED); porting a rule does not change what is known about it.
//!
//! Scaffold (W0-SCAFFOLD): every module file exists with its owner lane named in its header; lanes fill their
//! own files and never edit this `lib.rs` (architecture section 2.3).
