## 2026-11-26: The combat music manager and the spawn's balance tail

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-COMBAT-MUSIC-AND-AFTER, first stacked on #804 (whose `bcyl_tail` holds the
`GetCombatMusicManager` read this lane completes), then moved onto main 74b06d8 once #804 landed. Clean room: implemented from docs/verification/NATIVE_COMBAT_MUSIC_AND_AFTER.md (#801 as landed, r1)
alone; this lane did not read the executable. No session was run. Every rule is UNVERIFIED in the running game.

- **What was decided** (Off only; Reference binds nothing), in `ow-combat` `combat_music`:
  - **`BaseBalanceDefinition.ApplyGradeCustomizations`** (both keys): a balance that is not an
    `AIPawnBalanceDefinition` logs the engine's error line and answers false. On one: a None pawn false; a pawn
    itself; any other argument `NotRead` ("the actor's pawn query", by fit). Then the balance's own playthrough index
    (`GradeIndex` unused): the world info's `GRI` as a `WillowGameReplicationInfo`, its `CurrentPlaythrough`; a
    replication info that is not a Willow one 0 (a None `GRI` included, by fit: the cast fails); no world -1; a
    negative playthrough -1; clamped to the last `PlayThroughs` entry; out of range false. In range, that entry's
    `OnSpawnCustomizations` run through the port's `BehaviorBase.RunBehaviors` (self the pawn, instigator its
    `Instigator`, by fit), and true; an empty list runs nothing. The plan is write-free and load-free; a refusal after
    the first behaviour's write is prefixed;
  - **`WillowAIPawn.UpdatePlayThroughMaterial`**: with a `Mesh`, an `AIPawnBalanceDefinition` in
    `BalanceDefinitionState.BalanceDefinition` and the index in range, the entry's `MeshMaterial`: None nothing, set
    `NotRead` ("the material slots set"); any failed gate nothing. Write-free;
  - **`WillowGameInfo.GetCombatMusicManager`** (replacing #804's lazy-creation `NotRead`; bound before `bcyl_tail`,
    whose read stays): the field when set. When None, the preflight (the manager class in the class table, else
    `NotRead` "the manager class"; the world's globals, their globals definition through the class table with no
    class load, and its `CombatMusicProperties` resident; the fields declared), then a manager with the game info as
    its outer, from the class defaults (the resident class default's values when it is built, else the store builds
    it), stored in `GlobalCombatMusicManager`, its `CombatMusicParams` := `CombatMusicProperties`. A refusal after the
    creation is prefixed;
  - **`CombatMusicManager.PlayerDamagedByEnemy`**: nothing without `CombatMusicParams`, without an `Enemy`, or when
    the damage type's class default has `bCausedByWorld`; the declarations checked first (`CombatMusicParams`,
    `EnemyDamagesPlayerWeight`, `bCausedByWorld`); a damage type whose class default is not built is `NotRead`
    (building it loads). Past the gates, "the threat model" `NotRead`; **the hub's ruling on review of #801**: under the
    Off-only harness setting `ThreatModelNoOp` (a registration error outside Off or twice; logged at set-up), a
    no-op with one harness-input line per call, UNVERIFIED. The slice's combat world turns it on.
- **Tests** (invented classes): `combat_music_tests.rs`, the note's tests 1 to 4 (ApplyGradeCustomizations at 0, at 5
  clamped, negative, no `PlayThroughs`, no pawn, a non-pawn, a base balance, a one-behaviour plan, a non-Willow
  replication info, no world, an unloaded balance, each plan checked write-free; UpdatePlayThroughMaterial's gates
  and the set material's `NotRead`; GetCombatMusicManager's creation, its outer and class-default values, the second
  call write-free, an unloaded definition and a missing class refused before any write; PlayerDamagedByEnemy's three
  gates, an unbuilt damage default, an undeclared weight, the `NotRead`, the no-op's one line and the setting's Off-only
  registration). **Not tested:** test 5 (Reference unchanged), which holds by construction (`register` returns before
  binding outside Off); the behaviour run itself through `RunBehaviors` (only its plan); the store-built class default
  path of the creation.
- **Review of #808, r1:** `ApplyGradeCustomizations` runs its behaviours through `ow-behavior`'s new
  `run_behaviors_counted`, the native's walk with a write-aware policy: a refusal carries the partial-writes prefix
  only after a counted write (a heap write, a random draw or a load transition), so the first behaviour refusing, or a
  write-free one before a refusal, is unprefixed. The native `RunBehaviors` keeps its own policy. The lazy creation
  checks that `GlobalCombatMusicManager` accepts the manager and `CombatMusicParams` the params before anything is
  made (a mismatch is `VmError::Script`, a retry the same). The two definition classes come from the resident class
  table only (one the table lacks is `NotRead`, nothing enters the reflection cache), and `PlayerDamagedByEnemy`
  answers its no-params and no-enemy gates before the damage default, and reads the threat weight only past the
  world-damage gate. Tests: the counted walk on both sides of the first write, a mistyped params field, a cold
  definition class with the reflection count unchanged, and the gate order.
- **Measured:** see the GATES row of this lane. On the installed gate these natives are not reached yet: on main
  74b06d8 the spawns of links 3, 4, 5, 13 and 14 stop first at `AttributeInitializationDefinition.SetBaseValue` ("a
  definition it names is not loaded").
- **Verified:** the tests on invented data. **Not verified:** everything in the running game.
- **Sensitive files:** none.
