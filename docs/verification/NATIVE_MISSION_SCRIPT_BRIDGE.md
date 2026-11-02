# Native mission script bridge: accept, complete, turn-in, rewards, kickoff (2026-10-05)

AI-assisted (Claude), analyst lane C1. Read from the game executable with Ghidra (tools/ghidra/), written in our own
words; no decompiler output, pseudo-code or listing structure is reproduced here. **Every rule is UNVERIFIED in the
running game** unless a line says how it was confirmed. Script behaviour was read from the local WillowGame listing
(`research/script_disasm.py`); script signatures come from the package declarations; field names were mapped with
`tools/ghidra/class_layout.py`. This note extends [NATIVE_MISSION_DISPATCH.md](NATIVE_MISSION_DISPATCH.md) (section B)
and [NATIVE_PROGRESSION.md](NATIVE_PROGRESSION.md) and corrects both in places (last section).

## The one-paragraph picture

Accepting and turning in a mission are almost entirely **script driving a few natives**. `AcceptMission` and
`ServerCompleteMission` (script, on `WillowPlayerController`) call the native `MissionTracker.ActivateMission` /
`CompleteMission`. Those change the mission's status through one native routine (called "the status routine" below).
The status routine calls **back into script** (`WillowPlayerController.UpdateMissionStatus` on every local player
controller) **before** it notifies observers and fires the `Default` behavior event. The rewards are **not** a native
turn-in step: they are granted by the script `UpdateMissionStatus` when the status becomes Complete, via
`ServerGrantMissionRewards`, which calls natives (`MissionDefinition.Get*Reward`, `ExpEarn`,
`GetItemRewardsForPlayer`). The kickoff after acceptance is a **pending record the tracker's per-frame tick consumes**.

## Summary

| Native | Script signature (from the package) | Slice relevance | Confidence | Status |
|---|---|---|---|---|
| MissionTracker.ActivateMission | `native function ActivateMission(MissionDefinition InMission, optional WillowPlayerController WillowPC)` | Fire: accept | high (entry checks), medium (every side effect) | UNVERIFIED |
| MissionTracker.CompleteMission | `native function CompleteMission(MissionDefinition InMission, optional WillowPlayerController WillowPC)` | Fire: turn-in | high / medium | UNVERIFIED |
| MissionTracker.SetMissionStatus | `native function SetMissionStatus(MissionDefinition InMission, EMissionStatus MissionStatus, optional WillowPlayerController WillowPC)` | both (the shared routine) | medium | UNVERIFIED |
| MissionTracker.PlayKickoff / PlayKickoffDialogOnly / PlayTurnIn | `native function PlayX(MissionDefinition InMission)` | Fire: kickoff dialog, turn-in event | high | UNVERIFIED |
| MissionTracker tick (kickoff scheduling) and SetActiveMission | `native function SetActiveMission(...)` (internal tick has no script name) | Fire: what plays the kickoff | medium | UNVERIFIED |
| MissionTracker.SetKickoffHeard | `native function SetKickoffHeard(MissionDefinition InMission)` | minor | high | UNVERIFIED |
| MissionTracker.GetMissionStatus / MissionDependenciesMet / CanStartMission / CanEndMission | `native function ... (MissionDefinition InMission)` returning `EMissionStatus` / `bool` | availability, UI | high | UNVERIFIED |
| MissionDefinition.GetCurrencyRewardType | `native function ECurrencyType GetCurrencyRewardType(bool bGetAltReward)` | Fire: reward | high | UNVERIFIED |
| MissionDefinition.GetCurrencyReward | `native function int GetCurrencyReward(WillowPlayerController InWPC, bool bGetAltReward)` | Fire: reward (value 0) | medium | UNVERIFIED |
| MissionDefinition.GetOptionalCreditReward | `native function int GetOptionalCreditReward(WillowPlayerController InWPC)` | none for Fire | medium | UNVERIFIED |
| MissionDefinition.GetExperienceReward | `native function int GetExperienceReward(WillowPlayerController InWPC, bool bGetAltReward)` | Fire: XP (formula in NATIVE_PROGRESSION) | high | confirmed once in game for the amount (395), caller order UNVERIFIED |
| MissionDefinition.ShouldGrantAlternateReward | `native function bool ShouldGrantAlternateReward(out array<int> ObjectivesProgress)` | Fire: picks Reward vs AlternativeReward | low-medium | UNVERIFIED |
| MissionDefinition.GetItemRewardsForPlayer | `native function GetItemRewardsForPlayer(WillowPlayerController WillowPC, out PendingMissionRewardData MissionReward)` | Fire: empty result | medium | UNVERIFIED |
| WillowPlayerController.ExpEarn | `native final function ExpEarn(int Exp, EExperienceSource Source, optional EExperienceType ExpType)` | Fire: XP into the pool | high | UNVERIFIED |
| ExperienceResourcePool.ApplyExpPointsToExpLevel | `native function ApplyExpPointsToExpLevel(optional bool bCheated)` | Fire: level-up after XP | high | UNVERIFIED |

