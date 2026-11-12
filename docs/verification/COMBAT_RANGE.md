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
| Swing start (may a swing begin) | Maya inside the volume and within the kind's `melee_start_range`, the farthest range of its plain melee attack actions, read by `tools/prepare_range.py` from the stock AI definition; it stops running at the kind's live stand distance (adult 201, baby 111: the acceptance table below), else 60 uu inside its swing range | the stock attack range is data; the horizontal measure from the enemy's location and the choice of actions by animation name are UNVERIFIED. The live swings had the player within about 300 uu |
| Swing timing | damage 0.41 s after the swing start if Maya is still in the volume; next swing one cycle after the previous start: 1.25 s, the baby's 1.133 s (the acceptance table below) | AI_MELEE_TARGETING_LIVE 4 (the adult's 1.25; other kinds without a row borrow it); the den read (the baby) |
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
melee_miss, forget, passive_no_melee, swing_range (the first enemy's kind only), corpse_limp and corpse_cap (SKIP only when the manifest names no stock mesh), kind_melee (2026-11-11: each kind with a live melee row, see the acceptance table).
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
the corpse's numbers in the log. First run (2026-11-07, one rolled pistol and one rolled shotgun): rose 4 and 1 uu, drifted 84 and 298 uu, fastest body 2,970 and 2,783 uu/s, 274 and 189 speed caps. No launch
was seen, though the frames cannot show floor contact, and the cap is holding the throw down and hiding the cause, not curing it.

**Floor gap and the physics asset (follow-up, 2026-11).** `rose` cannot say whether a body is on the ground, so each simulated corpse now logs how far its lowest and highest bodies stand above the
floor below them, 1.4 s and 3 s after death and just before the respawn, as well as which body was fastest and when. Each ragdoll logs its physics asset's bodies, constraints and the body pairs
that do not collide. `tools/play_range.ps1 -KillRow -NoCorpseLimits` runs the kill row once without the depenetration limit and the speed caps, to see what they hide; it is a test switch (the host ignores `-owcorpsenolimits` without `-owrangekillrow` and logs one line at start-up either way), and how bodies
are thrown in play does not change. None of this has run yet. The import's asset is auto-generated from the mesh, not the stock asset, so what these lines show is about the host's asset, and
what they mean for the game's own is UNVERIFIED. The floor trace looks for world static geometry only, so a body lying on another corpse, or on an object of
another type, counts as "no floor below" (counted in the line) and is not measured against it.

**The den read.** `tools/real_game/cash_capture.ps1` has a `den_read` stage after the cash read, with its own result (`stage_results` in `session.json`): the player
stands immune 1,600 uu from an enemy with no input, and `scripts/den_probe.py` records melee calls, blows and positions. It reads one map's dens at one level; its
numbers don't speak for others.

First run (2026-11-07, one session, three trials, Three Horns Divide, Maya at level 8, immune: her health was unchanged after every trial, against 78 recorded blows). All of it UNVERIFIED beyond that:
- **First blows did not land as one:** 8.6 s and 5.7 s between different attackers in two trials, and one attacker in the third. No two attackers landed on one tick. The den was a mix of kinds and distances, not four equal melee enemies on an arc, so this does not say what such a den would do.
- **They can land close together:** two babies that had converged landed 14 pairs of blows within 0.05 s, on the same swing cycle.
- **Their first blows were mostly not melee:** a slinger's rock from 800 uu and more, at about 3 s, and leaps. The range's enemies have neither.
- **Swing start:** an adult's first swing came at 368 uu and a baby's first grab at 425, each inside its stock range, which supports the rule on those two samples. Regular bullymong and psycho: no read.
- **Closing:** once there, the adult stood about 190 to 225 uu from Maya and the baby about 106. The host's adult stops at 334 and its regular at 260.
- **Swing cycles:** adult 1.217 s, baby grab 1.133 s; the host uses 1.25 s for every kind.
- **Speed:** moving medians of 500 for the slinger, 300 to 450 for the baby and 608 to 866 for adults, leaps included, with long pauses. The host's 400 is not too fast by this read; no number is taken from it.
- **Notice:** members took a target 0.06 and 0.12 s apart in two trials: weak evidence, at 0.05 s sampling, that their updates are not in step.

Options for the maintainer, none taken at the time (2026-11-11: see the acceptance table below): swing cycles per kind; closer stopping distances; leaps and throws (the ranged lane). The money read in the same session gave no value: one collect took two pickups at once for +5, and the item's own value call returns 0, so the money stand-in stays.

## Acceptance table: melee per kind (2026-11-11)

AI-assisted (Claude Opus 5.5). This table was written first: the checks hold the host to the live values, and the host changed only where a value has no
other record against it. Each value comes from one session and is UNVERIFIED beyond that session. Two sources:
- **L-4:** AI_MELEE_TARGETING_LIVE section 4, one adult in one long fight with the player next to it.
- **Den read:** the 2026-11-07 session above. The numbers here come from that session's ignored `den.json` and are worked out the same way for each kind.
  The swing cycle is the gap between one attacker's consecutive melee calls with one definition, counting only gaps under 1.5 s (a longer one is a break:
  a leap, or a move). The stand distance is that attacker's sampled horizontal distance to the player (samples every 0.05 s), from its second call to its last.
  The first call is left out because it comes on the way in.

