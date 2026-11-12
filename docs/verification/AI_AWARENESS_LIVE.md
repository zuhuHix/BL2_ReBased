# Bullymong awareness, attack route, melee over time and the AI tree from the running game (lane L9, 2026-10-09)

AI-assisted (Claude), real-game lane L9. One launch of the original game driven through `tools/real_game/` (level-8 Maya, `-Character=Save0008.sav`,
`Install-Driver -BlockSavesAtStart`, run lock `local/ue_run.lock`, memory-only changes, closed through its window). This record holds counts, conclusions
and formulas only: no dump, no listing, no game data. The raw rows (about 100 files, 11 MB) stay under the ignored `local/realgame/L9/`. It answers the
**Live confirmation** lines of [NATIVE_AI_AWARENESS.md](NATIVE_AI_AWARENESS.md) (lane G26) that could be reached, and uses the method of
[AI_MELEE_TARGETING_LIVE.md](AI_MELEE_TARGETING_LIVE.md) (lane L8). Every rule below is **OBSERVED** (with how) or **UNVERIFIED**; where the earlier note and
the game disagree, the game wins and section 11 lists the corrections for the note's owner.

## 0. The answer: how a bullymong notices Maya

**Every bullymong in Three Horns Divide that was sampled (about 25 pawns, dens 15, 16, 17; the rakk drones' den 24 was listed too) is a member of a population den, and the notice rule reads the den,
not the pawn.** The parent of the pawn's `AIComponent` is a `WillowAIDenComponent` owned by a `PopulationOpportunityDen` (not a patrol den); the den's
component holds the record that `SetThreatLevel` writes (`AggroTime`, `ForgetTime`), each member holds a copy whose `ThreatLevel` follows it, and the level
is shared by the whole den: one member noticing alerts all of them. A member's `CurrentTarget` is set in the same target update (same frame) in which its
record reads `ETL_SeenInThreatRadius` or above. Level 2 is requested from a member's target update by either of two rules, and the numbers fit both:

| rule | condition (all must hold) | observed |
|---|---|---|
| **Sight** | the record's `Exposure` from the **previous** update is not `Exposure_None`; the record's `DotToTarget` (facing to the player) is above `WillowMind.SightViewCone` (-4.37e-8, i.e. the front half-plane); the player's **horizontal distance from the den's centre** is below the den's `ThreatRadius` | level 2 one update (0.209 to 0.220 s) after the exposure turned `Full`: 4,660 from the den fired, 4,680 did not (den `ThreatRadius` 4,665.625), member 78 was Full and in front (dot +0.905) from 0.24 s to the end of the 2.4 s dwell at 4,680 and the level stayed 1; at 4,640 with that member Full but behind (dot -0.67) it did not fire; the member's own distance to the player is irrelevant (in 5 of 8 placements of the first batch the nearest member was 5.0 to 7.7 km away and the level still rose) |
| **Proximity** | the member's 3D distance to the player minus its collision radius is below `GlobalsDefinition.AICloseThreatRadius` (768); the player's horizontal distance from the den's centre is below the den's `PatrolRadius` (the den is not a patrol den); no exposure and no cone needed | adult (collision radius 150) behind its facing (angle -132 deg, exposure None): level 2 at the update where d3 - 150 was 728.0; one update earlier it was 770.0 (above 768) and nothing happened; placed 650 behind a member it fired at the first update (0.024 s); with the player 6,200 from the den (above the 3,667.19 patrol radius) members stood within 234 uu of her for 1,886 frames without a level-2 request |

Both rules use the **stale** exposure: the threat iterator runs before the exposure iterator in the Knowledge order, so a change of exposure reaches the level
one update later. A member with a record but no notice stays at `ETL_InThreatRadius` (1) with no `CurrentTarget` for as long as neither rule holds
(minutes at a time in this session, several dens). Relation to lane L8: an adult with `Exposure_None` that never acquired her down to 2,925 uu is exactly
what these rules give (no exposure, 2,925 is far outside the 918 uu proximity reach); its 4,388 and 4,500 uu acquisitions are inside the den radii read here
(4,056 to 5,622) but were not measured against a den, so that part stays UNVERIFIED.
Not seen: a bullymong without a den (or with a patrol den). For that case the note's rule (origin = the pawn, or `ProvokedLocation` when provoked; radius = the
AI definition's `TargetSearchRadius` 4,096) stays UNVERIFIED.

## 1. Method

- **Set-up (as L3/L5/L5b/L7/L8):** run lock taken 09:34:38 when free, saves, config and the SDK settings backed up (`local/realgame/save-backup-20261009-093447`),
  60 files hashed before and after, `Install-Driver -BlockSavesAtStart`, level-8 Maya through the startup mod's `-Character=Save0008.sav` and "Continue",
  `Ice_P` by `WillowGameInfo.InitiateTravel`. Real input: the Enter at "Continue" and **one** left click (the shot of section 3). Everything else was SDK reads,
  Python hooks and script stand-ins (memory only): placing the player (`Pawn.Location` plus `ForceUpdateComponents`, hung with physics none), holding or turning
  an AI pawn, calling `WillowMind.NotifyAttackedBy`, `MeleeDefinition.MeleeAttack` and `ValidTarget` from script, setting `bCloaked` and clearing one pawn's
  `Allegiance` for one call each (both put back), moving one adult 1,200 uu from the player for the shot, and **one write of a den record** (a throwaway
  script wrote `ThreatLevel`, `AggroTime` and `ForgetTime` on the den's record of the player to reset it; it did not matter for the results).
- **Reads:** `tools/real_game/scripts/ai_awareness_probe.py` (new, built on `firing_probe.py` and `ai_melee_probe.py`): the den and its definition, the
  pawn's record and the den's record of the player, `GetThreatLevel`, `IsAwareOfTarget`, one row per den member (distance, angle, exposure, `DotToTarget`,
  threat copy, current target), the player's `NumAICurrentlyTargeting`, per-frame series, `MeleeState`, the tree's node fields.
- **Counts:** 4 approach walks (4,000 frames in all) and 2 dwell batches (17 placements, each with a natural reset between), 3 timelines on the threat level (a proximity run with a
  retreat, a real shot and a script call, each followed for 45 to 70 s; 2,845, 4,159 and 3,731 frames), 1 pawn-state probe for the target change, 1 `ValidTarget` probe (9 questions),
  2 charge runs (535 and 453 frames, 74 sweeps counted), 3 tree recordings of about 60 s (4,429, 4,560 and 4,551 frames; two were on an adult that did not act, section 7); hook rows:
  38 for the shot (several of them duplicates from hooks installed more than once), 1,072 for the tree run on the regular beast.
- **Traps (new; all in the script's docstring):** (1) a den's members **despawn** when the player is further than the den's spawn radius (8,000) for a while and a
  new set spawns when she comes back (pawn names 44 to 91 over the session): pick a live pawn by name at the start of each run. (2) A hung player is **pushed away**
  by melee knockback (once 2.7 km in 10 s) even with physics none, so a long run must re-place her. (3) `fp_player_immune()` hooks `WillowPawn.TakeDamage` only; the
  player pawn's class overrides it, so Maya's health fell from full to well under half once in the session (she was not downed and nothing was saved); a hook on
  `WillowPlayerPawn.TakeDamage` fixed it, and her health was set back in memory. (4) The player's `NumAICurrentlyTargeting` counts every AI that targets her, not
  one den. (5) The record's `Exposure` and `DotToTarget` are the values of the last update; reading them in the same frame as a placement shows the old place.

## 2. The threat level over time (OBSERVED)

- **Fresh records.** The first value visible for a new record of the player, in the den component and in each member, is `ETL_InThreatRadius` (1), not
  `ETL_NoThreat` (0): the 0 is only what `GetThreatLevel` answers when **no** record exists. A fresh member record has `AggroTime` 0, `ForgetTime` 0 (the den's record can
  carry a stale `ForgetTime` from an earlier engagement: 1,070 and 151 were seen on records at level 1), `Exposure_None`, `bCanSeeOrHearTarget` false, and
  `TargetExposureToMe` **-1.0** on its first update (0.0 or the exposure from the second; -1.0 also appeared once or twice mid-run, cause open). Records appeared when the
  player came within about 6.95 km of the member (6,963.75 at the first row; the L8 value) and the den's record 0.16 s earlier (7,206.9).
- **Stamp and refresh (level 2 by notice).** At the update that raises the level the den's record gets `ThreatLevel` 2, `AggroTime` = the world time of that frame
  and `ForgetTime` = AggroTime + **10.0** (the den definition's `MinEngagementTime`, read: 10.0; its `MinProvokedTime` is 25.0). While the notice condition keeps
  holding, `ForgetTime` is raised to now + 10 at **every** member update (797.29, 797.52, 801.27 over 4 s), and `AggroTime` stays. The members' own copies keep
  `AggroTime` 0.0 and `ForgetTime` 0.0 at level 2: the times live only on the den's record.
- **The level does fall back.** After the last refresh, ForgetTime stops; at the first den update after `now > ForgetTime` (the den component updates every 1.0 s:
  the fall came 0.03, 0.81 and 0.92 s after ForgetTime in three runs) the level drops. If the player is still inside the list radius it falls to `ETL_InThreatRadius` (1)
  **in place**: `AggroTime` back to 0.0, `ForgetTime` unchanged, the record stays (proximity run, player 6.2 km from the den). If she was beyond it (the two provoked runs, where
  knockback had pushed her to 9.0 km from the den) the den's record is removed, `GetThreatLevel` answers 0, the copies read `ETL_NoThreat` for one update and are removed 0.01 s
  later. The members' `CurrentTarget` is dropped at their next update (0.13 s later) because keeping a target needs level 2 or more. So the note's "ForgetTime has no reader, nothing
  lowers the level" is **refuted**; what exactly decides between "level 1 in place" and "record removed" (distance, or the level) is open.
- **`IsAwareOfTarget`.** False when no record exists (9.5 km, or after the removal); **True for a fresh record** (level 1, `AggroTime` 0, any time after 0.1 s); False for 0.1 s after
  the level-2 or level-3 stamp and True from then on: last False 0.098, 0.090, 0.084 s and first True 0.114, 0.102, 0.108 s after the stamp in three runs, i.e.
  `MinAggroTimeToConsiderAwareOfTarget` 0.1 + `AggroTime` < now, read as the note says. The fresh-record answer is the note's UNVERIFIED point: **confirmed True**.
  Consequence: an "unaware" bonus would only apply with no record (player beyond about 7 km) or in the 0.1 s after the stamp.

## 3. The attack route (OBSERVED, real shot and script call)

A real shot (the level-8 pistol, one click) at an adult put 1,200 uu from the player (and 6,200 from the den, outside both rules, level 1 for the whole den) hit it: `WillowAIPawn.TakeDamage`
(the gun's live damage, instigator the player's controller) ran, and inside it `WillowMind.NotifyAttackedBy(player pawn, bDamaged true)` (script-visible; the inner
`AIComponent.NotifyAttackedBy` and `SetThreatLevel` are native-to-native and were not seen). Result on the den's record: `ThreatLevel` **3** (`ETL_Provoked`), `AggroTime` = the world time of
the frame (one frame, 0.016 s, before the `g` the hook reads), `ForgetTime` = AggroTime + **25.0** (the den definition's `MinProvokedTime`; refreshed to hit time + 25 by a second call in the
same frame). The four members copied level 3 and acquired her 0.15 s later (their next update), each calling `SetLookAtTarget(player, zero offset)`. A script call
`mind.NotifyAttackedBy(player pawn, True)` gave the same record (stamp = call time, +25.0). The level fell back as in section 2 (0.03 s and 0.92 s after ForgetTime in the two provoked runs),
not at 10 s and not at once. A second hit inside the 25 s (which should refresh ForgetTime) was not tried; `LastAttackedTime`, `LastHurtTime` and `LastAttacker` were **not read**.

## 4. `ValidTarget` and the allegiance (OBSERVED, a regular beast, nine questions)

Player: **true**. `None`: false. The pawn itself: false. Another bullymong (`Allegiance_PrimalBeast`, friendly to it; `bCanTargetFriendlies` false): false. The player with `bCloaked` set: **false**; set back: true.
The pawn's `Allegiance` cleared (memory only, one call, restored): **false** for the player; restored: true. Not tried: a downed player (she cannot be put down safely with saves blocked), a friendly NPC
(none in this map), a vehicle, the `bCanTargetFriendlies` switch.

## 5. What the target change does (OBSERVED, one regular beast without a prior target, six AIs on the player)

At the update that acquired her: `WillowMind.bProvoked` False -> **True**, `ProvokedLocation` (0,0,0) -> **the pawn's location at acquisition** (27,492.73 / 39,511.00 / 3,882.84 against 27,492.94 / 39,520.18 /
3,883.34 a moment before), `SetLookAtTarget(player, zero offset)` called on the pawn in that frame (two hook rows per call; the hook may have resolved two declarations), the player's
`NumAICurrentlyTargeting` 0 -> 4 (four members) in one provoked run and 6 in another (all AIs on her), and back to 0 when the targets were lost. Pawns spawned by a den can already have `bProvoked` true (seen on four adults of den 15 before anything had noticed her), so the flag is not a record of "has been noticed". The three pawn notifications and the presentation calls were not visible.

## 6. Melee over time, `MeleeDef_Melee_Charge` (OBSERVED, script call of the definition on a regular beast)

Definition values read: `bMeleeOverPeriodOfTime` true, `bHitEachTargetOnlyOnce` **true**, `bStopMeleeOnLanded` true, `MaxMeleeDuration` 5.0, `AttackInterval` 0.0667, `DamageRadius` 512, `DamageHeight` 80, `DamageConeAngle` 64, `TraceDistance` 340.
- **Entry:** `MeleeAttack(pawn)` set `MeleeState.bIsMeleeActive`, `MeleeDefinition` (the charge), `MeleeStartTime` = the call time and `MeleeStartLocation` = the pawn's `Location` (no eye offset) in that frame; the first hit on the player (damage 24.98) came at the call time.
- **Repeats:** 74 accumulator subtractions in the 5.007 s of the melee (median spacing 0.0674 s, min 0.0475 and max 0.0836 at frame granularity; 5.007 / 0.0667 = 75.1): the repeat sweep runs at 1/15 s as the note says. The player was hit **once** in the 5 s (HitActors held her from the first call, a second actor joined later): the hit-once flag stops repeated damage, so the note's "expect hits every 0.0667 s" does not apply to this definition. With the player outside the volume at the call and moved in at t = 1.0 s the hit came 0.0035 s after (inside one sweep interval); moved out and in again at t = 3.0 s: no second hit.
- **End:** `MeleeState` turned inactive in the first frame past 5.0 s (5.0072 and 5.0077 s after the start), `HitActors` emptied, `MeleeDefinition` None.
- **`GetMeleeTraceSourceLocationAndRotation`** (callable with two empty structs; the out values come back as a tuple): the location equals the actor's `Location` exactly and the rotation equals the actor's `Rotation` exactly on the one pawn tried (confirms "no eye offset"; the aim-offset part was not separated because the pawn had no aim difference).

## 7. The AI tree (OBSERVED, one regular beast in a fight for 62 s; the pawn's own copy of the tree, 11 nodes)

The run state sits in the pawn's `AIDef.NodeList` copy; a node's `ActiveChild` is an **index into its Children array** (not a node index). Observed:
- **`TimedStart` re-arm:** on `Target_Acquired` (hook rows, in the same frame) the perch node's `NextTimedRun` became now + **10.0003** (`TimedStart` 10 to 10); the leaf changed in the same frame (the acquisition selected the leap path). `Target_Lost` and the change to Patrol were in the same frame too.
- **`TimedReset`:** when the perch (`Action_CombatPerch`, `TimedReset` 20 to 30) finished, `NextTimedRun` was written as now + **27.05** (inside the window); the node was chosen 0.89 s after its `NextTimedRun` had passed, in the update where its `CanRun` stopped answering `NoPathToPerch`, and ran 0.40 s.
- **Random node (weights 1.0 : 0.5, `LoopCountMax` 0, 0):** `LoopCount` 4 with `LastRandomChild` 1 -> **1 / 0** when child 0 finished (a different child resets the count to 1 and sets the last child) -> **2 / 0** and **3 / 0** on the next finishes of child 0 (same child, +1) -> **1 / 1** after child 1 (the leap) finished. As the note says. The `LoopCountMax` cap was **not exercised** (both maxima 0).
- **`ActionFailName` values seen:** NoPathToPerch, NotReachable, PathingFailed, NoTarget, NotScripted, InvalidLocation (the `CanRun` answers); a node with a non-None name was skipped in the same update (the tree went to the next priority child).
- **`LastFailTime` stayed 0.0 on every node for 62 s:** no action finished with `bFailed`, so the 1.0 s block after a failure and the "`Finish` re-selects at once" rule were **not exercised** (the re-selection on `Target_Lost` and `Target_Acquired` was seen, but those come from the state update, not from `Finish`).
- A separate adult (pawn 91, full hit points, the player as `CurrentTarget`, 425 uu away) showed **no** action activity in two recordings of 60 s (no hook rows, node state frozen after the first second); cause unknown (not held, `bDisabledDueToPopulationIrrelevance` false, no holds). Its neighbours acted normally.

## 8. Den and definition facts read on the way (OBSERVED)

| item | value |
|---|---|
| den 15 / 16 / 17 | `ThreatRadius` 4,056.25 / 4,665.625 / 5,621.875; `PatrolRadius` 3,490.625 / 3,667.1875 / 5,153.125; `SpawnRadius` 8,000 / 8,000 / 8,595.3125; `bPatrolDen` false for all; `MaxActiveActorsIsNormal` 2, `MaxActiveActorsThreatened` 4 / 4 / 3; `SharedThreatDens` empty |
| den definition (`WillowAIDenDefinition`) | `MinEngagementTime` 10.0, `MinProvokedTime` 25.0, `TargetSearchRadius` 4,096, same `Targeting_ShortRange`; den component `UpdateRate` 1.0; pawn component `UpdateRate` 0.2 |
| pawn side | `AIDef.bGetTargetsFromParents` false; `GetParentComponent()` is the den component; `MyDen` None on the pawn's own component; adult / regular beast collision radius 150 / 100 |
| mind | `SightViewCone` -4.371e-8 (a cosine threshold of 90 deg), `MinAggroTimeToConsiderAwareOfTarget` 0.1, `SightCounterInterval` 0.2, awareness zone collections all None (so the awareness iterator is not in play), `AtLeastOneKnownThreat` 0 |
| globals | `AICloseThreatRadius` 768 |
| alert effect | when the den's level rose, new members appeared in the same den (babies 72 and 73 in den 15 after the notice): consistent with `MaxActiveActorsThreatened` and the note's den helper, not shown to be caused by it |

## 9. Save safety

`Backup-Saves` copied `SaveData` and `Config` first (`local/realgame/save-backup-20261009-093447`, plus the SDK settings and Sanity Saver copies); 60 files hashed before and after. With saves blocked from the first
frame the 33 save and config files were byte-identical after the session. Two files differed, as in L3/L5/L5b/L7/L8: Sanity Saver's private `Save0008.json.gz` (rewritten at load) and the SDK's `python-sdk.json` (update-check stamp). Both were restored from the backup;
re-hash: **0 differences of 60**. The driver folder was removed, the game closed through its window (2 s), no process killed, the lock released. No crash, no Application Control block.

## 10. Status of the NATIVE_AI_AWARENESS.md rules

| rule (section of the note) | status | matching number |
|---|---|---|
| `GetTargetsFromParents` / lone case: "a lone bullymong has no parent" (GetTargets) | **refuted for den-spawned bullymongs**: the parent exists (a den component); `bGetTargetsFromParents` false (confirmed); den not a patrol den | about 25 pawns, 3 dens (and den 24 listed) |
| the threat rules use the pawn's location and the AI's 4,096 radius when there is no non-patrol den (CalcThreat) | **not exercised** (every pawn had a non-patrol den); the den branch is **confirmed** below | |
| CalcThreat sight rule: `Exposure` not None, in the cone (`DotToTarget` > `SightViewCone`), horizontal distance from the threat origin below the threat radius | **confirmed**, with the den as origin and `ThreatRadius` as radius; the exposure is the previous update's; the member's own distance is irrelevant | fired at 4,660, not at 4,680 (radius 4,665.625); not at 4,640 with dot -0.67; lag 0.209 to 0.220 s |
| CalcThreat proximity rule: horizontal distance from the origin below the patrol limit (den's `PatrolRadius` for a non-patrol den) and 3D distance minus collision radius below `AICloseThreatRadius`; vehicles skipped | **confirmed** (vehicles not tried) | 770.0 no, 728.0 yes (768); beyond the patrol radius (dorig 6,200 > 3,667.19) no, 1,886 frames |
| the gate "pawn has its collision component" in CalcThreat | **unchanged** (not tested) | |
| `SightViewCone` is a cosine threshold; patrol den skips the cone | **cone confirmed** (value -4.371e-8 = cos 90 deg, front half-plane; sign test only: +0.91 fires, -0.67 does not, the boundary near 0 not resolved); patrol-den skip not tried | |
| `ValidCurrentTarget`: level at least 2 to acquire and to keep; `CurrentTarget` stays None while the level is 0 or 1 | **confirmed**: no target for minutes at level 1 with records and Full exposure out of the cone or radius; set in the same update as level 2; kept 10.8 s with the player 6 km away; dropped 0.13 s after the level fell | v1, w1, t2, sh2, at1 |
| `ValidCurrentTarget`: the actor must not be `bHidden` | **unchanged** (not tested) | |
| `InitializeTargetRecord`: `ThreatLevel` set to `ETL_NoThreat` | **refuted as visible**: the first value seen for a record is `ETL_InThreatRadius` (1); 0 only without a record; `TargetExposureToMe` -1.0 on a fresh record's first update | den and member records, 4 runs |
| `TargetRemoved` hand-off | **not exercised** (no vehicle) | |
| `TargetChanged`: `bProvoked` and `ProvokedLocation` on acquisition; `SetLookAtTarget`; counters | **confirmed** for `bProvoked` False->True, `ProvokedLocation` = pawn location, `SetLookAtTarget(player, zero offset)` in that frame, `NumAICurrentlyTargeting` up by one per AI and back to 0; the three pawn notifications and the presentation calls **not visible**; `SetLookAtTarget` on hold/release not tried | pawn 90: provloc 27,492.73 / 39,511.00 / 3,882.84 |
| `NotifyAttackedBy` (component): `AddTarget` then `SetThreatLevel(attacker, Provoked)`; den helper before the change | **confirmed** for the level (3) and the stamps; the den helper's `TriggerProvokedEvents` **not observed** (the den spawned more members; not shown to be caused by it) | real shot and script call |
| `SetThreatLevel`: forwarded to the root; the level only rises; `AggroTime` stamped once; `ForgetTime` = max(old, now + d); d = 10 (`MinEngagementTime`) for level 2, 25 (`MinProvokedTime`) for level 3 when the AI definition is a den definition | **confirmed** for all of it on the den's (root) record, with d from the den definition (10 and 25; the note's 2.5 and 10 defaults for non-den definitions were not exercised); `ForgetTime` is refreshed at every update while the condition holds | Ag 365.758 / Fg 375.758; Ag 1188.052 / Fg 1213.052 |
| `SetThreatLevel` writes the pawn's own record | **refuted as written**: `AggroTime` and `ForgetTime` are written on the **den's** record only; the members' copies keep 0.0 / 0.0 and only copy `ThreatLevel` | member copies at level 2 and 3 |
| `GetThreatLevel`: walks up the chain; friendlies answer 2 | **chain confirmed** (answers the root level for members); the friendly case not tried | |
| "no reader of `ForgetTime`; nothing lowers a threat level" | **refuted**: after `now > ForgetTime` the level falls at the den's next update (to 1 in place, or the record is removed) | 0.03 / 0.81 / 0.92 s after ForgetTime |
| `WillowMind.NotifyAttackedBy`: component call plus `LastAttackedTime`, `LastHurtTime`, `LastAttacker` when not friendly | **partly confirmed**: the call from `TakeDamage` with `bDamaged` true (hook row); the three field writes **not read** | |
| `IsAwareOfTarget`: no record false; else `MinAggro + AggroTime < now` | **confirmed**; fresh-record (`AggroTime` 0) answer **True** (the note's UNVERIFIED point) | 0.1 s switch, 3 runs |
| `AddTargetable` / `NotifyTakeHit` | **not seen**: the hook on `NotifyTakeHit` never fired for a shot bullymong (native-to-native) | |
| Awareness zones: the bullymong's targeting has no awareness iterator | **confirmed** (all zone collections None) | |
| `ValidTarget` base and Willow rules (player true; None, self, friendly false; cloaked false; no allegiance false) | **confirmed for those six**; downed player, friendly NPC, vehicle and `bCanTargetFriendlies` **not tried** | 9 questions |
| `FindTargets` hidden-owner rule (Neutral allegiance skipped) | **not tested** (only the `ValidTarget` allegiance step was) | |
| IMeleeAttacker: trace source location = actor `Location` for an AI pawn | **confirmed** (equal to 0.01 uu); rotation = actor `Rotation` (equal; aim-offset case not separated) | 1 pawn, 2 reads |
| `MeleeAttack` calls `BeginMeleeOverTime` and sweeps once at once; state fields | **confirmed**: active flag, definition, start time = call time, start location = `Location` | charge run |
| per-frame repeat at `AttackInterval` clamped to at least 1/15 s; end after `MaxMeleeDuration`; the sweep of the frame still happens; hit-once list | **confirmed**: 74 sweeps in 5.007 s; ends 5.0072 and 5.0077 s after the start; `HitActors` cleared; one hit on the player in 5 s because `bHitEachTargetOnlyOnce` | charge runs |
| tree: node eligibility order, 1.0 s failure block | **partly**: `NextTimedRun` gating and `ActionFailName` gating seen; the 1.0 s block **not exercised** (`LastFailTime` stayed 0) | 62 s |
| tree: `TimedStart` re-arm on acquiring a target; `TimedReset` after a node finishes | **confirmed**: +10.0003 at `Target_Acquired`; +27.05 inside 20 to 30 after the perch finished | 1928.7108 - 1918.7108; 1902.607 - 1875.560 |
| tree: random node counters; `LoopCountMax` cap | **counters confirmed**; the cap **not exercised** (both maxima 0) | LoopCount 4->1->2->3->1 with last child 1->0->0->0->1 |
| `ActionSequence.Finish` re-selects at once | **not exercised** (the re-selection on `Target_Lost` / `Target_Acquired` in the same frame was seen) | |
| leaps, chase natives, combat zones, holds, leaders | **not tested** | |

## 11. Corrections for the owner of NATIVE_AI_AWARENESS.md

1. "Lone case": spawned bullymongs have a den parent; the threat rules and the stamps work on the den (threat origin = the den's location, radius = the den's `ThreatRadius`, patrol limit = the den's `PatrolRadius`); the pawn's own location and the 4,096 radius were not the operative values.
2. "`ForgetTime` has no reader, nothing lowers a threat level": the level falls at the den's next update after `ForgetTime` (to 1 in place, or the record is removed); `AggroTime` returns to 0.
3. "`SetThreatLevel` stamps the pawn's record": it stamps the den's record; members only copy the level.
4. "Fresh record starts at `ETL_NoThreat`": the first visible level is `ETL_InThreatRadius`; `GetThreatLevel` is 0 only when there is no record. `TargetExposureToMe` -1.0 is the fresh-record value of the first update.
5. Defaults for non-den definitions (2.5 s and 10 s) were not seen; the den definition's 10 and 25 are the operative ones for a den-spawned bullymong.
6. Add: exposure in `CalcThreat` is one update stale; level 2 and `CurrentTarget` come in the same update; the 0.1 s `IsAwareOfTarget` window and the fresh-record True.
7. Melee over time: add `bHitEachTargetOnlyOnce` as the reason a charge hits a target once.

## 12. Open items

- The cone boundary near 0 (a held, facing-locked member with the other members excluded; place the player at 85 and 95 degrees), and the sight rule's gate on the collision component.
- A bullymong without a den or in a patrol den (rakk drones, other maps), and `ProvokedLocation` as the origin there.
- Why the record is kept at level 1 in one geometry and removed in another after `ForgetTime` (list radius, or the level), and what the 6.95 km add / 7.07 km drop of L8 are in terms of the den (SpawnRadius 8,000 was not a drop at 7.2 to 8 km).
- Second hit refreshing `ForgetTime` (+25 each), `LastAttackedTime` / `LastHurtTime` / `LastAttacker` reads, `Hold` / `Release` with the phaselock, the leap numerics.
- An action finishing with `bFailed` to see `LastFailTime` and the 1.0 s block; a tree with a non-zero `LoopCountMax`; why one adult showed no action for 60 s.
- Downed player, friendly NPC and vehicle for `ValidTarget`; the `bCanTargetFriendlies` switch.