Script (not native) functions that carry most of the behaviour, listed so the VM is known to need them:
`WillowPlayerController.AcceptMission`, `ServerAcceptMission`, `ServerCompleteMission`, `UpdateMissionStatus`,
`ServerGrantMissionRewards`, `GetNumRewardChoices`, `ClientSpawnMissionRewardUI`, `ClientShowNoRewardScreen`,
`AcceptOrSaveUnclaimedReward`, `ReceiveWeaponReward`, `ReceiveItemReward`, `MissionRewardsReceived`, `ExpLevelUp`,
`OnExpLevelChange`, `IsFastForwardPromptValid`, `TryPromptForFastForward`; `WillowAIPawn`, `WillowInteractiveObject`
and `WillowPickup` `OnPlayerAcceptedMission` / `OnPlayerTurnedInMission`.

Status values (`EMissionStatus`, confirmed from the package enum): NotStarted 0, Active 1, RequiredObjectivesComplete 2,
ReadyToTurnIn 3, Complete 4, Failed 5. Experience source (`EExperienceSource`): Unknown 0, Combat 1, PlotMissionAward 2,
Discovery 3, SideMissionAward 4. Experience type (`EExperienceType`): Generic 0, Combat 1, Mission 2.

## MissionTracker.ActivateMission

- **Signature:** `(MissionDefinition InMission, optional WillowPlayerController WillowPC)`, no return value. The script
  caller is `WillowPlayerController.AcceptMission`, which passes the mission and `Self` (the accepting controller), on the
  tracker held by `WillowGameReplicationInfo.MissionTracker`. A behavior (`Behavior_ActivateMission`) also reaches the
  same routine with no controller.
- **Reads:** the mission's runtime record (status, progress array), `bRepeatable`, `Dependencies`,
  `ObjectiveDependency`, `InitialObjectiveSet`, number of `ObjectiveDefs`, `GameStageRegion`, `MissionWeapon`,
  `bActivateInitialObjectiveSet`, `SecondsToComplete`, `StartBlockingSet`.
- **Does (in order):** the native itself has **no role/authority check and no blocked-mission check** (the script
  forwards to the server first; blocking is tested by `CanStartMission`, which the UI calls). It does nothing when the
  mission has no record in the tracker, when its status is not one of NotStarted / Failed / (Complete and
  `bRepeatable`), or when `MissionDependenciesMet` is false. Otherwise it runs the status routine with Active and the
  given controller (next section: that is where every effect lives).
- **Calls into script:** all through the status routine (see `SetMissionStatus`).
- **Edge cases:** the status routine adds its own refusals (more than 20 objectives; objectives but no
  `InitialObjectiveSet`; a non-repeatable mission whose progress array is not empty). A refusal is silent: no event, no
  notification, no return value.
- **Implementer checklist:** (1) refuse silently on the three entry conditions above; (2) reject without side effects
  when the status routine refuses; (3) do not look at the role; (4) do not test `IsMissionBlocked` here.
- **Open:** whether a client calling it directly matters (script routes clients through `ServerAcceptMission`).

## MissionTracker.CompleteMission

- **Signature:** `(MissionDefinition InMission, optional WillowPlayerController WillowPC)`, no return value. Called by
  `WillowPlayerController.ServerCompleteMission(Mission, optional IMissionDirector)` with the mission and `Self`; also by
  `Behavior_CompleteMission`.
- **Does (in order), regardless of whether step 1 was accepted:**
  1. Runs the status routine with **Complete** and **no controller**. The routine accepts it only from ReadyToTurnIn or
     RequiredObjectivesComplete; from any other status it silently does nothing, **but steps 2 to 4 still run**
     (reading of the code; a Complete call on an Active mission would still chain the next mission and clear the
     tracked one).
  2. **Chain:** if `NextMissionInChain` is set and that mission is startable (status NotStarted / Failed / repeatable
     Complete, and its dependencies are met), it is activated through the status routine with the controller passed to
     `CompleteMission`. If there is **no** `NextMissionInChain` and the tracker's active (tracked) mission is the
     completed one, the tracked mission is cleared (the same call as `SetActiveMission(None)`).
  3. **Unlock queue:** recompute which not-yet-startable missions now have all dependencies met (the tracker's
     `DependentMissions` list, whose entries pointing at the completed mission are removed, feeds
     `MissionsWithCompletedDependencies`). A newly unlocked plot-critical mission is inserted at the front of that
     queue (at position 1 when the unlocked mission belongs to a DLC and the current head is plot-critical), any other
     is appended. The completed mission is skipped.
  4. **Fast-forward prompt:** if the queue is not empty after step 3, `FastForwardTriggerMission` is set to the completed
     mission and, for every `WillowPlayerController` in the world's controller list, the script function
     `IsFastForwardPromptValid()` is called; for each that returns true, `TryPromptForFastForward()` is called on it.
     The script says valid means: local controller, primary player, and the game has exactly one player. Prompting shows
     a yes/no dialog (`dlgFastForward`) unless a blocking movie is playing. This is presentation; a headless run may
     see the call and must tolerate it.
