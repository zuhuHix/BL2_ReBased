# The gun range outside Sanctuary: static dummies and moving enemies in the UE host (range lane, PR1, 2026-11-03)

AI-assisted (Claude; host C++ drafted by a Claude Haiku sub-agent from a written spec and reviewed by the lead). A host test area for trying guns,
not a stock map feature: BL2 has no gun range. Everything in this record is **host behaviour**; no rule here was checked in the running game,
so each is `UNVERIFIED` unless a line says which record it reproduces.

## What it is

- `ow.Range [x y z [yaw]]` (console) builds the range and teleports Maya to it. Without arguments the spot comes from `-owrangeat=X,Y,Z[,Yaw]`,
  else `local/range/range.json` (`{"spot": {"x", "y", "z", "yaw"}}`), else the **scout**: rings of 6,000 to 14,000 uu around the plaza
  (the centre of the inspection views), 24 candidates per ring, closest ring first. A candidate is accepted on a flat floor
  (normal Z >= 0.9) within 1,500 uu of the plaza's height, with 8 probes on a 700 uu circle at the same height (+-100 uu), nothing solid in a
  250 uu sphere 300 uu up, and open sky above. That the scout's winner is "outside the town gate" is **not shown** until a frame of it is
  looked at (the integration run captures one); a spot found to be good is fixed in `local/range/range.json`.
