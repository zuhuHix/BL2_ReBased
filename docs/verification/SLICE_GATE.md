# Slice gate: one scripted end-to-end run of the Phase 4 loop

AI-assisted (Claude), lane slice-gate, 2026-11-04. Host behaviour only: nothing in this record was checked against the
running game. A PASS means "the host did this, in this order, in one session". It does not mean the rule matches
Borderlands 2. The rules each link relies on are recorded, with their own verification status, in the documents named
below.

## What it is

ROADMAP.md's Phase 4 gate is one full loop on Sanctuary with Maya: spawn, fight, loot a gun, equip it, use Phaselock,
complete one mission, die, respawn. Other lanes build the pieces. This suite chains them in **one** editor session and
reports each link as PASS, FAIL or NOT_AVAILABLE:

- **NOT_AVAILABLE** means the host does not have that piece yet, or a precondition is missing. The run then uses a stated
  fallback, so that the later links still run. A detail starting "test setup gap" is this test's own setup or local data
  (no `ow.RollGun` source, no weapon generator manifest), not something the host lacks: the link did not test the host.
  The checker gives it to slice-gate and counts it on its own line.
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
- `tests/check_slice_gate_test.py` holds the checker's synthetic tests (67 since review round 3 of the unforced variant, 39 before it; all
  made-up log lines, plus three that read the runner script's text).

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
  NOT_AVAILABLE only with the stand-in, which has no attack.
- **Fire is held for the selected enemy's first hit (range mode).** Otherwise a quick kill could end the fight before that
  enemy attacks. The driver fires one shot every 0.5 s until the enemy has lost health (fixture `one shot at a time until
  the selected enemy was wounded`), then holds fire until the enemy has hit Maya. One shot at a time is needed because
  `FireOnce` has no fire-rate gate and a shot takes time to land: firing every tick put 6 or 7 shots in flight before the
  first landed (see "With the hold on fire"). A run where the enemy hits before it is wounded never holds. The hold ends
  one of four ways, and links 3 and 4 name which as a fixture; link 3's detail
  also gives the seconds held:
  - `fire held until the selected enemy hit Maya`: the enemy hit her. This wins if the hit and its death share a tick.
  - `fire held, ended by timeout`: 15 s passed. A real hit from the enemy after this is still a PASS, with this fixture.
  - `fire held, ended by Maya down`: she went down first, and a kill can still give her a second wind.
  - `fire held, ended by enemy died`: it died with no hit on her, for example from damage over time. Link 4 FAILs in range
    mode, because it never attacked.
- **A down before the selected enemy's first hit FAILs link 4**, held or not, even if the enemy hits her later, when she is
  Up again. The driver records this at her first down of the fight, from her down state and from the down time
  (`FOpenWillowMayaState::DownAt`), so a down and a second wind between two of its ticks still counts. It compares against
  the selected enemy's hits counted on that tick, so a hit landing in the same tick as the down counts as before it.
- **What the checker sees of this.** It overrules a link 4 PASS naming `ended by Maya down` or `ended by enemy died`. With no
  hold there is no such fixture, so only the host enforces a down before the enemy's first hit.
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
  - The checker reads the ids itself. On either link, a PASS becomes a FAIL unless `aimed_drop`, `card_id` and `picked_id`
    are all there, not `none`, and the same item, with `accepted=1`. On the decoy link, `nearer_decoy` must also be there,
    not `none`, and a different item. A NOT_AVAILABLE on either link whose detail carries the ids also becomes a FAIL: the
    pickup attempt was made, so nothing was missing.
  - The two-guns fixture is named only once both test drops exist. If one could not be made, the link FAILs and its detail
    says how far the setup got.
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
- **The lethal damage is a fixture** (`ApplyPointDamage`, 1e6). It was written before the range, when no enemy could attack
  Maya; the range's enemies attack her now (link 4), but none has downed her in a gate run. It is named on `go_down`, and
  `bleed_out_respawn` names the down state it started from. The unforced variant below leaves it out.
- **`FireOnce` ignores the gun's fire rate.** It fires whenever the driver calls it, every tick outside the wait for the
  wound, so link 3's shot counts and kill times are not what the gun would do in play. Link 3 does not grade time.