- **Calls into script (full list, from step 1):** see `SetMissionStatus` for Complete; step 4 as above. The turn-in
  script then calls the director and `PlayTurnIn` (below).
- **Edge cases:** `NextMissionInChain` activation uses the controller argument as the "accepting player" for the
  kickoff record of the next mission.
- **Implementer checklist:** the order is status change first (rewards happen inside it, see the reward path), then
  chain/untrack, then the unlock queue and prompts. `ServerCompleteMission` calls the director's
  `OnPlayerTurnedInMission(Self, Mission)` only after `CompleteMission` returned, then `PlayTurnIn`.
- **Open:** what the unlock queue is used for besides the prompt; the exact meaning of the insert-at-1 rule.

## MissionTracker.SetMissionStatus (the status routine)

`SetMissionStatus(InMission, MissionStatus, optional PC)` is the registered native for this routine; Activate and
Complete are thin front ends for it. It does nothing for a mission without a record and for status NotStarted. Each
target status first checks its allowed source status and returns silently when it is not met (table in NATIVE_MISSION_DISPATCH
B4: Active from NotStarted/Failed/repeatable Complete; RequiredObjectivesComplete from Active; ReadyToTurnIn from Active
or RequiredObjectivesComplete; Complete from ReadyToTurnIn or RequiredObjectivesComplete; Failed from Active and only
when the mission can fail, meaning `bCanBeFailed`, or `SecondsToComplete` > 0, or a defend-mission setting of 2).

Status-specific effects, in the order they occur (all before the common tail):

- **Active.** Refused when the progress array is not empty and the mission is not repeatable, when the mission has more
  than 20 objectives, or when it has objectives but no `InitialObjectiveSet`. For a repeatable mission being restarted
  from Complete the active objective set is cleared first. Then: status = Active; the progress array is replaced by one
  zero per entry of `ObjectiveDefs`; the **game stage is locked**: if the mission has a `GameStageRegion` the region's game
  stage function supplies the stage (see NATIVE_PROGRESSION section 2) and it is stored in `GameStage` and
  `bGameStageLocked` is set, with no region the stage is 0 and it is also locked (reading); then the **script hook
  `UpdateMissionStatus(Mission, Active)` runs on every local player controller**, then `ClientReceiveMissionStatus` goes
  to remote controllers only (below); if `SecondsToComplete` > 0 an entry (mission, seconds, 0) is appended to
  `ActiveTimedMissions`; if `MissionWeapon` is set the mission weapon is granted (below); if
  `bActivateInitialObjectiveSet` is true the initial set is activated (B3 of the dispatch note); then
  `SetActiveMission(mission, fromActivation = true, PC)` is applied (tracked mission and **pending kickoff**, below);
  finally, when `StartBlockingSet` is not set, mission blocking starts (a plot-critical mission blocks everything if it
  is a global blocker, else it blocks each of its `BlockedMissions` that is not plot-critical; with a
  `StartBlockingSet` the blocking waits for that set).
- **RequiredObjectivesComplete.** Status set, then the script hook and remote notification, nothing else.
- **ReadyToTurnIn.** Status set; script hook, remote notification; if the mission is the tracked one the tracked-mission
  marker/waypoint is re-evaluated; if it was timed or a defend mission those entries are removed and, when tracked, the
  defend target is cleared.
- **Complete.** Status set; script hook (**this is where the rewards are granted**), remote notification; the mission
  weapon is removed if the mission has one; the stat listeners of the active objective set (and of its branch
  objectives) are unregistered (the `ActiveObjectiveSet` field itself is **not** cleared here); when there is no
  `StopBlockingSet`, mission blocking ends.
- **Failed.** Needs the "can fail" test; status set; progress array cleared; active set cleared (so it can restart);
  script hook and remote notification.

**Common tail** (after any accepted change except NotStarted), in order:
1. mission observers are told "status changed" (the `IMission.MissionReactionStatusChanged` reaction on every
   registered observer; lane G2 owns what those reactions do); an observer list for a Complete non-repeatable mission is
   dropped afterwards;
