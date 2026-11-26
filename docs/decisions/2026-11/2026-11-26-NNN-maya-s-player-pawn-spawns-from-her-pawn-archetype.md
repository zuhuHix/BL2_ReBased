## 2026-11-26: Maya's player pawn spawns from her class's pawn archetype; the old mesh release follows the attach record

AI-assisted (Claude Code, Claude Opus 5.5). Lane IMPL-MAYA-PAWN-FROM-ARCHETYPE, the implementer half of the clean
room. Lane F wrote NATIVE_MAYA_PAWN_SPAWN_SOURCE.md (PR #762), and this lane read it from its branch while #762 was in
review; it did not read the executable. On main 5f43a0b9 (#760, decision 2026-11-26-17, landed). No bl-sdk session
was granted. **Every rule is UNVERIFIED in the running game.**

**What changed, Off only** (route B of the note's section 6, the hub's pick). Reference is unchanged.
- **The bridge's player pawn.** When the controller has a `PlayerClass`, `pawn_for` spawns the pawn through the world's
  spawn (`ow_world::actors::spawn`) with the class's pawn archetype as `ActorTemplate`. That includes the spawn's own
  instancing plan and component update, so the archetype's own components are copied and attached. The spawn runs at
  the bridge's `WorldInfo` with no collision placement asked; play has not begun, so no begin-play event runs. The
  preflight is write-free and load-free:
  - the class definition is resident and its `PawnArchetypePath` is a string;
  - the pawn data manager's lookup (`FindAlreadyLoadedObject`, now also `pawn_data::find_already_loaded` for callers
    outside the native) finds a resident `WillowPlayerPawn` at that path.
  Otherwise the answer is `NotYet { NotRead }`, "the player pawn's archetype is not loaded (a load is a write)". The
  heap's archetype field (decision 2026-11-26-14) is set to the archetype. `Controller` is set afterwards, as before,
  and `ensure_off`'s `ApplyCharacterClassDefaults` is unchanged. With no `PlayerClass` the pawn stays the
  class-default one (decisions 2026-11-26-16 and -17).
- **The hub's release ruling** (the note's section 6.3, following the game). In `InitializeBodyComposition`'s
  `PostInitBodyComposition` step, an old `Mesh` or `CylinderComponent` that the attach record does not hold under the
  pawn is released with nothing to detach, and the new one is written. One the record holds keeps "releasing the old
  Mesh" / "... CylinderComponent" (`NotYet { NotRead }`; the detach and the global list are not read). The ruling names
  `Mesh`. It is applied to `CylinderComponent` too: the note's 6.3 covers both, and the archetype pawn arrives with
  both set. A second attachment of either in one build keeps the refusal.

**Tests** (the note's section 8):
1. a toy player class whose archetype holds `ToyEye` 7: the spawned pawn has 7, its archetype on the heap, a place in
   the level list, and the controller;
2. an archetype nothing has loaded: the `NotRead`, with no pawn made and nothing loaded;
3. no `PlayerClass`: the class-default pawn (the existing bridge tests);
4. the archetype's inherited `Components` entry: a copy of its own sub-object, outered to the pawn and attached;
5. the release ruling: an attached old component keeps the refusal, and one not attached is replaced.

The bridge's toy `Actor` and `WorldInfo` now declare the fields the world's spawn reads and writes.

**Measured** (installed Off strict `--slice-gate`, release build, under the machine lock). Before (main 5f43a0b9)
and after: 6 ok / 8 NotYet, 47.0%. Every row is unchanged, the stop texts included. Links 1, 3, 4, 5, 13 and
14 still stop at 2.A's "WillowPawn.InitializeRBPhysics: a component's rigid body" (`LayerAbsent`), and links 8 to 12
stay ok (the mission links 10 to 12 included).

A local probe (temporary log lines, reverted) showed two pawns made in one gate run. The combat run's pawn (links 1, 3,
4, 5, 13 and 14) is now spawned from the archetype. The other, by inference the mission links' world, whose
controller has no `PlayerClass`, is the class-default pawn; the probe did not show which world made it. So the
archetype route reaches the note's predicted stop (section 7, item 3), the same 2.A `LayerAbsent` the class-default
pawn reached after #760, now on the archetype's own components.

**Review of #760's LOWs** (a separate commit, on the class-default pawn of decisions 2026-11-26-16 and -17):
1. a failure of the component attach's writes now frees the pawn and its copies, and drops any attach-record entries
   already published for it, as the earlier failure paths free them. The pawn is not cached and nothing has
   registered it, unlike the spawn's actor, which stays once it is in the level list;
2. a test of a refusal from the attach's own preflight: the class default's sub-object with `bAttached` set is its
   `NotRead`, with no pawn made or cached.

The reviewer of #760 noted that the release rule above is already in NATIVE_INITIALIZE_BODY_COMPOSITION.md section
4.5 and its preflight. The hub's ruling brings `body_composition.rs` into line with that note.
