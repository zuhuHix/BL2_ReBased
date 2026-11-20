# WillowPlayerController.RefreshBalanceDataFromMissionCompletion (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5). Analyst lane ANALYST-REFRESH-BALANCE-DATA-FROM-MISSION-COMPLETION (decision
2026-11-19-170, a placeholder), for slice link 12. **Analyst only:** a different lane implements it (clean room). This
lane's author implemented `MissionTracker.AllExpansionSideMissionsComplete` (#554), not this native, and will not
implement this note.

**Sources:**
- the installed script, read with `research/script_disasm.py` and a local scan of every script function in Core,
  Engine, GameFramework, GearboxFramework and WillowGame for the name;
- the class and struct layouts, from the cooked packages through the project's layout tool;
- installed values read with `ow-package --def-dump`: the globals definition's `RegionBalanceData` and the four
  first-playthrough balance definitions;
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, one headless run at a time
  under the machine lock).

There is no bl-sdk session and no recorded live evidence on this native. Raw output stays under ignored
`local/analysis/rbd/`. This note is in our own words: no decompiler output, pseudo-code, addresses, offsets or listing
structure. **Every rule here is UNVERIFIED** in the running game.

**Why it matters.** With #554 on #546's stack, link 12's turn-in gets past `AllExpansionSideMissionsComplete` and
stops at this native, which the port does not bind.

## 1. The script path and its callers

- **The only script caller** in the five packages is `WillowPlayerController.UpdateMissionStatus`, in its "complete"
  case (an old status of `MS_RequiredObjectivesComplete` or `MS_ReadyToTurnIn`, a new status of `MS_Complete`). After
  the turn-in's status write, rewards, fanfare, the side-mission statistic and achievements
  (NATIVE_UNLOCK_ACHIEVEMENT_IF_CONDITIONS_MET.md, NATIVE_ALL_EXPANSION_SIDE_MISSIONS_COMPLETE.md), the optional
  objectives statistic and `CheckForSlaughterAchievement`, it calls
  `RefreshBalanceDataFromMissionCompletion(Mission)` as a statement. Its result is not used.
- **Declaration:** a native, final, non-static function of `WillowPlayerController`, one object parameter (the
  mission). No net flag: it runs where `UpdateMissionStatus` runs.
- **After it**, `UpdateMissionStatus` goes on (script) with `CheckPlaythroughComplete(<playthrough>)` and, when that
  is false, `CheckPlaythroughAlmostComplete(<playthrough>)`. Both are script functions that walk
  `GetAllMissions` for plot-critical missions. Then come the "changed" tail's
  `WillowLeviathanService.RecordMissionStatusChangedEventForPlayer` (a native) and `UpdateLcdMissionStatus`.

## 2. The shapes (from the packages)

- `MissionDefinition.DlcExpansion` (a `DownloadableExpansionDefinition`).
- `GlobalsDefinition.RegionBalanceData` and `DownloadableExpansionDefinition.RegionBalanceData`: arrays of
  `GlobalsDefinition.PlayThroughData` (`PlayThroughNumber` int, `BalanceDefinitions` array of `GameBalanceDefinition`).
- `GameBalanceDefinition.BalanceByRegion`: an array of `GameBalanceDefinition.RegionBalanceData` (`Region`, a
  `RegionDefinition`; the default stage fields; `MissionOverrides`, an array of
  `GameBalanceDefinition.MissionStateBalanceAdjustment`, whose first field is `Mission`).
- `WillowPlayerController.RegionGameStages`: an array of `PlayerSaveGame.RegionGameStageData` (`RegionDef`,
  `GameStage`, `PlaythroughIdx`). It is the controller's cache of region game stages (NATIVE_PROGRESSION.md, "Region
  game stage", step 4), loaded from and saved to the save game (`ApplyGameStageData`, `SaveGameStageData`).

## 3. What the native does

**Inputs:** the receiver (the controller) and the completed mission. **Result:** a bool (true when an override names
the mission); the caller ignores it.

1. **The globals:** the game's `WillowGlobals` and its globals definition. With either missing, false; nothing more.
2. **The table:** the mission's `DlcExpansion.RegionBalanceData` when `DlcExpansion` is set, else the globals
   definition's `RegionBalanceData`. The mission is read with no None test: the game assumes a mission.