- The lent mission pistol is readied with `EquipItem` + `DrawItemById`, as in `test_quest`.
- **Enemies on fallback shapes are stated up front.** If the selected range enemy has no stock mesh (the range's
  `fallback=shapes`, for example a manifest with no mesh), `reach_fight_area` names the fixture `selected enemy on fallback
  shapes, no stock mesh`. It does not stop the run.
- **Range mode only** (when `ow.Range` exists):
  - `ow.RangeLoot always` before the fight (fixture `guaranteed loot roll`): a range kill rolls the stock 0.085 chance
    of the guns-and-gear pool (a gun about 0.043, see the unforced variant), and this switch retries up to 8 seeds until a
    gun comes.
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
  - If the pack already has her down when the fight ends, the lethal fixture is skipped and the revive runs on that real
    down: `Maya revived to full health after the fight` without `lethal damage at the end of the fight` means this.

## Current link table

Batch Z16: #126 at 5a39f57 merged with #133 at 51deb95, five enemies with a Psycho, zuhu, 2026-11-07, all three variants.
This merge is not on main. **14 PASS, 0 FAIL, 0 NOT_AVAILABLE of 14 in all three**, stock enemies (`fallback=shapes` 0). The
fixtures are those of the table below, plus these on links 3 and 4: `one shot at a time until the selected enemy was
wounded` and `fire held until the selected enemy hit Maya`.

- **Link 3:** the Psycho was selected in all three, and the hold started on the first wound and lasted 3.0 s. Three shots
  killed it, one a critical hit, in 3.5 to 3.6 s.
- **Link 4:** 1 hit by the selected enemy and 4 by others, in all three. All five enemies hit Maya in the same round.
- **Maya after the fight:** 28/213 without a shield, then healed or revived by the fixture. One round of the pack took her
  that low, so the kill came before a second round could down her. The margin is small: the second round was due about
  0.7 s after the kill (arithmetic from the 1.25 s swing interval, not measured).
- **No enemy downed her** and no second wind happened. The down at link 13, and the forced one under `-DownAfterFight`, came
  from the test's lethal damage.
- `-Shield` used a manifest shield under the level override, with the hits absorbed by it.

Everything in "What this does not show" below still applies. Three runs with the same selection and the same three shots
vary little.

### Main cdf05d7 + c365984, before the hold on fire

Main cdf05d7 plus this branch at c365984, yorad PC, 2026-11-06. Three runs, about 50 s of game time each. **14 PASS, 0
FAIL, 0 NOT_AVAILABLE of 14 in all three**, graded by the current checker.

| # | Link | Result in all three | Fixtures |
|---|---|---|---|
| 1 | spawn_maya | PASS | none |
| 2 | reach_fight_area | PASS | none |
| 3 | kill_enemy | PASS | `-Shield` only: `level rule overridden by the test: shield <id> of level <n> equipped on Maya of level 8` |
| 4 | enemy_attacks_maya | PASS: 1 hit by the selected enemy, 3 by others | `-Shield` only: the same level-override fixture |
| 5 | loot_drop_on_kill | PASS, 61 uu from the corpse | `range set passive after the fight`; `guaranteed loot roll`. No switch also: `Maya healed to full after the fight`. `-DownAfterFight` also: `lethal damage at the end of the fight`; `Maya revived to full health after the fight` |
| 6 | pick_up_gun | PASS, the real drop, `accepted=1` | `teleport` |
| 7 | pick_up_aimed_not_nearest | PASS, the aimed drop past the nearer decoy, different ids, `accepted=1` | `two guns rolled and dropped by the test`; `teleport` |
| 8 | equip_gun | PASS | none |
| 9 | phaselock_enemy | PASS: a range enemy lifted, 13 s lock | `phaselock point granted` |
| 10 | mission_accept | PASS | `range cleared before the mission`; `teleport` |
| 11 | mission_objective | PASS | `teleport` |
| 12 | mission_turn_in | PASS: status 3, +395 XP, lent pistol taken back | `teleport` |
| 13 | go_down | PASS | `lethal damage applied by the test` |
| 14 | bleed_out_respawn | PASS | `down state from the test's lethal damage` |

