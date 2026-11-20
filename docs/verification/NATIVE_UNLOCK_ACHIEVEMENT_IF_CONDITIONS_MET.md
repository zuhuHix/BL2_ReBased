# Native rules of WillowPlayerController.UnlockAchievementIfConditionsMet (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-LINK12-UNLOCK-ACHIEVEMENT (decision 2026-11-19-152, number
pending). Analyst only: a different lane implements it (clean room).

**Sources:**
- the installed script, read with `research/script_disasm.py`;
- the parameter names from the installed package's declaration;
- the installed globals data, read through the port's definition store (a local probe, not committed);
- `Borderlands2.exe`, read locally with the project's Ghidra tooling (read-only queries, under the machine lock).

There is no bl-sdk session and no recorded live evidence on this native. Raw output stays under ignored
`local/analysis/unlock/`. This note is in our own words: no decompiler output, pseudo-code, addresses or listing
structure. **Every rule here is UNVERIFIED** in the running game.

**Why it matters.** On a throwaway local merge (main with #533, #496, #497, #509, #512, #524 and #526), slice links 10
and 11 pass, and link 12 (`mission_turn_in`) passes 1 of 6 steps (`use`) and stops in `turnin` at this native,
unbound.

## 1. The declaration and its callers

`UnlockAchievementIfConditionsMet(AchievementUnlockType, StringData, IntData, bCanBeGreater, bCheckStringAndIntData,
bCanBeLess)` is a native final function of `WillowGame.WillowPlayerController`.
- **Parameters**, as the installed package declares them: a byte, a string, an int and three bools.
- **Return value:** a bool.
- **Key:** `WillowPlayerController.UnlockAchievementIfConditionsMet(byte,string,int,bool,bool,bool)`.

The script calls it at six sites, **all as statements: no caller reads the returned bool.** This covers 12,968 of the
12,978 decoded script functions; the 10 that fail to decode are not achievement code.
- **`WillowPlayerController.UpdateMissionStatus`, on the turn-in path:** in the branch for a side mission's
  completion, right after the side-missions-completed stat is incremented.
  - The unlock type is 16 (`AUT_SideMissionsCompleted`).
  - The string is omitted.
  - The int is the player's `STAT_SIDE_MISSIONS_COMPLETED` value (`PlayerStats.GetIntStat`).
  - `bCanBeGreater` is true; the other two flags are omitted (false).
- **The other sites, not on link 12's path:**
  - `ClientOnExpLevelChange` (the player's level);
  - `SpendBadassTokens`;
  - `ServerPurchaseBlackMarketUpgrade`;
  - `WillowPlayerPawn.ServerTellPassengersToUnlockAchievement` (twice: the vehicle's passengers, and the pawn's own
    controller).

## 2. What it does

1. **The matching achievements.** The game's globals definition (`GearboxGlobals.TheGlobalsDefinition`, a
   `GlobalsDefinition`) is read.
   - **The game's first use:** when the field is empty, the definition is loaded from its configured path and written
     into the field, and its challenge list is numbered (NATIVE_WILLOW_GLOBALS.md section 4). That is a write the first
     time.
   - Each entry of the definition's `AchievementData` (`AchievementUtility.AchievementUnlockData`: `UnlockType`,
     `Achievement`, `StringData`, `IntData`) is then checked, in order.
   - **Selected** when the entry's `Achievement` is not 0, its `UnlockType` equals `AchievementUnlockType`, and:
     - **with `bCheckStringAndIntData` clear:** the string match or the int match holds;
     - **with it set:** both hold.
   - **The string match:** the entry's `StringData` is not empty and equals the call's `StringData`, case-insensitive
     (the exact folding is UNVERIFIED).
   - **The int match:** the entry's `IntData` is at least 1, and the call's `IntData` equals it, or is above it with
     `bCanBeGreater` set, or is below it with `bCanBeLess` set.
   - With no globals object, or a definition that could not be loaded, nothing is selected.
   - **`AchievementUtility.ShouldUnlockAchievementFromUnlockType`** takes the same six inputs and returns the selected
     ids. Whether this native shares that function's selection, or keeps its own copy of the rule, was not read
     (UNVERIFIED, open question 3).
2. **The unlock.** For each selected entry, in order, the controller's script function
   **`ClientUnlockAchievement(AchievementId)`** is called with the entry's `Achievement` (the engine calls it by name,
   as a script event on the controller).
3. **The answer:** true when at least one entry was selected, else false.

The native writes nothing else: no stat, no profile setting, no counter.

**`ClientUnlockAchievement`** (script; a reliable client function):
- returns at once:
  - on a console build in split screen (with a further condition, not needed here);
  - for an id outside 1 to 100;
  - in the `PlayerLobby` state;
- otherwise, with an `OnlineSub` that has a `PlayerInterface`, it asks that interface's `UnlockAchievement` for the
  local player's controller id and the id;