| # | Quantity | Live value (spread, count, source) | Host before | Host now | Check | Before | After |
|---|---|---|---|---|---|---|---|
| 1 | Adult swing cycle | **Two live reads disagree, open.** L-4: median 1.249 s, 33 of 34 gaps 1.221 to 1.386 (one 1.813 after another action). Den read: mean 1.2168 s, 1.216 to 1.217, 9 gaps, one adult (a second adult, swinging at something 2,850 uu from the player, gave 1.208 to 1.217 over 8 gaps; not counted) | 1.25 | 1.25 (kept: the larger read) | light: `range_kind_melee_test` (host row = table); in engine: `kind_melee` `cycle_min/max` in 1.221 to 1.386 (+ `max_frame`) | light FAIL (no per-kind row) | light PASS; engine not run |
| 2 | Adult stand distance | Den read: median 201 uu, 190 to 225, 181 samples, one adult; 192.0 to 198.2 at the damage moment (9 calls). No record disagrees: L-4 has the player "within about 300 uu", AI_AWARENESS_LIVE has members within 234 uu of her, and the radii of section 5 (adult 150, player 42) make contact 192 | 334 (394 - 60) | 201 | light as row 1; in engine: `kind_melee` `stand_min/max` in 190 to 225 | FAIL (334) | light PASS; engine not run |
| 3 | Adult first swing inside the stock range | Den read: 368 uu, stock range 394 (one swing) | the rule since PR #133 | unchanged | `kind_melee` `first_swing` <= `start_range` | PASS (by the rule; not run) | engine not run |
| 4 | Baby swing cycle (its grab) | Den read: mean 1.1334 s, 1.133 to 1.134, 31 gaps, two babies; no other record | 1.25 | 1.133 | light as row 1; in engine: `kind_melee` in 1.128 to 1.139 (+ `max_frame`) | FAIL (1.25) | light PASS; **the default range spawns no baby, so the engine check does not run there** |
| 5 | Baby stand distance | Den read: median 111 uu, 106 to 118, 347 samples, one baby (the second was not sampled); 106.0 to 112.3 at the damage moment (31 calls) | 420 (480 - 60) | 111, but **192 in the host** (171 with Maya crouched): no stand goes inside body contact, and the host's baby has the borrowed adult-sized radius 150 (+ Maya's 42 standing, 21 crouched) | light as row 1; in engine as row 4, 106 to 118 | FAIL (420) | as row 4; when a baby is spawned this row FAILS on the body size, not on the rule |
| 6 | Baby first swing inside the stock range | Den read: 425 uu, stock range 480 (one grab) | the rule | unchanged | as row 3 | PASS (by the rule) | as row 4 |
| 7 | Regular bullymong: cycle and stand distance (three of the four default enemies) | **no live read** in the den read; session 2 of 2026-11-12 (below): cycle median 1.220 to 1.223 s, 96 gaps over 3 recordings; distance at its swing calls 142.0 uu at the closest (100 + 42), median 209 to 227 uu (a call distance, not this table's sampled stand distance) | 1.25 (the adult's), 260 (320 - 60) | unchanged | `melee_interval` (enemy 0 is a regular one) against its own value, 0.1 s | PASS (stand-in) | unchanged |
| 8 | Slinger's melee, psycho | **no live read** of either swinging at the player (the den read's slingers only threw; its SavageLee swung at something 3,021 uu away). Session 2 of 2026-11-12 (below): a slinger's melee at the player was seen, cycle median 1.222 s (28 gaps, one recording), closest call 192.0 uu; still no psycho swing at the player | 1.25, stop rule | unchanged | none | | |
| 9 | First blow, leaps and thrown rocks | see "The main open gap" below | not modelled | not modelled | none | | |