These runs came before #109's second review round. The current checker regrades their three logs to the same 14 / 0 / 0.
The driver changed after them only in the setup-gap wording, the two-guns fixture's place and the fallback-shapes fixture.
These runs had no setup gap and stock enemies, and both test drops were made, so the same logs would be expected on the
newer driver, but that has not been run.

**What this does not show:**
- **Enemies have not downed Maya.** Every down state in these runs came from the test's lethal damage. The revive ran only
  when forced, and no real fight has reached it.
- **No run shows Maya carrying on unaided after the fight.** Without the shield she ended the fight at 60/213 and a
  fixture healed her. With the shield the hits never reached her health (213/213), but it is far above her level and much
  stronger than a level-8 shield, so that run says little about a real fight. The fixture overrides the level rule, so
  this is not normal equip behaviour. The recharge values read from its card are `UNVERIFIED`.
- **Loot is forced and mission travel is by teleport.** The pickup is checked on card state only (canvas HUD). The gate
  captures no frames, so nothing here was checked visually.
- **Nothing is checked against the real game.** The three runs also vary little: the same 11-shot kill in about 2.4 s.
- **Link 4's margin was small in these runs.** The first hits landed about 0.6 to 0.7 s before the kill, with fire not
  held back. The driver now holds fire for that first hit (see the grading section), which these runs do not cover.

### With the hold on fire, before one shot at a time (zuhu, integration batches, 2026-11-06 and 07)

Both batches fired every tick until the enemy was wounded. Neither is the current driver, and neither is on main. This lane
read their logs; the hub's runner ran them. All three variants in each.

- **Z10, #126 at 7f59924 merged with #133 at d02c70a, four enemies: 14 / 0 / 0 each.** The hold ended by a hit each time,
  after 3.3 s of hold. No shield: 53/213 after the fight. This passed by margin, not by design: the opening burst left the
  selected bullymong with very little health before the hold started, and one or two more shots in flight would have killed
  it.
- **Z13, #126 at 7f59924 merged with #133 at 716302b, five enemies with a Psycho: 13 / 1 / 0 each.** Link 4 FAILed, correctly.
  The Psycho was the nearest enemy, so it was selected. It died from the burst about 0.1 s into the fight, with the fixture
  `fire held, ended by enemy died`, before any enemy reached Maya. She did not go down. The den had already noticed her by
  sight 0.5 to 0.9 s before the fight, in both batches, so the first shot is not what alerts it.

### Before the Phaselock targeting fix (#111)

