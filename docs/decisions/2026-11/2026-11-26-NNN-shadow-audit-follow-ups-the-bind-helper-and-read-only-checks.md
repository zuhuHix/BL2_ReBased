## 2026-11-26: Shadow audit follow-ups: the bind helper and read-only checks

AI-assisted (Claude Code)

Lane SHADOW-AUDIT-FOLLOWUPS (queue 0x: the LOW findings of #748's review), with Claude Opus 5.5. Code in `ow-combat`
`hit.rs`, `ow-vm` `api.rs`, `ow-vm` `interp/eval.rs`, `ow-behavior` `kernel/run.rs`, `ow-mission` `bridge.rs` and the
`ow-cli` test allowance. No dependency, no binding change, and no bounds check changed.

- **The bind helper:** `ow_combat::hit::bind` and `bind_fn` are `#[track_caller]`, so the shadow audit records the
  file that called the helper. That closes the blind spot decision 2026-11-26-07 stated. The all-packs test's allowance
  for `ResolveContext` and `AddAttributeModifier` now names `ow-skills/src/action_skill.rs` as the checking file.
- **What the closed blind spot showed:** the installed census (`slice_gate_shadow_tests`, run under the machine lock)
  then found one more shadow, in the equip world. `Actor.Trace` is bound by `ow-combat/src/go_down.rs` and given up by
  `ow-skills/src/action_skill.rs`. Both bind the one implementation, `ow_world::world::actor_trace::trace`, by design
  (each side's comment says so), so nothing is lost. The allowance names it by its exact key and those two files.
- **Read-only checks:** `Vm::is_bound_unaudited` answers like `is_bound` without the audit's record. The three
  read-only callers use it:
  - the call-entry check (`interp/eval.rs`, `check_entry`);
  - the behavior kernel's read-only query gate (`kernel/run.rs`);
  - the mission bridge's map-route globals check (`bridge.rs`).

  `is_bound`'s doc comment says the audit is for set-up code. Other read-only callers (the census, `check_call`) were
  not listed by the review and are unchanged.
- **Decision 2026-11-26-07** gets two dated lines: the blind spot is closed, and the phaselock and mission worlds leave
  `GetInstancedMaterialInstanceConstant` unbound (an unbound-native refusal there). Both worlds' packs were checked in
  code: the phaselock world registers the core, skill-tree and action-skill packs only, and the mission world none.
- **Verified:** host tests, `crate_edges_test.py` and the installed census, as in the GATES row. Nothing about the game
  is claimed.

Edits: docs/decisions/2026-11/2026-11-26-07-the-native-shadow-audit-and-two-placeholders-removed.md

The edit marks the blind spot closed and adds the unbound-native line for the phaselock and mission worlds.
