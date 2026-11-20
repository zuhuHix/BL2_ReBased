## 2026-11-19: WillowAIPawn.DropCombatPerch in Off

AI-assisted (Claude Code). Lane IMPL-DROPCOMBATPERCH, slice link 9. The decision number is pending (the landing run
numbers it). Coded from docs/verification/NATIVE_DROP_COMBAT_PERCH.md alone, the note at 08edad6d (PR #620 after its review r1; clean room:
lane ANALYST-DROPCOMBATPERCH wrote it). The executable and the analysis output were not opened. Synthetic fixtures only.
Stacked on fix/rust-is-staggered @ f0066f0f (#619), because both bind in `ow-combat` `hit/pools.rs`.

- **What changes:** `ow-combat` `hit/pools.rs` binds `WillowAIPawn.DropCombatPerch()` in Off, beside `IsStaggered`,
  through the keep-existing `bind_fn`, to `drop_combat_perch`. Reference keeps the stub policy.
- **The rule, in the note's order.** It is write-free, load-free and RNG-free:
  1. The receiver must be a `WillowAIPawn` or a child (`by_path` and `is_child_of` only). Otherwise a
     `VmError::Script`.
  2. **D1**, `NotYet { NotRead }`, with two texts: an unloaded handle, and a slotless stand-in. Both name the object
     (its class path and name), and the stand-in test is `ow-attr`'s condition, as in #619 after its review r1.
  3. **D2**, `NotYet { NotRead }`: a native descendant (the `NATIVE_AI_PAWN_DESCENDANTS` list shared with
     `IsStaggered`: `WillowAICranePawn` and `DeployableTurretActor`, the note's explicit list, checked with
     `is_child_of`, no load) or a child of one. Script-only children take the rule.
  4. The declarations, by name: `Role` a byte, `PerchProjectile` an object, `bPerchAttached` a bool. Otherwise a
     `VmError::Script` naming the field.
  5. The guard: `Role` not 3 (`ROLE_Authority`, compared as the raw byte, UNVERIFIED), `PerchProjectile` None, or
     `bPerchAttached` clear answers nothing, with nothing written.
  6. **D3**, `NotYet { NotRead }`, when the guard holds: the perch projectile's spawn and launch was read only to its
     edge. It refuses before any write.
- **`death.rs`'s no-op (review r1 of #620):** `death.rs` bound `DropCombatPerch` as a presentation no-op. It now
  binds it under Reference only (the key stays in `PRESENTATION_NO_OPS`, skipped in Off with a comment), so in Off
  the rule answers whichever pack registers first, and `Died` and `Staggered.BeginState` reach it. COMBAT_DEATH.md's
  presentation row says so. A test runs a perched pawn's `Died` in Off (D3) in both registration orders, and
  the no-op under Reference.
- **Verified:** host unit tests on invented packages, covering the note's ten tests (a missing or non-byte `Role`, D2 for each listed
  class and an invented child of each, and the death path among them). Every answer and refusal leaves
  #518's counters unchanged. The installed Off strict slice gate is identical to main's, row by row.
- **Not verified:** nothing is checked against the running game, so every rule is **UNVERIFIED** in game.