2. the mission-waypoint/minimap bookkeeping is refreshed for the tracked mission;
3. every registered director gets its status-changed interface call (the three script implementers have empty bodies);
4. the **`Default` behavior event** fires on the tracker's consumer handle with provider = `MissionDefinition.BehaviorProvider`,
   no payload, and link id **6 + new status** (Active 7 ... Failed 11).

**Where the script hook sits (correction to dispatch note B4):** the script hooks are *inside* each status branch, so
`UpdateMissionStatus` and the `TriggerMissionStatusChangedDelegates` call run **before** observers and the `Default`
event, not after.

**Script callbacks made by the routine, exactly:**
- `WillowPlayerController.UpdateMissionStatus(Mission, NewMissionStatus)` on every `WillowPlayerController` among the
  engine's `GamePlayers` (the local players), one call each, in player order. Arguments: the mission and the new status
  byte. After the loop: the directors' presentation refresh (director icons/particles) and then
  `MissionTracker.TriggerMissionStatusChangedDelegates()` on the tracker itself (no arguments; fires the
  `OnMissionStatusChanged` delegates).
- `WillowPlayerController.ClientReceiveMissionStatus(MissionStatusData{Mission, Status}, GameStage)` on every controller
  in the world's controller list that is a `WillowPlayerController` **and is not a local controller** (the controller's
  "is local" virtual says no) and that passes one further connection test whose meaning was not identified. GameStage is the mission's locked stage when the new status is
  Active, otherwise -1. In single player nothing receives it.

**What `UpdateMissionStatus` needs on the controller side (script, relevant to the VM):** it looks the mission up in the
controller's own `MissionPlaythroughs[currentPlaythrough].MissionList` (`NativeGetMissionIndex`, -1 means "not there",
old status then reads as NotStarted) and branches on the new status. Active is accepted from old NotStarted / Failed /
repeatable Complete (adds the record with `AddMission`, refreshes the UI list, contextual prompt). ReadyToTurnIn from old
Active / RequiredObjectivesComplete (copies the tracker's objective progress, feedback message, fanfare). **Complete
only counts from old ReadyToTurnIn / RequiredObjectivesComplete**: that is the only path that grants rewards. If the
controller's record never saw ReadyToTurnIn (for example because the host skipped that status) the Complete branch
shows `ClientShowNoRewardScreen(Mission, OldStatus)` instead (not in the editor) and **no XP is paid**. Each status
must therefore be delivered to `UpdateMissionStatus` in order.

- **Mission weapon granting (Active):** for every local player controller: the weapon balance in `MissionWeapon` is
  generated at the mission's game stage (and awesome level), marked, given to the player's pawn, then the controller's
  `ShowMissionWeaponTraining(weapon)` script function is called. Only done when the weapon's mission-objective owner is
  this mission and the mission has a `GameStageRegion`. Item generation internals: see the loot lane.
- **Edge cases:** a refused transition is silent. The routine does not check `IsMissionBlocked`.
- **Open:** the exact argument values passed to the observers (not recovered), whether the ">20 objectives" and
  "non-empty progress" refusals are reachable in the shipped data, the timed/defend internals.

## Kickoff after acceptance: SetActiveMission, the tracker tick, PlayKickoff*, SetKickoffHeard

This answers "what plays the kickoff after acceptance", which the dispatch note listed as not found.

- **Pending record (written during Active).** `SetActiveMission(mission, bFromActivation, PC)` (native, also reachable
  from script) works on the server/authority only. It (a) possibly returns early (a tracked mission already exists, the
  new mission is not plot-critical or `bFromActivation` is false, and a player-level gate native holds; that gate's
  exact test was not identified) and (b) writes the tracker's `PendingMissionKickoff` record {Mission, PlayerThatAccepted =
  the controller argument, bFromActivation} **only if** the mission has a record, its status is neither NotStarted nor
  Failed, its `bHeardKickoff` flag is clear, and either the mission is plot-critical or no kickoff is pending yet. The
  Fire mission is not plot-critical (reading), so a second accept while one is pending does not overwrite it. It then picks
  the tracked mission (rules around DLC and status not read in detail) and calls the script functions
  `TriggerActiveMissionChangedDelegates()` on the tracker.
- **Tick (consumption).** On every tracker tick on the authority: if a pending record exists **and** the current level is
  not named "Loader" (and a world flag read as "networked/editor" guard is not set), then in order: (1) the script function
  `IsMissionMoviePlaying()` is invoked on the accepting controller (no use of its result was seen); (2) if the tracked
  mission differs from the pending one, `SetActiveMission` runs for the pending mission; (3) if the mission has both
  `DialogEvent` and `DialogTalker`, a dialog request goes to the world's dialog manager (not read further; the Fire mission
  data lists neither); (4) **if bFromActivation was true, `PlayKickoff(mission)` is called, else
  `PlayKickoffDialogOnly(mission)`** (both are virtual natives, so a script override would be honoured); (5) the mission's
  `bHeardKickoff` flag is set (and replicated to remote controllers); (6) the pending record is cleared. While the level
  is "Loader" the record simply stays and is retried every tick.