3. **The playthrough entry:** the controller's current-playthrough virtual (the same virtual
   NATIVE_PLAYTHROUGH_QUERY.md settles) gives an index into the table **by position**. `PlayThroughNumber` is not
   compared. An index not below the table's length gives false. A negative index trips the engine's array check (not
   a case the virtual produces). **This is not the clamped lookup** of NATIVE_PROGRESSION.md, "Region game stage"
   (step 3, "clamped to the table"): past the end this native answers false and searches nothing, so an implementer
   must not reuse that clamp here (review r1 of #559).
4. **The search**, in array order: every `BalanceDefinitions` entry of that playthrough entry (read with no None
   test), every `BalanceByRegion` entry of it, and every `MissionOverrides` entry of that. An override whose
   `Mission` is the completed mission (object identity) makes the result true and resets that entry's `Region`
   (step 5) at once, before the search goes on. The search does not stop at the first match. **One reset runs per
   matching override, in search order, duplicates included:** two overrides naming the mission for the same region
   reset that region twice (each reset removes at most one record). Nothing collects or de-duplicates the regions
   (review r1 of #559).
5. **The reset of a region** (the same routine as `WillowPlayerController.ResetGameStageForRegion`): walk
   `RegionGameStages` in order. The first record whose `RegionDef` is that region **and** whose `PlaythroughIdx` is
   the current playthrough (the virtual again) is removed from the array, and the reset stops. A record of that
   region for another playthrough is passed over. No such record: nothing.
6. **Nothing else is written.** No stage is recomputed here. The effect is that the next game-stage query for that
   region and playthrough finds no stored stage and computes it afresh (NATIVE_PROGRESSION.md, "Region game stage",
   steps 4 and 5), now with the mission complete.

## 4. What the slice's turn-in takes

- The mission `GD_Z1_RockPaperGenocide.M_RockPaperGenocide_Fire` has a None `DlcExpansion`, so the table is the
  globals definition's (`GD_Globals.General.Globals`).
- Its `RegionBalanceData` has two entries: `PlayThroughNumber` 1 with four balance definitions
  (`GD_GameStages.Balance.Balance_P1_Zone1`, `…_Zone1_Interlude`, `…_Zone2`, `…_Zone3`) and `PlayThroughNumber` 2
  with four more.
- At playthrough index 0 those four hold 48 region entries and 78 mission overrides. **None names the Fire mission.**
  So on the slice the native answers false and writes nothing, whatever `RegionGameStages` holds.

## 5. Proposed ParityMode Off answer (for a separate implementer)

**Bind** `WillowPlayerController.RefreshBalanceDataFromMissionCompletion(object)` in Off, first-wins; Reference keeps
its stub policy. Every rule below is **UNVERIFIED**.

**Write-free preflight** (reads only, loads nothing; under #518's counters a load is a write):
1. **The receiver:** a `WillowPlayerController`, else `VmError::Script`. Its current-playthrough index comes from
   the port's one shared rule (NATIVE_PLAYTHROUGH_QUERY.md section 4, with its subclass `NotYet { NotRead }`). A
   negative index: `NotYet { NotRead }`, "RefreshBalanceDataFromMissionCompletion: a negative playthrough index".
2. **The mission:** an object reference that is a `MissionDefinition`. None, or another class: `VmError::Script`
   (the game assumes a mission). An unloaded mission handle cannot occur from the caller (it read the mission's
   fields already); treat one as `NotYet { LayerAbsent }`, the store's load.
3. **The globals:** as the port answers `GetWillowGlobals()` and `GetGlobalsDefinition()`. None of either: answer
   false, write nothing. An unloaded globals definition: `NotYet { LayerAbsent }` (the store's load).
4. **The table and the entry** (section 3, steps 2 and 3). An index past the table: false, nothing written. A set
   `DlcExpansion` that is an unloaded handle: `NotYet { LayerAbsent }`.
5. **The search** (section 3, step 4):
   - a None `BalanceDefinitions` entry is `VmError::Script` (the game dereferences it);
   - **a `GameBalanceDefinition` handle the store has not loaded** is `NotYet { LayerAbsent }`,
     "RefreshBalanceDataFromMissionCompletion: the region balance definition <path>", layer "the definition store's
     load of a region balance definition (the native loads nothing)". See the note for the hub below;
   - an override's `Mission` is compared with the completed mission by **object identity: the `ObjRef` itself**. The
     port's store gives one object per export and loads it in place, so an unloaded handle that refers to the
     completed mission is the same `ObjRef` as the loaded mission. The handle is **never resolved or loaded** for the
     comparison: `ensure_loaded`, `find` or a read of its properties would load it, which is a write. (The store's
     object path of the export, `ow_defs::identity::full_path`, names the same identity and serves for messages.) The
     override's `Region` is taken the same way, as an `ObjRef`, without loading it.
6. **The plan:** one reset per matching override, in search order, duplicates included (section 3, step 4): for each,
   the index of the `RegionGameStages` record it removes, if any. Plan them one after another against the array as
   the earlier removals leave it. A record that is not the `RegionGameStageData` shape is `VmError::Script`.

**The writes:** `RegionGameStages` with the planned records removed, one array write. Answer true when any override
matched, else false. No refusal comes after the write.

**The counter rule:** the native's own refusals all come from the preflight, so they are unmarked. Its caller,
`UpdateMissionStatus`, wrote the entry's status first, so the caller layer marks them.

**The balance definitions' residency, and the hub's ruling.** On the installed data the four first-playthrough
`GameBalanceDefinition`s are references from the globals definition. A second local probe on the same stack
(section 6) read them at the turn-in: **all four (`Balance_P1_Zone1`, `Balance_P1_Zone1_Interlude`,
`Balance_P1_Zone2`, `Balance_P1_Zone3`) are unloaded store handles** when this native is reached. So, without
help, the answer above refuses `LayerAbsent` on link 12, even though the result would be false.

**Hub ruling (review r1 of #559):** the native keeps the `LayerAbsent` refusal while any of them is an unloaded
handle. The slice harness loads them as a harness input before the turn-in, as #541 and #546 describe the globals
preload. That load is the harness's, not the native's.

## 6. Link 12 after it (a local probe)

A local, uncommitted probe stacked #554 (`fix/rust-all-expansion-side-missions` @ 6571aabb) with #546, #533, #497,
#509, #512, #524 and #526, and bound this native locally to answer false and write nothing (the slice's answer,
section 4).
- Installed Off strict, under the lock: 4 ok / 1 NotYet / 9 errors.
- **Link 12 is ok, 6 of 6 driver steps:** `use`, `turnin`, `tick:0.1`, `tick:1`, `confirmreward`, `close`. On that
  stack this native is link 12's last stop.
- The probe's binding did not load the balance definitions. A second probe, on the same stack with a binding that
  only reported their state, found all four `Balance_P1_*` handles **unloaded** at the turn-in. Under the hub's
  ruling (section 5) the harness preloads them, and the native's real answer is then the same false with no write.

## Open questions

1. **Settled for the port:** the four first-playthrough balance definitions are unloaded handles at the turn-in
   (section 6, the second probe). Per the hub's ruling the slice harness preloads them; the native refuses
   `LayerAbsent` while any is unloaded.
2. **UNVERIFIED:** overrides of the current-playthrough virtual in subclasses of `WillowPlayerController` (as in
   NATIVE_PLAYTHROUGH_QUERY.md).
3. **UNVERIFIED:** a None `Region` in a matching override resets the first record with a None `RegionDef` for that
   playthrough (the routine compares identities and does not test for None).

## Live checks that would confirm the rules (bl-sdk, the RTSE way; none run, no session granted)

1. Before and after turning in the Fire mission on a fresh first playthrough, read `PC.RegionGameStages`. Expect it
   unchanged: no first-playthrough override names that mission.
2. Turn in a mission that one of `Balance_P1_*`'s `MissionOverrides` names, with its region's record present for the
   current playthrough. Expect exactly that record removed, then (after re-entering the region) a recomputed record.
3. Call `PC.ResetGameStageForRegion(<region>)` with records of that region for two playthroughs. Expect only the
   current playthrough's record removed.
4. Hook `RefreshBalanceDataFromMissionCompletion` and read its return value (expected true exactly when an override
   names the mission).
