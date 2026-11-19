# Native rules of WillowPlayerController.UnlockAchievementIfConditionsMet (draft, 2026-11-19)

AI-assisted (Claude Code, Opus 5.5), analyst lane ANALYST-LINK12-UNLOCK-ACHIEVEMENT (decision 2026-11-19-NNN, number
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

The script calls it at six sites, **all as statements: no caller reads the returned bool.**
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
   `GlobalsDefinition`) is read. It is loaded from its configured path on first use and cached in that field, which
   is a write the first time. Each entry of its `AchievementData` (`AchievementUtility.AchievementUnlockData`:
   `UnlockType`, `Achievement`, `StringData`, `IntData`) is checked, in order.
   - **Selected** when the entry's `Achievement` is not 0, its `UnlockType` equals `AchievementUnlockType`, and:
     - **with `bCheckStringAndIntData` clear:** the string match or the int match holds;
     - **with it set:** both hold.
   - **The string match:** the entry's `StringData` is not empty and equals the call's `StringData`, case-insensitive.
   - **The int match:** the entry's `IntData` is at least 1, and the call's `IntData` equals it, or is above it with
     `bCanBeGreater` set, or is below it with `bCanBeLess` set.
   - With no globals or no globals definition, nothing is selected.
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
  - With `bCanBeGreater` set, the entry needs the count at 20 or above, so it is not selected.
  - **Nothing is selected, `ClientUnlockAchievement` is not called, and the answer is false.**
  - The only possible write is the globals definition's first-use load, if the port's globals do not already hold it.

## 4. Proposed ParityMode Off answer (for a separate implementer)

**The rule, Off only** (Reference keeps the stub policy):
1. **Read the globals definition** (`TheGlobalsDefinition`).
   - Follow the port's existing globals rule (IMPL-WILLOW-GLOBALS): a definition handle not loaded yet is the existing
     lazy-handle `NotYet`, unless the hub rules the load a tracked write (then load it and count it).
   - No globals or no definition: false, nothing done.
2. **Select the entries** by section 2, step 1 (reads only).
3. **None selected** (the slice's case): return false. Nothing is written, so no preflight is needed.
4. **Some selected:** call `ClientUnlockAchievement(id)` on the controller through the VM, once per selected entry, in
   order, then return true. Its own refusals propagate; a refusal after an earlier call's writes is the caller's to
   mark.

**A smaller alternative for the hub** (R3 style): the port has no online subsystem (the empty `OnlineSub` slot of #501),
so `ClientUnlockAchievement` would do nothing in the port. Answering false with no script call matches the slice's case
exactly. It differs only when an entry matches, by skipping that script function's early checks (a console-build query,
a state test), which write nothing. That is a port answer, UNVERIFIED, and the hub's call.

**Argument errors:** ordinary VM argument errors apply. A data error (an `AchievementData` that is not an array of
that struct) is `VmError::Script`, naming the field.

## 5. Link 12's next stop (local probe)

On the same throwaway merge, with this native bound locally to false (uncommitted), the installed Off `--slice-gate`
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

## Live check that would confirm the rule (bl-sdk, the RTSE way)

Hook `UnlockAchievementIfConditionsMet` and `ClientUnlockAchievement` while turning in the first side mission. Expect
one call of the first with type 16, int 1 and `bCanBeGreater` true, a false return, and no call of the second.