- **PlayKickoff / PlayKickoffDialogOnly / PlayTurnIn.** With a non-null mission each does exactly one thing: fire the
  `Default` behavior event on the tracker's consumer handle for the mission's `BehaviorProvider`, no payload, link id
  **12 / 13 / 14** respectively. They play no dialog themselves; any dialog comes from behaviors hanging on those links
  (in the Fire mission, a Marcus dialog behavior). With a null mission they do nothing.
- **SetKickoffHeard(mission):** sets `bHeardKickoff` on the record and, on the authority, replicates it. It does not
  fire events.
- **Implementer checklist:** model the pending record and consume it on the next tick, not inside `ActivateMission`;
  clear it afterwards; `Default` 12 is fired a tick after acceptance (one frame at minimum), so the Fire mission's
  "set `GoToRange_ObjSet` after Marcus finishes" chain starts a frame after the Active event (id 7).
- **Open:** the early-exit gate in `SetActiveMission`; the tracked-mission choice; the dialog-manager call.

## Availability queries

- `GetMissionStatus(mission)`: the record's status, NotStarted when there is no record.
- `MissionDependenciesMet(mission)`: every `Dependencies` entry has a record with status Complete (a missing record
  fails); then `ObjectiveDependency` as in dispatch B6. (Entry condition of Activate and of the unlock queue.)
- `CanStartMission(mission)`: status NotStarted / Failed / (Complete and repeatable), dependencies met, **and not
  blocked**. Blocked means: a global blocker mission is active and it is not this mission, or this mission is listed
  in the tracker's `BlockedMissions` data.
- `CanEndMission(mission)`: status ReadyToTurnIn or RequiredObjectivesComplete and not blocked.

## IMissionDirector implementers

`IMissionDirector.OnPlayerAcceptedMission(PlayerAccepting, MissionAccepted)` and `OnPlayerTurnedInMission(...)` are
**script** on all three implementers; there are no natives for them:
- `WillowAIPawn.OnPlayerAcceptedMission`: if no controller uses the pawn ("PawnsUsingMe" empty) returns; if the first
  user's controller is the accepting player, appends the mission to `MissionsAcceptedByPrimaryUser`.
- `WillowAIPawn.OnPlayerTurnedInMission`: same user test, then `PlayMissionTurnedInDialog(user, mission)` (script).
- `WillowInteractiveObject`: accept is empty; turn-in calls `InteractiveObjectDefinition.OnMissionTurnedIn(handle, player,
  mission)` when a definition exists.
- `WillowPickup`: accept calls the accepting controller's `ServerPickupSpecific(Self)` (a mission pickup is picked up on
  accept); turn-in is empty.
`MissionStatusChanged` is empty on `WillowAIPawn` and `WillowPickup`. The director argument of `AcceptMission` /
`ServerCompleteMission` is `None` for plain calls and is skipped by the script when `None`.

## The turn-in reward path

**Who calls what, in order, when a mission becomes Complete** (the script runs inside the status routine, see above):

1. `WillowPlayerController.UpdateMissionStatus(Mission, Complete)`; when the controller's old status for the mission is
   ReadyToTurnIn or RequiredObjectivesComplete it stores Complete, sets the record's `bNeedsRewards`, copies the
   record's objective progress and computes `bAlt = Mission.ShouldGrantAlternateReward(progress)`, then calls
   `ServerGrantMissionRewards(Mission, bAlt)`, then a feedback message, `ClientDoMissionStatusFanfare(7, true, Mission)`,
   and the statistics/achievement bookkeeping (side-mission count when not plot-critical,
   `CheckAllSideMissionsCompleteAchievement`, optional-objective stat, `CheckForSlaughterAchievement`,
   `RefreshBalanceDataFromMissionCompletion(Mission)`, playthrough-complete checks).
2. `ServerGrantMissionRewards(Mission, bGrantAltReward)` (script, in this order): currency type =
   `GetCurrencyRewardType(bAlt)`; amount = `GetCurrencyReward(Self, bAlt)`; if amount > 0, the replication info's
   `AddCurrencyOnHand(type, amount)`; if the type is not credits, `GetOptionalCreditReward(Self)` is added as credits
   when > 0. Then `xp = GetExperienceReward(Self, bAlt)`; if > 0, **`ExpEarn(xp, source, ExpType omitted)`** with source
   **PlotMissionAward (2) when the mission is plot-critical, SideMissionAward (4) otherwise**; the type stays Generic (0).
   Then the pending-reward struct is filled: `Mission`, `bGrantAltReward`, then `Mission.GetItemRewardsForPlayer(Self,
   out MissionReward)`, and finally `ClientSpawnMissionRewardUI(MissionReward)`. So **credits are paid before XP, XP before
   items, all before the reward UI**.