Main dee84f6 (the range, #99 and #101 are on main) plus this branch at d5c9cfd, yorad PC, 2026-11-06. Three runs, about
54 s of game time each. Link 9 FAILed in all three, and these were graded by the checker of that time.

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

**Why link 9 FAILed (read from the code and these logs; #111 fixed it, and the runs above confirm it in the host).** The
logged reason, "cast refused: on
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
override, not how equipping works. It ran in the current table's `-Shield` run: a manifest shield was equipped and the
walker read its capacity from the card.

**Why link 4 was NOT_AVAILABLE in two runs.** This was a bug in this lane's driver, and its detail was false: "Maya was
hit, but not by the selected enemy". The range log shows all four enemies hitting Maya before the kill in all three runs,
the selected one included. In those two runs its hit landed in the same tick as the others. The driver compared the hit
count once per tick and credited only the last causer, so it also counted 1 hit where there were 4. It passed in the
`-Shield` run only because the selected enemy's hit came 0.03 s later. The walker now counts hits per causer where they
land, and the driver reads those counts. Only the selected enemy's hits still count. No longer fight or fixture was
needed. The numbers in the table above are from the old counting and the old grading; the current table has the new ones.

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

Since 2026-11-08 the plain run starts with the stand-in shield, as normal play does; the `-Shield` and `-DownAfterFight` runs pass `-ownostartshield` and keep the no-start-shield baseline below (see [COMBAT_DOWN_RESPAWN.md](COMBAT_DOWN_RESPAWN.md), "Start shield"). The older text below that says Maya has no shield at spawn describes those runs and the earlier plain runs.

Maya fights with a shield, so the fight path can run without the restore fixture: only a downed Maya is restored, and
her health after the fight is kept. She gets a shield from the gear manifest when it has one. Otherwise she gets a host
stand-in with her maximum health as its capacity and the default recharge, which is **not stock data**. Both are named
as fixtures, on `kill_enemy` and `enemy_attacks_maya`. If every manifest shield is above her level, the lowest is
equipped under a fixture that says the level rule was overridden (see "Why `-Shield` used the stand-in" above). The
stock-shield path has run only under that override.

**The two machines' `-Shield` runs do not exercise the same path.** Which one runs depends on the machine's gear
manifest. On the yorad PC (zuhu), the run used a manifest shield under the level override. On pc-hummer, it used the host
stand-in, which is not stock data. A PASS on one says nothing about the other's path.

Earlier runs, on a throwaway local merge of f5d5d60 + this variant + `origin/feature/slice-range-loot` 90a7547 (pc-hummer,
2026-11-04), not on main:
- **With `-Shield`:** 13 PASS, 0 FAIL, 1 NOT_AVAILABLE of 14. The stand-in shield was 213; 3 hits were credited to the
  selected enemy and 4 came from other causers; Maya ended the fight at health 89/213, Up, with no restore.
- **Without it:** 13 PASS, 0 FAIL, 1 NOT_AVAILABLE of 14; `Maya healed to full after the fight` fixture.
- **Both:** `loot_drop_on_kill` PASS (the drop 62-63 uu from the corpse, with the range's drop fix), and `pick_up_gun`
  PASS on that real drop, without the test-drop fixture.

## The unforced variant (`-Unforced`, 2026-11-11)

AI-assisted (Claude Opus 5.5), lane unforced-gate. **Not run yet, and the C++ was not compiled when this was written:** the
build slot was held by a test batch. Everything in this section describes the driver and the checker as written. None of it
is a result, and none of it is checked against the real game.

The three older variants show that the host can do each step. Four of their links pass only on stand-ins: the forced gun
(`ow.RangeLoot always`), the granted Phaselock point, the lethal-damage down, and the respawn that follows from that down.
The mission is accepted and turned in with the use key alone, the range is cleared before it, and Maya is healed after the
fight. `tools/test_slice_gate.ps1 -Unforced` (host switch `-owslicegateunforced`) runs the same 14 links in the same order
without those stand-ins. Each link then shows whether the loop got there on its own.

**What it leaves out, link by link:**
- **Loot (link 5).** The range's loot mode is set to `normal`, so each kill rolls the stock chance. The link kills range
  enemies, the nearest living one each time, until a gun drops near a corpse. The fight's kill counts as the first. The budget
  is 100 kills or 720 s of extra kills, whichever comes first. A PASS says `how=kill_roll kills=<n> budget=100`. Any gun that
  was not on the ground before the fight counts when it lies within 1,500 uu of the last kill or of any range enemy that died
  from the pickup snapshot before the fight to the end of the link (a stray shot's kill, or a death during the fight or the
  recovery, included); a gun far from all of them FAILs the link, but only after the 1 s drop wait.
- **No heal or revive.** After the fight the range is not set passive and Maya is not restored. A link group that is due while
  she is down waits up to 50 s for her own second wind or her respawn. If she is still down after that, the older variants'
  skip applies.
- **Phaselock (link 9).** The point is bought on the skills page: the page opens on the action skill, and Enter is pressed
  once a second for up to 30 s. For the first 6 s Enter goes through Slate, as a keyboard sends it. After that it is also
  dispatched on the page itself (`SendPageKey`), because the page only sees a Slate key when it has the focus. The PASS says
  `how=skills_page`, `key=slate` or `key=page_event` for the kind of the last Enter before the spend, and how many presses of
  each kind were sent. The token covers both kinds: each runs the page's own spend path (`spendSelected`, then the HUD's
  `TrySpend`), but `key=page_event` is a synthetic browser event, not proof that a keyboard press reaches the page. If the page buys nothing, the point is granted as in the
  other variants. That is named as the fixture, so the cast is still tried, and the checker fails the link.
- **Mission (links 10 and 12).** At Marcus, the use key opens his mission page. Enter on it accepts. At the turn-in, Enter
  turns the mission in and a second Enter takes the reward. The keys go through Slate to the page's own key handler
  (`UOpenWillowQuest::OnScreenKey`), the way `tools/test_mission_page.ps1` presses them. The `how=mission_page` token is
  printed only when the page did the work: at the accept, when the status became 1 after an Enter on the offer page; at the
  turn-in, when the status went from 2 to 3 after an Enter sent while the turn-in page was up (the status tells the turn-in
  page, 2, from the reward page, 3). If no page is shown, the use key falls back to the older path and the link names
  `accepted by the use key without the mission page` (or `turned in ...`). The turn-in fixture is also named when the use key
  turned the mission in and a page showed only the reward. If the offer page is up but Enter does not accept within 8 s,
  Escape is sent once and, on the next tick whatever the page did, `UOpenWillowQuest::Accept` is called directly, named
  `accepted by a direct call after the mission page did not accept`, so the objective and turn-in links still run. Enter
  closes the offer page inside the key call (by accepting or refusing), so a page still up means the keys do not reach it;
  the driver then closes it by calling the page's key handler (`OnScreenKey`) with Escape directly, logs that it did, and
  accepts. No wait in this step is open-ended. Whether that direct call accepts with the screen closed is
  `UNVERIFIED`; if it does not, the link FAILs and the run goes on to the down link, as before.
  The runner starts the page server (`tools/movie_hud.ps1`, port 8786) and passes `-owflashskills` and
  `-owflashmissionscreen`. The gameplay HUD stays the canvas one, as in the other variants.
- **The range during the mission (links 10 to 12).** It is neither cleared nor made passive. Its enemies may follow Maya,
  attack her or take the mission's shots. This is the first gate run of the range and the mission together.
- **Down (link 13).** Maya is placed 250 uu from the nearest living range enemy and stops fighting back. She fires no shot and
  casts no Phaselock, so no kill can give her a second wind, and she aims at the nearest enemy. If that enemy is more than
  1,200 uu away she is placed by it again, at most every 10 s. The time limit is 90 s. A PASS needs her down with at least one
  hit whose causer is an actor (`how=enemies`). With no down in 90 s the link FAILs. The detail gives the hits, her health and
  shield, and the enemies within 1,500 uu. The test's lethal damage then puts her down so that link 14 still runs, and link 14
  names it. "From an enemy" means a hit whose causer is a range enemy (`AOpenWillowEnemy`; those keep their actor through a
  death and respawn). From the end of the Recover step the driver watches, every tick, the walker's per-causer count for
  those enemies and keeps the time of the last tick on which a new such hit was seen while Maya was Up, or within 0.5 s of
  her going down (`DownHitMargin`). A down, during the wait or before it, is put down to an enemy only when that time is at
  or after the down time (`FOpenWillowMayaState::DownAt`) minus 0.5 s. A hit minutes or a respawn older than the down, or
  hits that land while she is already down, do not count. Without such a hit the cause is unknown (a fall, say) and the link
  FAILs; link 14 then lacks its token and FAILs too. What is left: an enemy hit up to 0.5 s before a down with another cause
  would still be credited.
- **Respawn (link 14).** The walker's own bleed-out and respawn, after the enemy down (`how=after_enemy_down`).
- **Kept:** teleports between places (`teleport`), the fight's hold on fire and one shot at a time, `FireOnce` ignoring the
  fire rate, the ammo refill, and the decoy link's two test drops. These are test shortcuts of the gate, not stand-ins for
  the four links above. The checker does not fail them.

**How it is graded** (`tools/check_slice_gate.py`; host grading rules, `UNVERIFIED` against the game):
- The start line ends `unforced=1`. The runner passes `--expect-unforced`, and a log without the marker (a build without the
  switch) then fails. The start record is parsed strictly, in every variant, by one anchored pattern of what the host
  prints: `map=<map> quest=<n> range_command=<0|1>`, optionally ` unforced=1`, and nothing else (a trailing carriage return
  and trailing spaces are stripped). The map is one token of the characters a package name may hold, and it is never
  searched for the markers. Any other form (`unforced=1x`, a line cut off after `range_command=1 unforc`, extra words), a
  second start line, or start lines that disagree on the variant fail the log with a line that says why. A malformed marker is graded
  strictly as well. The host prints only the accepted forms, so its logs grade as before.
- Any link whose fixtures name a stand-in for play is a FAIL, whatever the host said. The list in the checker
  (`FORCING_FIXTURES`) has the forced gun, a gun the test dropped, the granted point, the passive or cleared range, the heal
  and the revive, all three lethal-damage fixtures, both use-key mission fixtures, and the `-Shield` fixtures.
- A PASS on links 5, 9, 10, 12, 13 or 14 needs its `how=` token in the detail.
- Every NOT_AVAILABLE is a FAIL, so the run exits 0 only when all 14 links passed. A link group skipped because Maya was
  still down (for example the mission, with links 13 and 14 then passing on enemy hits) cannot leave the run green. With the
  range there, links 5 and 13 say why: its enemies can drop a gun and can down her.
- The report adds a line `unforced run: <n> of 14 links got there with no stand-in for play; not unforced: <links>`, and the
  JSON gains `unforced` and a `forced` list per link. The older variants' output is unchanged. Their synthetic tests pass as
  before, and 400 random older-variant logs graded the same with the checker on main and this one (exit code, table, JSON,
  markdown).

**Choices made here** (simpler option taken each time; host choices, `UNVERIFIED`):
- **The kill budget.** The gun chance per kill is estimated at 0.043: `Pool_GunsAndGear` fires at 0.085 (COMBAT_LOOT.md), and
  in the stage-8 table weapons are 50.8 % of what it gives (WEAPON_BALANCE_DECODE.md); the host's roll drops nothing for a
  shield or grenade mod pick (COMBAT_RANGE.md). The range draws an item stage per death (4 to 7 for an adult at level 6,
  COMBAT_RANGE.md); the share at the stages a level-8 range draws was not checked. With that
  chance, 100 kills miss a gun with probability 0.957^100, about 1.2 %. Three runs miss at least once about 3.6 % of the time.
  A run needs about 23 kills on average. **Not measured:** the 1.2 % holds only if 100 kills fit in 720 s, one kill every
  7.2 s including the 1 s drop wait and the walk between targets. The kill time is unknown (one adult took 6.3 s in an early
  run, the Psycho 3.5 s with the hold). If only 60 kills fit, a working drop misses about 7 % of the time (0.957^60). A FAIL
  states the miss probability for the kills it made.
- **"Stops fighting back" and the 90 s limit.** She stops firing and casting, nothing more: no walking away, no crouching.
  Z16 measured one round of the pack at 185 of 213 health, the stand-in start shield holds 383, and the swing interval is
  about 1.25 s, so a few rounds should be enough. The 90 s leaves room for noticing, closing in and the shield's recharge.
  Those numbers are arithmetic, not a run.
- **Fallbacks keep the later links running.** When a link cannot get there unforced, the older variants' stand-in is used and
  named (granted point, test drop, lethal damage). The checker fails that link and its dependants, and the later links are
  still tested.
- **The runner's time limit** is 2,100 s for `-Unforced` unless `-TimeoutSeconds` is given. The sum it covers: editor start
  (about 2 min), 720 s of kills, one 50 s wait for a downed Maya, the 30 s buy, the mission (about 100 s), the 90 s down
  wait and the 40 s bleed-out, about 1,700 s. A stacked worst case (the 240 s kill timeout of the fight and a 50 s wait at
  each of the seven link groups) can still exceed it, and a run cut off there logs no SUMMARY and grades every unreached link
  FAIL.
- `-Unforced` refuses `-Shield` and `-DownAfterFight`. The run starts with the stand-in start shield, as normal play does.

**What it will not show even when it passes:** travel is still by teleport, the fight still uses the hold on fire and
`FireOnce` without the fire rate, the pages are driven by key events rather than a person, and frames are not captured.
Nothing is checked against the real game.

**Expected first result:** `go_down` FAIL or NOT_AVAILABLE (no gate run has had an enemy down her), and a FAIL on
`bleed_out_respawn` from the lethal fallback. Links 10 to 12 may show what the live range does to the mission.

## Gaps, with the owning lane

| Gap | Link(s) | Owner | What closes it |
|---|---|---|---|
| No enemy has downed Maya in a gate run | revive fixture, 13, 14 | range / slice-gate | Every down so far came from the test's lethal damage. The `-Unforced` variant now asks the range's enemies to do it (90 s, Maya not firing). Not run yet |
| The gun drop is forced | 5, 6 | slice-gate | `-Unforced` kills until a gun drops (budget 100 kills). Not run yet |
| The Phaselock point is granted | 9 | slice-gate | `-Unforced` buys it on the skills page. Not run yet |
| The mission is taken with the use key alone, and the range is cleared first | 10 to 12 | slice-gate | `-Unforced` uses the mission page's Enter and leaves the range as it is. Not run yet; the range and the mission have never run together in the gate |
| `-Shield` needs the level override | stock-shield path | maya / skills | A manifest shield at or below Maya's level 8 would remove the level-override fixture. Since 2026-11-08 the plain run starts with the stand-in start shield ([COMBAT_DOWN_RESPAWN.md](COMBAT_DOWN_RESPAWN.md), "Start shield"); the older variants keep the restore fixture |
| The equip link does not grade the gun's firing | 8 | projectiles | Generated guns take their firing block from templates since 2026-11-06 (WEAPON_PROJECTILES.md, "Generated guns take their firing block from templates"); that firing is `UNVERIFIED` in game |

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
| 2026-11-06 | main cdf05d7 + c365984, `-Shield`, yorad PC | 14 / 0 / 0 of 14 | the #109 review fixes' first run; a manifest shield under the level-override fixture; ctest 22/22, verify_packages exact, checker tests 27/27 |
| 2026-11-06 | main cdf05d7 + c365984, no switch, yorad PC | 14 / 0 / 0 of 14 | link 4 PASS with per-causer counting; ended the fight at 60/213, healed by the fixture |
| 2026-11-06 | main cdf05d7 + c365984, `-DownAfterFight`, yorad PC | 14 / 0 / 0 of 14 | revive fixture ran (forced) |
| 2026-11-06 | combined build of this branch at 402d81e with #112 and #113, refreshed data, pc-hummer, all three variants | 14 / 0 / 0 of 14, each | reported by the hub; this lane did not read those logs. `-Shield` used the host stand-in, not a manifest shield |
| 2026-11-06 | Z10: #126 at 7f59924 + #133 at d02c70a, four enemies, zuhu, all three variants | 14 / 0 / 0 of 14, each | hold ended by a hit; passed by margin |
| 2026-11-07 | Z13: #126 at 7f59924 + #133 at 716302b, five enemies, zuhu, all three variants | 13 / 1 / 0 of 14, each | link 4 FAIL: the burst killed the selected Psycho before the hold started; fixed in the driver by one shot at a time |
| 2026-11-07 | Z16: #126 at 5a39f57 + #133 at 51deb95, five enemies, zuhu, all three variants | 14 / 0 / 0 of 14, each | one shot at a time and the hold both ran; Maya 28/213 after the fight without a shield; no enemy downed her |
| 2026-11-07 | Z23: main 7eee9ee, five enemies, zuhu, no switch twice, `-Shield`, `-DownAfterFight` | 14 / 0 / 0 of 14, each | the first runs of the 14 links on `main` itself; no `fallback=shapes` line in the four logs; ctest 28/28, verify_packages exact; logs under ignored `local/` |
| 2026-11-07 to 11-11 | hub batches Z26, Z35, Z37 (three runs), Z41 to Z52b, each on that batch's candidate or main, zuhu | 14 / 0 / 0 of 14, as reported | from the hub's log (ignored, local): it records "gate 14" or "14/0/0" per batch. This lane did not read those logs, and the log does not always say which variants ran |
| 2026-11-11 | `-Unforced` added (feature/slice-gate-unforced) | not run | the driver was not compiled (build slot held by a batch); checker tests 55/55 |
| 2026-11-11 | `-Unforced`, review round 1 of #218 | not run | every NOT_AVAILABLE fails an unforced run; the mission-page token needs the page to have done the work; an enemy hit is required for a down; the accept falls back to a named direct call; still not compiled; checker tests 58/58 |
| 2026-11-11 | `-Unforced`, review round 2 of #218 | not run | the accept step can no longer wait on a page the keys do not reach; strict start record; corpses recorded from the pickup snapshot; a down needs a range-enemy hit within 0.5 s before it; still not compiled; checker tests 63/63 |

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
