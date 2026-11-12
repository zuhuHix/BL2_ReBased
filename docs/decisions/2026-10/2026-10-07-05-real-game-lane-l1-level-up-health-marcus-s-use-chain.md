## 2026-10-07: real-game lane L1: level-up health, Marcus's use chain, interface table and UpdateGrade observed in game

AI-assisted (Claude), real-game lane L1. Method as in the 2026-10-02 entry (one game process under the run lock, saves backed up first, saves blocked, memory-only, driver removed),
now with `Install-Driver -BlockSavesAtStart` so the save block holds from the first frame even when a startup mod loads a character. The level-8 Maya was loaded with the startup
mod's `-Character=Save0008.sav` argument. Confirmed in game (each by hooks on script-visible functions plus reads of the live objects; details in REALGAME_GROUND_TRUTH.md):
(1) a level-up refills current health to the new maximum (three level-ups from 25 to 50 % health), writes the new maximum inside `RecalculateAttributeInitializedState`
(base `80 x 1.13^L`, times the profile's Badass modifier), adds one skill point at the start of `ExpLevelUp`, leaves current and maximum shield alone but raises the shield recharge rate
by half the maximum per second for 4 s (stock `PlayerBehavior_LevelUp`), and for a natural level-up adds a 30 s weapon-damage scale of +1.0 (`PlayerBehavior_LevelUpNaturally`); no other
numeric attribute of the controller, pawn, replication info or pools changed apart from the game stage; the cooldown effect of `PlayerBehavior_LevelUp` was not shown to do anything
(inconclusive). (2) Marcus's use: nine `IsBehaviorSequenceEnabled` calls in the noted order, all false for a character with none of the six missions, then the dialog behavior, `HasMissions`
and `ShowMissionInterface`; the result rule (unknown name, `None` provider, handles -1 and 0 are false) by direct calls; tag `VO_NPC_OnUse_MissionsAvailable` with 1 eligible, 0 in progress,
0 redeemable missions; `GetDialogGroups` gives 127 entries in the noted order; consumer handle 49. (3) the real interface table matches the engine's own class check on all 66 stand-in
false positives and all 203 non-`Core.Interface` stand-in misses. (4) `Skill.UpdateGrade(N)` writes `Grade` and its base together. Refuted or corrected: `UpdateGrade` stores
`max(N, 1)`, not "N plus one"; the NATIVE_CLASS_SERIAL_LAYOUT count of `Core.Interface` pairs is 772, not 771. Not reached: the second (native) dialog lookup through the Trigger act and the
`TriggerEvent` return, "no change notification" for opcode 5F, the cooldown effect, skill points below level 5. Save evidence: 33 files hashed before and after, all byte-identical.
Verified in game: the items above; automated checks: ctest 11/11, verify_packages 9/9; nothing in `src/` changed.

Also: `tools/test_inventory_actions.ps1` deleted the run lock as stale after 15 minutes while this lane's game session still held it. A lock now counts as stale only when it is older than 15 minutes and no Unreal editor or Borderlands 2 process is running.