3. `ClientSpawnMissionRewardUI`: with no quest movie playing, it does not draw a page; it goes to
   `AcceptOrSaveUnclaimedReward`; with the quest-accept movie open and the selected mission equal to the reward's
   mission it calls the movie's `DisplayRewardsPage(MissionReward)`; otherwise it refreshes the mission list first.
   `GetNumRewardChoices` counts how many of the two weapon slots and two item slots are filled (loop over index 0 and
   1, a slot counts when its weapon type or item definition is set; so 0, 1 or 2). `AcceptOrSaveUnclaimedReward`:
   with more than one choice (or when `bShowDelayedRewardForAllMissions`) the reward is stored in `UnclaimedRewards`
   once and a contextual prompt shown; otherwise a filled first weapon slot goes to `ReceiveWeaponReward` (backpack add
   from definition data) or a filled first item slot to `ReceiveItemReward`, and `MissionRewardsReceived(Mission)`
   clears `bNeedsRewards`. **Which of the two the player picks (two choices) is the UI's job**: the movie calls back into
   the same accept function.
4. If the old status was not ReadyToTurnIn/RequiredObjectivesComplete: `ClientShowNoRewardScreen(Mission, OldStatus)`
   and nothing is paid.

For the Fire mission the data gives: `Reward.ExperienceRewardPercentage` the 5 percent reward, `CreditRewardMultiplier` the
constant 0, **no `RewardItems` and no `RewardItemPools`**, no alternative reward set. So the turn-in pays XP only, the
item struct stays empty (0 choices), and the UI path ends in `MissionRewardsReceived`.

### MissionDefinition.GetCurrencyRewardType(bAlt)
Returns `Reward.CurrencyRewardType` when `bAlt` is false, `AlternativeReward.CurrencyRewardType` when true. ECurrencyType
0 is credits (taken from the way the script compares it).

### MissionDefinition.GetCurrencyReward(PC, bAlt)
- **Does:** refreshes the mission's game stage through the balanced-actor game-stage getter (`GetGameStage`, so a
  region-based stage is recomputed unless locked, NATIVE_PROGRESSION section 2), then:
  - credits type: `trunc( eval(GlobalsDefinition.MissionCreditRewardFormula, PC) * eval(RewardData.CreditRewardMultiplier, PC) )`,
    both evaluations by the attribute-initialization evaluator (NATIVE_PROGRESSION section 1); the formula is a
    `ValueFormula` read from the census as multiplier = attribute `Att_BaseCredits_MissionRewards`, level = attribute
    `Att_UniversalPriceIncreasePerLevelScaler`, power = attribute `D_Attributes.Balance.GameStage` (so
    `base * scaler ^ gameStage`; the attribute values were not decoded here);
  - any other type: `eval(RewardData.OtherCurrencyReward, PC)`.
  Then, when the mission has at least one `ObjectiveDefs` entry, for each objective that is optional
  (`bObjectiveIsOptional`), **complete**, and whose `OptionalCurrencyRewardType` equals the reward's currency type, it adds
  `trunc(formula * objective.OptionalCreditRewardMultiplier)` (credits) or `eval(objective.OptionalOtherCurrencyReward)`
  (others).
- **Edge case:** with objectives present and no usable first-local-player controller / mission tracker on it (same guard as
  `GetExperienceReward`) it returns 0 for the whole call.
- **Fire:** multiplier constant 0, no optional objectives: 0 credits (the guard passes in the real game, as seen for XP).
- **Implementer checklist:** reproduce credit as above; the product is truncated once at the end of each term.
- **Open:** attribute values of the credit formula (needed only for missions with a non-zero multiplier).

### MissionDefinition.GetOptionalCreditReward(PC)
Returns the credits for the **first** complete optional objective whose own currency type is credits
(`trunc(formula * that objective's OptionalCreditRewardMultiplier)`), and 0 when none; 0 also when the same first-local-player
guard fails. (It stops at the first match, reading.)

### MissionDefinition.GetExperienceReward(PC, bAlt)
Exactly as NATIVE_PROGRESSION section 2 (confirmed there for the amount 395 at game stage 8). Added here: it is called
from `ServerGrantMissionRewards` with `Self` and the `bAlt` that `ShouldGrantAlternateReward` produced, and the same
"first local player has a mission tracker or the whole result is 0 when objectives exist" guard applies. The
`GetGameStage` call inside refreshes the stage once more: for the Fire mission, which is locked after acceptance, it
returns the locked stage.

### MissionDefinition.ShouldGrantAlternateReward(ObjectivesProgress)
- **Reads:** `InitialObjectiveSet` and its `NextSet` chain, the `ObjectiveDefinitions` of the **last set of that chain**,
  `ObjectiveDefs`, the passed progress array.
