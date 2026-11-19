## 2026-11-19: Read the spawn side of loot on a kill as analyst

AI-assisted (Claude Code / Claude Opus 5.5).

Lane FIX-POOL-SPAWN-A, analyst only (slice link 5; SLICE_RUN_PLAN.md section 3.4 lane 2, which asked for an analyst
first because NATIVE_LOOT.md did not read the loot-spawn behaviors, decision 2026-11-16-41). The new own-words note
`docs/verification/NATIVE_LOOT_SPAWN.md` and four `NATIVE_INDEX.md` rows record:

- The stock regular bullymong spawns no loot through behaviors: its one behavior provider has none. Link 5's loot
  comes only from the death roll, `WillowPawn.DropLootOnDeath`.
- What happens to each spawned inventory: the drop condition, `DropFrom` and `CreatePickup` (pickup spawn, physics,
  torque) and `WillowPickup.InitializeFromInventory`. This is installed script.
- `InitializeInventory` on weapons, items and the base class, also script, and the natives inside it:
  `GenerateUniqueID` (two C-runtime draws, never 0), `ComputeMonetaryValue` (the definition's value, truncated, capped
  by currency at 99,999,999, 500 or 999) and `GetManufacturer`.
- `GetItemTossVelocity`: the base pawn rule, which the bullymong uses, and the AI pawn's ballistic rule toward its
  toss targets. `GetGameStageForSpawnedInventory` is a field read.
- The five loot-spawn behaviors are script with no native. In `ow-behavior` terms they need the kernel to run their
  installed `ApplyBehaviorToContext` with the consumer, which today's handler signature does not carry
  (decision 2026-11-15-30).

Raw output stays under ignored `local/analysis/loot-spawn/`; the Ghidra runs were read-only and took the machine lock.
Nothing is implemented: every native named stays unbound (or `NotYet`) in Off until a fresh lane codes it from the
note. Every rule is UNVERIFIED in the running game; the note lists the bl-sdk checks and five open questions.
