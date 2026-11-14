//! AI: pawns, minds, awareness, targeting, melee and population factories. Nothing in `src/` to port; written from
//! the `NATIVE_AI_*` notes and the live records (lanes W7-AI).
//!
//! Engine-agnostic gameplay rules. Each rule keeps the evidence label it has in `docs/verification/` (verified
//! how, or UNVERIFIED); porting a rule does not change what is known about it.
//!
//! Scaffold (W0-SCAFFOLD): every module file exists with its owner lane named in its header; lanes fill their
//! own files and never edit this `lib.rs` (architecture section 2.3).
