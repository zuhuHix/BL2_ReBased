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
- `tests/check_slice_gate_test.py` holds the checker's synthetic tests (18, all made-up log lines).

### How a link is graded (review round 1)

- **Line format:** `OWSLICEGATE link=<name> result=<PASS|FAIL|NOT_AVAILABLE> fixtures=[a; b] detail=<why>`.
  `fixtures` lists the test fixtures that link relied on. The report shows them with the result, for example
  `PASS (fixture: phaselock point granted)`, in the table, the JSON (a `fixtures` list per link) and the markdown.
- **Run structure:** the checker fails the run unless it finds:
  - exactly one start line;
  - then each of the 13 links at most once, in the expected order;
  - then exactly one SUMMARY line that agrees with the link lines.

  A shuffled, repeated or unknown link fails the run, and so does a link line before the start or after the SUMMARY.
- **`enemy_attacks_maya` counts only the selected enemy's hits.** It counts only hits whose causer is the enemy the
  fight selected. The walker now keeps the causer of the last counted hit (`GetLastHitCauser`; 3 lines in
  `OpenWillowWalker.h/.cpp`). Hits from other causers, or with no causer, are reported in the detail and leave the link
  NOT_AVAILABLE.
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

## Current link table

Origin/main 8bb74a0 plus this branch, pc-hummer, 2026-11-04.
**10 PASS, 0 FAIL, 3 NOT_AVAILABLE of 13**, in 183 s of game time.

| # | Link | Result | Detail (abridged) |
|---|---|---|---|
| 1 | spawn_maya | PASS | level 8 (slice gear level), health 213/213, grounded, holding a slice pistol |
| 2 | reach_fight_area | NOT_AVAILABLE | no `ow.Range` on main yet; fallback: the host stand-in target (`AOpenWillowCombatTarget`) 600 uu from Maya in Sanctuary |
| 3 | kill_enemy | PASS (fixture: ammo reserve refilled) | stand-in killed with 466 `FireOnce` shots in 136 s (two reserve refills) |
| 4 | enemy_attacks_maya | NOT_AVAILABLE | the stand-in has no attack |
| 5 | loot_drop_on_kill | NOT_AVAILABLE | no pickup within 1,500 uu of the corpse in 4 s |
| 6 | pick_up_gun | PASS (fixture: teleport; gun rolled and dropped by the test) | no enemy drop yet, so the test rolls a gun (`ow.RollGun GD_Itempools.WeaponPools.Pool_Weapons_All_01_Common 8 1`) and drops it 150 uu ahead of Maya (as `-owpickupcardshots` does). Aimed at it, the pickup card (PR #95) selects that drop (`OpenWillowPickupCard::FindAimedPickup`) and its state names the item before E; E then picks it up (`PickupNearby`). Card state only: this run uses the canvas HUD, so the movie's drawing of the card is not checked |
| 7 | equip_gun | PASS | the rolled gun equipped into the free 4th slot and drawn (it is the held weapon) |
| 8 | phaselock_enemy | PASS (fixture: phaselock point granted) | the respawned stand-in lifted (`LastPhaselockHit`, target `IsPhaselocked`), 13 s lock |
| 9 | mission_accept | PASS (fixture: teleport) | the use key at Marcus accepted the Fire mission; the lent pistol is in the backpack |
| 10 | mission_objective | PASS (fixture: teleport) | the trigger was touched and the dummy registered as targetable; one shot from the lent pistol completed the objectives (status 2) |
| 11 | mission_turn_in | PASS (fixture: teleport) | two use-key presses at Marcus (turn-in, then the reward page): status 3, +395 XP, the lent pistol taken back |
| 12 | go_down | PASS (fixture: lethal damage applied by the test) | the lethal fixture put Maya in Fight For Your Life (12 s bleed-out) |
| 13 | bleed_out_respawn | PASS (fixture: down state from the test's lethal damage) | she bled out with no kill and respawned 20 s later at the decoded station, health full, mission still complete |

## Gaps, with the owning lane

| Gap | Link(s) | Owner | What closes it |
|---|---|---|---|
| No gun range or real enemies on main | 2, 3 | range | PR #76 (`ow.Range`). The suite runs `ow.Range` when the command exists and fights the nearest moving enemy (class name containing `Enemy`) instead of the stand-in |
| No enemy attacks | 4 (and 12's fixture) | range | PR #83 (enemy AI: notice, approach, melee). Once an enemy hits Maya during the fight, link 4 passes. The down link could then use an enemy's damage instead of the fixture; that change belongs to this lane |
| No loot drop when an enemy dies | 5 (and 6's test drop) | range | Loot PR3: spawn an `AOpenWillowInventoryPickup` near the corpse. The VM already rolls the pools on death (COMBAT_LOOT.md); the host does not spawn them yet. With a real drop, link 6 uses it and drops the "gun rolled and dropped by the test" fixture |
| The use key takes the nearest pickup, not the aimed one | 6 | inventory | PICKUP_CARD.md's known gap: E uses `PickupNearby` (nearest within 220 uu); the card uses `FindAimedPickup`. With one drop in reach both agree, so link 6 does not catch the difference |
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