- **Does:** follows `NextSet` from the initial set to the last set (no loop guard), then, for each objective of that
  last set, finds its index in `ObjectiveDefs`; if the index exists in the progress array and the progress value is
  **below the objective's required progress** (the objective's own virtual that turns `ObjectiveCount` into the required
  progress, which is the plain count unless the objective remembers items by bit mask) it returns true. Otherwise (all
  complete, no initial set, an objective not in `ObjectiveDefs`, an index past the progress array end after the first
  miss) it returns false. `bEnableAltReward` is not consulted here (the data flag is read elsewhere or only by the UI).
- **Fire:** the last set is `RocksPaper_FinalObj` whose objective `Fire` is complete: false, so the normal `Reward`.
- **Open:** low confidence on the "required progress" virtual; reading is "alternate reward when the final set was not
  fully progressed", which fits a branch-skipping completion.

### MissionDefinition.GetItemRewardsForPlayer(PC, out MissionReward)
- **Reads:** the reward data selected by `MissionReward.bGrantAltReward` (`Reward` or `AlternativeReward`), its
  `RewardItems` (InventoryBalanceDefinition list) and `RewardItemPools` (ItemPoolDefinition list), the mission's game
  stage and awesome level, the controller's character class.
- **Does:** does nothing when the controller is not a `WillowPlayerController` with a character class. Otherwise it
  creates item actors: for each `RewardItems` entry that has no class restriction, or whose restriction (the first
  non-null class restriction found walking up the balance's parent chain) equals the player's class, one item is
  generated from the balance at (game stage, awesome level, a third level-like value that is 50 by default and has a
  playthrough-2 adjustment) for that controller; then **each** `RewardItemPools` entry is rolled once with the same
  levels (pool rolling is the loot lane's G1 area, not read here). The resulting items are taken in order, **at most
  two**: a weapon's definition data is copied into `WeaponRewards[i]` and any other item's into `ItemRewards[i]` (the same
  index `i` counts both kinds, so a mixed reward uses slot 0 of one array and slot 1 of the other); a rewarded item's
  owner field is set from the mission's balanced-actor hook, and the temporary item actors are destroyed. Items beyond
  the second are destroyed unused.
- **Edge cases:** empty `RewardItems` and `RewardItemPools` (Fire): the out struct keeps its default (empty) value.
- **Implementer checklist:** the struct is data only; the Fire mission needs just the empty case. Pool rolling and
  balance generation belong to the loot implementation.
- **Open:** the third level-like value; the pool-roll call; ownership registration.

### WillowPlayerController.ExpEarn(Exp, Source, optional ExpType)
- **Reads:** the controller's experience resource pool (its `ExpPool` reference, which must resolve to an
  `ExperienceResourcePool`), the pool's owner controller, `GetMaxExpLevel`.
- **Does:** if the controller has a valid experience pool:
  1. `multiplier` = pool `ExpCombatPointsScale` when ExpType is Combat (1), `ExpMissionPointsScale` when Mission (2),
     otherwise 1.0 (Generic). The amount is then `Exp * multiplier * ExpAllPointsScale` (all three are attributes on the
     pool; ExpAllPointsScale is applied to every type).
  2. cap = `ExpPointsRequiredForLevel(GetMaxExpLevel)` (NATIVE_PROGRESSION section 3) when the pool's owner is a
     `WillowPlayerController`, 2,147,483,647 otherwise. Nothing happens if the pool's current value is already at or
     above the cap.
  3. new value = current + amount, clamped to [0, cap]. Only if the new value is **greater** than the current value:
     the gain (new - current, truncated to int) is reported to the online telemetry (the owner and the source byte; no
     gameplay effect, not read further), the pool's `CurrentValue` is set to the new value, and the pool's value-changed
     hook runs (it stamps a timer and calls an empty virtual).
  4. If the controller is a local controller, a presentation update of the HUD experience bar runs (no state change).
- **Does not:** level up. `ExpEarn` never calls `ExpLevelUp`, `OnExpLevelChange` or `ApplyExpPointsToExpLevel`.
- **Calls into script:** none directly.
- **Edge cases:** negative or zero amounts never lower the pool; an invalid pool does nothing (no error).
- **Implementer checklist:** XP from a mission with Exp 395, type Generic, an `ExpAllPointsScale` of 1 and no cap
  trouble raises `CurrentValue` by 395 on the call; the controller's level changes on the pool's next update, not in
  the call.
- **Open:** the value of `ExpAllPointsScale` in the base data (assumed 1, not decoded here).

### ExperienceResourcePool.ApplyExpPointsToExpLevel(bCheated) and the tick that calls it
- **What calls it:** the experience pool's per-frame update (the pool class overrides the base per-frame resource update:
  it runs the base update, then calls `ApplyExpPointsToExpLevel(false)`). The only script caller is the developer
  "free levels" command, which explicitly calls it with true after an `ExpEarn`. So **a gain made by `ExpEarn` becomes a
  level-up on the next pool update**.