**How the bands were chosen.** `tools/check_range.py` grades the numbers, and only there. The host's `kind_melee` verdict covers only what the bands
don't: a row exists, four swings happened, and the first one started inside the stock range.
- Adult cycle: L-4's spread (1.221 to 1.386). The den read's 1.217 falls outside it, so the gate follows the older, larger read, and rejects the den
  read's value, until a third read settles which is right. A test asserts that rejection.
- Baby cycle: the measured 1.133 to 1.134, widened 0.005 s each way because both logs round to the millisecond.
- The one host allowance, on every cycle band: the upper edge is raised by `max_frame`, the longest frame the step saw (the host logs it). The host starts a
  swing on the first frame after it is due, so a slow frame makes it late by up to that frame. A `max_frame` above 0.2 s (one state update) is a stall, and the
  line fails.
- Stand distances: the measured minimum to maximum, with no allowance. The host clamps the last step of the run to the stand distance, so a slow frame
  cannot carry the enemy past it. The host's value is the median of the samples. The old 334 and 420 are outside the bands.

**What changed in the host.** `KindMeleeRows` (`OpenWillowEnemyAI.cpp`) gives the two kinds with a live read their cycle and stand distance. Every other kind
keeps `SwingInterval` (1.25 s) and the old stop (60 uu inside its swing range). A live stand distance never goes inside body contact: the kind's collision
radius plus Maya's capsule radius, because the enemy's root does not collide and nothing else stops it. When that floor applies, the enemy logs it once
(`... is inside body contact ...: it stops at ...`). Maya's radius is the walker's capsule: 42 standing (set in `BeginPlay`), 21 crouched. For the adult,
150 + 42 = 192 is under 201 (171 crouched), so the floor does not apply. The run's last step is limited by the gap to Maya's current position, the same
measure as the stop test, so an enemy follows a Maya who walks away instead of trailing her by one state update. The enemy logs one line per spawn,
`OWRANGE enemy <name> kind_melee kind=... swing_interval=... stand=... source=live_read|stand-in_no_live_read`. So on the default range (three regular, one
adult) the only behaviour that changes is where the adult stops: 201 uu instead of 334. A swing still starts once Maya is inside the stock range, so the
adult's first swing still comes from up to 394 uu, and then it closes in.

**The in-engine step.** `kind_melee` (heavy, part of `tools/test_range.ps1`, about 6 s per kind; 37 range steps now, 36 before): for each kind on the range that
has a row, its first enemy is placed back home and Maya 600 uu in front of it. That is outside both stock swing ranges and inside the proximity rule, so the
enemy notices her at once. The step records four swing starts with their distances and logs `kind=... cycle_min cycle_max stand_min stand_max first_swing
start_range max_frame ... stop_at`. Swings 2 to 4 give the stand distances and two full cycles. Maya's health is put back to full between blows (a test fixture, like the den
read's immunity). The step fails, and does not skip, when no kind with a row is on the range. The checker ties each result to the log's spawn records (`OWRANGE enemy <name> kind_melee kind=... source=live_read`): every kind spawned with a live row needs exactly one passing result, from an enemy (`enemy=`) spawned as that kind. Each field must appear exactly once, `swings` must be at least 4, every number must be finite, `max_frame` must be above 0, and each minimum must not exceed its maximum. Anything else fails. `melee_interval` now compares enemy 0's gap with that enemy's own
cycle; for the regular bullymong that is the same 1.25 s as before.

