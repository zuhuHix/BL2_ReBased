## 2026-11-19: MissionTracker.AllExpansionSideMissionsComplete in Off from the analyst note

AI-assisted (Claude Code). Lane IMPL-ALL-EXPANSION-SIDE-MISSIONS, implementer (clean room). This lane coded the native
from `docs/verification/NATIVE_ALL_EXPANSION_SIDE_MISSIONS_COMPLETE.md` alone (analyst lane
ANALYST-LINK12-EXPANSION-SIDE-MISSIONS, #550 at 690ae348) and the hub's ruling on #550's review. This lane did not read
the executable. There was no real-game session, so every rule stays **UNVERIFIED** in the running game. No dependency
or crate edge was added and Reference is unchanged. The lane is based on #496 (`fix/rust-mission-list-build` @
9b591ada), which builds the tracker's list at set-up. The decision number is a placeholder.

- **`ow-mission` `side_missions.rs` and the bridge binding**, Off only, write-free, loading nothing:
  - arguments: each an object or None (`VmError::Script` otherwise); a set expansion must be a
    `DownloadableExpansionDefinition`;
  - a None controller is false;
  - a controller that is not a `WillowPlayerController` is `VmError::Script`; a strict subclass is
    `NotYet { NotRead }` (whether it overrides the current-playthrough virtual is not read);
  - the playthrough index is the bridge's current playthrough (on this base, its replication object; #497 replaces
    the controller virtual with the world rule, and the two should share that function once both land); an index
    outside `MissionPlaythroughs` is false;
  - **the list (the hub's ruling on #550's review):** the set-up build's refusal is replayed unchanged; while no
    list was built, `NotYet { LayerAbsent }`, "MissionTracker.AllExpansionSideMissionsComplete (every record of the
    tracker's MissionList)", layer "the tracker's full mission list (not built in this world)", as `GetAllMissions`
    answers. The kind changes from the earlier `NotRead` to `LayerAbsent`;
  - the walk over `crate::mission_list::missions` in its stand-in order (UNVERIFIED): a None mission skipped; an
    unloaded handle `NotYet { LayerAbsent }`; the four-field side-mission filter; the first playthrough entry by
    identity must be `MS_Complete`, read by name in the enum the entry's `Status` field declares; the first failure
    answers false; all passed is true.
- **UNVERIFIED:** whether the built list equals the game's in-memory `MissionDefinition`s (the build's stand-in for
  the game's list, decision of #496), and so whether the answer matches the game's on a real save.
- **Tests:** 7 rule tests on invented classes (a toy enum with `MS_Complete` at another position, so a pass proves the
  by-name read); 1 new bridge test (the replayed refusal equals `GetAllMissions`'s; the `LayerAbsent`, write-free);
  #496's `the_mission_list_queries_in_off_mode` now expects the walk's false. The bridge fixture's
  `MissionDefinition` gains `bRepeatable` and `MarketingUnlock`.
- **The slice** (installed, Off, strict, on a local merge with #546, #533, #497, #509, #512, #524 and #526, never
  pushed): 3 ok / 1 NotYet / 10 errors. Links 10 and 11 ok. Link 12 passes 1 of 6 driver steps (`use`) and gets past
  this native to the unbound `WillowPlayerController.RefreshBalanceDataFromMissionCompletion(object)` in `turnin`.