- **Does:** as in NATIVE_PROGRESSION section 3: only for a pool whose owner is a controller with a replication info and
  character class; while `CurrentValue >= ExpPointsNextLevelAt` (> 0) and `GetMaxExpLevel` is above the current
  `ExpLevel`, the script `WillowPlayerController.ExpLevelUp(bCheated)` runs; an internal flag on the controller marks
  whether any level was gained during the pass; after at least one level `LevelUpCount` is incremented and two
  bookkeeping calls follow. **Correction:** the script parameter is named `bCheated`, not `bFeedback`; the tick passes
  false, which makes `ExpLevelUp` call `OnExpLevelChange(true, not bCheated)`.
- **Implementer checklist:** call it from the pool update, after the base update; the loop re-reads the pool's value each
  time, so several levels from one grant are handled.

## Not read yet

- Tracked-mission selection inside `SetActiveMission`, the early-exit gate and its player-level test; the fast-travel /
  waypoint marker routine used when the tracked mission is ReadyToTurnIn (not needed for the slice).
- Observer reaction arguments (`MissionReaction*`, lane G2) and the exact semantics of the director refresh.
- Item generation from a balance definition and pool rolling (lane G1); the mission-weapon item bookkeeping beyond the
  script calls listed.
- Timed (`SecondsToComplete`) and defend-mission internals; `IsMissionBlocked` blocked-set bookkeeping beyond the test.
- The dialog-manager request in the kickoff tick; the telemetry call in `ExpEarn`; the HUD update in `ExpEarn`.
- `RemoteUpdateMissionStatus` and the other `Remote*` client paths.
- Values of the credit-reward attributes and of `ExpAllPointsScale`.

## Corrections to earlier notes

- **NATIVE_MISSION_DISPATCH.md B4, "After every accepted change, in order: observers, directors, script hooks, then
  Default":** the script hooks (`UpdateMissionStatus`, `ClientReceiveMissionStatus`, `TriggerMissionStatusChangedDelegates`)
  run **first**, inside each status branch; observers, waypoint refresh, directors and the `Default` event follow.
  `UpdateMissionStatus` runs on local controllers and `ClientReceiveMissionStatus` only on non-local controllers.
- **NATIVE_MISSION_DISPATCH.md B4 ("Complete: ... the active set cleaned up") and "Not read yet" (what plays the
  kickoff):** Complete only unregisters the set's stat listeners; the kickoff is the tracker tick consuming the pending
  record written by `SetActiveMission` (above). Also `ActivateMission`'s checks match the note, and `CompleteMission`
  additionally chains `NextMissionInChain`, untracks, builds the unlock queue and may prompt fast-forward.
- **NATIVE_PROGRESSION.md, "What was not read: the script side of the turn-in":** now read: the XP reaches the player
  through `UpdateMissionStatus(Complete)` -> `ServerGrantMissionRewards` -> `GetExperienceReward` -> `ExpEarn`
  (source PlotMissionAward or SideMissionAward, type Generic). Also, `ExpLevelUp`'s parameter is `bCheated`.
- **NATIVE_PROGRESSION.md section 3 ("script event `ExpLevelUp(bFeedback)`"):** the parameter is `bCheated`; the level-up
  is driven by the pool's per-frame update.

## What the implementer needs (checklist)

1. A tracker status routine with the allowed-source table, refusals and the order: status/progress/stage lock, script
   hook to local controllers, remote notification, timed entry, weapon grant, initial set, set-active/pending kickoff,
   blocking; then observers, waypoint refresh, directors, `Default` with id 6 + status.
2. `UpdateMissionStatus` delivered to the controller for **every** status in order; its old-status record must exist.
3. `ServerGrantMissionRewards` order: credits, (non-credit) optional credits, XP via `ExpEarn(xp, 2 or 4)`, item
   struct, reward UI; Fire pays XP only (395 at stage 8, confirmed for the amount).
4. `ExpEarn` raises the pool by `Exp * ExpAllPointsScale` (Generic type) capped at the max-level requirement; levels
   come from the pool update calling `ApplyExpPointsToExpLevel(false)`.
5. A pending-kickoff record consumed on the tracker tick (skip while the level is "Loader"), calling `PlayKickoff` when
   the record came from an activation, then marking `bHeardKickoff`.
6. After `CompleteMission`: `OnPlayerTurnedInMission` on the director, then `PlayTurnIn` (`Default` id 14); chain, untrack,
   unlock queue and the (single-player) fast-forward prompt tolerated.
