# Maya's player pawn: what the game spawns it from, and what the bridge should (2026-11-26)

AI-assisted (Claude Code, Opus 5.5). Lane ANALYST-MAYA-PAWN-SPAWN-SOURCE (decision 2026-11-26-19), the analyst half
of the clean room: **another lane implements this note.** Raw output stays under ignored `local/analysis/pawnsrc/`.
This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing structure. **Every
rule here is UNVERIFIED in the running game.**

**Why.** The mission bridge makes Maya's player pawn from `WillowPlayerPawn`'s class defaults. On main e947919
(#757) that is `pawn_for` → `player_pawn_with_components` in Off (`ow-mission` `bridge.rs`): the class defaults, with
the pawn's own copy of each component the class default owns. This note reads what the game spawns her pawn from,
which properties differ, and which of them the slice's links read.

**Script or native** (review r1 of #762): `SwitchPlayerClass`, both `RestartPlayer`s (`GameInfo`'s and
`WillowGameInfo`'s), `SpawnDefaultPawnFor`, both `SetPlayerDefaults` and `ApplyCharacterClassDefaults` are script.
`Actor.Spawn` is native.

## 1. Evidence

- **bl-sdk live reads:** none. No session was granted.
- **The script** (`research/script_disasm.py`):
  - `WillowPlayerController.SwitchPlayerClass` (NATIVE_PLAYER_CLASS.md section 1);
  - `GameInfo.RestartPlayer` and `SetPlayerDefaults`, and `WillowGameInfo.SpawnDefaultPawnFor` and `SetPlayerDefaults`;
  - `WillowPlayerController.ApplyCharacterClassDefaults` and `OnPlayerClassChange`.
- **The port's code** on main 457c46e4, re-read on e947919 (#757's `player_pawn_with_components`):
  - `ow-mission` `bridge.rs` (`pawn_for`);
  - `ow-combat` `run.rs` (the `maya` step) and `run/player.rs` (`ensure_off`);
  - `ow-cli` `cmd/combat.rs` (`apply_class_inputs`);
  - `ow-world` `actors.rs` (`spawn` with a template), `world/body_composition.rs`, `world/player_pawn_tail.rs`;
  - `world/mapload/bringup.rs` (the only `begin_play` call).
- **A local probe on the older base 457c46e4** (not committed; three temporary log lines, reverted): an installed
  `--combat-run maya:8`. It showed:
  - the bridge makes its class-default pawn before the stop;
  - no spawn from a template runs;
  - that base's link 1 stop, `WillowPawn.InitializeRBPhysics`' not-loaded `NotRead`, met
    `WillowGame.Default__WillowPlayerPawn.SpecialMoveComponent0`, a class-default subobject the store had not
    loaded, on that pawn. #757 has since given the pawn its own copies.
- **Measured on main e947919** (release build, under the machine lock, review r1 of #762): `--combat-run maya:8` and the
  installed Off strict `--slice-gate` stop links 1, 3, 4, 5, 13 and 14 at **`Actor.GetBoundingCylinder`'s colliding
  bounds `LayerAbsent`**, "the colliding components' bounds"; the gate is 6 ok / 8 NotYet. #760 (in review) adds the
  spawn's component attach to the bridge pawn, which moves that stop to #738's 2.A.
- **Data** (`ow-package --def-dump`, local): `GD_Siren_Streaming.Pawn_Siren` against
  `WillowGame.Default__WillowPlayerPawn`.

## 2. What the game spawns her pawn from

1. **`SwitchPlayerClass`** finds the pawn archetype: the pawn data manager's `FindAlreadyLoadedObject` of the class's
   `PawnArchetypePath` (Maya: `GD_Siren_Streaming.Pawn_Siren`), as a `WillowPlayerPawn`. With an archetype it applies
   the class's starting values. With a pawn already, it discards that pawn's inventory (`DiscardInventory(true)`) and
   destroys it. It clears the game info's `bRestartLevel`, calls `RestartPlayer`, and then sets `bRestartLevel` back to
   the game info class's default.
2. **`WillowGameInfo.RestartPlayer`**: with more than one player, `SyncKismetSetProperties(controller)` first; then
   `GameInfo.RestartPlayer`:
   - finds a start (`FindPlayerStart`, else the controller's `StartSpot`). With neither, it returns;
   - **only when the controller has no pawn**, it asks `SpawnDefaultPawnFor`. A spawn that gives no pawn sends the
     controller to the `Dead` state (and a player controller's client to `Dead`), and stops;
   - then `SetAnchor` to the start, the start's anchored pawn cleared and `LastStartSpot` and `LastStartTime` set,
     then `Possess` (whose `PossessedBy` copies the controller's `PlayerReplicationInfo` to the pawn),
     `PlayTeleportEffect`, `ClientSetRotation`, then `SetPlayerDefaults`, then `AddDefaultInventory` (unless the world
     forbids it), the level's player-spawned events, and `SpawningProcessComplete`.
3. **`WillowGameInfo.SpawnDefaultPawnFor`**, with a controller that has a `PlayerClass`: the same archetype lookup, then
   **`Spawn(archetype's class, at the start's location and yaw, with the archetype as `ActorTemplate`)`**. So the pawn
   is made from the archetype, not from the class defaults. Then the manager is told the archetype's reference was
   applied.
4. **`WillowGameInfo.SetPlayerDefaults`** (no call to its super) calls the controller's
   `ApplyCharacterClassDefaults(pawn, class)` and then `ApplyGlobalPlayerMovementSettings`.
   `ApplyCharacterClassDefaults` reads the archetype again and hands its `BodyClass` to the pawn's `BodySelectClass`,
   which runs `ApplyBodyClassStartingValues`.

The spawn itself, once play has begun, also runs the pawn's `PostBeginPlay`, whose `BodySelectClass(BodyClass)` runs
`ApplyBodyClassStartingValues` a first time (NATIVE_INITIALIZE_BEHAVIOR_PROVIDERS.md section 1).

## 3. What the port does today (maya:8)

1. `apply_class_inputs` preloads the archetype and its same-package references, then runs the installed
   `ServerPlayerSelectClass` (covered).
2. `SwitchPlayerClass` finds the archetype and calls `RestartPlayer`. **By reading, not probed:** the headless world
   has no player start, and the controller's `StartSpot` is set only later (`down::prepare_player_pawn`). So
   `RestartPlayer` returns at its start check, and **no pawn is spawned**. The probe confirms that no spawn ran.
3. The `maya` step's health stand-in (`ensure_off`) asks the bridge for the player pawn. **The bridge makes it from
   the class defaults** (on e947919, `player_pawn_with_components`: the class defaults with its own component copies,
   not spawned and not in the attach record). Then it calls the controller's installed
   `ApplyCharacterClassDefaults(P = that pawn, class)`. That reaches `BodySelectClass(archetype's BodyClass)` and
   `ApplyBodyClassStartingValues` on that pawn. `ApplyGlobalPlayerMovementSettings` is not called on this route.
4. Play has not begun in this world (only the map load calls `begin_play`), so no spawn would run `PostBeginPlay`
   anyway.

## 4. How the archetype differs from the class default

**19 properties differ** (by name; values are not reproduced here):

| Kind | Properties |
|---|---|
| component references: the archetype names its own subobject, the class default a `Default__` subobject | `SMComponent`, `StatusEffectComp`, `DialogComponent`, `LightEnvironment`, `ArmsDynamicLightEnvironment` |
| component references: the archetype names its own subobject, the class default None | `Mesh`, `HeadMesh`, `CylinderComponent`, `CollisionComponent` |
| object references the class default leaves None | `BodyClass`, `LinkedOnDemandPackage` |
| plain values | `EyeHeight`, `BaseEyeHeight`, `RotationRate`, `MeshSlopeOffsetPercent`, `MeshBodyCompositionIndex`, `bUpdateAfterMapCommit` |
| the pool handles' GUID field | `HealthPool`, `ShieldArmor` |

**`Components` decodes the same on both:** the archetype does not tag it, so it inherits the class default's list, whose
entries name `Default__WillowPlayerPawn` subobjects. In the game the archetype's component instancing points those
entries at its own subobjects of the same names. In the port, a spawn from the archetype does the same through the
instancing plan (#435: "an inherited reference the plan stands the template's own sub-object for").

## 5. Which differences the slice's links read

Links 3, 4, 5, 13 and 14 sit behind link 1 (`spawn_maya`), so on main they stop where link 1 stops.

| Property | Read by | With the class default (today) | With the archetype | On main |
|---|---|---|---|---|
| `Components` | `InitializeRBPhysics` and `GetBoundingCylinder` (link 1, maya:8) | on e947919 the pawn's own copies of the class default's subobjects, not in the attach record: **today's stop is the bounding cylinder's colliding bounds** (on 457c46e4 it was the unloaded `Default__` handles) | the archetype's own components, copied and attached by the spawn | covered (#738's rule; the attach record) |
| `BodyClass` | `ApplyCharacterClassDefaults` → `BodySelectClass` | set from the archetype by that call | set already, and set again | covered |
| `Mesh`, `CylinderComponent` | `InitializePawnMeshes` (the body composition's `PostInitBodyComposition`, NATIVE_INITIALIZE_BODY_COMPOSITION.md section 4.5) | None: nothing to release | **set: main refuses "releasing the old Mesh"** (`body_composition::releasing_old`), stricter than its note | the refusal is on main; the attach-record rule is ruled (6.3); **the global list is not read** |
| `HeadMesh`, `CollisionComponent`, `MeshBodyCompositionIndex` | the same build, which writes them | overwritten | overwritten | covered |
| `ArmsDynamicLightEnvironment` | `ApplyBodyClassStartingValues`' player part (`Arms.SetLightEnvironment`) | a `Default__` handle | the archetype's own copy | after #738's stops: not reached on main |
| `SMComponent`, `StatusEffectComp`, `DialogComponent`, `LightEnvironment` | special moves, status effects, dialog, the mesh's light (links 3, 4, 9, 13, 14 later) | `Default__` handles, not loaded | the archetype's own copies | not reached on main |
| `EyeHeight`, `BaseEyeHeight` | the pawn's view location (by fit, the shot origin on links 3 and 4) | the class default's | the archetype's | not reached on main; whether the port's fire reads them was not checked |
| `HealthPool`, `ShieldArmor` (the GUID field) | the pool handles | written by `SetHealthResourcePoolDefinition` and `CreatePool` before any read | the same | covered |
| `RotationRate`, `MeshSlopeOffsetPercent`, `bUpdateAfterMapCommit`, `LinkedOnDemandPackage` | none of the slice's links, as far as read | | | unread |

## 6. The answer for the port

**Yes: in Off the bridge should make Maya's pawn from her archetype**, as `SpawnDefaultPawnFor` does. Two routes:
- **A. The game's own route:** give the controller a start before the class switch, so `RestartPlayer` spawns,
  possesses and sets defaults itself. It is the faithful route, but it pulls in `RestartPlayer`'s unread tail
  (`Possess`, `AddDefaultInventory`, the player-spawned events, `SpawningProcessComplete`). Each of those is a new stop.
- **B. The bridge spawns it (recommended first):** when the controller has a `PlayerClass`, `pawn_for(Player)` **replaces
  `player_pawn_with_components` with `ow_world::actors::spawn`, the archetype as the template.** The spawn plans the
  component copies from the archetype itself (its source), which is what `player_pawn_with_components` does from the
  class default, and it also runs the component update, which publishes each copy to the attach record
  (`component_attach.rs`, the spawn's apply). So B does not feed the archetype's plan to
  `player_pawn_with_components`. Everything else stays as today, including `ensure_off`'s
  `ApplyCharacterClassDefaults`. Without a `PlayerClass` (the mission links' touches), `player_pawn_with_components`
  stays.
  - **What B skips** that the game runs: `WillowGameInfo.SetPlayerDefaults`' `ApplyGlobalPlayerMovementSettings`, and
    the rest of `RestartPlayer` (`Possess`, `AddDefaultInventory` and the others in 2).
- Route A's new stops, each not read: `SyncKismetSetProperties` (more than one player only), `SetAnchor`,
  `PlayTeleportEffect`, `ClientSetRotation`, `Possess`, `ApplyGlobalPlayerMovementSettings`, `AddDefaultInventory`, the
  player-spawned events, `SpawningProcessComplete`, and the `Dead` state on a spawn that gives no pawn.

**The Off answer for B, per step:**
1. **Preflight**, write-free and load-free:
   - the controller's `PlayerClass` set, and its `PawnArchetypePath` a string. Unset: today's class-default pawn;
   - the archetype found by the pawn data manager's routine (`ow-world` `pawn_data.rs`, `FindAlreadyLoadedObject`),
     resident and a `WillowPlayerPawn`. Not found or not resident: `NotYet { NotRead }`, "the player pawn's archetype
     is not loaded (a load is a write)". The harness preload already loads it;
   - the spawn's own preflight (the instancing plan, the component update's plan), as `spawn` runs it.
2. **The spawn:** the archetype's class at the bridge's place (the world origin, as today's pawn), with
   `Controller` set afterwards as `pawn_for` does today. A refusal after the actor is made carries the spawn's
   "stopped after partial writes" mark (its post-insertion boundary). No begin-play events run (play has not begun).
3. **The old `Mesh` and `CylinderComponent`.** The archetype pawn arrives with both set, but **not attached**: neither is
   in its `Components` list, so the spawn's component update does not attach them.
   - NATIVE_INITIALIZE_BODY_COMPOSITION.md already says what to do: section 4.5 item 1 (the release detaches only an
     attached component, and removes it from a global list, not read), and its preflight ("an old field that is None,
     or set but not attached, releases nothing").
   - **Main's `body_composition.rs` is stricter than its own note.** Its two `old_set` tests refuse "releasing the old
     Mesh / CylinderComponent" whenever the field is set.
   - So this step brings the code in line with the note. **The hub has ruled yes** (on #762): the release refuses only
     when `attached_to` shows the old component attached to the pawn; otherwise nothing is released and the new one is
     written. UNVERIFIED in the game.

## 7. Predicted effect (a prediction, not a run)

For link 1, and with it links 3, 4, 5, 13 and 14:
1. **today (measured on e947919):** `Actor.GetBoundingCylinder`'s colliding-bounds `LayerAbsent`, on the bridge's
   class-default pawn, whose component copies are not in the attach record. With #760 (the spawn's component attach
   for that pawn), the stop moves to #738's 2.A;
2. **with B, before the code follows 6.3:** earlier, in `InitializePawnMeshes`, `NotYet { NotRead }`
   "InitializeBodyComposition: releasing the old Mesh";
3. **with B and the code following 6.3:** the build passes. The spawn has published the archetype's component copies
   to the attach record (`component_attach.rs`, the spawn's apply), as #760 does for the bridge pawn. So
   `GetBoundingCylinder` and `InitializeRBPhysics` meet attached, resident components, and the answer is **#738's
   physics `LayerAbsent`** (NATIVE_LINK1_RBPHYSICS_AND_AFTER.md 2.A): the same stop #760 reaches, from the archetype;
4. later steps read the archetype's component copies (special moves, status effects, the arms' light environment)
   instead of unloaded `Default__` handles.

No link's count is predicted to change until link 1 passes 2.A.

## 8. Tests (synthetic fixtures)

1. A toy player class whose `PawnArchetypePath` names a loaded toy archetype with an int field 7: the bridge's player
   pawn has 7, its archetype is the toy archetype, and it is in the level list.
2. The archetype not loaded: the not-loaded `NotRead`, no pawn made, and the store not asked to load.
3. No `PlayerClass`: today's class-default pawn.
4. An archetype whose `Components` inherits the class default's subobject references: the spawned pawn's entries are
   copies of the archetype's own same-named subobjects.
5. **The release ruling (if made):** an old `Mesh` that is set but not in the attach record is replaced, with nothing
   detached; one in the attach record keeps the refusal.

## Open questions

1. **UNVERIFIED:** every rule above.
2. **Not read:**
   - the global list the old-mesh release removes from;
   - why `RestartPlayer` makes no pawn in the port (read as the missing start, not probed);
   - `RestartPlayer`'s tail for route A;
   - whether the port's fire reads `EyeHeight`.
3. **The hub's call:** route A or B. The release of 6.3 is ruled (the attach record decides).
4. **A conflict to settle** (review r1 of #762): `Default__WillowPlayerPawn.CylinderComponent` decodes None, and
   `Components[0]` is None on both the archetype and the class default. NATIVE_INITIALIZE_BODY_COMPOSITION.md section
   4.5 item 2 says stock `Pawn` defaults give the pawn a cylinder, so the release runs on a first build. Either the
   decoder misses an inherited value, or that note's line is wrong for this class.

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. After Maya spawns, read her pawn's `ObjectArchetype`: expect `GD_Siren_Streaming.Pawn_Siren`.
2. Read `Mesh`, `CylinderComponent` and `SMComponent` and their `Outer`: expect her pawn's own copies.
3. Read `EyeHeight` and `BaseEyeHeight`: expect the archetype's values.
