# MissionTracker.AllExpansionSideMissionsComplete (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5). Analyst lane ANALYST-LINK12-EXPANSION-SIDE-MISSIONS (decision 2026-11-19-NNN, a
placeholder), for slice link 12. **Analyst only:** a different lane implements it (clean room).

**Sources:**
- the installed script, read with `research/script_disasm.py` and a local scan of every script function in Core,
  Engine, GameFramework, GearboxFramework and WillowGame for the name;
- the struct, class and enum layouts, from the cooked packages through the project's layout tool and a local read
  of the enum's value names;
- one installed value read with `ow-package --def-dump`: the slice mission's flags;
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, one headless run at a time,
  under the machine lock).

There is no bl-sdk session and no recorded live evidence on this native. Raw output stays under ignored
`local/analysis/aesmc/`. This note is in our own words: no decompiler output, pseudo-code, addresses or listing
structure. **Every rule here is UNVERIFIED** in the running game.

It restates and re-checks the rule already given in NATIVE_MISSION_OBJECTIVES.md ("MissionTracker.
AllExpansionSideMissionsComplete"); the reading there holds. New here: the link 12 path that reaches it, the slice
mission's own flags, the enum value by name, and a proposed Off answer.

**Why it matters.** On link 12, the turn-in reaches the port's existing refusal of this native (the bridge's
`n_expansion_side_complete`: false for a None controller, otherwise `NotYet { NotRead }`). It is the turn-in's
"all side missions complete" achievement check. That stop was seen on a local stack, not on main: the throwaway merge
of NATIVE_UNLOCK_ACHIEVEMENT_IF_CONDITIONS_MET.md section 5, with `UnlockAchievementIfConditionsMet` bound to false
locally. On main, links 10 to 12 stop earlier, at accept (section 4).

## 1. The script path on link 12

- **`WillowPlayerController.UpdateMissionStatus(Mission, NewMissionStatus, ...)`** finds the mission's entry in the
  controller's current playthrough (`MissionPlaythroughs[GetCurrentPlaythrough()].MissionList`). On a turn-in (an old
  status of `MS_RequiredObjectivesComplete` or `MS_ReadyToTurnIn`), it:
  1. writes the entry's `Status` to the new status first (so the mission being turned in already counts as complete
     when the native reads it), then sets `bNeedsRewards`, grants the rewards and plays the fanfare;
  2. for a mission that is **not** `bPlotCritical`, increments the player's `STAT_SIDE_MISSIONS_COMPLETED`, calls
     `UnlockAchievementIfConditionsMet` with that count (NATIVE_UNLOCK_ACHIEVEMENT_IF_CONDITIONS_MET.md), then calls
     `CheckAllSideMissionsCompleteAchievement(Mission.DlcExpansion)`.
- **`WillowPlayerController.CheckAllSideMissionsCompleteAchievement(DlcExpansion)`**, the native's only script caller in
  those five packages:
  1. picks the achievement: the expansion's `AllSideMissionsCompleteAchievement` when `DlcExpansion` is set, else the
     globals definition's `AllSideMissionsCompleteAchievement` (through `GetWillowGlobals().GetGlobalsDefinition()`);
  2. stops if that achievement is 0;
  3. takes the tracker from `WorldInfo.GRI`, cast to `WillowGameReplicationInfo`, `.MissionTracker`;
  4. only when the tracker is set, its `IsDataValid()` is true, and
     `Tracker.AllExpansionSideMissionsComplete(Self, DlcExpansion)` is true, calls
     `ClientUnlockAchievement(Achievement)`.
- **The slice mission** `GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire` (installed values): `bPlotCritical` false,
  `bRepeatable` false, `DlcExpansion` None, `MarketingUnlock` None. So it is a base-game side mission: link 12's
  turn-in reaches the native, with the controller and a None expansion.

## 2. The shapes (from the packages)

The sizes are from the project's package layout tool (`tools/ghidra/class_layout.py`).
- `MissionTracker.MissionList`: an array of `IMission.MissionData` (40 bytes each), whose first field is `MissionDef`.
- `WillowPlayerController.MissionPlaythroughs`: an array of `MissionPlaythroughData` (56 bytes each), whose first field
  is `MissionList`, an array of `IMission.MissionStatusPlayerData` (44 bytes each: `MissionDef`, then `Status`, ...).
