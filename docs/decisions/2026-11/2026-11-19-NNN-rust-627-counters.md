## 2026-11-19: GetBoundingCylinder's tests assert #518's counters

AI-assisted (Claude Code). Lane FOLLOWUP-627-COUNTERS, test-only. The decision number is pending (the landing run
numbers it). Synthetic fixtures only.

Edits: docs/decisions/2026-11/2026-11-19-231-rust-bounding-cylinder-template-outer-chain.md (its promise that the
counter assertion would follow #595's rebase is now kept, so its text says the assertion is in).

- **What changes:** with #595 and #627 on main, `ow-world` `world/bounding_cylinder_tests.rs` compares #518's three
  counters (`Heap::committed_writes`, `GameRng::draws`, `DefStore::load_transitions`) and the name table's length.
  Every refusal checks them, and so does every answer the tests take through `answers`, plus the two direct checks
  (a host call that refuses, and a skipped lazy entry). The base predated the counters, so this replaces #627's
  stand-in, which compared the RNG state; the heap fingerprint stays.
- **Verified:** `cargo test -p ow-world bounding_cylinder`: 14 passed, and no counter moves on any refusal or answer.
- **Not verified:** nothing in the running game; the rules stay **UNVERIFIED**.