**The main open gap: leaps and thrown rocks (not implemented).** In the den read, most first blows on the player were not the plain melee. Trial 1: a slinger's
rock 2.8 s after it took its target, then a baby's leap 3.9 s after its target. Trial 2: a baby's grab, 4.1 s. Trial 3: an adult's first hit (249.8 damage) came
with no melee call, 6.7 s after its target, so it was something other than the plain swing, probably a leap (unread). The range has neither leaps nor
throws, so its enemies only hit once they are close. A future read should capture, for each kind:
- what triggers the attack: the distance at take-off or throw, and the time since the target was taken;
- timing: take-off to landing (L-4 already has `Attack` to `Landed` 1.08 to 3.48 s, mean 1.68, n 157, kinds not separated), throw to impact, and the cycle between leaps or throws;
- damage per blow, with momentum, and the definition or projectile behind each;
- how often each kind leaps or throws compared with a plain swing.

**Also open (host):** with Maya inside a kind's live stand distance but outside the swing volume (far above it, for example), the enemy stands and turns
to her with no swing. Nothing else happens until she moves; that was so before this change as well (it ran under her then). What the game does there is unread.

## Live-read questions from the acceptance table

1. **The adult cycle conflict.** Is the cycle the swing clip's length plus a whole number of frames? It is only an observation (UNVERIFIED), but both
   den-read cycles are whole numbers of sixtieths of a second: 1.2167 = 73/60 and 1.1333 = 68/60. L-4's 1.221 to 1.386 would fit the same clip at a lower,
   uneven frame rate. Two cheap sources answer this:
   - the clip lengths, from package data with no game needed. No record holds them yet: the clips are the ones `AnimRandom_PrimalBeastMelee` picks for the
     adult's Close Melee, and the baby's grab animation. Decoding them is not this lane's job.
   - one live read at a fixed frame rate that logs each frame's time next to each gap: adult and baby, in one session, with the player standing still and
     then moving.
2. **The regular bullymong** (three of the four default enemies): its swing cycle and stand distance at the player, ten or more gaps, sampled every 0.05 s.
3. **Is the stand distance just contact?** We need each kind's collision cylinder radius and the player's, in the same session as the distances. The adult's
   150 + 42 = 192 matches its damage-moment 192.0. **The small kind's (the baby's) collision radius is in no record**: it is needed next to its distances,
   since the host's baby borrows the adult-sized 150 and so cannot stand at 111.
4. **Leaps and rocks:** the list under "The main open gap" above.
5. **Time to the first blow** after a member takes its target, per kind, against a player who stands still and one who moves.

## Den reads of session 2 (2026-11-12)

AI-assisted (Claude Code). Written in our own words from a private analysis of the session's `tools/real_game/cash_capture.ps1` runs (the
`den_read`, `den_down` and cash stages); the recordings stay under ignored `local/`. Three Horns Divide (Ice_P), Maya level 8, six den recordings in
one day (two from earlier attempts), about 120 fps. SDK mods loaded, with the three loot mods switched off in memory for the cash runs. **What the
capture did:** Maya stood immune during the reads, she was moved 1,600 uu from a pack, each money target was set to 1 health before its kill, and
she was moved onto each money pickup by a location write. One map, one level, one session: nothing below speaks for other dens. The acceptance
table's rows 1 to 6 are left as they are here; rows 7 and 8 point to this section.

