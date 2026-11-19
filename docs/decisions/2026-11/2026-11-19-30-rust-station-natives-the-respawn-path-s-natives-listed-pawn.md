## 2026-11-19: Rust station natives: the respawn path's natives listed, Pawn.IsDead in Off, the analyst note

AI-assisted (Claude Code). Lane FIX-STATION-NATIVES (slice link 14, bleed out and respawn; SLICE_RUN_PLAN.md
section 3.7, "the station natives respawn selection needs", #361's open item). Number 30 was given by the hub. No
change to parsing. Nothing here was checked in the running game: every rule is **UNVERIFIED** there.

- **The natives the respawn reaches.** They were listed from the installed script (`research/script_disasm.py` on
  `ResurrectPlayer`, `ResurrectAtLocation`, `GetBestPlayerPlacementPoint`, the three `CanResurrectHere`,
  `GetNextExitPoint`, `GetBestPlayerStartPoint` and the registration functions; the listings stay under ignored
  `local/p2/fix-station-natives/`). They were checked against the Off `--combat-run` world after its mission bridge
  is built (a temporary print, not kept) and against the stubs the Reference S-f run reports. The selection itself is
  script and reaches only bound operators and the interpreter's `AllActors`. The rest of the path, the arrival's
  registration included, reaches about twenty unbound natives (NATIVE_RESPAWN_STATIONS.md, section 2).
- **Implemented in Off, from a record:** `Pawn.IsDead` is the actor's `bDeleteMe` or the pawn's `bIsDead`
  (NATIVE_DOWN_TIMERS.md), in `crates/ow-combat/src/down.rs` next to `Pawn.IsInjured`. That file is the down
  state's, not the hit path. The C++ binds none, so Reference keeps the stub policy (R3). The synthetic fixture
  (`run/fixture.rs`) gains the two flags and the native declaration.
- **Recorded but not implemented:** `Actor.CollidingActors` (NATIVE_ENGINE_CORE.md) needs the world's collision
  query, which the VM world does not have, and an engine iterator the interpreter does not run. It stays unbound.
- **Analyst** (NATIVE_RESPAWN_STATIONS.md, new; two read-only Ghidra queries, field names from `class_layout.py`):
  - read: `Controller.IsUsingVehicleEx`, `WillowGlobals.GetEffectCoordinator`, `WillowEffectCoordinator.HasEffect`,
    `RecordCharacterRevivedEventForPlayer`;
  - read in part: `PopEffect`, `AddStationToNameListInOrder`;
  - not readable here: `GetTravelStationDefinition` (all three declarations), `Controller.IsDead`, `AddFastTravelLocation`,
    `IsStationInList` and the pawn virtual `IsUsingVehicleEx` asks. Each forwards to a native virtual, and their
    classes have no RTTI names, so the vtables were not found;
  - not read: `MissionTracker.FastTravelStationRegistered`, a mission-objective routine too deep for this lane.
  Every one stays unbound in Off for a fresh implementer, or for a live read. Nothing this lane analysed is
  implemented here.
- **Proof on the installed game** (desktop, 2026-11-19, Off; outputs under ignored `local/p2/fix-station-natives/`):
  - `--slice-gate`: before and after, all 14 rows identical. Link 14 (`bleed_out_respawn`) stops at step `maya:8` on
    the unbound `WillowGlobals.GetPlayerPawnDataManager()`, link 1's stop, before any down step runs. Links 3, 4, 5
    and 13 stop there too. Links 10 to 12 stop at `AIComponent.ActivateEvent`. Links 2 and 6 to 9 have no driver.
  - `--combat-run` S-f (`maya:8 players:1 spawn:adult:7 cash:3000`, four melee swings, `bleed:50`): before and after,
    `maya` stops at the same native, the spawn at `TargetableAttributeValueResolver`, and the swings find no pawn. No
    respawn is reached, so `Pawn.IsDead` and the station natives are not reached in Off yet.
  - Reference S-f, before and after: 17 of 17 rows and the stub list identical. It still respawns at the
    controller's `StartSpot`, because its world has no stations.
- **Verified on invented data:** `ow-combat` 136 passed (one new test: Off answers by the two flags, false for a
  downed pawn, and Reference binds no `IsDead`).
- **Open:**
  - Link 14 needs link 1 first: `GetPlayerPawnDataManager`, and the spawn's `TargetableAttributeValueResolver`.
  - Then a respawn with stations needs a map world that spawns Sanctuary's stations (`stations()`, D60) and the
    unbound natives above, in the order of the note's section 1.
  - The live reads D60 names (`GetTravelStationDefinition` on each station) would settle the virtuals.
  - `SetLocation` and `SetRotation` have no record.
