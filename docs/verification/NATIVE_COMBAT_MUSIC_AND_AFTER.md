# The combat music manager, the spawn's balance tail, and where links 4, 13 and 14 end (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-COMBAT-MUSIC-AND-AFTER (decision 2026-11-26-NNN), the analyst half of
the clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/cmusic/`. This
note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every rule
here is UNVERIFIED in the running game.**

**Why.** NATIVE_BOUNDING_CYLINDER_AND_AFTER.md (#799) puts the swings' next stop at
`WillowGameInfo.GetCombatMusicManager`'s lazy creation. On main 03a8a09, which now runs #788's balance step and
SetDefaultPhysics' rule, the spawns of links 3 and 4 also meet new stops first. This note reads both.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`): `Pawn.ApplyBalanceDefinitionCustomizations`.
- **The natives:** Ghidra 12.1.4, headless and read-only, under the machine lock. Read: the execs named below and
  their class-table slots (the NATIVE_COMPONENT_SKIP_FLAG.md section 6 method), in the primary tables of:
  - `BaseBalanceDefinition`, `AIPawnBalanceDefinition`;
  - `CombatMusicManager`.
- **Field places:** the local copy of `tools/ghidra/class_layout.py`. Every field below lands exactly where the routine
  reads it.
- **`ow-package --def-dump`:**
  - the two slice balances' `PlayThroughs`;
  - `Default__CombatMusicManager`;
  - the globals definition's `CombatMusicProperties`.
- **A local probe** (temporary code, reverted, never committed): an installed Off `--combat-run` on main 03a8a09,
  under the machine lock, with a call trace. It ran the gate's steps for links 1, 3, 4, 13 and 14, and stubbed:
  - #793's, #796's and #799's stops not yet implemented on main, each by its note's rule or stand-in;
  - the base search with its `NoLevel` stand-in (nothing hit, `Base` unchanged);
  - then each stop below in turn.

## 2. The spawns' new stops on main

