# Native rules of the mission tracker's record queries: the active set, the mission list, the expansion check (draft, 2026-11-19)

AI-assisted (Claude), analyst part of lane FIX-MISSION-OFF (decision 2026-11-19-05). Read from the game executable with
Ghidra (`tools/ghidra/run.ps1 -ReadOnly`), written in our own words. No decompiler output, pseudo-code, addresses or
listing structure is reproduced here. Field names and record sizes come from the cooked packages
(`tools/ghidra/class_layout.py`). **Every rule is UNVERIFIED in the running game**. Each section ends with the live
observation that would confirm it.

**Clean room.** This lane implemented `MissionTracker.GetObjectivesProgress` from NATIVE_SLICE_OFF_STOPS.md *before* it
opened the executable, and it does not implement anything in this note. In the Rust port these three natives stay
unbound in Off, so the gameplay stub policy refuses them. Under Reference they keep the C++ stub. They stay that way
until a fresh implementer codes them from this note alone.

**Implemented (2026-11-19, lane FIX-MISSION-OBJECTIVES, decision 2026-11-19-10),** by a fresh implementer from this
note alone, in Off only (Reference keeps the C++ stubs):
- `GetActivePrimaryObjectiveSet`, in the bridge's terms below: the bridge's mission answers its active set, any other
  mission None.
- `GetAllMissions`: `NotYet { LayerAbsent }`. The rule is settled, but it walks the tracker's whole `MissionList` and
  the bridge holds one record; one entry would tell the turn-in's playthrough checks that it is the game's whole list.
- `AllExpansionSideMissionsComplete`: false for a None controller; otherwise `NotYet { NotRead }` (the controller
  virtual behind the playthrough index, and the same whole-list walk).

## Summary

| Native | Script signature (from the package) | Reached on the Fire route | Confidence | Status |
|---|---|---|---|---|
| `MissionTracker.GetActivePrimaryObjectiveSet` | `(MissionDefinition)` returning an objective set (key `(object)`) | the controller's `SetPlayersMissionProgressToWorldsMissionProgress` and `IsEligibleForMission`; the next Off stop of `--mission-run` and `--slice-run` | high | UNVERIFIED |
| `MissionTracker.GetAllMissions` | `(out array<MissionDefinition>)` (key `(array)`) | the turn-in's `CheckPlaythroughComplete` / `CheckPlaythroughAlmostComplete` (and the status menu) | medium-high | UNVERIFIED |
| `MissionTracker.AllExpansionSideMissionsComplete` | `(WillowPlayerController, DownloadableExpansionDefinition)` returning bool (key `(object,object)`) | the turn-in's `CheckAllSideMissionsCompleteAchievement` | medium | UNVERIFIED |

## The record lookup the first one shares

The tracker's records are `MissionTracker.MissionList`, an array of `IMission.MissionData` (40 bytes each: `MissionDef`,
`Status`, `ObjectivesProgress`, `ActiveObjectiveSet`, `SubObjectiveSets` and three flags). The lookup of a mission's
record is a **binary search** of that array:

- The list is kept **sorted by the mission object's name**. The search compares the probed record's mission name with
  the wanted mission's name, and stops on a record whose `MissionDef` *is* the wanted object (identity, not name).
  The name comparison routine itself was not opened; it is read as the engine's ordinary name ordering.
  - **Correction (2026-11-19, NATIVE_MISSION_LIST_BUILD.md):** the comparison, opened there, orders names by their
    entry in the engine's global name table, then by instance number, not alphabetically. The build that fills the
    list uses the same key.
- A None mission, an empty list or no matching record gives "no record".
- Consequence for a port that holds one record (the bridge): "no record" for every mission but its own; the order rule
  matters only once the tracker holds the whole list.

## MissionTracker.GetActivePrimaryObjectiveSet

- **Inputs:** a mission definition.
- **Does:** finds the mission's record (the lookup above) and returns that record's `ActiveObjectiveSet`.
- **No record:** None. Nothing else is read or changed; there are no side effects.
- **What "primary" means here:** only the record's `ActiveObjectiveSet` field is returned; `SubObjectiveSets` is not
  consulted.
- **For an implementer, in the bridge's terms:** the bridge's own mission answers the tracker's active set as an object
  (None when the mission has no active set, as before the kickoff), and any other mission answers None.
- **Live confirmation:** with bl-sdk, call it on the Fire mission before acceptance, right after `ActivateMission`,
  after the kickoff dialog and after the range objective, and compare with the `ActiveObjectiveSet` of the record in
  `MissionList` and with the set the HUD shows (DISPATCH note, "In the original game": the set is expected to be None
  until the kickoff ends).

## MissionTracker.GetAllMissions

- **Inputs:** an out array of mission definitions.
- **Does:** appends the `MissionDef` of every record of `MissionList`, in list order (so in the name order above), to
  the array. Each record gives one entry, a None `MissionDef` included.
- **The array is not emptied first:** the native writes into the caller's variable and grows it. Every script caller
  seen on the route passes a fresh local, so this shows only if a caller reuses a filled array. Medium confidence: this
  rests on how the out-parameter storage is handed to the native, which is the engine's general mechanism and not
  specific to this function.
- **Live confirmation:** call it with an empty array and with an array holding one invented entry, and compare the
  length with `MissionList`'s; check that the order is the name order.

## MissionTracker.AllExpansionSideMissionsComplete

- **Inputs:** a player controller and an expansion definition (`DownloadableExpansionDefinition`; None for the base
  game).
- **Result false** when the controller is None, or when the controller's playthrough index is outside its
  `MissionPlaythroughs` array. The index comes from a virtual function of the controller. That virtual was not
  resolved by name; it is read as the current playthrough (`GetCurrentPlaythrough`), not proven.
- **Then, for every record of the tracker's `MissionList`** whose mission is set and is a *side mission of that
  expansion*, the controller's list must show it complete. A side mission of that expansion is one that:
  - is not `bPlotCritical`;
  - has `DlcExpansion` equal to the given expansion (identity; None matches None);
  - has no `MarketingUnlock`;
  - is not `bRepeatable`.
  Each such mission must have an entry in that playthrough's `MissionList` (`IMission.MissionStatusPlayerData`, found
  by `MissionDef` identity, in list order) whose `Status` is 4 (`Complete`). A missing entry or any other status gives
  **false** at once.
- **Result true** when every such mission passed, including when there is none.
- **Side effects:** none.
- **Live confirmation:** on a save with some Sanctuary side missions done, call it with the controller and None, and
  compare with a count of the base-game side missions (not plot critical, no marketing unlock, not repeatable) whose
  status is Complete in `MissionPlaythroughs[current].MissionList`. Then the same on a save where they are all done.

## Not read

- The name comparison routine behind the record lookup.
- The controller virtual that gives `AllExpansionSideMissionsComplete` its playthrough index. Settled in
  NATIVE_PLAYTHROUGH_QUERY.md (UNVERIFIED): it is `WillowPlayerController.GetCurrentPlaythrough`'s virtual.
- What keeps `MissionList` sorted (the insertion routine); this note relies on it being sorted, as the search requires.