- **The kind is the AI class.** Every enemy's actor class is `WillowAIPawn`; the kinds are told apart by the AI class (`CharClass_PrimalBeast`, the
  regular, and `_Adult`, `_Ranged` the slinger, `_Badass`, `_Baby`). Regular bullymongs were present in every den recording and swung at Maya.
- **Fields, read live on every pawn of those kinds:** `GroundSpeed` 500 for the regular, the adult, the slinger and the badass, 300 for the baby;
  `WalkingPct` 0.25 for all; `DrawScale` 1.0. Collision cylinder (radius / half-height): baby 64 / 48, regular 100 / 90; Maya 42 / 80.
- **The closest swing call is contact: the attacker's radius plus Maya's 42, standing.** Adult and slinger 192.0 uu (a radius of 150 for both),
  regular 142.0, baby 106.0. None of 189 baby calls came closer than 106.0, median 106.0 to 106.1 per recording. This answers live-read question 3 for
  these kinds; the crouched floor was not observed.
- **Moving speeds** (medians of moving stretches, leaps and knockback included): slinger 500 in 4 of 4, adult 500 in 6 of 11, baby 300 in 4 of 8.
  For those kinds the field is the chase speed. **The regular read 375 in 4 of 6 medians** (the others 549 and 750), below its field of 500;
  why is unexplained (UNVERIFIED).
- **Swing cycles** (one attacker's consecutive calls with one definition, gaps of 3 s or less, medians per recording): adult 1.2200 to 1.2230 s, 407
  gaps over 4 recordings; regular 1.2200 to 1.2227 s, 96 gaps over 3 (it uses the adult's `MeleeDef_PrimalBeast_Melee`); slinger melee 1.2224 s,
  28 gaps, one recording; baby grab 1.1330 to 1.1337 s, 131 gaps over 3. The adult value is a third read beside L-4's 1.249 s and the 2026-11-07 den
  read's 1.2168 s, and it agrees with the den read. These are the values a clip rounded to whole frames at 60 fps would give (1.2167, 1.1333) although
  the game ran at about 120 fps, so the cycle does not seem to follow the frame rate as live-read question 1 supposed (UNVERIFIED: the frame time was
  not fixed). The gap window here (3 s) is wider than the acceptance table's (1.5 s).
- **The regular, beyond the table:** its median call distance stands 70 to 85 uu beyond contact, where the adult's sits at contact (why: open).
- **The psycho:** `CharClass_SavageLee`, a named enemy, swung only at targets 1,192 to 2,986 uu away, so there is still no psycho swing at the player.
- **Money:** at item level 5 two pickups paid 2 each, each the only pickup taken in its collect; at level 7 a pair touched in the same frame paid 4
  together. With [COMBAT_GOLDEN_TRACE.md](COMBAT_GOLDEN_TRACE.md) item 8 (three level 7 pickups paid 9 together), one value per level cannot give both
  sums, so "one value per level" is contradicted (weakly: one pair). The money items carry part definitions, a candidate cause (UNVERIFIED). The
  range's `round(4 x 1.13^level)` stays wrong-high and stays a stand-in. All 8 money items whose balance was read were `Money_Little`, none big, which
  at that count says nothing about the big share.
- **The money credit is not an `AddCurrencyOnHand` call:** five money pickups raised the cash and no such call came; the only calls in the session
  were the respawn fees. Money is taken on touch with no key: `TouchedPickupable` then `PickupSomething` (`bReadyAfterPickup` false) in the same
  frame, five touches. The item's own value calls (`GetMonetaryValue`, `MonetaryValue`) return 0 again.
- **Some pickups took no touch** when the capture moved Maya onto them, so the cash stage ended early each time. Why (still settling, lying in
  geometry, or a location write raising no touch) is UNVERIFIED.

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
Session 2 of 2026-11-12 read two more values and contradicts one value per level ("Den reads of session 2" above); the stand-in stays.

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

## Batch 7 (zuhu PC, 2026-11-04, default spot: 24 pass, 3 fail, 2 skip)

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
