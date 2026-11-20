## 2026-11-19: GetBoundingCylinder's tests assert #518's counters

AI-assisted (Claude Code). Lane FOLLOWUP-627-COUNTERS, test-only. The decision number is pending (the landing run
numbers it). Synthetic fixtures only.

Edits: docs/decisions/2026-11/2026-11-19-231-rust-bounding-cylinder-template-outer-chain.md

Why decision 231 changes: its promise that the counter assertion would follow #595's rebase is now kept, so its text
says what the checks are.

- **What changes:** with #595 and #627 on main, `ow-world` `world/bounding_cylinder_tests.rs` checks one snapshot
  before and after each call: the heap fingerprint (every object's class, slots, `is_unloaded` and `is_failed`, and
  the name table's size), the RNG streams' whole state, #518's three counters (`Heap::committed_writes`,
  `GameRng::draws`, `DefStore::load_transitions`), and the name table's length. Every refusal (`refuses`), every
  direct answer (`answers`), each host call that succeeds or refuses, and each script error (`same`) check it
  (review r1 of #642).
- **Verified:** `cargo test -p ow-world bounding_cylinder`: 14 passed; nothing in the snapshot moves on any call.
- **Not verified:** nothing in the running game; the rules stay **UNVERIFIED**.