- `MissionDefinition` fields read: `bPlotCritical`, `bRepeatable`, `MarketingUnlock` (a `MarketingUnlockDefinition`),
  `DlcExpansion` (a `DownloadableExpansionDefinition`).
- `IMission.EMissionStatus`, in the installed order: 0 `MS_NotStarted`, 1 `MS_Active`, 2
  `MS_RequiredObjectivesComplete`, 3 `MS_ReadyToTurnIn`, 4 `MS_Complete`, 5 `MS_Failed`, 6 `MS_MAX`.

## 3. What the native does

**Inputs:** the receiver (the tracker), a player controller, and an expansion definition (None for the base game).
**Output:** a bool. **Writes:** nothing. No event, no log, no load.

In order:
1. **No controller:** false.
2. **The playthrough index** is the controller's current-playthrough virtual: `WillowPlayerController.
   GetCurrentPlaythrough`'s virtual, whose rule NATIVE_PLAYTHROUGH_QUERY.md settles (the world's
   `WorldInfo.GRI.CurrentPlaythrough` for a `WillowGameReplicationInfo`, else 0). An index below 0, or not below the
   length of the controller's `MissionPlaythroughs`, gives **false**.
3. **The walk:** every record of the receiver's `MissionList`, in array order. A record whose mission is None is
   skipped. A record's mission counts only when it is a **side mission of that expansion**:
   - not `bPlotCritical`;
   - `DlcExpansion` is the given expansion (object identity; None matches None);
   - `MarketingUnlock` is None;
   - not `bRepeatable`.

   The tracker record's own `Status` and the mission's other fields are not read.
4. **For each counted mission,** the controller's entry is found in that playthrough's `MissionList`. The search is
   linear and the first entry whose `MissionDef` is the same object wins. **No entry, or a `Status` other than
   `MS_Complete` (4), gives false at once.**
5. **True** when every counted mission passed, including when none counted.

**What it does not read:** the controller's `UnloadableDlcMissionList`, any licence or ownership state of the
expansion (only the definition's identity), and the controller's other playthroughs.

## 4. The port today

- `ow-mission` `bridge.rs` `n_expansion_side_complete`, Off only: false for a None controller, otherwise
  `NotYet { NotRead }` (decision 2026-11-19-10). Its text names the two reasons, and both are now answered by reading:
  the virtual (NATIVE_PLAYTHROUGH_QUERY.md) and the whole-list walk (this note).
- The bridge's tracker holds one record, the slice's own mission. `GetAllMissions`, which walks the same list,
  refuses `NotYet` for that reason today ("MissionTracker.GetAllMissions (every record of the tracker's MissionList)").
  On main's installed gate, links 10 to 12 stop there first, at accept.
- **Why the one-record list cannot answer here:** with only the Fire mission in the list, the walk would count one
  mission, find it complete (the turn-in wrote it first) and answer **true**. In the game the list is one record per
  `MissionDefinition` in memory, with no configured list (NATIVE_MISSION_LIST_BUILD.md, #496). Which side missions that
  is at the turn-in is UNVERIFIED; the port's stand-in is the exports of the loaded packages, and whether they match
  the game's set is UNVERIFIED too. Only when the list holds other base-game side missions that are not complete is
  the answer false. A one-record answer would unlock the achievement on a state where the game, with more missions in
  memory, would not.

## 5. Proposed ParityMode Off answer (for a separate implementer)

**Bind** `WillowGame.MissionTracker.AllExpansionSideMissionsComplete` in Off (key `(object,object)`), replacing the
bridge's refusal; Reference keeps its policy.

**Write-free throughout:** it reads only, and loads nothing (a handle that needs loading is refused, as below). It
writes nothing, so its own refusals are never marked. A caller that wrote first (`UpdateMissionStatus` wrote the
entry's status) marks them.

1. **Arguments:** argument 0 an object reference or None, argument 1 an object reference or None; any other kind is
   `VmError::Script`. A set argument 1 that is not a `DownloadableExpansionDefinition` is `VmError::Script`.
2. **A None controller:** false (as today).
3. **The controller:**
   - not a `WillowPlayerController`: `VmError::Script`;
   - a subclass of `WillowPlayerController` that overrides the current-playthrough virtual: `NotYet { NotRead }`
     (NATIVE_PLAYTHROUGH_QUERY.md, section 4, the subclass refusal);
   - otherwise the index is that note's rule, computed with the port's one shared function for it (not a VM dispatch
     of `GetCurrentPlaythrough`, as NATIVE_PLAYTHROUGH_QUERY.md says for direct callers).
4. **The index range:** outside `MissionPlaythroughs`, false.
5. **The tracker's list,** as #496 (`fix/rust-mission-list-build`, the MissionList build) has `GetAllMissions` do it,
   so that the two natives that walk the list never disagree about whether it is there:
   - first, replay the bridge's stored `list_refusal` when the set-up build refused;
   - then, while `list_built` is false, refuse `NotYet { LayerAbsent }`, as #496's `n_all_missions` does ("... the
     tracker's full mission list (not built in this world)"), with this native's name in the text;
   - otherwise walk the records `crate::mission_list` built, in the stand-in's order (first insertion, the hub's
     stand-in for the name-table order, UNVERIFIED).

   **The refusal kind changes** from today's `NotRead` to `LayerAbsent`. The implementer updates #496's test
   `the_mission_list_queries_in_off_mode`, which pins this native's present answer.
6. **Each record, in order, only when reached** (the game returns at the first failure, so a later record is never
   looked at):
   - a None mission: skipped;
   - a mission handle the definition store has not loaded: `NotYet { LayerAbsent }`, naming the store's load of a
     `MissionList` mission (the walk loads nothing);
   - the four-field filter of section 3, step 3;
   - the controller entry by identity, first match; none: false; `Status` not `MS_Complete`: false. Read the value
     **by name** in the loaded `IMission.EMissionStatus`, not as the literal 4. A missing enum or name is
     `VmError::Script`.
7. **All passed:** true.

**Expected on link 12 once bound.** The call is (Maya's controller, None).
- **While the slice's world has no built list** (its bridge world today), link 12's turn-in stops at this native's
  new refusal: "stopped after partial writes: MissionTracker.AllExpansionSideMissionsComplete ... (not built in this
  world)", `NotYet { LayerAbsent }`. The prefix is the caller's, since `UpdateMissionStatus` wrote first.
- **With a built list,** the answer is the base-game side-mission rule over the slice's state. It is false only if the
  built list holds other base-game side missions that are not complete (UNVERIFIED, section 4); then
  `ClientUnlockAchievement` is not called and the turn-in goes on.
- Link 12 reaches this only after links 10 and 11 get past `GetAllMissions`, which needs the same list.

## Open questions

1. **UNVERIFIED:** overrides of the current-playthrough virtual in subclasses of `WillowPlayerController` (not read;
   the slice's controller is exactly one).
2. **UNVERIFIED:** whether the game's `MissionList` ever holds a record with a None mission (the native skips one;
   nothing here shows one exists).
3. **UNVERIFIED:** whether a controller's playthrough list can hold the same mission twice (the native takes the
   first).

## Live checks that would confirm the rule (bl-sdk, the RTSE way)

1. On a save with some Sanctuary side missions done, call `GRI.MissionTracker.AllExpansionSideMissionsComplete(PC,
   None)`. Compare it with the rule computed from the live data: every record of `MissionTracker.MissionList` whose
   mission is not plot critical, has a None `DlcExpansion` and `MarketingUnlock`, and is not repeatable, looked up in
   `PC.MissionPlaythroughs[PC.GetCurrentPlaythrough()].MissionList` for status `MS_Complete`. Expect false while any
   is not complete.
2. The same with an installed expansion's `DownloadableExpansionDefinition`, on a save with that expansion's side
   missions done and not done.
3. Hook `WillowPlayerController.ClientUnlockAchievement` and `CheckAllSideMissionsCompleteAchievement` across the turn-in
   of the Fire mission. Expect the check to run once with a None expansion, and the unlock not to be called while
   other base-game side missions remain.
4. Read the playthrough entry's `Status` inside `CheckAllSideMissionsCompleteAchievement`. Expect the Fire mission
   already `MS_Complete` there (the status is written before the check).