- Three **static dummies** in a row 900 uu ahead: the Fire mission's stock target dummy (`AOpenWillowCombatTarget::UseStockPawn` with the slice
  manifests, as the quest spawns it), else the engine-shape dummy. On the range they respawn (the mission's dummy still does not).
- **Moving enemies** (`AOpenWillowEnemy`, a subclass of the combat target, so every existing hit path reaches them): the manifest's spawns on an
  arc 1,600 uu ahead, facing away from the spot (a host layout, not a game rule, see "The first playtest's fight"). PR1 gives them a patrol
  (random floor spots within 500 uu of home, 150 uu/s, pauses of 1 to 3 s); noticing Maya, approaching and melee are PR2.
- **One way in:** `tools/play_range.ps1 [-Mode normal|passive|invulnerable|both]` starts the host with `-owrangestart`, which builds the range
  and moves Maya to it once she stands. Sanctuary only: the host has no other map, and the spot rules read Sanctuary's terrain.
- `ow.RangeMode normal|invulnerable|passive|both`: invulnerable targets record every hit (damage number, flash, log) but keep their health
  and shield; passive enemies stand still. `ow.RangeLevel N` (0 = Maya's level) rebuilds with that enemy level. `ow.RangeClear`, `ow.RangeReport`.

## Numbers and where they come from

| Item | Value | Source | Status |
|---|---|---|---|
| Enemy max health per level | `max_health.<class>.L<level>` of `ow-package --combat-run spawn:<kind>:<level>` (regular, adult, baby, ranged; levels 1 to 50) | the VM runs the pawn's installed spawn script and health pool (COMBAT_HIT_CHAIN.md, COMBAT_DEATH.md) | UNVERIFIED: no live row of a fresh pawn's maximum. The L3 adult at level 5 started the recorded fight at 360.876, the VM's adult L5 maximum is 614.084; the live pawn may not have been at full health (open item) |
| Critical multiplier | the hit event's `CritBonus` (2.0 base) | `CurrentInstantHitCriticalHitBonus` (NATIVE_COMBAT_GROUNDWORK 3.4; base confirmed live, COMBAT_GOLDEN_TRACE) | the shooter fills the bonus; 2.0 for damage without an event |
| Critical region | a sphere (radius 18 uu, manifest `head_radius`) on the first bone whose name has "head"; the engine-shape enemy's head sphere | host stand-in | the stock body class's hit regions are not read |
| Shield | shield first, the rest to health | the order recorded for the player (COMBAT_GOLDEN_TRACE item 9) | bullymongs have no shield (L3: "no shield"); the order on an enemy shield is UNVERIFIED and only exercised by a test fixture |
| Ragdoll | 0.84 s after death | the delay `src/combat_run.cpp` uses for the player | UNVERIFIED for enemies |
| Respawn | 8 s (manifest `respawn_seconds`) | host choice | a den's respawn rules are not modelled |
| Health bar | a red bar (blue shield bar above when there is a shield) over the enemy, facing the camera | host stand-in for BL2's HUD target bar | UNVERIFIED look |
| Critical damage number | orange, 1.25x | host stand-in | UNVERIFIED look |

**Critical hits use the hit event** (`FOpenWillowHitEvent`, the projectiles lane's API, `OpenWillowHitEvent.h`): the enemy crits only when the event allows it
(`bCanCrit`, never a `StatusTick`), multiplies by the event's `CritBonus` (2.0 without an event: test hits and other sources), and writes `bCritical`
and `Applied` back into the event, so `OpenWillowHit::OnHit()` listeners see the same result as the damage number. The region test: the event's bone
(else the point hit's) containing "head", the critical sphere as the hit component, or, because the body capsule (from the pawn's collision cylinder)
encloses the head, a shot whose line from the impact through the body crosses the head sphere (host stand-in).

**Dead targets take no hits:** `CanTakeHits()` is false while dead (dying, ragdolled, waiting to respawn), so explosions skip corpses. A status effect
the shooter rolls after the lethal hit is cleared from a dead range target (`UOpenWillowStatusEffects::ClearAll`) so it does not tick into the next life;
gating the roll itself on `CanTakeHits()` is in the projectiles lane's `HitTarget` (asked for, not this lane's file).

**The formation is validated as a whole:** every slot (3 dummies 900 uu ahead, one arc slot per manifest enemy 1,600 uu ahead) needs walkable floor
(normal Z >= 0.85, within 150 uu of the spot's height) before anything spawns, for the scout's candidates and for `ow.Range` at a given spot; an
incomplete range is a failed build and nothing is left standing. The test's placement step checks the dummy and enemy counts against the manifest.

## Data (all ignored, under `local/`)

- **Before you prepare:** `local/sanctuary/inspection-views.json` is hand-written (no tool generates it) and `tools/prepare_range.py` needs its `plaza_*` views; a worktree's copy may not have them, so pass the main checkout's with `--views` (see "Where the spot comes from").
- `python tools/prepare_range.py [--levels 1-50]` writes `local/range/range_manifest.json` (schema `ow-range-v1`): the health tables from one VM run
  (about 2 minutes, most of it the first load of Ice_Dynamic) and the enemy assets from `local/phaselock/target/host_target.json`
  (`tools/import_phaselock_target.ps1`, the bullymong). Walk, run, hit and melee clips are picked by name from the imported clips next to the idle
  (a name match; the manifest says which file was taken). One imported bullymong serves all four bullymong kinds (the adult, baby and slinger
  meshes are not imported). Without the manifest the host uses one engine-shape stand-in kind with 400 health and says so.
- Without the imported bullymong, the enemies are engine shapes (logged `fallback=shapes`). The bandit and the psycho are not in the VM's
  spawnable kinds, so they are not on the range yet.

## Test

`tools/test_range.ps1 [-At X,Y,Z[,Yaw]] [-Shots]` (heavy: one editor, the machine-wide UE lock) starts Sanctuary_P with `-owrangetest`; the host's
driver logs one `OWRANGETEST step=... result=PASS|FAIL|SKIP` line per step and `tools/check_range.py` grades them: teleport, placement, body_hit,
crit_hit, popup_crit, crit_minds_eye (may SKIP only when the local skill effects file does not exist), crit_trace, shield_order, invulnerable, passive, patrol, death, ragdoll (may SKIP without a stock mesh or physics asset), respawn,
dummy_respawn. The body_hit and crit_hit hits are applied with `ApplyPointDamage` as a shot would apply them; crit_minds_eye's go through the walker's
`ApplyShotDamage`, the path real shots take; crit_trace is a real camera trace at the head.
`tests/range_tools_test.py` covers the two Python tools with synthetic data.

Results: none yet (the hub's integration run). Automated: ctest 21/21; verify_packages 9 packages, all class bodies exact.

## The psycho

`tools/prepare_range.py` adds the psycho as a range kind: health, melee and loot chances per level from the VM's spawn of the stock pawn. Only the bullymongs' melee numbers
reproduce live rows; the psycho's are the VM's answer alone, and its manifest entry says so (`melee_status`: VM only, UNVERIFIED). It uses its own imported asset, never the
bullymong's, and joins the range's spawns only when that asset exists, last, so the enemies the test uses by index don't change. `tools/prepare_phaselock_target.py` imports its mesh
and clips (idle, a death, walk, run, melee, a flinch, the four PhaseLock clips): no live read of this pawn exists, so its collision cylinder and scale are the archetype's, and the set's
only clip named as a death is `Death_Shield`, which may not be what the game plays. It melees with the range's brain, which is a host stand-in for its stock AI tree (its leaps, signals
and suicide runs are not modelled). The manifest also carries what a pawn wears on its bones (the psycho's mask and axe, the adult bullymong's spikes), and range enemies attach it as a test pawn does, with the same UNVERIFIED placement. The test's `psycho_melee` step checks a swing lands for the psycho's own table value; it may SKIP without the imported mesh.

## Phaselock on range pawns

The targetable-list rule of `AOpenWillowWalker::PreferredPhaselockTarget` applies only to the mission's dummy (`OpenWillowPhaselockListRefuses`). It used to cover every stock
pawn while the quest was on, so with `-owquest` and `ow.Range` no range pawn could be targeted (slice gate link `phaselock_enemy`). By reading the code, the mission's dummy is the
quest's only stock pawn, so nothing there loses the rule. Range dummies, which use the same stock data, are candidates too, as they already were with the quest off. Whether the
game's auto-aim takes these pawns is UNVERIFIED; `OpenWillow.Phaselock` tests the host's choice only.

## Enemy AI (PR2): notice, approach, melee

A host state machine (`FOpenWillowEnemyBrain`, `OpenWillowEnemyAI.cpp`), not the stock AI tree. The rules and numbers are the live records'; each is
`UNVERIFIED` as a host reproduction (nothing was compared with the running game in this lane).

| Rule | Host | Record |
|---|---|---|
| One threat record per den, shared by its members | the range is one den, centred on the spot | AI_AWARENESS_LIVE 0 |
| State update cadence | every 0.2 s per enemy (the line-of-sight trace is taken on these updates only, and not at all while passive, lifted or dead); den fall-back check every 1.0 s | AI_MELEE_TARGETING_LIVE 4; AI_AWARENESS_LIVE 2 |
| Notice by sight | the previous update's line of sight (a Visibility trace, stand-in for the exposure iterator), Maya in the front half-plane, Maya within the den's ThreatRadius (4,665.625, the one den read) of the den centre | AI_AWARENESS_LIVE 0 |
| Notice by proximity | 3D distance minus the enemy's collision radius below 768 (AICloseThreatRadius), Maya within the den's PatrolRadius (3,667.19) | AI_AWARENESS_LIVE 0 |
| Notice stamps | level 2, AggroTime = now, ForgetTime = now + 10 (MinEngagementTime), refreshed while a rule holds | AI_AWARENESS_LIVE 2 |
| Damage by Maya | level 3, ForgetTime = now + 25 (MinProvokedTime) | AI_AWARENESS_LIVE 3 |
| Fall-back | at the first den update after ForgetTime the level drops to 1 (record removal beyond the list radius not modelled) | AI_AWARENESS_LIVE 2 |
| Awareness | attack decisions only after AggroTime + 0.1 s | AI_AWARENESS_LIVE 2 |
| Own target, apart from the threat record | each enemy keeps its own target: valid while the den level is 2 or more and Maya is within the den's ThreatRadius of its centre; it goes for where it last saw her (line of sight) or heard her (proximity), and after losing the target it walks to that spot once, then patrols | AI_AWARENESS_LIVE 2 (targets drop at the member's next update once its rules fail); the validity rule is a host stand-in |
| Approach | turn at 360 deg/s, run at `run_speed` (400 uu/s, stand-in) with the run clip (walk as fallback) | host stand-in |
| Blocked chase | under 10 uu of progress in 1 s counts as blocked: side-steps (70 degrees, alternating sides); blocked for 3 s, the enemy drops the target for 5 s | host choice |
| Melee volume (does a swing land) | horizontal 640 from a point 100 uu behind the enemy, 64 degree half angle, Maya's centre -143 to +339 uu from the enemy's location | AI_MELEE_TARGETING_LIVE 5 (fitted) |
| Swing start (may a swing begin) | Maya inside the volume and within the kind's `melee_start_range`, the farthest range of its plain melee attack actions, read by `tools/prepare_range.py` from the stock AI definition; it stops running 60 uu inside it | the stock attack range is data; the horizontal measure from the enemy's location and the choice of actions by animation name are UNVERIFIED. The live swings had the player within about 300 uu |
| Swing timing | damage 0.41 s after the swing start if Maya is still in the volume; next swing 1.25 s after the previous start | AI_MELEE_TARGETING_LIVE 4 |
| Melee damage | the VM's swing rows per kind and level (`arrival` x `ratio`, both from `ow-package --combat-run maya:L spawn:<kind>:L swing:<n>:melee`), x the level scale (1.0 at or below Maya, 1.1 / 1.2 one / two levels up; a straight line to 3.0 at sixteen beyond that, stand-in) | COMBAT_MELEE (the VM rows reproduce adult L5/6/7 73.6974 / 83.2781 / 94.1042, regular L6 24.9834, baby L6 12.4917, ratios 0.715 / 0.728 / 0.741) |
| Delivery | one function, `RangeDamagePlayer`: the maya lane's `OpenWillowApplyDamageToPlayer` (PR #72), shield first, then health, then Fight For Your Life; the test's melee checks compare health plus shield | host |

Maya's teleports (`TeleportMaya`: `ow.Range`, the test's placements) call `AOpenWillowWalker::NotifyTeleported` (PR #77), so a teleport is never taken
as a fall.

`ow.RangeAI on|off` (on by default): off stops every brain, so the enemies only patrol (for gun testing without being attacked). The range
test runs its target steps (1-15) and its loot steps with the AI off and its AI steps with it on, topping Maya up to full health before
each AI step (`ApplyScriptHealth`, a test fixture): in batch 2 the alerted range swarmed Maya during the target steps and put her down,
after which no AI rule could fire (a downed Maya is no target).

Not modelled: momentum on Maya (75,000 for the adult swing), leaps and the slinger's throw (the slinger has melee numbers from its rock but no throw),
the stock AI tree's perch, random and leap nodes.

Test steps added to `-owrangetest` and `tools/check_range.py`: notice_proximity, notice_sight, no_notice_far, provoked, approach, melee_hit, melee_interval,
melee_miss, forget, passive_no_melee, swing_range (the first enemy's kind only), corpse_limp and corpse_cap (SKIP only when the manifest names no stock mesh).
The host builds no range, says why on screen and has the slice gate report `range refused: <reason>` on `reach_fight_area`, when any spawn has no kind, no melee tables or no swing range (an old or `--no-vm` manifest), however it was started, so no
tool can pass on the old rule. `tools/check_range.py --preflight` refuses the same manifests. A run with no psycho reads "no Psycho exercised" in its result, and `-RequirePsycho`
makes it fail.

## The first playtest's fight (2026-11-06)

The scripted playtest (Maya level 8, 213 health, no shield, no input) saw all four enemies deliver on one tick 3.31 s after the build and Maya down at 4.56 s.
What the records and the data say, with no game run for this note:

- **All engaging at once is in the records.** One member noticing alerts the den, and the members acquire at their next update (AI_AWARENESS_LIVE 0, 3). Nothing
  here limits how many attack: no attacker limit was found in the targeting data read so far. The range keeps this.
- **The one tick was the host's.** The enemies spawned facing Maya, so sight fired at the first update; they stood at equal distance and ran at one speed on one
  clock. The arithmetic matches: 1,060 uu at 400 uu/s, plus 0.1 s of awareness and 0.41 s to the blow.
- **Swings started too far out.** The host started them at the damage reach, about 540 uu ahead. The stock attack actions have their own, much shorter ranges.
- **Not changed, because the data does not say to:** the damage (stock numbers, the adult's multiplier included), and the run speed. 400 uu/s is a stand-in, and the
  stock `GroundSpeed` field is 500 for the bullymongs, so slowing them has no support.
- **First-update lockstep addressed; the fight is not staggered.** No stagger number was invented: the record of four members acquiring together does not show that
  update clocks differ. The change delays the first notice and lets the patrol move the den a little before it. Once the den is alerted, the other members take their
  target with no facing check, and enemies of one kind at similar distances can still converge and land together. About 152 damage a round still takes an unshielded
  Maya down in two rounds once they arrive. That is the missing shield and standing still, not the timing.
- The `level_scale` 1.84 on a line with hit=0 was a logging artefact: Maya's level was only read while she was up. It applied no damage. Fixed.

**Critical hits from behind.** With the den facing away, `crit_trace` failed on a shot through the back at the head: the check followed the shot only a capsule's width past the
impact, and the head stands further than that from behind. Any shot from behind had that gap; the old layout's test only shot from the front. The check now reaches the head from
any side, but the shot's line, followed on from the impact, must still cross the head sphere itself (radius from the manifest, on the head bone). `crit_trace` now shoots from directly
behind, from 500 uu with the enemy at home, turned until both lines are clear (a patrol can walk it behind a rock, and a low shot from Maya can meet the ground). It requires the head to
stand past the old reach, so putting that reach back fails the step, and a shot at the small of the back must hit the body and not count. Against a capsule, this can count a head hidden by the back as hit (a host stand-in, UNVERIFIED: the game's hit regions aren't read).

First host read of the no-input fight (one run, 2026-11-06, the batch's baseline in brackets): first notice t=6.32 (8.31), swings at 9.54, 9.57, 9.61 and 9.75 (all four at 11.06),
first blows at 9.95 and 9.98 (all at 11.47). So they no longer swing as one, with a spread of 0.21 s. This is host behaviour only, not parity with the game. The slice gate passed all
14 links in that run, plain, with a shield and going down after the fight, with #126's hold-fire.

Open, for the den read below: the first-blow spread of a real den, the real chase speed, and where real swings start.

**Corpses.** The playtest suspected shot momentum. The code hands the ragdoll no velocity (the death zeroes the knock), so that is not the path. The suspect is the
moment physics takes over with bodies overlapping each other or the floor (UNVERIFIED). The host limits that push (`SetMaxDepenetrationVelocity`), caps how fast a
body may rise or travel sideways (a fall stays free), and logs each corpse's rise, drift, fastest body and number of caps. These limits are host stand-ins for a limp
body, not game values, and the stock momentum on a ragdoll (`RigidBodyMomentumScale`, NATIVE_DAMAGE_DEATH) is not modelled. `corpse_limp` checks a body rises at most
200 uu and drifts at most 500 uu, after a test kill without a shot; the integration run's frames decide whether real kills look right.

`corpse_cap` throws one body upward at a known speed and requires the cap to act: it shows the cap runs, not that it is what real kills need.

First host read (one run, 2026-11-06, a test kill with no shot): the body rose 19 uu and drifted 52 uu, its fastest body reached 2,101 uu/s, and the caps acted 278 times. So
bodies are thrown with no shot behind them. The cause in the physics is UNVERIFIED, and whether real kills now look right is still for the frames.

`tools/play_range.ps1 -KillRow` is the row for real kills: a rolled pistol and a rolled shotgun each kill a stock enemy from about 700 uu, with frames 1.4 s and 3 s after each death and
the corpse's numbers in the log. It has not run yet.

**Floor gap and the physics asset (follow-up, 2026-11).** `rose` cannot say whether a body is on the ground, so each simulated corpse now logs how far its lowest and highest bodies stand above the
floor below them, 1.4 s and 3 s after death and just before the respawn, as well as which body was fastest and when. Each ragdoll logs its physics asset's bodies, constraints and the body pairs
that do not collide. `tools/play_range.ps1 -KillRow -NoCorpseLimits` runs the kill row once without the depenetration limit and the speed caps, to see what they hide; it is a test switch, and how bodies
are thrown in play does not change. None of this has run yet. The import's asset is auto-generated from the mesh, not the stock asset, so what these lines show is about the host's asset, and
what they mean for the game's own is UNVERIFIED.

**The den read.** `tools/real_game/cash_capture.ps1` has a `den_read` stage after the cash read, with its own result (`stage_results` in `session.json`): the player
stands immune 1,600 uu from an enemy with no input, and `scripts/den_probe.py` records melee calls, blows and positions. It reads one map's dens at one level; its
numbers don't speak for others.

First run (2026-11-07, one session, three trials, Three Horns Divide, Maya at level 8, immune: her health was unchanged after every trial, against 78 recorded blows). All of it UNVERIFIED beyond that:
- **First blows did not land as one:** 8.6 s and 5.7 s between different attackers in two trials, and one attacker in the third. No two attackers landed on one tick. The den was a mix of kinds and distances, not four equal melee enemies on an arc, so this does not say what such a den would do.
- **They can land close together:** two babies that had converged landed 14 pairs of blows within 0.05 s, on the same swing cycle.
- **Their first blows were mostly not melee:** a slinger's rock from 800 uu and more, at about 3 s, and leaps. The range's enemies have neither.
- **Swing start:** an adult's first swing came at 368 uu and a baby's first grab at 425, each inside its stock range, which supports the rule on those two samples. Regular bullymong and psycho: no read.
- **Closing:** once there, the adult stood about 190 to 220 uu from Maya and the baby about 106. The host's adult stops at 334 and its regular at 260.
- **Swing cycles:** adult 1.217 s, baby grab 1.133 s; the host uses 1.25 s for every kind.
- **Speed:** moving medians of 500 for the slinger, 300 to 450 for the baby and 608 to 866 for adults, leaps included, with long pauses. The host's 400 is not too fast by this read; no number is taken from it.
- **Notice:** members took a target 0.06 and 0.12 s apart in two trials: weak evidence, at 0.05 s sampling, that their updates are not in step.

Options for the maintainer, none taken: swing cycles per kind; closer stopping distances; leaps and throws (the ranged lane). The money read in the same session gave no value: one collect took two pickups at once for +5, and the item's own value call returns 0, so the money stand-in stays.

## Range spot: candidates and the textured-floor rule (PR2 follow-up)

`tools/prepare_range.py` writes `spot_candidates` from the level import's terrain probes (`local/sanctuary/terrain-runtime.json`), Sanctuary_P
Terrain_2 first (the terrain the gate's floor lies in, SANCTUARY_ARTIFACT_PASS.md), then the other terrains by distance from the plaza. The host tries
them (each point, then rings of 600 and 1,200 uu) before its ring scout. Every formation slot needs a **textured** floor: the face material the trace
hits must not be null, an engine material or a collision / fallback / neutral placeholder, **and it must sample at least one texture that is not an
engine texture** (`GetUsedTextures`; batch 1's scouted canyon floor drew dark with no texture). `tools/test_range.ps1 -Survey` saves one frame per
candidate (`local/range/shots/survey_<i>.png`) with the material and the reason it was or was not taken; the default spot will be pinned from those
frames. `prepare_range.py` refuses an `--out` outside `local/` (the manifest holds game-derived coordinates).

Batch 4 (pc-hummer, 2026-11-04) rejected all 8 candidates and every ring point; the rules were then changed (host choices, UNVERIFIED):
- **Terrain is textured through its blend layers.** Every terrain hit was `M_TerrainBlend`, the import's terrain material (`tools/prepare_terrain.py`
  writes it with a `terrain_blend` block): it samples no texture of its own. A floor now also counts as textured when a texture parameter of the
  material names a texture that is not an engine texture, or when the material or one of its parents is the terrain blend material.
- **Slopes.** The eight probes started 300 uu above the centre's floor, so on a rising slope they began under the ground and met nothing ("a floor
  probe found no floor"). They now reach 1,500 uu up and down; a walkable floor is one with normal Z >= 0.85 (about 32 degrees), a probe may lie
  250 uu above or below the centre (was 100) and a formation slot 400 uu (was 150). Rings of 2,400 and 3,600 uu around a candidate were added.
- **`-owrangeat=X,Y,Z,Yaw`** was cut at the first comma (`FParse::Value` stops at separators by default) and fell through to the candidates; it is
  now read whole.

## Loot on death (PR3)

`tools/prepare_range.py` kills each spawned pawn in the VM (`hit:<n>:999999:DmgType_Normal:Shotgun`) and keeps its death roll per kind and level: the
item stage and every pool of the pawn's item pool list with the chance the VM evaluated (COMBAT_LOOT.md). For the adult at level 7 they are COMBAT_LOOT's
table: guns and gear 0.085, vehicle skins 0.05, money 0.25, eridium bar 0.0015, stick 0.008, health 0.03, the two ammo pools 0.2 each. The host
(`OpenWillowRangeLoot.cpp`) rolls them itself in COMBAT_LOOT's order: one draw per pool, tried when the chance is above 0 and the draw is at most the
chance, from a seeded stream (`-owrangeseed=N`, default 20261009, so a run repeats; the game's own seed is unknown).

| Pool kind | What the host does | Status |
|---|---|---|
| weapon (`Pool_GunsAndGear`) | `FOpenWillowWeaponGen::RollPoolWeapon` at the item stage (the roll `ow.RollGun` uses), dropped through the inventory pickup's existing API | the pool's own gun choice is the weapon generator's; a pick that is not a gun (shield, grenade mod) drops nothing here |
| money (`Pool_Money_1or2`) | quantity a random integer in -1 to 2, at least 1 (the data's range); one range pickup per item, worth `round(4 x 1.13^level)` | **stand-in value, UNVERIFIED**: the value of `ItemGrade_Currency_Money_Little` is not decoded (a live read is wanted) |
| ammo (the two ammo pools) | one pickup of the held gun's ammo type, a magazine's worth, only when that reserve is below half | stand-in: COMBAT_LOOT shows these pools drop nothing while ammo is not low; the threshold and amount are host choices |
| other (skins, eridium, health) | logged as not hosted | |

**Item stage.** The adult's stage carries a random offset (`Random_Minus02-Plus01`): `prepare_range.py` samples it over 24 kills per kind
at level 20 and stores each level's range (`item_stage_min` / `item_stage_max`; adult level 6: 4 to 7). The host draws one stage per death from
the loot stream before the pools (the game draws it at spawn; host order, UNVERIFIED).

**Determinism.** Every tried weapon pool draws its gun seed whether or not the weapon generator's manifest is installed, so a seed gives the
same money and ammo rolls on every machine. Money and ammo counters add with saturation (no wrap at the int32 limit), and `ow.RangeLevel` is
clamped to the levels the manifest's tables cover (with a warning).

**Drops and clean-up.** A drop goes to the first point on three rings around the enemy (60, 120 and 180 uu) with a floor and a free 30 uu
sphere above it, else the enemy's feet. The range's own pickups expire after 60 s and its gun drops after 120 s (host choice); `ow.RangeClear`
removes every drop with the range.

**Known wrong: the money stand-in reads high.** At item level 7 it gives 9 per pickup, about 3 times the one live number on record (COMBAT_GOLDEN_TRACE.md item 8: three `Money_Little` pickups raised the cash by 9, so 3 each). A real-game read is queued; the stand-in stays until that gives the value at more levels, since one point would not give a rule. Only the range's own pickups use it.

`ow.RangeLoot normal|always|off`: `always` tries every weapon, money and ammo pool (for testing guns), `off` drops nothing. The range pickups (money,
ammo) are engine-shape stand-ins collected on touch. Test steps: loot_roll, loot_money_pickup, loot_weapon_item (may SKIP without the weapon
generator manifest), loot_off, loot_seeded_roll.

## Batch 6 (pc-hummer, 2026-11-04) and the pinned spot

- **Default spot.** The survey accepted 2 of 8 candidates; the hub picked one from the frames, the only one with open snow and no buildings
  (floor `M_TerrainBlend`). The choice is kept in ignored local data, never as coordinates in a tracked file (see "Where the spot comes from").
- **AI steps lay out from home.** approach and forget read level 1 in batch 6. Each step placed Maya relative to enemy 0 where it stood, and the
  earlier steps' chases had moved it, so the placements could land outside the den's ThreatRadius or PatrolRadius from its centre (cause inferred
  from the rules; the batch log was not read here). Every AI step now starts with enemy 0 back home, facing the spot, with no target or chase and
  its patrol held for 3 s (a test fixture), and the threat checks log the scene (Maya's distances to the enemy and the den centre, the radii).
- **Gun drops in `always` mode** try up to 8 seeds until the weapon pool gives a gun (the pool also holds shields and grenade mods, and a pick that is
  not a gun drops nothing); each try's drops are logged. Normal mode keeps the one stock roll. `-owrangeloot=always|normal|off` sets the mode at
  start-up (for the slice gate, whose kill in range mode otherwise has the stock 0.085 gun chance).
- **Drops at the origin (slice gate report).** The drop spot was taken from the enemy's collision centre, which reads zero once the death has
  turned the collision off, so every ring trace missed and the fall-back put a gun at the world origin, 5,894 uu from the corpse. The rings now
  centre on the actor's location (its floor point), and the test's new `loot_near_corpse` step checks every drop lies within 400 uu of the corpse.

## Batch 7 (yorad PC, 2026-11-04, default spot: 24 pass, 3 fail, 2 skip)

- **Sight at this spot.** approach stayed at level 1 with the enemy still. No notice fired: the ground between the enemy's home and the place "2000 uu in
  front" blocks the one centre-to-centre line. notice_sight had passed there only on a stale sample from the step before (the rule reads the previous
  update's exposure). A brain stop now forgets its sample, the exposure stand-in also tries the top of the body to Maya's eye height, and the test
  places Maya where that sight line is verified clear (`PlaceInSight`, the place it took is in the step's log line). UNVERIFIED as the game's exposure.
- **"No floor 6000 uu from the spot".** A fixed direction ran off the terrain. `PlaceFar` searches rings outside the den's ThreatRadius; provoked now really
  hits from afar (it had silently stayed put).
- **Guns at stage 1.** The stock `Pool_GunsAndGear` gives nothing at stage 1: `ow-weapongen --pool` with the same manifest prints empty drops there and guns
  from stage 2, so a level-1 enemy's roll is empty. Normal mode keeps that; `always` mode (gun testing) rolls at stage 2 at least.
- **"No room" on sloped snow.** The room sphere's bottom was 14 uu above the floor, and a 32 degree slope rises 19 uu over its 30 uu radius, so it met the
  ground itself. Its bottom is 30 uu above the floor now.

## Where the spot comes from (local data only)

No map coordinate is in a tracked file. The spot is resolved at run time, in this order: `-owrangeat`, `local/range/range.json`, the manifest's default
candidate, the other candidates, the ring scout. `tools/prepare_range.py` writes the manifest (ignored, under `local/`) from the level import's ignored files:
the candidates from `local/sanctuary/terrain-runtime.json`, the scout centre from `local/sanctuary/inspection-views.json` (where the plaza views' axes cross),
and `default_spot_candidate`, an index into the candidates: the one the hub picked from the batch 6 survey frames, on the gate's terrain, which the candidate
rules turn into a point and a yaw. The tool stops when either file is missing or the views file has no usable `plaza_*` views, unless `--allow-missing-spot-data` is passed.

**The `plaza_*` views are not generated by any tool.** `inspection-views.json` is written by hand as the input of `tools/test_ue_viewer.ps1 -Inspect` (docs/TOOLING.md,
"Repeatable inspection viewpoints"), and other inspection work replaces its views, so a worktree's provisioned copy may hold a different set. The main checkout's copy has
the four plaza views: pass it with `--views` (and `--terrain` when the worktree lacks the terrain file). Batch 9 ran into this: the tool stopped, its old manifest stayed in
place and the run went green at candidate 0. `tools/test_range.ps1` therefore checks the manifest first (`tools/check_range.py --preflight`) and refuses to start, before any
build or editor, when it has no usable `default_spot_candidate` and `scout_centre`, unless the run passes `-At` or there is a usable `local/range/range.json`.
 Because the scout centre's
definition changed, the yaw and so the accepted point may differ slightly from the surveyed one: the run's `OWRANGE spot source=default_candidate` line says what
it found.

The test's `provoked` step fails before its hit when Maya cannot be placed outside the den's radius (near her a notice rule could raise the level and the step
would prove nothing), and `loot_near_corpse` also fails when a drop used the no-room fall-back at the corpse's feet (the fall-back is counted and logged).