- with no `OnlineSub`, or no player interface, it does nothing.

Its only effect is the online subsystem's achievement call.

## 3. What the slice's turn-in passes, and the answer

- **The installed data** (`GD_Globals.General.Globals`) holds 9 `AchievementData` entries. Exactly one has unlock type
  16: `WA_BountyHunter`, with `IntData` 20 and no string.
- **The slice's turn-in** is its first side-mission completion, so `IntData` is 1.
  - With `bCanBeGreater` set, the entry needs the count at 20 or above. Any count below 20 (1 on the port's fresh
    stats) leaves it unselected.
  - **Nothing is selected, `ClientUnlockAchievement` is not called, and the answer is false.**
  - **The port's slice world leaves the definition empty.** The bridge world builds `WillowGlobals` from its class
    defaults, so `TheGlobalsDefinition` is None there. The main proposal (section 4) therefore stops at the globals
    refusal unless the driver preloads the definition, as the pickup route's world set-up does for links 6 and 7.
    That preload is a harness input (world set-up before the run), not a write of this native.

## 4. Proposed ParityMode Off answer (for a separate implementer)

**The rule, Off only** (Reference keeps the stub policy):
1. **Read the globals definition** through the existing helper, `ow_inventory::globals::globals_definition`
   (IMPL-WILLOW-GLOBALS). Its refusals stay as they are, both not-read refusals:
   - **an empty field** is the game's first use (section 2), "the globals definition's first load, which numbers the
     challenge list". A load from an empty field stays not read;
   - **a definition handle the store has not loaded** is the helper's lazy-handle refusal. If the hub rules that load a
     tracked write, it is the store's handle load, `DefStore::ensure_loaded`, with one counted load transition (the
     store's counter of #518, open).

   **The hub's ruling for the implementer:** the main proposal, with the driver preloading the installed definition
   (as `equip_run.rs`'s pickup world does), so the slice's turn-in reaches the selection. The preload is a
   harness input (world set-up before the run, like `pickup_world`'s), not the native's write.
   - **A missing world globals object** stays the existing layer-absent refusal of the port's globals lookup, where
     the game selects nothing (section 2).
2. **The declared field types are checked** after the definition refusal and before any value is read:
   `AchievementData` an array of `AchievementUnlockData`, with `UnlockType` a byte, `Achievement` a byte, `StringData`
   a string and `IntData` an int. Otherwise it is a data error, `VmError::Script`, naming the field.
3. **Select the entries** by section 2, step 1 (reads only), through one selection function. If open question 3
   finds that `ShouldUnlockAchievementFromUnlockType` shares the rule, that native uses the same function.
4. **None selected** (the slice's case): return false. Nothing is written, so no preflight is needed.
5. **Some selected:** call `ClientUnlockAchievement(id)` on the controller through the VM, once per selected entry, in
   order, then return true. Its own refusals propagate; a refusal after an earlier call's writes is the caller's to
   mark.

**A smaller alternative for the hub** (R3 style): the port has no online subsystem (the empty `OnlineSub` slot of #501),
so `ClientUnlockAchievement` would do nothing in the port. Answering false with no script call matches the slice's case
exactly. It differs only when an entry matches, by skipping that script function's early checks (a console-build query,
a state test), which write nothing. That is a port answer, UNVERIFIED, and the hub's call.

**Argument errors:** ordinary VM argument errors apply.

## 5. Link 12's next stop (local probe)

This probe tested **the smaller alternative**, not the main proposal: the native bound locally to a plain false
(uncommitted), with no globals read. On the same throwaway merge, the installed Off `--slice-gate`
gives 3 ok / 2 NotYet / 9 errors. Links 10 and 11 stay ok. Link 12 still passes 1 of 6 (`use`) and now stops in
`turnin` at:
- "stopped after partial writes: MissionTracker.AllExpansionSideMissionsComplete with a controller (the controller
  virtual that gives the playthrough index is not resolved by name; the test walks the tracker's full MissionList,
  which the bridge does not hold)";
- `NotYet { NotRead }`, record NATIVE_MISSION_OBJECTIVES.md.

That is the port's own existing refusal, not a new unbound native.

## Open questions

1. **UNVERIFIED:** the further condition of `ClientUnlockAchievement`'s console split-screen return (not on the slice's
   path).
2. **UNVERIFIED:** whether the globals definition is already loaded when the turn-in runs in game (by the time of a
   turn-in, other natives have read it).
3. **UNVERIFIED:** whether this native shares `AchievementUtility.ShouldUnlockAchievementFromUnlockType`'s selection
   (the same six inputs, returning the ids) or keeps its own copy of the rule.

## Live check that would confirm the rule (bl-sdk, the RTSE way)

Hook `UnlockAchievementIfConditionsMet` and `ClientUnlockAchievement` while turning in the first side mission. Expect
one call of the first with type 16, int 1 and `bCanBeGreater` true, a false return, and no call of the second.
