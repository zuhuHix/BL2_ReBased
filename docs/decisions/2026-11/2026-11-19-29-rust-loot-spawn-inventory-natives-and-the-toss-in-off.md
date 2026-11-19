## 2026-11-19: Loot spawn, part B: the three `WillowInventory` natives in Off, the toss NotYet

AI-assisted (Claude Code, Claude Opus 5.5). Lane FIX-POOL-SPAWN-B (implementer; slice link 5, SLICE_RUN_PLAN.md
section 3.4). It implements from `docs/verification/NATIVE_LOOT_SPAWN.md` (analyst lane FIX-POOL-SPAWN-A, decision
2026-11-19-23) alone: no raw analysis output, Ghidra or REA was opened. That note lands with the analyst's branch,
which has to land first.

- **`WillowInventory.GenerateUniqueID`, `ComputeMonetaryValue`, `GetManufacturer`: bound in Off**
  (`crates/ow-inventory/src/definition.rs`, registered by the inventory pack). The rules are the note's, "The natives
  inside it":
  - `GenerateUniqueID`: two draws, the first the low half, and 0 becomes 1.
  - `ComputeMonetaryValue`: truncate toward zero, then the currency caps: credits 99,999,999, Eridium 500, the other
    in-range values 999, past the last entry 0.
  - `GetManufacturer`: `DefinitionData.ManufacturerDefinition`.
- **The generator.** The two ID draws come from `CrtStream::LootPool`, the stream the Off loot path already shares
  (ow-inventory's pool pick and ow-combat's `OFF_STREAM`), so the note's order stays on one stream. That order is per
  pool (as the review of #408 settled it): the pool's roll, with each new inventory's two ID draws and its parts roll
  inside the pick loop; then that pool's toss draws for its dropped items; then the next pool. The game's own
  sequence is not reproduced (decision D5), UNVERIFIED.
- **A choice where the note is silent (UNVERIFIED).** "The inventory definition" is read as the one definition the
  inventory's `DefinitionData` holds: `WeaponTypeDefinition` or `ItemDefinition`.
- **`NotYet { NotRead }`, each naming its question.**
  - A `MonetaryValue` that reads an attribute or an initialization definition (the evaluation's context is the
    note's open question 2; a constant needs no context).
  - A NaN or out-of-range truncation.
  - A currency enum that is not the note's 13 entries, or a byte on its `_MAX` entry.
  - An inventory with no `DefinitionData`, or with neither definition field.
- **`GetItemTossVelocity` (ow-combat, `death/toss.rs`).** Bound in Off as `NotYet { NotRead }` before any draw:
  - the `WillowPawn` rule needs the forward rotation, and which one it is (facing, view or controller) is the note's
    open question 3;
  - the `WillowAIPawn` toss toward a target needs `ItemTossSpeed` (identified by meaning only) and the short-flight
    vector (question 3);
  - with no target, the AI pawn takes the `WillowPawn` rule.

  The settled arithmetic (the random direction, three draws a try; the 200 / 50 / 150 terms; the cap at 400) is a
  tested function, `base_toss`, ready for the lane that answers question 3. It is not wired to the native. Which draw
  goes to which axis is not stated in the note yet (review of #408); the function's X, Y, Z order is UNVERIFIED.
  Its sines, cosines and square roots go through `ow_core::num` (rule N3).
- **`GetGameStageForSpawnedInventory` is unchanged.** It stays the existing plain field read in both modes
  (NATIVE_LOOT.md). The new note reads the same rule ("a plain read of the pawn's `GameStageForSpawnedInventory`")
  and lists no open question for it; its field identification is UNVERIFIED, as before. The hub brief named it
  for NotYet. This lane kept it, because the note settles it, and leaves the call to the hub.
- **Reference.** `src/` has no stand-in for any of these natives, so all of them stay unbound in Reference.
- **Verified (automated, local).**
  - Seven ow-inventory tests on invented classes: the ID rule and its stream, the truncation and every cap, the
    NotYet cases, the manufacturer read, and Reference unbound with nothing drawn.
  - Seven ow-combat toss tests: the arithmetic on a scripted generator, and the natives' NotYet in Off and unbound in
    Reference.
- **Installed runs, Off (link 5).** Before and after, the slice gate's link 5 stops upstream of the kill, at the
  unbound `WillowGlobals.GetPlayerPawnDataManager()` in the `maya:8` step. `--combat-run` with `player:8` instead
  stops the baby's spawn at `TargetableAttributeValueResolver` (not read). No installed Off run reaches the death
  roll, and even there `SpawnBalancedInventoryFromPool` is still `NotYet { NotPorted }` before any draw (decision
  2026-11-16-41). These natives therefore change nothing an installed run shows yet.
- **UNVERIFIED.** Every rule above. Nothing was checked in the running game; the note's last section lists the
  bl-sdk reads that would confirm them.
