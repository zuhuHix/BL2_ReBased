## 2026-11-19: Read the mission list build as analyst

AI-assisted (Claude Code / Claude Opus 5.5).

Lane ANALYST-MISSIONLIST-BUILD, analyst only, for slice links 10 to 12. Every two-digit number of 2026-11-19 is taken,
so `NNN` is a placeholder until the maintainer rules on three-digit numbers; the decisions check refuses it until then.

Stacked on `fix/rust-link10-mission-list` (#486, 3bb3f6de). Links 10 to 12 stop in `accept` at the mission bridge's
`MissionTracker.GetAllMissions` `NotYet` (layer absent): the bridge holds one record, not the game's list. This lane
asked what builds that list in the game.

The new own-words note `docs/verification/NATIVE_MISSION_LIST_BUILD.md` and one `NATIVE_INDEX.md` row record, as
behaviour only:
- **Not script.** No UnrealScript function builds the list or spawns the tracker, and no placed tracker stands in the
  Sanctuary packages.
- **The build** is a native virtual of the tracker. It gives one zeroed record to every `MissionDefinition` in memory.
  On each definition object it clears `bGameStageLocked` and sets `MissionNumber` to -1 and `GameStage` to 0. It keeps
  the list ordered by the definition's name as the engine's name handle compares it: by the global name-table entry,
  then the instance number, not alphabetically.
- **The numbering** runs right after the build, per `DlcExpansion` group, from the expansion's `MissionNumberBase`
  (0 for the base game):
  - the main chain first: the first plot-critical mission with no dependencies, then its `NextMissionInChain` links;
  - then side missions, filed under the highest story number they depend on and ordered by their dependencies;
  - side missions with no plot-critical mission behind them stay at -1.
- **When it runs** is not settled: the engine step that calls the virtual was not found.
- **Off answers, as recommendations for the hub:**
  - the build happens once, at world set-up, as a labelled port choice;
  - a declared, preloaded definition set, or else `NotYet` (layer absent) "the in-memory mission definition set";
  - no lazy loads (a lazy load is a write);
  - preflight of the declarations and of cycles before the first write;
  - order-dependent answers are `NotYet` (layer absent) "the engine's name-table order of mission names".

**Correction:** NATIVE_MISSION_OBJECTIVES.md read the lookup's comparison as ordinary name ordering without opening it.
A pointer to the name-table order is added there in place.

**Clean room.** Nothing is implemented here. The raw analysis stays under ignored `local/analysis/`. No
real-game session took place, so every rule is **UNVERIFIED**; the note lists five bl-sdk checks that would confirm
it. No code, test, dependency or parsing change.

**Links 10 to 12 (reading at 3bb3f6de):** link 10 drives 3 of 8 steps and stops at the accept on `GetAllMissions`
`NotYet` (layer absent); links 11 and 12 are blocked behind it. The next stop is a hub ruling on the build's moment
and on the declared definition set, not another native.