Main now runs `SetDefaultPhysics`' rule and the spawn's balance input (#788). So each spawn meets, in order:
1. **`SetPhysics`' base search**: the `LayerAbsent` or `NotRead` of NATIVE_LINK34_AFTER_MIND.md section 2. The
   `NoLevel` stand-in is the hub's ruling on review of #798;
2. **`BaseBalanceDefinition.ApplyGradeCustomizations(object,int)`**, unbound (section 3);
3. **`WillowAIPawn.UpdatePlayThroughMaterial()`**, unbound (section 4).

Past these, both spawns complete in the probe.

## 3. `ApplyGradeCustomizations(Pawn, GradeIndex)`

**The script:** `Pawn.ApplyBalanceDefinitionCustomizations` runs it when `BalanceDefinitionState.bGradeCustomizationsApplied`
is clear and a balance is set. It stores the answer in `bGradeCustomizationsApplied`.

**The native** (the exec calls the balance's class-table virtual):
- **`BaseBalanceDefinition`:** an engine error log line, then false;
- **`AIPawnBalanceDefinition`:**
  1. **the pawn:** the argument through an actor query that gives the pawn (not named; by fit, a pawn gives itself).
     With none, the answer is false;
  2. **the index:** it does **not** use `GradeIndex`. It asks its own playthrough index:
     - the current world's `WorldInfo.GRI` as a `WillowGameReplicationInfo`, and that object's playthrough virtual (by
       fit, `GetCurrPlaythrough`, the port's `CurrentPlaythrough` rule);
     - clamped to the last entry of the balance's `PlayThroughs`;
     - a negative playthrough, or no engine or world, gives -1. A replication info that is not a Willow one gives 0;
  3. **out of range** (below 0, or at least the number of `PlayThroughs`): false;
  4. **otherwise:** that entry's `OnSpawnCustomizations` behaviours are run on the pawn, with the pawn's `Instigator`
     (by fit, the same routine as `BehaviorBase.RunBehaviors`), and the answer is true.

**Proposed Off answer:**
- that rule. Every field must be declared with its kind, else `VmError::Script`, and each object read must be
  resident, else `NotRead`;
- a non-empty `OnSpawnCustomizations` runs through the port's `RunBehaviors` rule. The behaviours' writes come after
  the index read, so a refusal among them carries the partial-writes prefix;
- with no `WillowGameReplicationInfo`, the 0 case;
- the pawn query: for a pawn argument the pawn itself; any other argument is `NotRead`, "ApplyGradeCustomizations: the
  actor's pawn query".

**On the slice:** `PawnBalance_PrimalBeast_Monglet` and `PawnBalance_PrimalBeastAdult` each list two `PlayThroughs`
(1 and 2), with `OnSpawnCustomizations` empty in both. The harness's replication info has `CurrentPlaythrough` 0, so
the index is 0 and the answer is true, with nothing run.

## 4. `WillowAIPawn.UpdatePlayThroughMaterial()`

**The rule:**
- **gates:** with a `Mesh`, `BalanceDefinitionState.BalanceDefinition` as an `AIPawnBalanceDefinition`, and section 3's
  playthrough index in range, it takes that `PlayThroughs` entry;
- **with the entry's `MeshMaterial` set:** that material is set on a range of the mesh's material slots, from a start
  slot the routine computes up to the mesh's material count. The range was not read;
- **otherwise:** nothing.

**Proposed Off answer:**
- the gates, and a None `MeshMaterial`: nothing, write-free;
- a set `MeshMaterial`: `NotRead`, "UpdatePlayThroughMaterial: the material slots set";
- every field must be declared with its kind, else `VmError::Script`.

**On the slice:** index 0's `MeshMaterial` is None for both balances; only playthrough 2's entries set one. So nothing
happens.

## 5. `WillowGameInfo.GetCombatMusicManager()`: the lazy creation (completing #799 section 7)

- **When `GlobalCombatMusicManager` is None:**
  1. a new `CombatMusicManager` is made, with the game info as its outer, and stored in the field;
  2. then, with the Willow globals and their globals definition, the manager's `CombatMusicParams` := the
     definition's `CombatMusicProperties`. That is the manager's one-line initialiser;
- **in every case:** the answer is the field.

The installed globals definition (`GD_Globals.General.Globals`) names `GD_Globals.CombatMusic.DefaultCombatMusicParams`.

**Proposed Off answer:**
- **preflight, write-free and load-free:**
  - `WillowGame.CombatMusicManager` must be in the class table (no load);
  - the globals definition and its `CombatMusicProperties` must be resident, else `NotRead`;
  - the fields must be declared, else `VmError::Script`;
- **writes:**
  - the new object, from the class defaults (`bCombatMusicManagerEnabled` true, `CombatMusicManagerState` 2,
    `CombatMusicParams` None);
  - the field;
  - `CombatMusicParams`.

  A refusal after the creation carries the partial-writes prefix;
- with the field set: the field, write-free.

## 6. `CombatMusicManager.PlayerDamagedByEnemy(Player, Enemy, Damage, DamageType)`

The exec calls the manager's virtual:
- **gates:**
  - nothing without `CombatMusicParams`, or without an `Enemy`;
  - nothing when `DamageType` is set and its default object's `bCausedByWorld` is set;
- **otherwise, the threat model** (outlined, not read to the end):
  - a threat is computed from the player, the enemy and the parameters' `EnemyDamagesPlayerWeight`. **The damage
    amount itself is not used.** The weight is divided by a clamped factor read from a world object (not named), then
    multiplied by one parameter chosen by band of the enemy's level minus the player's (5 and above, 4, 3, 2, 1, and so
    on);
  - the threat is then added to `CurrentThreatValue`, with a cap and a floor from the parameters (the floor only while
    `TargetedPlayerStates` is not empty), and a moratorium test against `LastTimeMusicTimedOut` and
    `MaxDurationOfThisMoratorium`;
  - the writes are `TimeOfNewCombatAction`, `CurrentThreatValue` and `UpdateThreatDelta`.

**Proposed Off answer:**
- the gates as read, write-free;
- past them: `NotRead`, "CombatMusicManager.PlayerDamagedByEnemy: the threat model";
- **for the hub, a possible stand-in:** the manager's state drives the music only, and nothing read on the slice reads
  it back. So a logged no-op could stand in behind an Off-only harness setting. UNVERIFIED: that no gameplay rule reads
  the threat was not searched.

**On the slice:** `CombatMusicParams` is set by section 5, so the swing reaches the threat model.

## 7. Where links 4, 13 and 14 end (the probe, with every earlier stop stubbed)

- **Link 4:**
  - its spawn passes sections 2 to 4;
  - its swing passes #799's stops, section 5 and section 6 (as a no-op);
  - **the swing then completes with no error:** arrival 18.82, applied 13.95, ratio 0.741, and the link's `ratio.` row
    appears;
  - so **with every earlier answer in, link 4 needs nothing further on this path.** Its remaining refusals are the ones
    listed, in order: the base search, sections 3 to 6, then #799's and #796's.
- **Link 13:** its three swings complete the same way, each taking 13.95 health. Maya ends at 156.9 of 212.7, not
  down, so the `down.total.` row does not appear. The hub's no-cap ruling (#796) stands: what the rules do is what the
  slice shows. **Link 13 ends on its evidence, not on a stop.**
- **Link 14:** with no down, `bleed:50` finds no injured state and does nothing; its `down.respawn_health.` row does not
  appear.
- **Link 3** (by the way): past #799's `GetParent`, the kill stops at an unbound
  `BaseBalanceDefinition.GetDisplayNameAtGrade(int)` (the kill's name text).

## 8. Tests (synthetic fixtures)

1. **`ApplyGradeCustomizations`:**
   - a toy `AIPawnBalanceDefinition` with two `PlayThroughs` and an empty behaviour list: true at playthrough 0, true at
     5 (clamped to 1);
   - a negative playthrough: false;
   - no `PlayThroughs`: false;
   - a non-pawn argument: the `NotRead`;
   - a base `BaseBalanceDefinition`: false;
   - a one-behaviour list: the behaviour runs through `RunBehaviors`.
2. **`UpdatePlayThroughMaterial`:**
   - a None `MeshMaterial`: nothing;
   - a set one: the `NotRead`;
   - no `Mesh`, or a non-AI balance: nothing.
3. **`GetCombatMusicManager`:**
   - None: a manager made from class defaults, stored, with its `CombatMusicParams` from a toy globals definition;
   - set: the same object, no writes;
   - an unloaded globals definition: `NotRead` before the creation.
4. **`PlayerDamagedByEnemy`:**
   - no params, no enemy, or a `bCausedByWorld` damage type: nothing;
   - otherwise the `NotRead` (or, under a hub-ruled setting, the logged no-op).
5. Reference: unchanged.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:**
   - the threat model past its gates;
   - the material-slot range;
   - the actor's pawn query in section 3.
3. **By fit:**
   - the playthrough virtual;
   - the behaviour runner;
   - the pawn query.
4. **For the hub:**
   - the music manager's no-op stand-in (section 6);
   - link 13's down, which the rules do not produce at these numbers (#796's no-cap ruling).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After an enemy spawns, read its `BalanceDefinitionState.bGradeCustomizationsApplied` and its mesh's materials, in
   Normal and True Vault Hunter Mode.
2. Read `GlobalCombatMusicManager` before and after the first hit, and its `CurrentThreatValue` after a few hits.
3. Count enemy melee hits until Maya goes down, at level 8, with her shield.
