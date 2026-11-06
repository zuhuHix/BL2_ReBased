# Slice gate: one scripted end-to-end run of the Phase 4 loop

AI-assisted (Claude), lane slice-gate, 2026-11-04. Host behaviour only: nothing in this record was checked against the
running game. A PASS means "the host did this, in this order, in one session". It does not mean the rule matches
Borderlands 2. The rules each link relies on are recorded, with their own verification status, in the documents named
below.

## What it is

ROADMAP.md's Phase 4 gate is one full loop on Sanctuary with Maya: spawn, fight, loot a gun, equip it, use Phaselock,
complete one mission, die, respawn. Other lanes build the pieces. This suite chains them in **one** editor session and
reports each link as PASS, FAIL or NOT_AVAILABLE:

- **NOT_AVAILABLE** means the host does not have that piece yet. The run then uses a stated fallback, so that the later
  links still run.
- A FAIL whose detail starts with "blocked:" failed only because an earlier link left it nothing to work with.

- `tools/test_slice_gate.ps1` does the following:
  - builds the module (Build.bat `-MaxParallelActions=12`; it waits while 2 UnrealBuildTool processes run);
  - takes the machine-wide UE lock (`tools/ue_lock.ps1`);
  - starts `Sanctuary_P` with the Fire mission's manifests (the same ones `tools/test_quest.ps1` uses) plus `-owslicegate`;
  - grades the log with `tools/check_slice_gate.py`.

  Outputs go under ignored `local/slicegate/`: the log, `links-<stamp>.json`, and `links-<stamp>.md` (the table below).
  The exit code is 0 when no link FAILs.
- `host/.../OpenWillowSliceGate.cpp` is the sequence. The walker adds it in BeginPlay when `-owslicegate` is set; that is
  the only shared-file change, three lines in `OpenWillowWalker.cpp`. It drives the host only through public calls:
  - the use key (a simulated E press);
  - `FireOnce`;
  - `EquipItem` / `DrawItemById`, which the inventory page's equip action also calls;
  - `UsePhaselock`;
  - the quest's `Status`, `MinimapMarkers` and `IsRegisteredTargetable`;
  - the `ow.RollGun` and `ow.Range` console commands.

  It implements no gameplay.
- `tests/check_slice_gate_test.py` holds the checker's synthetic tests (27, all made-up log lines).

### How a link is graded (review round 1)

- **Line format:** `OWSLICEGATE link=<name> result=<PASS|FAIL|NOT_AVAILABLE> fixtures=[a; b] detail=<why>`.
  `fixtures` lists the test fixtures that link relied on. The report shows them with the result, for example
  `PASS (fixture: phaselock point granted)`, in the table, the JSON (a `fixtures` list per link) and the markdown.
- **Run structure:** the checker fails the run unless it finds:
  - exactly one start line;
  - then each of the 14 links at most once, in the expected order;
  - then exactly one SUMMARY line that agrees with the link lines.

  A shuffled, repeated or unknown link fails the run, and so does a link line before the start or after the SUMMARY.
- **`enemy_attacks_maya` counts only the selected enemy's hits.** It counts only hits whose causer is the enemy the
  fight selected. The walker counts hits per causer where they land (`GetHitCountFrom`; a few lines in
  `OpenWillowWalker.h/.cpp`), because several hits can land in one tick. It counts only while the gate tracks them, during
  the fight; normal play keeps no such count. Hits from other causers, or with no causer, are reported in the detail. No
  hit from the selected enemy is a FAIL in range mode, and the checker overrules a NOT_AVAILABLE there. It is
  NOT_AVAILABLE only with the stand-in, which has no attack. The fight lasts about 2.4 s and the first hits land about
  1.7 s in, so a much faster kill could FAIL this link: that would show up as a FAIL, not pass unnoticed.
- **NOT_AVAILABLE means a missing capability or precondition**, never a miss. `loot_drop_on_kill` with the gun roll forced
  FAILs if no drop comes, and the checker overrules a NOT_AVAILABLE there too.
- **Every link line carries the fixtures field.** The checker fails a line without it. Logs older than these rules lack
  `accepted=` and fail the current checker, so the tables below stand as graded at the time. A fixture is attached before any result its link can log.
- **The pickup links (review of #98).** Each logs `card_id=<id> picked_id=<id>`: the card's item in the tick of the E
  press, once Maya is on the ground, and the item E put in the backpack.
  - `pick_up_gun` passes only when both are the aimed drop.
  - `pick_up_aimed_not_nearest` repeats the pickup with a nearer decoy 45 uu to Maya's side, about 90 degrees off her
    aim. E must take the aimed drop (#101). If E takes the decoy, the link FAILs. It runs whether or not the kill
    dropped a real gun: the roll source for its two test drops is read before the check for a drop. Rolled ids come from
    the balance and the seed, so two rolls can share one: the link FAILs at setup if the two test drops do.
  - Each link also logs `accepted=<0|1>`: the press made a pickup attempt and it was accepted.
  - The checker reads the ids itself: a PASS on either link with missing or different ids, or without `accepted=1`,
    becomes a FAIL, and so does a PASS on the decoy link whose `nearer_decoy` is missing or has the picked-up id.
  - A test drop with no floor within 450 uu below its point fails; it is never left in the air.
- **A link never starts with Maya down.** If Maya is down when a link group is due to start, every link of that group
  is logged NOT_AVAILABLE ("Maya down from a previous link"), not a misleading FAIL.
- **A drop in the wrong place is a FAIL.** If a new pickup appears after the kill but more than 1,500 uu from the
  corpse, `loot_drop_on_kill` FAILs and gives the drop's position. The corpse position is the enemy's position on its
  last live tick, because a dead actor may be moved.
- **`mission_turn_in` passes only when all three hold:**
  - the mission status is 3 (complete);
  - the experience rose;
  - the lent pistol is neither in the backpack nor held.

### Test shortcuts and fixtures (each one is named in the `fixtures` field of the link that relied on it)

- Maya **teleports** between the places the slice uses: Marcus, the waypoint trigger, a drop. She does not walk there
  (the quest suite does the same). Fixture `teleport`. Aim is set straight on the control rotation.
- One action-skill point is spent on Phaselock at the start, the way the skills page spends it (fixture `phaselock point
  granted`, reported on `phaselock_enemy`).
- When a gun's magazine and reserve are both empty, its reserve is refilled (fixture `ammo reserve refilled`). The stand-in target has 20,000 health, sized
  for level-30 guns, so a level-8 pistol needs about 470 shots.
- **The lethal damage is a fixture** (`ApplyPointDamage`, 1e6), because no enemy can attack Maya yet. It is named on
  `go_down`, and `bleed_out_respawn` names the down state it started from.
- The lent mission pistol is readied with `EquipItem` + `DrawItemById`, as in `test_quest`.
- **Range mode only** (when `ow.Range` exists):
  - `ow.RangeLoot always` before the fight (fixture `guaranteed loot roll`): a range kill rolls the stock 0.085 gun
    chance, and this switch retries up to 8 seeds until a gun comes.
  - After the fight, `ow.RangeMode passive` (fixture `range set passive after the fight`). The enemies stay alive as
    Phaselock targets.
  - Before the mission, `ow.RangeClear` (fixture `range cleared before the mission`), so no range actor attacks Maya or
    takes the mission's shots.
- **Restoring Maya after the fight.** Maya has no shield, and four bullymongs can put her down during the fight (the
  hub's bisect: range #89 alone flipped both `phaselock_enemy` and `mission_objective` that way).
  - The fixture `Maya revived to full health after the fight` (or `healed`, if she is Up but hurt) sets her Up at full
    health through `FOpenWillowMayaState::SetHealth`.
  - If she has already bled out, the walker's own respawn is waited for.
  - **No fight has left her down yet:** in every recorded run she was Up when the fight ended, because the kill gave her
    a second wind or nothing downed her. So the revive has only run when forced: `tools/test_slice_gate.ps1
    -DownAfterFight` (`-owslicegatedownafterfight`) applies lethal damage after the fight (fixture `lethal damage at the
    end of the fight`, named on the next link). In that run the log shows health 0 restored to full and Maya no longer
    down, and the later links ran. This shows the fixture works, not that a real fight reaches it.

## Current link table

Main dee84f6 (the range, #99 and #101 are on main) plus this branch at d5c9cfd, yorad PC, 2026-11-06. Three runs, about
54 s of game time each. **The gate is not green: link 9 FAILs in all three.**

| # | Link | `-Shield` | no switch | `-DownAfterFight` |
|---|---|---|---|---|
| 1 | spawn_maya | PASS | PASS | PASS |
| 2 | reach_fight_area | PASS | PASS | PASS |
| 3 | kill_enemy | PASS (stand-in shield) | PASS | PASS |
| 4 | enemy_attacks_maya | PASS (stand-in shield): 1 hit by the enemy, 1 by another | NOT_AVAILABLE: 1 hit, by another | NOT_AVAILABLE: 1 hit, by another |
| 5 | loot_drop_on_kill | PASS, 61 uu from the corpse | PASS, 61 uu; healed fixture | PASS, 61 uu; lethal and revived fixtures |
| 6 | pick_up_gun | PASS, the real drop | PASS, the real drop | PASS, the real drop |
| 7 | pick_up_aimed_not_nearest | PASS | PASS | PASS |
| 8 | equip_gun | PASS | PASS | PASS |
| 9 | phaselock_enemy | FAIL | FAIL | FAIL |
| 10-12 | mission accept, objective, turn-in | PASS | PASS | PASS |
| 13-14 | go_down, bleed_out_respawn | PASS | PASS | PASS |
| | **PASS / FAIL / NOT_AVAILABLE** | **13 / 1 / 0** | **12 / 1 / 1** | **12 / 1 / 1** |

Link 7 is the first real result for #101: E took the aimed drop past the nearer decoy, and the card and picked-up ids
agree. Card state only: this run uses the canvas HUD. The gate captures no frames.

**Why link 9 FAILs (read from the code and these logs; the fix has not been run).** The logged reason, "cast refused: on
cooldown", was misleading, and the driver now reports what happened. Nothing cast before the gate did, and the first
cast was not refused. It went out and missed, 11 times in the no-switch run ("no preferred target"), and each miss
starts a 1 s cooldown; the retry at the 12 s limit fell inside one. The candidates are rejected because:
- `AOpenWillowWalker::PreferredPhaselockTarget` skips a stock pawn that the quest has not registered as targetable,
  whenever the quest is enabled (5a4762f, 2026-10-02, written for the mission's stock dummy);
- the range's enemies and dummies load stock meshes (72e020f, #76), and nothing registers them.

So in any session with both the quest and `ow.Range`, Phaselock cannot target the range's stock pawns. The Phaselock and
range suites do not pass `-owquest`, so they do not meet this. The pc-hummer runs did pass this link with the same rule
in the code; the likely difference is that its enemies fell back to engine shapes, which is `UNVERIFIED` because those
logs were not read. The cooldown data, the save and the range's AI are not involved: the cooldown read 13 s, the run
starts from a fresh save, and the log has no cast before the gate's. Owner: the range lane.

**Why `-Shield` used the stand-in.** All the shields in this PC's gear manifest are well above Maya's level, and
`EquipGearById` refuses an item above the player's level, which is 8 here. The old fixture text said "no shield in the
gear manifest", which was wrong. The driver now equips the lowest of them by passing its own level to the call, named as
the fixture `level rule overridden by the test: shield <id> of level <n> equipped on Maya of level <m>`. This is a test
override, not how equipping works. It has not run yet, so the stock-shield path is still unproven.

**Why link 4 was NOT_AVAILABLE in two runs.** This was a bug in this lane's driver, and its detail was false: "Maya was
hit, but not by the selected enemy". The range log shows all four enemies hitting Maya before the kill in all three runs,
the selected one included. In those two runs its hit landed in the same tick as the others. The driver compared the hit
count once per tick and credited only the last causer, so it also counted 1 hit where there were 4. It passed in the
`-Shield` run only because the selected enemy's hit came 0.03 s later. The walker now counts hits per causer where they
land, and the driver reads those counts. Only the selected enemy's hits still count. No longer fight or fixture was
needed. This has not run yet; the numbers in the table above are from the old counting and the old grading.

### Before the range was on main

Origin/main 195a016 (after #97, #81, #87) plus this branch, pc-hummer, 2026-11-04.
**10 PASS, 0 FAIL, 4 NOT_AVAILABLE of 14**, about 185 s of game time.

| # | Link | Result | Detail (abridged) |
|---|---|---|---|
| 1 | spawn_maya | PASS | level 8 (slice gear level), health 213/213, grounded, holding a slice pistol |
| 2 | reach_fight_area | NOT_AVAILABLE | no `ow.Range` on main yet; fallback: the host stand-in target (`AOpenWillowCombatTarget`) 600 uu from Maya in Sanctuary |
| 3 | kill_enemy | PASS (fixture: ammo reserve refilled) | stand-in killed with 466 `FireOnce` shots in 136 s (two reserve refills) |
| 4 | enemy_attacks_maya | NOT_AVAILABLE | the stand-in has no attack |
| 5 | loot_drop_on_kill | NOT_AVAILABLE | no pickup within 1,500 uu of the corpse in 4 s |
| 6 | pick_up_gun | PASS (fixture: teleport; gun rolled and dropped by the test) | no enemy drop yet, so the test rolls a gun (`ow.RollGun GD_Itempools.WeaponPools.Pool_Weapons_All_01_Common 8 1`) and drops it 150 uu ahead of Maya (as `-owpickupcardshots` does). Aimed at it, at the E press, with Maya on the ground, the card's state names the drop (`card_id`), and E picks up that same item (`picked_id`). Card state only: this run uses the canvas HUD, so the movie's drawing of the card is not checked |
| 7 | pick_up_aimed_not_nearest | NOT_AVAILABLE (fixture: teleport; two guns rolled and dropped by the test) | the card named the aimed drop at the press; E took the nearer decoy (`PickupNearby`) |
| 8 | equip_gun | PASS | the rolled gun equipped into the free 4th slot and drawn (it is the held weapon) |
| 9 | phaselock_enemy | PASS (fixture: phaselock point granted) | the respawned stand-in lifted (`LastPhaselockHit`, target `IsPhaselocked`), 13 s lock |
| 10 | mission_accept | PASS (fixture: teleport) | the use key at Marcus accepted the Fire mission; the lent pistol is in the backpack |
| 11 | mission_objective | PASS (fixture: teleport) | the trigger was touched and the dummy registered as targetable; one shot from the lent pistol completed the objectives (status 2) |
| 12 | mission_turn_in | PASS (fixture: teleport) | two use-key presses at Marcus (turn-in, then the reward page): status 3, +395 XP, the lent pistol taken back |
| 13 | go_down | PASS (fixture: lethal damage applied by the test) | the lethal fixture put Maya in Fight For Your Life (12 s bleed-out) |
| 14 | bleed_out_respawn | PASS (fixture: down state from the test's lethal damage) | she bled out with no kill and respawned 20 s later at the decoded station, health full, mission still complete |

### With the range stack (throwaway local merge of main + `origin/feature/slice-range-loot`, 2026-11-04)

This is not a merged state: the merge was local and was deleted after the run. **12 PASS, 1 FAIL, 1 NOT_AVAILABLE of
14**:
- `reach_fight_area`: `ow.Range` at the range's spot, which now comes from ignored local data (COMBAT_RANGE.md).
- `kill_enemy`: an adult bullymong killed with 24 shots in 6.3 s.
- `enemy_attacks_maya`: 2 hits credited to the selected enemy (3 more came from other causers).
- `phaselock_enemy`: a range enemy lifted.
- `mission_objective`: PASS, after the range was cleared.
- **`loot_drop_on_kill` FAIL:** the forced roll dropped a gun ("Oppressed Rifle"), but its pickup spawned at the world
  origin, (0, 0, 12), 5,894 uu from the corpse. The range log says "drop_spot no room ... at its feet". The probable
  cause is that the range takes its drop spot from the dead enemy's collision centre, which reads zero. Reported to the
  range lane through the hub.
- `pick_up_aimed_not_nearest` NOT_AVAILABLE: as on main.

### Shield variant (`-owslicegateshield`, `tools/test_slice_gate.ps1 -Shield`)

Maya fights with a shield, so the fight path can run without the restore fixture: only a downed Maya is restored, and
her health after the fight is kept. She gets a shield from the gear manifest when it has one. Otherwise she gets a host
stand-in with her maximum health as its capacity and the default recharge, which is **not stock data**. Both are named
as fixtures, on `kill_enemy` and `enemy_attacks_maya`. If every manifest shield is above her level, the lowest is
equipped under a fixture that says the level rule was overridden (see "Why `-Shield` used the stand-in" above). The
stock-shield path has not run.

Last runs, on a throwaway local merge of f5d5d60 + this variant + `origin/feature/slice-range-loot` 90a7547 (pc-hummer,
2026-11-04), not on main:
- **With `-Shield`:** 13 PASS, 0 FAIL, 1 NOT_AVAILABLE of 14. The stand-in shield was 213; 3 hits were credited to the
  selected enemy and 4 came from other causers; Maya ended the fight at health 89/213, Up, with no restore.
- **Without it:** 13 PASS, 0 FAIL, 1 NOT_AVAILABLE of 14; `Maya healed to full after the fight` fixture.
- **Both:** `loot_drop_on_kill` PASS (the drop 62-63 uu from the corpse, with the range's drop fix), and `pick_up_gun`
  PASS on that real drop, without the test-drop fixture.

## Gaps, with the owning lane

| Gap | Link(s) | Owner | What closes it |
|---|---|---|---|
| With the quest on, Phaselock rejects the range's stock pawns | 9 | range | The range's pawns count as targetable, or the walker's rule applies only to the mission's dummy (`Why link 9 FAILs` above) |
| No fight has left Maya down | revive fixture | slice-gate | Only `-DownAfterFight` runs the revive so far |
| No low-level stock shield in the gear manifest | stock-shield path | maya / skills | A manifest shield at or below Maya's level 8 would remove the level-override fixture. Maya still has no shield at spawn, so the restore fixture stays |
| Generated guns have no firing block | 7 (not graded) | projectiles | `ow.RollGun` guns fire the host's fallback trace (WEAPON_PROJECTILES.md, "Generated guns have no firing block yet"). The equip link does not grade the gun's firing |
| The stand-in is a 20,000-health dummy | 3 | range | Real enemy health per level comes with PR #76 (`range_manifest.json`) |

Links that pass now but rest on rules still `UNVERIFIED` in game are covered in their own records:
- Phaselock: PHASELOCK_STOCK_DATA.md, NATIVE_PHASELOCK_TARGETING.md.
- Mission flow: SANCTUARY_RPG_MISSION.md, FIRE_MISSION_GOLDEN_TRACE.md.
- Down and respawn: COMBAT_DOWN_RESPAWN.md.

## Run history

| Date | Base | Result | Notes |
|---|---|---|---|
| 2026-11-04 | main be94f4a | 9 / 0 / 4 | first green run, after four runs that found bugs in this lane's own driver (see below) |
| 2026-11-04 | main f3947ed (after PR #80) | 9 / 0 / 4 | same links; ctest 21/21, verify_packages exact |
| 2026-11-04 | main 6a2f35f, review round 1 | 9 / 0 / 4 | stricter grading (causer, full turn-in, run structure, fixtures); same links; ctest 21/21, verify_packages exact, checker tests 18/18 |
| 2026-11-04 | main b945b49 (after #91, #92, #95) | 10 / 0 / 3 | link 6 now drops the rolled gun and checks the pickup card names it before E: NOT_AVAILABLE -> PASS (fixture); ctest 21/21, verify_packages exact |
| 2026-11-04 | main 998780c (after #96, #86) | 10 / 0 / 3 | same links; ctest 21/21, verify_packages exact |
| 2026-11-04 | main 8bb74a0 (after #94, #88) | 10 / 0 / 3 | same links; ctest 22/22, verify_packages exact, checker tests 18/18 |
| 2026-11-04 | main 195a016 (after #97, #81, #87), #98 review 1 and batch 6 fixes | 10 / 0 / 4 of 14 | new link 7 (aimed pickup past a nearer decoy) NOT_AVAILABLE; the card and picked ids are cross-checked; ctest 22/22, verify_packages exact, checker tests 21/21 |
| 2026-11-04 | throwaway merge with the range stack | 12 / 1 / 1 of 14 | links 2 to 4 PASS with real range enemies; loot drop at the world origin (FAIL, range); passive, restore and clear fixtures named |
| 2026-11-04 | throwaway merge with #89 90a7547, with and without `-Shield` | 13 / 0 / 1 of 14, both | loot drop at the corpse; link 7 did not test the decoy (its roll source was not read with a real drop; fixed since) |
| 2026-11-06 | main dee84f6 + d5c9cfd, `-Shield`, yorad PC | 13 / 1 / 0 of 14 | link 7 PASS; link 4 names the stand-in shield; link 9 FAIL; ctest 22/22, verify_packages exact, checker tests 21/21 |
| 2026-11-06 | main dee84f6 + d5c9cfd, no switch, yorad PC | 12 / 1 / 1 of 14 | link 4 NOT_AVAILABLE (the one hit came from another enemy); link 9 FAIL |
| 2026-11-06 | main dee84f6 + d5c9cfd, `-DownAfterFight`, yorad PC | 12 / 1 / 1 of 14 | revive fixture ran for the first time (forced); link 4 NOT_AVAILABLE; link 9 FAIL |

The first four runs found bugs in this lane's own driver, not in the host:
1. The kill timeout was 90 s; the level-8 pistol needs about 136 s against 20,000 health.
2. The accept step lost its press window while it waited for the Phaselock lock to end.
3. `ow.RollGun` ran once per tick inside a 50 ms window (9 guns).
4. The use ray was aimed at Marcus's feet (his actor origin), so it hit the counter.

The fixes:
1. The kill timeout is now 240 s.
2. The accept step restarts its clock after the lock ends.
3. Every step action now runs once, behind a flag.
4. The ray is aimed 70 uu above Marcus's origin, and each press logs what the view line hits.

The run is repeated after each hub merge, and a new row is added here.
